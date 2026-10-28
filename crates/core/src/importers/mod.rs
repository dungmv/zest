pub mod models;
pub mod openapi;
pub mod postman;

pub use models::{
    ImportFormat, ImportOptions, ImportResult, ImportSummary, ImportedEnvironment, ImportedFolder,
    ImportedRequest,
};

use crate::storage::models::{EnvVarFile, EnvironmentFile, RequestFile};
use crate::storage::slug::slugify;
use crate::storage::WorkspaceStore;
use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};

/// Split a folder name or path by '/' into non-empty, trimmed segments.
///
/// Leading, trailing, and consecutive slashes are ignored.
/// Example: `"merchant/Thông quan"` -> `["merchant", "Thông quan"]`
pub fn split_folder_segments(name: &str) -> Vec<String> {
    name.split('/')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Push a hierarchy of folders into `folders`, deduplicating by full folder path.
///
/// Given a parent prefix (e.g. `["v1"]`) and segments (e.g. `["merchant", "Thông quan"]`),
/// creates each intermediate folder step if it doesn't already exist in `seen`:
/// - `["v1", "merchant"]` with name `"merchant"`
/// - `["v1", "merchant", "Thông quan"]` with name `"Thông quan"` and `description` (only on leaf)
///
/// Returns the full leaf path `Vec<String>`.
pub fn push_folder_chain(
    parent_prefix: &[String],
    segments: &[String],
    description: Option<String>,
    folders: &mut Vec<ImportedFolder>,
    seen: &mut HashSet<Vec<String>>,
) -> Vec<String> {
    if segments.is_empty() {
        return parent_prefix.to_vec();
    }

    let mut current = parent_prefix.to_vec();
    let num_segments = segments.len();

    for (i, seg) in segments.iter().enumerate() {
        current.push(seg.clone());
        if seen.insert(current.clone()) {
            let is_leaf = i == num_segments - 1;
            folders.push(ImportedFolder {
                path: current.clone(),
                name: seg.clone(),
                description: if is_leaf { description.clone() } else { None },
            });
        }
    }

    current
}

pub fn parse_content(content: &str, format: ImportFormat) -> Result<ImportResult> {
    match format {
        ImportFormat::OpenApi3 => openapi::parse_openapi(content),
        ImportFormat::PostmanCollection => postman::parse_postman(content),
    }
}

pub fn apply_import_to_workspace(
    store: &dyn WorkspaceStore,
    result: &ImportResult,
    options: &ImportOptions,
) -> Result<ImportSummary> {
    let mut summary = ImportSummary::default();

    // Determine root folder prefix for import
    let mut base_folder_rel: Option<String> = options.target_folder_rel.clone();

    if options.create_enclosing_folder {
        let enclosing_name = options
            .enclosing_folder_name
            .as_deref()
            .unwrap_or(&result.title);
        let safe_name = if enclosing_name.trim().is_empty() {
            "Imported Collection"
        } else {
            enclosing_name.trim()
        };

        let (_dir_name, rel) = store
            .create_group(base_folder_rel.as_deref(), safe_name)
            .map_err(|e| anyhow!("Failed to create enclosing folder '{}': {}", safe_name, e))?;

        summary.created_folder_paths.push(rel.clone());
        summary.total_folders += 1;
        base_folder_rel = Some(rel);
    }

    // Map hierarchy path (e.g. ["Users", "Auth"]) to relative folder path on disk
    let mut folder_path_map: HashMap<Vec<String>, String> = HashMap::new();

    // Sort folders by depth so parents are always created before children
    let mut sorted_folders = result.folders.clone();
    sorted_folders.sort_by_key(|f| f.path.len());

    for folder in &sorted_folders {
        if folder.path.is_empty() {
            continue;
        }

        // Parent relative path
        let parent_rel = if folder.path.len() == 1 {
            base_folder_rel.clone()
        } else {
            let parent_key = folder.path[..folder.path.len() - 1].to_vec();
            folder_path_map
                .get(&parent_key)
                .cloned()
                .or_else(|| base_folder_rel.clone())
        };

        match store.create_group(parent_rel.as_deref(), &folder.name) {
            Ok((_dir, rel)) => {
                folder_path_map.insert(folder.path.clone(), rel.clone());
                summary.created_folder_paths.push(rel);
                summary.total_folders += 1;
            }
            Err(e) => {
                // If group already exists or error, fallback to parent
                tracing::warn!("Failed to create folder '{}': {}", folder.name, e);
            }
        }
    }

    // Import Requests
    for req in &result.requests {
        // Resolve target folder on disk
        let req_folder_rel = if !req.folder_path.is_empty() {
            folder_path_map
                .get(&req.folder_path)
                .cloned()
                .or_else(|| base_folder_rel.clone())
        } else {
            base_folder_rel.clone()
        };

        let (_slug, rel_path) = match store.create_request(
            req_folder_rel.as_deref(),
            &req.name,
            req.method,
            &req.url,
        ) {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!("Failed to allocate request '{}': {}", req.name, e);
                continue;
            }
        };

        let req_file = RequestFile {
            name: req.name.clone(),
            method: req.method,
            url: req.url.clone(),
            params: req.params.clone(),
            headers: req.headers.clone(),
            auth: req.auth.clone(),
            body: req.body.clone(),
            settings: Default::default(),
            script: req.script.clone(),
            examples: req.examples.clone(),
        };

        if let Err(e) = store.write_request(&rel_path, &req_file) {
            tracing::warn!("Failed to write request '{}': {}", rel_path, e);
            continue;
        }

        summary.created_request_paths.push(rel_path);
        summary.total_requests += 1;
    }

    // Import Environments if requested
    if options.import_environments {
        for env in &result.environments {
            let env_slug = slugify(&env.name);
            let env_file = EnvironmentFile {
                variables: env
                    .variables
                    .iter()
                    .map(|v| EnvVarFile {
                        name: v.key.clone(),
                        value: Some(v.initial_value.clone()),
                        secret: false,
                        enabled: true,
                    })
                    .collect(),
            };

            if let Err(e) = store.save_environment(&env_slug, &env_file) {
                tracing::warn!("Failed to save environment '{}': {}", env.name, e);
            } else {
                summary.total_environments += 1;
            }
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::HttpMethod;
    use crate::storage::fs_store::FsWorkspaceStore;

    #[test]
    fn test_parse_openapi_3_1_json() {
        let json_spec = r#"{
            "openapi": "3.1.0",
            "info": {
                "title": "Pet Store API",
                "version": "1.0.0"
            },
            "servers": [
                { "url": "https://api.petstore.com/v1" }
            ],
            "paths": {
                "/pets": {
                    "get": {
                        "summary": "List all pets",
                        "tags": ["Pets"],
                        "parameters": [
                            {
                                "name": "limit",
                                "in": "query",
                                "required": false,
                                "schema": { "type": "integer", "default": 10 }
                            }
                        ],
                        "responses": {
                            "200": {
                                "description": "A paged array of pets",
                                "content": {
                                    "application/json": {
                                        "example": "[{\"id\": 1, \"name\": \"Dog\"}]"
                                    }
                                }
                            }
                        }
                    },
                    "post": {
                        "summary": "Create a pet",
                        "tags": ["Pets"],
                        "requestBody": {
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "type": "object",
                                        "properties": {
                                            "name": { "type": "string", "example": "Doggie" }
                                        }
                                    }
                                }
                            }
                        },
                        "responses": {
                            "201": {
                                "description": "Pet created"
                            }
                        }
                    }
                }
            }
        }"#;

        let res = parse_content(json_spec, ImportFormat::OpenApi3).unwrap();
        assert_eq!(res.title, "Pet Store API");
        assert_eq!(res.requests.len(), 2);
        assert_eq!(res.folders.len(), 1);
        assert_eq!(res.folders[0].name, "Pets");

        let get_req = res
            .requests
            .iter()
            .find(|r| r.method == HttpMethod::Get)
            .unwrap();
        assert_eq!(get_req.name, "List all pets");
        assert_eq!(get_req.url, "{{baseUrl}}/pets");
        assert_eq!(get_req.params.len(), 1);
        assert_eq!(get_req.params[0].key, "limit");
        assert_eq!(get_req.params[0].value, "10");
        assert_eq!(get_req.examples.len(), 1);

        let post_req = res
            .requests
            .iter()
            .find(|r| r.method == HttpMethod::Post)
            .unwrap();
        assert_eq!(post_req.name, "Create a pet");
        match &post_req.body {
            crate::models::RequestBody::Json { content: body } => {
                assert!(body.contains("Doggie"));
            }
            _ => panic!("Expected JSON body"),
        }
    }

    #[test]
    fn test_parse_openapi_3_1_yaml() {
        let yaml_spec = r#"
openapi: 3.1.0
info:
  title: User Service
  version: 1.0.0
servers:
  - url: https://users.example.com
paths:
  /users/{id}:
    get:
      summary: Get User By ID
      tags:
        - Users
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
        - name: Authorization
          in: header
          required: true
          schema:
            type: string
            example: Bearer abc
      responses:
        '200':
          description: OK
"#;

        let res = parse_content(yaml_spec, ImportFormat::OpenApi3).unwrap();
        assert_eq!(res.title, "User Service");
        assert_eq!(res.requests.len(), 1);
        let req = &res.requests[0];
        assert_eq!(req.name, "Get User By ID");
        assert_eq!(req.url, "{{baseUrl}}/users/{{id}}");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "Authorization");
        assert_eq!(req.headers[0].value, "Bearer abc");
    }

    #[test]
    fn test_parse_postman_collection_json() {
        let postman_json = r#"{
            "info": {
                "name": "My Postman API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "variable": [
                { "key": "baseUrl", "value": "https://httpbin.org" }
            ],
            "item": [
                {
                    "name": "Auth",
                    "item": [
                        {
                            "name": "Login",
                            "request": {
                                "method": "POST",
                                "url": {
                                    "raw": "{{baseUrl}}/auth/login?source=app",
                                    "query": [
                                        { "key": "source", "value": "app" }
                                    ]
                                },
                                "header": [
                                    { "key": "X-Client", "value": "Mobile" }
                                ],
                                "body": {
                                    "mode": "raw",
                                    "raw": "{\"user\": \"alice\"}",
                                    "options": {
                                        "raw": { "language": "json" }
                                    }
                                }
                            },
                            "response": [
                                {
                                    "name": "200 Success",
                                    "code": 200,
                                    "body": "{\"token\": \"xyz\"}"
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;

        let res = parse_content(postman_json, ImportFormat::PostmanCollection).unwrap();
        assert_eq!(res.title, "My Postman API");
        assert_eq!(res.environments.len(), 1);
        assert_eq!(res.environments[0].variables[0].key, "baseUrl");
        assert_eq!(
            res.environments[0].variables[0].initial_value,
            "https://httpbin.org"
        );
        assert_eq!(res.folders.len(), 1);
        assert_eq!(res.folders[0].name, "Auth");
        assert_eq!(res.requests.len(), 1);

        let req = &res.requests[0];
        assert_eq!(req.name, "Login");
        assert_eq!(req.method, HttpMethod::Post);
        assert_eq!(req.url, "{{baseUrl}}/auth/login");
        assert_eq!(req.params.len(), 1);
        assert_eq!(req.params[0].key, "source");
        assert_eq!(req.params[0].value, "app");
        assert_eq!(req.headers.len(), 1);
        assert_eq!(req.headers[0].key, "X-Client");
        assert_eq!(req.headers[0].value, "Mobile");
        assert_eq!(req.examples.len(), 1);
        assert_eq!(req.examples[0].name, "200 Success");
        assert_eq!(req.examples[0].status, 200);
        assert_eq!(
            req.examples[0].body.as_deref(),
            Some("{\"token\": \"xyz\"}")
        );
    }

    #[test]
    fn test_apply_import_to_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = FsWorkspaceStore::new(temp_dir.path());
        store.init_workspace("Test WS").unwrap();

        let json_spec = r#"{
            "openapi": "3.1.0",
            "info": { "title": "Store API" },
            "paths": {
                "/items": {
                    "get": {
                        "summary": "Get Items",
                        "tags": ["Inventory"]
                    }
                }
            }
        }"#;

        let result = parse_content(json_spec, ImportFormat::OpenApi3).unwrap();
        let options = ImportOptions {
            target_folder_rel: None,
            create_enclosing_folder: true,
            enclosing_folder_name: Some("Store API".to_string()),
            import_environments: true,
        };

        let summary = apply_import_to_workspace(&store, &result, &options).unwrap();
        assert_eq!(summary.total_requests, 1);
        assert!(summary.total_folders >= 2); // Enclosing folder + Inventory folder

        let tree = store.scan_tree().unwrap();
        assert!(!tree.folders.is_empty());
    }

    #[test]
    fn test_split_folder_segments() {
        assert_eq!(split_folder_segments("Auth"), vec!["Auth"]);
        assert_eq!(
            split_folder_segments("merchant/Thông quan"),
            vec!["merchant", "Thông quan"]
        );
        assert_eq!(
            split_folder_segments("  v1  /  users  /  details  "),
            vec!["v1", "users", "details"]
        );
        assert_eq!(split_folder_segments("//a///b//"), vec!["a", "b"]);
        assert!(split_folder_segments("///").is_empty());
        assert!(split_folder_segments("").is_empty());
        assert!(split_folder_segments("   ").is_empty());
    }

    #[test]
    fn test_push_folder_chain_deduplication() {
        let mut folders = Vec::new();
        let mut seen = HashSet::new();

        let chain1 = push_folder_chain(
            &[],
            &["merchant".to_string(), "Thông quan".to_string()],
            Some("Mô tả thông quan".to_string()),
            &mut folders,
            &mut seen,
        );
        assert_eq!(chain1, vec!["merchant", "Thông quan"]);
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].name, "merchant");
        assert_eq!(folders[0].description, None);
        assert_eq!(folders[1].name, "Thông quan");
        assert_eq!(folders[1].description.as_deref(), Some("Mô tả thông quan"));

        // Second push with shared parent
        let chain2 = push_folder_chain(
            &[],
            &["merchant".to_string(), "Kho hàng".to_string()],
            None,
            &mut folders,
            &mut seen,
        );
        assert_eq!(chain2, vec!["merchant", "Kho hàng"]);
        // Only the new child folder should be added
        assert_eq!(folders.len(), 3);
        assert_eq!(folders[2].name, "Kho hàng");
    }

    #[test]
    fn test_openapi_nested_folder_and_vietnamese_slug() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = FsWorkspaceStore::new(temp_dir.path());
        store.init_workspace("Test OpenAPI Nested").unwrap();

        let json_spec = r#"{
            "openapi": "3.1.0",
            "info": { "title": "Merchant API" },
            "paths": {
                "/customs/declaration": {
                    "post": {
                        "summary": "Tạo tờ khai",
                        "tags": ["merchant/Thông quan"]
                    }
                }
            }
        }"#;

        let result = parse_content(json_spec, ImportFormat::OpenApi3).unwrap();
        assert_eq!(result.folders.len(), 2);
        assert_eq!(result.folders[0].path, vec!["merchant"]);
        assert_eq!(result.folders[0].name, "merchant");
        assert_eq!(result.folders[1].path, vec!["merchant", "Thông quan"]);
        assert_eq!(result.folders[1].name, "Thông quan");

        let options = ImportOptions {
            target_folder_rel: None,
            create_enclosing_folder: false,
            enclosing_folder_name: None,
            import_environments: false,
        };

        let summary = apply_import_to_workspace(&store, &result, &options).unwrap();
        assert_eq!(summary.total_requests, 1);
        assert_eq!(summary.total_folders, 2);

        // Check filesystem paths
        let collections_dir = temp_dir.path().join("collections");
        let parent_dir = collections_dir.join("merchant");
        let child_dir = parent_dir.join("thong-quan");

        assert!(
            parent_dir.exists(),
            "Parent folder 'merchant' must exist on disk"
        );
        assert!(
            child_dir.exists(),
            "Child folder 'thong-quan' must exist on disk with transliterated slug"
        );

        // Verify folder display name in _folder.yaml
        let folder_yaml = std::fs::read_to_string(child_dir.join("_folder.yaml")).unwrap();
        assert!(
            folder_yaml.contains("name: Thông quan"),
            "Folder display name must be preserved with diacritics"
        );

        // Verify request file in child folder
        let req_file = child_dir.join("tao-to-khai.yaml");
        assert!(
            req_file.exists(),
            "Request file 'tao-to-khai.yaml' must exist in child folder"
        );
    }

    #[test]
    fn test_postman_nested_folder_and_vietnamese_slug() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = FsWorkspaceStore::new(temp_dir.path());
        store.init_workspace("Test Postman Nested").unwrap();

        let postman_json = r#"{
            "info": {
                "name": "Postman Nested API",
                "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
            },
            "item": [
                {
                    "name": "merchant/Thông quan",
                    "item": [
                        {
                            "name": "Kiểm tra / Xác nhận",
                            "request": {
                                "method": "GET",
                                "url": "https://example.com/api/check"
                            }
                        }
                    ]
                }
            ]
        }"#;

        let result = parse_content(postman_json, ImportFormat::PostmanCollection).unwrap();
        assert_eq!(result.folders.len(), 2);
        assert_eq!(result.folders[0].path, vec!["merchant"]);
        assert_eq!(result.folders[1].path, vec!["merchant", "Thông quan"]);
        assert_eq!(
            result.requests[0].folder_path,
            vec!["merchant", "Thông quan"]
        );
        // Request name is NOT split:
        assert_eq!(result.requests[0].name, "Kiểm tra / Xác nhận");

        let options = ImportOptions {
            target_folder_rel: None,
            create_enclosing_folder: false,
            enclosing_folder_name: None,
            import_environments: false,
        };

        let summary = apply_import_to_workspace(&store, &result, &options).unwrap();
        assert_eq!(summary.total_requests, 1);
        assert_eq!(summary.total_folders, 2);

        let child_dir = temp_dir
            .path()
            .join("collections")
            .join("merchant")
            .join("thong-quan");
        assert!(child_dir.exists());

        let folder_yaml = std::fs::read_to_string(child_dir.join("_folder.yaml")).unwrap();
        assert!(folder_yaml.contains("name: Thông quan"));

        // Request name 'Kiểm tra / Xác nhận' slugified into 'kiem-tra-xac-nhan.yaml'
        let req_file = child_dir.join("kiem-tra-xac-nhan.yaml");
        assert!(
            req_file.exists(),
            "Request file must exist at {:?}",
            req_file
        );
    }
}
