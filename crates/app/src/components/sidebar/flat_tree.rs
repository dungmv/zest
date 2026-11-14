use super::git_status::{compute_all_git_statuses, GitStatusIndex, SidebarGitStatus};
use super::request_node::{SidebarExampleItem, SidebarReqItem};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use zest_core::git::GitStatusResult;
use zest_core::models::{Folder, RequestItem};

/// Represents a single uniform-height row in the virtualized sidebar tree.
#[derive(Clone, Debug)]
pub enum FlatSidebarRow {
    PinnedHeader {
        is_collapsed: bool,
    },
    PinnedRequest {
        req: Arc<SidebarReqItem>,
        pin_index: usize,
        git_status: SidebarGitStatus,
    },
    Folder {
        folder_id: String,
        name: String,
        depth: usize,
        is_collapsed: bool,
        git_status: SidebarGitStatus,
    },
    Request {
        req: Arc<SidebarReqItem>,
        depth: usize,
        is_collapsed: bool,
        git_status: SidebarGitStatus,
    },
    Example {
        parent_id: String,
        idx: usize,
        total_ex: usize,
        ex: SidebarExampleItem,
        depth: usize,
    },
    RootDropzone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SidebarCacheKey {
    pub tree_version: usize,
    pub url_version_if_filtering: usize,
    pub git_status_version: usize,
    pub unsaved_version: usize,
    pub ui_version: usize,
}

#[derive(Default)]
pub struct SidebarFlatCache {
    pub key: Option<SidebarCacheKey>,
    pub req_items_version: Option<(usize, usize)>,
    pub req_items: Arc<Vec<Arc<SidebarReqItem>>>,
    pub req_raw_items: Arc<Vec<SidebarReqItem>>,
    pub rows: Arc<Vec<FlatSidebarRow>>,
}

/// Borrowed inputs for [`flatten_sidebar_tree`], grouped so the builder keeps
/// a single parameter instead of a ten-argument signature.
pub struct FlatTreeInput<'a> {
    pub folders: &'a [Folder],
    pub raw_requests: &'a [SidebarReqItem],
    pub arc_requests: &'a [Arc<SidebarReqItem>],
    pub pins: &'a [String],
    pub query_lower: &'a str,
    pub collapsed_folders: &'a HashSet<String>,
    pub collapsed_requests: &'a HashSet<String>,
    pub pinned_collapsed: bool,
    pub git_status: Option<&'a GitStatusResult>,
    pub unsaved_requests: &'a HashSet<String>,
}

pub fn build_sidebar_req_items(
    requests: &[RequestItem],
) -> (Vec<SidebarReqItem>, Vec<Arc<SidebarReqItem>>) {
    let mut raw_vec = Vec::with_capacity(requests.len());
    let mut arc_vec = Vec::with_capacity(requests.len());

    for r in requests {
        let initial_name = {
            let eff = crate::state::effective_request_name(r);
            if !r.name.trim().is_empty() && r.name != "New Request" && r.name != "Untitled Request"
            {
                r.name.clone()
            } else {
                eff
            }
        };
        let examples = r
            .examples
            .iter()
            .map(|e| SidebarExampleItem {
                name: e.name.clone(),
                status: e.status,
            })
            .collect();

        let item = SidebarReqItem {
            id: r.id.clone(),
            folder_id: r.folder_id.clone(),
            name: r.name.clone(),
            display_name: initial_name,
            method: r.method,
            protocol: r.protocol,
            url: r.url.clone(),
            last_status_code: r.last_status_code,
            examples,
        };
        arc_vec.push(Arc::new(item.clone()));
        raw_vec.push(item);
    }

    (raw_vec, arc_vec)
}

/// Adjacency and match state shared by the recursive flatten helpers.
struct FlatTreeCtx<'a> {
    folders: &'a [Folder],
    query_lower: &'a str,
    child_folder_indices: HashMap<&'a str, Vec<usize>>,
    folder_req_indices: HashMap<&'a str, Vec<usize>>,
    req_matches: Vec<bool>,
    folder_matches: Vec<bool>,
    folder_statuses: Vec<SidebarGitStatus>,
    collapsed_folders: &'a HashSet<String>,
    has_query: bool,
}

