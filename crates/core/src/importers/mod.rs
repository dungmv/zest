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
            req.protocol,
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
            protocol: req.protocol,
            grpc: None,
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
mod tests;
