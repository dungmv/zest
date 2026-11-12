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

    let rows = flatten_sidebar_tree(FlatTreeInput {
        folders: &folders,
        raw_requests: &raw_reqs,
        arc_requests: &arc_reqs,
        pins: &pins,
        query_lower: "",
        collapsed_folders: &collapsed_folders,
        collapsed_requests: &collapsed_requests,
        pinned_collapsed: false,
        git_status: None,
        unsaved_requests: &unsaved,
    });

    // Expected: PinnedHeader, PinnedRequest(health), Folder(auth, depth 0),
    // Folder(auth/oauth, depth 1), Request(token, depth 2), Example(200 OK, depth 3),
    // Request(health, depth 0), RootDropzone -> 8 rows
    assert_eq!(rows.len(), 8);
    assert!(matches!(
        rows[0],
        FlatSidebarRow::PinnedHeader {
            is_collapsed: false
        }
    ));
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
    let rows_collapsed = flatten_sidebar_tree(FlatTreeInput {
        folders: &folders,
        raw_requests: &raw_reqs,
        arc_requests: &arc_reqs,
        pins: &pins,
        query_lower: "",
        collapsed_folders: &collapsed_folders,
        collapsed_requests: &collapsed_requests,
        pinned_collapsed: false,
        git_status: None,
        unsaved_requests: &unsaved,
    });
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

    // Test collapsing pinned folder: PinnedRequest should be hidden
    let rows_pinned_collapsed = flatten_sidebar_tree(FlatTreeInput {
        folders: &folders,
        raw_requests: &raw_reqs,
        arc_requests: &arc_reqs,
        pins: &pins,
        query_lower: "",
        collapsed_folders: &collapsed_folders,
        collapsed_requests: &collapsed_requests,
        pinned_collapsed: true,
        git_status: None,
        unsaved_requests: &unsaved,
    });
    // Expected: PinnedHeader (collapsed=true), Folder(auth, collapsed=true), Request(health), RootDropzone -> 4 rows
    assert_eq!(rows_pinned_collapsed.len(), 4);
    assert!(matches!(
        rows_pinned_collapsed[0],
        FlatSidebarRow::PinnedHeader { is_collapsed: true }
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
    let rows = flatten_sidebar_tree(FlatTreeInput {
        folders: &folders,
        raw_requests: &raw_reqs,
        arc_requests: &arc_reqs,
        pins: &[],
        query_lower: "",
        collapsed_folders: &HashSet::new(),
        collapsed_requests: &HashSet::new(),
        pinned_collapsed: false,
        git_status: Some(&git_status),
        unsaved_requests: &HashSet::new(),
    });
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