impl FlatTreeCtx<'_> {
    /// Bottom-up folder search-match check with memoization, O(F + R).
    fn check_folder_match(
        &self,
        f_idx: usize,
        memo: &mut [Option<bool>],
        visited: &mut [bool],
    ) -> bool {
        if let Some(res) = memo[f_idx] {
            return res;
        }
        if visited[f_idx] {
            return false;
        }
        visited[f_idx] = true;

        let f = &self.folders[f_idx];
        let mut matched = f.name.to_lowercase().contains(self.query_lower);

        if !matched {
            if let Some(reqs) = self.folder_req_indices.get(f.id.as_str()) {
                if reqs.iter().any(|&r_idx| self.req_matches[r_idx]) {
                    matched = true;
                }
            }
        }

        if let Some(children) = self.child_folder_indices.get(f.id.as_str()) {
            for &child_idx in children {
                if self.check_folder_match(child_idx, memo, visited) {
                    matched = true;
                }
            }
        }

        memo[f_idx] = Some(matched);
        matched
    }

    fn emit_folder(
        &self,
        f_idx: usize,
        depth: usize,
        emit_request: &impl Fn(usize, usize, &mut Vec<FlatSidebarRow>),
        rows: &mut Vec<FlatSidebarRow>,
        visited: &mut [bool],
    ) {
        if visited[f_idx] || !self.folder_matches[f_idx] {
            return;
        }
        visited[f_idx] = true;

        let f = &self.folders[f_idx];
        let is_collapsed = self.collapsed_folders.contains(&f.id) && !self.has_query;
        rows.push(FlatSidebarRow::Folder {
            folder_id: f.id.clone(),
            name: f.name.clone(),
            depth,
            is_collapsed,
            git_status: self.folder_statuses[f_idx],
        });

        if !is_collapsed {
            if let Some(children) = self.child_folder_indices.get(f.id.as_str()) {
                for &child_idx in children {
                    self.emit_folder(child_idx, depth + 1, emit_request, rows, visited);
                }
            }

            if let Some(reqs) = self.folder_req_indices.get(f.id.as_str()) {
                for &r_idx in reqs {
                    if self.req_matches[r_idx] {
                        emit_request(r_idx, depth + 1, rows);
                    }
                }
            }
        }
    }
}

