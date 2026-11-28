use super::*;
use crate::models::HttpMethod;

#[test]
fn ancestry_and_repath_keep_request_references_aligned() {
    let mut parent = Folder::new("workspace", "Old");
    parent.id = "old".into();
    let mut child = Folder::new("workspace", "Child");
    child.id = "old/child".into();
    child.parent_id = Some(parent.id.clone());
    let mut folders = vec![parent, child];
    let mut request = RequestItem::new("workspace", "Request", HttpMethod::Get, "");
    request.id = "old/child/request.yaml".into();
    request.folder_id = Some("old/child".into());
    let mut requests = vec![request];
    let mut pins = vec![requests[0].id.clone()];
    let mut errors = HashMap::from([(requests[0].id.clone(), "Invalid file".into())]);

    assert_eq!(folder_ancestors(&folders, Some("old/child"))[0].id, "old");
    let remapped = repath_subtree(
        &mut folders,
        &mut requests,
        &mut pins,
        &mut errors,
        "old",
        "new",
    );
    assert_eq!(remapped.len(), 3);
    assert_eq!(requests[0].id, "new/child/request.yaml");
    assert_eq!(requests[0].folder_id.as_deref(), Some("new/child"));
    assert_eq!(folders[1].parent_id.as_deref(), Some("new"));
    assert_eq!(pins, ["new/child/request.yaml"]);
    assert!(errors.contains_key("new/child/request.yaml"));
}
