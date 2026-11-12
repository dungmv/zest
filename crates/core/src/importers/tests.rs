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