/// Flattens the workspace folder & request hierarchy into a flat list of uniform-height rows
/// in a single O(F + R + G) pass.
pub fn flatten_sidebar_tree(input: FlatTreeInput<'_>) -> Vec<FlatSidebarRow> {
    let FlatTreeInput {
        folders,
        raw_requests,
        arc_requests,
        pins,
        query_lower,
        collapsed_folders,
        collapsed_requests,
        pinned_collapsed,
        git_status,
        unsaved_requests,
    } = input;

    if folders.is_empty() && raw_requests.is_empty() {
        return Vec::new();
    }

    let has_query = !query_lower.is_empty();
    let git_index = GitStatusIndex::from_status(git_status);
    let (req_statuses, folder_statuses) =
        compute_all_git_statuses(folders, raw_requests, &git_index, unsaved_requests);

    let mut rows = Vec::with_capacity(folders.len() + raw_requests.len() + pins.len() + 2);

    // 1. Pinned requests section (O(P) via index map)
    if !pins.is_empty() {
        let req_by_id: HashMap<&str, usize> = raw_requests
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.as_str(), i))
            .collect();

        let pinned_indices: Vec<usize> = pins
            .iter()
            .filter_map(|pin_id| req_by_id.get(pin_id.as_str()).copied())
            .collect();

        if !pinned_indices.is_empty() {
            let is_collapsed = pinned_collapsed && !has_query;
            rows.push(FlatSidebarRow::PinnedHeader { is_collapsed });
            if !is_collapsed {
                for (pin_index, r_idx) in pinned_indices.into_iter().enumerate() {
                    rows.push(FlatSidebarRow::PinnedRequest {
                        req: arc_requests[r_idx].clone(),
                        pin_index,
                        git_status: req_statuses[r_idx],
                    });
                }
            }
        }
    }

    // 2. Build folder & request adjacency lists in O(F + R)
    let folder_id_set: HashSet<&str> = folders.iter().map(|f| f.id.as_str()).collect();
    let mut root_folder_indices = Vec::new();
    let mut child_folder_indices: HashMap<&str, Vec<usize>> = HashMap::with_capacity(folders.len());

    for (f_idx, f) in folders.iter().enumerate() {
        if let Some(pid) = f.parent_id.as_deref().filter(|p| folder_id_set.contains(p)) {
            child_folder_indices.entry(pid).or_default().push(f_idx);
        } else {
            root_folder_indices.push(f_idx);
        }
    }

    let mut req_matches = Vec::with_capacity(raw_requests.len());
    let mut root_req_indices = Vec::new();
    let mut folder_req_indices: HashMap<&str, Vec<usize>> = HashMap::with_capacity(folders.len());

    for (r_idx, r) in raw_requests.iter().enumerate() {
        let matched = if !has_query {
            true
        } else {
            r.name.to_lowercase().contains(query_lower)
                || r.url.to_lowercase().contains(query_lower)
                || r.examples
                    .iter()
                    .any(|e| e.name.to_lowercase().contains(query_lower))
        };
        req_matches.push(matched);

        if let Some(fid) = r.folder_id.as_deref().filter(|f| folder_id_set.contains(f)) {
            folder_req_indices.entry(fid).or_default().push(r_idx);
        } else {
            root_req_indices.push(r_idx);
        }
    }

    let mut ctx = FlatTreeCtx {
        folders,
        query_lower,
        child_folder_indices,
        folder_req_indices,
        req_matches,
        folder_matches: Vec::new(),
        folder_statuses,
        collapsed_folders,
        has_query,
    };

    // 3. Compute folder search match flags in a single bottom-up pass O(F + R)
    ctx.folder_matches = if !has_query {
        vec![true; folders.len()]
    } else {
        let mut memo: Vec<Option<bool>> = vec![None; folders.len()];
        let mut visited = vec![false; folders.len()];
        for f_idx in 0..folders.len() {
            ctx.check_folder_match(f_idx, &mut memo, &mut visited);
        }
        memo.into_iter().map(|m| m.unwrap_or(false)).collect()
    };

    // 4. Emit flattened rows via DFS
    let emit_request = |r_idx: usize, depth: usize, rows: &mut Vec<FlatSidebarRow>| {
        let req = &arc_requests[r_idx];
        let is_req_collapsed = collapsed_requests.contains(&req.id) && !has_query;
        rows.push(FlatSidebarRow::Request {
            req: req.clone(),
            depth,
            is_collapsed: is_req_collapsed,
            git_status: req_statuses[r_idx],
        });

        if !req.examples.is_empty() && !is_req_collapsed {
            let total_ex = req.examples.len();
            for (idx, ex) in req.examples.iter().enumerate() {
                rows.push(FlatSidebarRow::Example {
                    parent_id: req.id.clone(),
                    idx,
                    total_ex,
                    ex: ex.clone(),
                    depth: depth + 1,
                });
            }
        }
    };

    let mut visited_emit = vec![false; folders.len()];
    for f_idx in root_folder_indices {
        ctx.emit_folder(f_idx, 0, &emit_request, &mut rows, &mut visited_emit);
    }

    for r_idx in root_req_indices {
        if ctx.req_matches[r_idx] {
            emit_request(r_idx, 0, &mut rows);
        }
    }

    rows.push(FlatSidebarRow::RootDropzone);
    rows
}

#[cfg(test)]
#[path = "flat_tree_tests.rs"]
mod tests;
