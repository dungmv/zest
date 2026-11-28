use super::{FolderNode, RequestNode, WorkspaceTree};
use crate::models::{Folder, RequestItem};
use std::collections::{HashMap, HashSet};

pub(super) fn order_requests(
    mut requests: HashMap<String, RequestNode>,
    order: &[String],
) -> Vec<RequestNode> {
    let mut ordered = Vec::with_capacity(requests.len());
    ordered.extend(order.iter().filter_map(|stem| requests.remove(stem)));
    let mut remaining: Vec<_> = requests.into_iter().collect();
    remaining.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    ordered.extend(remaining.into_iter().map(|(_, request)| request));
    ordered
}

pub struct CollectionSnapshot {
    pub requests: Vec<RequestItem>,
    pub folders: Vec<Folder>,
    pub parse_errors: HashMap<String, String>,
}

impl CollectionSnapshot {
    pub fn from_tree(tree: WorkspaceTree, workspace_name: &str) -> Self {
        let total_reqs = tree.count_requests();
        let total_folders = tree.count_folders();
        let mut all_requests = Vec::with_capacity(total_reqs);
        let mut all_folders = Vec::with_capacity(total_folders);
        let mut errors = HashMap::new();

        for req_node in tree.root_requests {
            let mut item = req_node
                .file
                .into_item(&req_node.rel_path, workspace_name, None);
            item.last_status_code = req_node.last_status_code;
            all_requests.push(item);
            if let Some(err) = req_node.parse_error {
                errors.insert(req_node.rel_path, err);
            }
        }

        fn process_folder(
            folder_node: FolderNode,
            parent_id: Option<String>,
            ws_name: &str,
            all_folders: &mut Vec<Folder>,
            all_requests: &mut Vec<RequestItem>,
            errors: &mut HashMap<String, String>,
        ) {
            let mut f = Folder::new(ws_name, &folder_node.file.name);
            f.id = folder_node.rel_path.clone();
            f.parent_id = parent_id;
            f.headers = folder_node.file.headers;
            f.auth = folder_node.file.auth;
            all_folders.push(f);

            let folder_rel = folder_node.rel_path;
            for req_node in folder_node.requests {
                let mut item =
                    req_node
                        .file
                        .into_item(&req_node.rel_path, ws_name, Some(folder_rel.clone()));
                item.last_status_code = req_node.last_status_code;
                all_requests.push(item);
                if let Some(err) = req_node.parse_error {
                    errors.insert(req_node.rel_path, err);
                }
            }

            for sub in folder_node.subfolders {
                process_folder(
                    sub,
                    Some(folder_rel.clone()),
                    ws_name,
                    all_folders,
                    all_requests,
                    errors,
                );
            }
        }

        for f_node in tree.folders {
            process_folder(
                f_node,
                None,
                workspace_name,
                &mut all_folders,
                &mut all_requests,
                &mut errors,
            );
        }

        Self {
            requests: all_requests,
            folders: all_folders,
            parse_errors: errors,
        }
    }

    /// Retains locally edited requests even when their disk path has disappeared.
    /// Returns the missing IDs so the app can present a conflict.
    pub fn preserve_requests(
        &mut self,
        mut preserved: HashMap<String, RequestItem>,
    ) -> Vec<String> {
        if !preserved.is_empty() {
            for req in &mut self.requests {
                if let Some(mut kept) = preserved.remove(&req.id) {
                    kept.folder_id = req.folder_id.clone();
                    kept.last_status_code = kept.last_status_code.or(req.last_status_code);
                    self.parse_errors.remove(&req.id);
                    *req = kept;
                }
            }
        }

        let mut missing: Vec<_> = preserved.into_values().collect();
        missing.sort_by(|a, b| a.id.cmp(&b.id));
        let missing_ids = missing.iter().map(|req| req.id.clone()).collect();
        for request in &mut missing {
            if request
                .folder_id
                .as_ref()
                .is_some_and(|folder| !self.folders.iter().any(|existing| &existing.id == folder))
            {
                request.folder_id = None;
            }
        }
        self.requests.extend(missing);

        missing_ids
    }
}

/// Returns the valid ancestors in workspace-to-leaf order.
pub fn folder_ancestors<'a>(folders: &'a [Folder], folder_id: Option<&str>) -> Vec<&'a Folder> {
    let mut current = folder_id;
    let mut seen = HashSet::new();
    let mut chain = Vec::new();
    while let Some(id) = current {
        let Some(folder) = folders.iter().find(|folder| folder.id == id) else {
            break;
        };
        if !seen.insert(folder.id.as_str()) {
            break;
        }
        chain.push(folder);
        current = folder.parent_id.as_deref();
    }
    chain.reverse();
    chain
}

/// Remaps a folder path and every descendant, including dependent IDs.
pub fn repath_subtree(
    folders: &mut [Folder],
    requests: &mut [RequestItem],
    pins: &mut Vec<String>,
    parse_errors: &mut HashMap<String, String>,
    old_id: &str,
    new_id: &str,
) -> Vec<(String, String)> {
    let old_prefix = format!("{}/", old_id);
    let new_prefix = format!("{}/", new_id);
    let repath = |id: &str| -> String {
        if let Some(rest) = id.strip_prefix(&old_prefix) {
            format!("{}{}", new_prefix, rest)
        } else if id == old_id {
            new_id.to_string()
        } else {
            id.to_string()
        }
    };

    let mut pairs = Vec::new();

    for f in folders.iter_mut() {
        let new_fid = repath(&f.id);
        if new_fid != f.id {
            pairs.push((f.id.clone(), new_fid.clone()));
            f.id = new_fid;
        }
        let new_parent = f.parent_id.as_ref().map(|p| repath(p));
        if new_parent != f.parent_id {
            f.parent_id = new_parent;
        }
    }

    for r in requests.iter_mut() {
        let new_folder = r.folder_id.as_ref().map(|p| repath(p));
        if new_folder != r.folder_id {
            r.folder_id = new_folder;
        }
        let new_rid = repath(&r.id);
        if new_rid != r.id {
            pairs.push((r.id.clone(), new_rid.clone()));
            r.id = new_rid;
        }
    }

    let mut pins_changed = false;
    let mut new_pins = Vec::with_capacity(pins.len());
    for p in pins.iter() {
        let np = repath(p);
        if np != *p {
            pins_changed = true;
        }
        new_pins.push(np);
    }
    if pins_changed {
        *pins = new_pins;
    }

    let mut errors_changed = false;
    let mut new_errors = HashMap::with_capacity(parse_errors.len());
    for (k, v) in parse_errors.iter() {
        let nk = repath(k);
        if nk != *k {
            errors_changed = true;
        }
        new_errors.insert(nk, v.clone());
    }
    if errors_changed {
        *parse_errors = new_errors;
    }

    pairs
}

#[cfg(test)]
#[path = "collection_tests.rs"]
mod tests;
