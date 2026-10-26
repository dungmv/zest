use super::git_status::{compute_all_git_statuses, GitStatusIndex, SidebarGitStatus};
use super::request_node::{SidebarExampleItem, SidebarReqItem};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use zest_core::git::GitStatusResult;
use zest_core::models::{Folder, HttpRequestItem};

/// Represents a single uniform-height row in the virtualized sidebar tree.
#[derive(Clone, Debug)]
pub enum FlatSidebarRow {
    PinnedHeader,
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

pub fn build_sidebar_req_items(
    requests: &[HttpRequestItem],
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
            url: r.url.clone(),
            last_status_code: r.last_status_code,
            examples,
        };
        arc_vec.push(Arc::new(item.clone()));
        raw_vec.push(item);
    }

    (raw_vec, arc_vec)
}

/// Flattens the workspace folder & request hierarchy into a flat list of uniform-height rows
/// in a single O(F + R + G) pass.
#[allow(clippy::too_many_arguments)]
pub fn flatten_sidebar_tree(
    folders: &[Folder],
    raw_requests: &[SidebarReqItem],
    arc_requests: &[Arc<SidebarReqItem>],
    pins: &[String],
    query_lower: &str,
    collapsed_folders: &HashSet<String>,
    collapsed_requests: &HashSet<String>,
    git_status: Option<&GitStatusResult>,
    unsaved_requests: &HashSet<String>,
) -> Vec<FlatSidebarRow> {
    if folders.is_empty() && raw_requests.is_empty() {
        return Vec::new();
    }

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
            rows.push(FlatSidebarRow::PinnedHeader);
            for (pin_index, r_idx) in pinned_indices.into_iter().enumerate() {
                rows.push(FlatSidebarRow::PinnedRequest {
                    req: arc_requests[r_idx].clone(),
                    pin_index,
                    git_status: req_statuses[r_idx],
                });
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

    let has_query = !query_lower.is_empty();
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

    // 3. Compute folder search match flags in a single bottom-up pass O(F + R)
    let folder_matches: Vec<bool> = if !has_query {
        vec![true; folders.len()]
    } else {
        let mut memo: Vec<Option<bool>> = vec![None; folders.len()];
        let mut visited = vec![false; folders.len()];

        #[allow(clippy::too_many_arguments)]
        fn check_folder_match(
            f_idx: usize,
            folders: &[Folder],
            child_folder_indices: &HashMap<&str, Vec<usize>>,
            folder_req_indices: &HashMap<&str, Vec<usize>>,
            req_matches: &[bool],
            query_lower: &str,
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

            let f = &folders[f_idx];
            let mut matched = f.name.to_lowercase().contains(query_lower);

            if !matched {
                if let Some(reqs) = folder_req_indices.get(f.id.as_str()) {
                    if reqs.iter().any(|&r_idx| req_matches[r_idx]) {
                        matched = true;
                    }
                }
            }

            if let Some(children) = child_folder_indices.get(f.id.as_str()) {
                for &child_idx in children {
                    if check_folder_match(
                        child_idx,
                        folders,
                        child_folder_indices,
                        folder_req_indices,
                        req_matches,
                        query_lower,
                        memo,
                        visited,
                    ) {
                        matched = true;
                    }
                }
            }

            memo[f_idx] = Some(matched);
            matched
        }

        for f_idx in 0..folders.len() {
            check_folder_match(
                f_idx,
                folders,
                &child_folder_indices,
                &folder_req_indices,
                &req_matches,
                query_lower,
                &mut memo,
                &mut visited,
            );
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

    #[allow(clippy::too_many_arguments)]
    fn emit_folder<'a>(
        f_idx: usize,
        depth: usize,
        folders: &'a [Folder],
        child_folder_indices: &HashMap<&'a str, Vec<usize>>,
        folder_req_indices: &HashMap<&'a str, Vec<usize>>,
        folder_matches: &[bool],
        req_matches: &[bool],
        folder_statuses: &[SidebarGitStatus],
        collapsed_folders: &HashSet<String>,
        has_query: bool,
        emit_request: &impl Fn(usize, usize, &mut Vec<FlatSidebarRow>),
        rows: &mut Vec<FlatSidebarRow>,
        visited: &mut [bool],
    ) {
        if visited[f_idx] || !folder_matches[f_idx] {
            return;
        }
        visited[f_idx] = true;

        let f = &folders[f_idx];
        let is_collapsed = collapsed_folders.contains(&f.id) && !has_query;
        rows.push(FlatSidebarRow::Folder {
            folder_id: f.id.clone(),
            name: f.name.clone(),
            depth,
            is_collapsed,
            git_status: folder_statuses[f_idx],
        });

        if !is_collapsed {
            if let Some(children) = child_folder_indices.get(f.id.as_str()) {
                for &child_idx in children {
                    emit_folder(
                        child_idx,
                        depth + 1,
                        folders,
                        child_folder_indices,
                        folder_req_indices,
                        folder_matches,
                        req_matches,
                        folder_statuses,
                        collapsed_folders,
                        has_query,
                        emit_request,
                        rows,
                        visited,
                    );
                }
            }

            if let Some(reqs) = folder_req_indices.get(f.id.as_str()) {
                for &r_idx in reqs {
                    if req_matches[r_idx] {
                        emit_request(r_idx, depth + 1, rows);
                    }
                }
            }
        }
    }

    let mut visited_emit = vec![false; folders.len()];
    for f_idx in root_folder_indices {
        emit_folder(
            f_idx,
            0,
            folders,
            &child_folder_indices,
            &folder_req_indices,
            &folder_matches,
            &req_matches,
            &folder_statuses,
            collapsed_folders,
            has_query,
            &emit_request,
            &mut rows,
            &mut visited_emit,
        );
    }

    for r_idx in root_req_indices {
        if req_matches[r_idx] {
            emit_request(r_idx, 0, &mut rows);
        }
    }

    rows.push(FlatSidebarRow::RootDropzone);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use zest_core::git::{GitFileStatus, GitStatusItem, GitStatusResult};
    use zest_core::models::{Auth, HttpMethod};

    fn make_folder(id: &str, parent_id: Option<&str>, name: &str) -> Folder {
        Folder {
            id: id.to_string(),
            workspace_id: "ws".to_string(),
            parent_id: parent_id.map(ToString::to_string),
            name: name.to_string(),
            sort_order: 0,
            headers: Vec::new(),
            auth: Auth::Inherit,
        }
    }

    fn make_req(
        id: &str,
        folder_id: Option<&str>,
        name: &str,
        examples: Vec<SidebarExampleItem>,
    ) -> SidebarReqItem {
        SidebarReqItem {
            id: id.to_string(),
            folder_id: folder_id.map(ToString::to_string),
            name: name.to_string(),
            display_name: name.to_string(),
            method: HttpMethod::Get,
            url: format!("https://api.example.com/{}", id),
            last_status_code: Some(200),
            examples,
        }
    }

    #[test]
    fn test_flatten_sidebar_tree_hierarchy_and_collapse() {
        let folders = vec![
            make_folder("auth", None, "Auth"),
            make_folder("auth/oauth", Some("auth"), "OAuth"),
        ];
        let raw_reqs = vec![
            make_req(
                "auth/oauth/token.yaml",
                Some("auth/oauth"),
                "Get Token",
                vec![SidebarExampleItem {
                    name: "200 OK".to_string(),
                    status: 200,
                }],
            ),
            make_req("health.yaml", None, "Health Check", vec![]),
        ];
        let arc_reqs: Vec<_> = raw_reqs.iter().cloned().map(Arc::new).collect();
        let pins = vec!["health.yaml".to_string()];
        let collapsed_folders = HashSet::new();
        let collapsed_requests = HashSet::new();
        let unsaved = HashSet::from(["auth/oauth/token.yaml".to_string()]);

        let rows = flatten_sidebar_tree(
            &folders,
            &raw_reqs,
            &arc_reqs,
            &pins,
            "",
            &collapsed_folders,
            &collapsed_requests,
            None,
            &unsaved,
        );

        // Expected: PinnedHeader, PinnedRequest(health), Folder(auth, depth 0),
        // Folder(auth/oauth, depth 1), Request(token, depth 2), Example(200 OK, depth 3),
        // Request(health, depth 0), RootDropzone -> 8 rows
        assert_eq!(rows.len(), 8);
        assert!(matches!(rows[0], FlatSidebarRow::PinnedHeader));
        assert!(matches!(
            &rows[2],
            FlatSidebarRow::Folder {
                folder_id,
                depth: 0,
                git_status: SidebarGitStatus::Modified,
                ..
            } if folder_id == "auth"
        ));
        assert!(matches!(
            &rows[3],
            FlatSidebarRow::Folder {
                folder_id,
                depth: 1,
                git_status: SidebarGitStatus::Modified,
                ..
            } if folder_id == "auth/oauth"
        ));
        assert!(matches!(
            &rows[4],
            FlatSidebarRow::Request {
                depth: 2,
                git_status: SidebarGitStatus::Modified,
                ..
            }
        ));
        assert!(matches!(&rows[5], FlatSidebarRow::Example { depth: 3, .. }));

        // Collapse root "auth" folder: children should be hidden, but folder git_status remains Modified
        let collapsed_folders = HashSet::from(["auth".to_string()]);
        let rows_collapsed = flatten_sidebar_tree(
            &folders,
            &raw_reqs,
            &arc_reqs,
            &pins,
            "",
            &collapsed_folders,
            &collapsed_requests,
            None,
            &unsaved,
        );
        // Expected: PinnedHeader, PinnedRequest, Folder(auth, collapsed=true), Request(health), RootDropzone -> 5 rows
        assert_eq!(rows_collapsed.len(), 5);
        assert!(matches!(
            &rows_collapsed[2],
            FlatSidebarRow::Folder {
                folder_id,
                is_collapsed: true,
                git_status: SidebarGitStatus::Modified,
                ..
            } if folder_id == "auth"
        ));
    }

    #[test]
    fn test_flatten_sidebar_large_collection_performance() {
        // 80 folders (nested up to 4 levels) + 500 requests + 500 git status items
        let mut folders = Vec::with_capacity(80);
        for i in 0..20 {
            let root_id = format!("f_{}", i);
            folders.push(make_folder(&root_id, None, &format!("Folder {}", i)));
            for j in 0..3 {
                let sub_id = format!("{}/sub_{}", root_id, j);
                folders.push(make_folder(
                    &sub_id,
                    Some(&root_id),
                    &format!("Sub {}-{}", i, j),
                ));
            }
        }

        let mut raw_reqs = Vec::with_capacity(500);
        let mut untracked = Vec::with_capacity(500);
        for i in 0..500 {
            let folder_idx = i % folders.len();
            let fid = folders[folder_idx].id.clone();
            let req_id = format!("{}/req_{}.yaml", fid, i);
            raw_reqs.push(make_req(
                &req_id,
                Some(&fid),
                &format!("Request Item {}", i),
                vec![SidebarExampleItem {
                    name: "Default 200".to_string(),
                    status: 200,
                }],
            ));
            untracked.push(GitStatusItem {
                path: format!("collections/{}", req_id),
                status: GitFileStatus::Untracked,
                staged: false,
            });
        }
        let arc_reqs: Vec<_> = raw_reqs.iter().cloned().map(Arc::new).collect();
        let git_status = GitStatusResult {
            is_repo: true,
            untracked_files: untracked,
            ..Default::default()
        };

        let start = std::time::Instant::now();
        let rows = flatten_sidebar_tree(
            &folders,
            &raw_reqs,
            &arc_reqs,
            &[],
            "",
            &HashSet::new(),
            &HashSet::new(),
            Some(&git_status),
            &HashSet::new(),
        );
        let elapsed = start.elapsed();

        // 80 folders + 500 requests + 500 examples + 1 RootDropzone = 1081 rows
        assert_eq!(rows.len(), 1081);
        // Should easily complete under 25ms even in unoptimized debug builds
        assert!(
            elapsed.as_millis() < 50,
            "Flattening 500 requests took too long: {:?}",
            elapsed
        );
    }
}
