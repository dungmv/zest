//! Stateless OpenAPI 3.0 export helpers operating over [`CollectionState`].

use crate::state::CollectionState;
use zest_core::exporters::OpenApiExport;
use zest_core::models::{Protocol, RequestItem};

/// Cheap counts for the export modal: `(exportable requests, folders,
/// requests skipped because their protocol is not OpenAPI-representable)`.
pub fn openapi_export_stats(collection: &CollectionState) -> (usize, usize, usize) {
    openapi_export_stats_for(collection, None)
}

/// Folder-scoped variant of [`openapi_export_stats`]: counts only requests
/// inside `folder_id`'s subtree. Folders are counted from the subtree too.
pub fn openapi_export_stats_for(
    collection: &CollectionState,
    folder_id: Option<&str>,
) -> (usize, usize, usize) {
    let scope_ids = folder_id.map(|id| folder_subtree_ids(collection, id));
    let scoped_requests = requests_in_scope(collection, scope_ids.as_deref());
    let exportable = scoped_requests
        .iter()
        .filter(|r| r.protocol.uses_http_transport())
        .count();
    // Exclude the export root itself from the folder count.
    let folders = scope_ids
        .as_ref()
        .map_or(collection.folders.len(), |ids| ids.len() - 1);
    (exportable, folders, scoped_requests.len() - exportable)
}

/// Builds an OpenAPI 3.0 export of the whole workspace.
pub fn build_openapi_export(
    collection: &CollectionState,
    workspace_name: &str,
    workspace_description: Option<&str>,
) -> OpenApiExport {
    build_openapi_export_for(collection, None, workspace_name, workspace_description)
}

/// Folder-scoped variant of [`build_openapi_export`]: only requests inside
/// `folder_id`'s subtree are exported, tags become the folder chain relative
/// to the export root, and the folder name is the document title.
pub fn build_openapi_export_for(
    collection: &CollectionState,
    folder_id: Option<&str>,
    workspace_name: &str,
    workspace_description: Option<&str>,
) -> OpenApiExport {
    let scope_ids = folder_id.map(|id| folder_subtree_ids(collection, id));
    let scoped_requests = requests_in_scope(collection, scope_ids.as_deref());
    let root_chain_len = folder_id.map(|id| collection.get_folder_hierarchy(Some(id)).len());

    let exported: Vec<zest_core::importers::ImportedRequest> = scoped_requests
        .into_iter()
        .map(|req| {
            let effective = if req.protocol == Protocol::GraphQL {
                req.to_graphql_http_item().unwrap_or_else(|_| req.clone())
            } else {
                req.clone()
            };
            let (headers, auth) = collection.resolve_request_hierarchy(&effective);
            let folder_path: Vec<String> = collection
                .get_folder_hierarchy(req.folder_id.as_deref())
                .into_iter()
                .skip(root_chain_len.unwrap_or(0))
                .map(|f| f.name.clone())
                .collect();
            zest_core::importers::ImportedRequest {
                name: effective.name.clone(),
                method: effective.method,
                url: effective.url.clone(),
                protocol: req.protocol,
                params: effective.params.clone(),
                headers,
                auth,
                body: effective.body.clone(),
                script: None,
                examples: effective.examples.clone(),
                folder_path,
                description: None,
            }
        })
        .collect();

    let (title, description) = match folder_id {
        Some(id) => (
            collection
                .folders
                .iter()
                .find(|f| f.id == id)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| "Folder".to_string()),
            None,
        ),
        None => (
            workspace_name.to_string(),
            workspace_description
                .filter(|d| !d.trim().is_empty())
                .map(str::to_string),
        ),
    };
    zest_core::exporters::requests_to_openapi(&title, description.as_deref(), &exported)
}

fn requests_in_scope<'a>(
    collection: &'a CollectionState,
    scope_ids: Option<&[String]>,
) -> Vec<&'a RequestItem> {
    match scope_ids {
        None => collection.requests.iter().collect(),
        Some(ids) => collection
            .requests
            .iter()
            .filter(|r| {
                r.folder_id
                    .as_deref()
                    .is_some_and(|fid| ids.iter().any(|id| id == fid))
            })
            .collect(),
    }
}

fn folder_subtree_ids(collection: &CollectionState, folder_id: &str) -> Vec<String> {
    let mut ids = vec![folder_id.to_string()];
    let mut i = 0;
    while i < ids.len() {
        let current = ids[i].clone();
        i += 1;
        let child_ids: Vec<String> = collection
            .folders
            .iter()
            .filter(|f| f.parent_id.as_deref() == Some(current.as_str()))
            .map(|f| f.id.clone())
            .collect();
        for child in child_ids {
            if !ids.contains(&child) {
                ids.push(child);
            }
        }
    }
    ids
}
