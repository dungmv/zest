//! Read-only hierarchy and inheritance resolution (headers & auth) over [`CollectionState`].

use super::CollectionState;
use zest_core::models::{Auth, Folder, KeyValuePair, RequestItem};
use zest_core::transport::{resolve_inherited_headers, InheritedHeader};

/// The auth a request would effectively use when its own auth is `Auth::Inherit`,
/// together with the scope (folder or workspace) that supplies it.
#[derive(Debug, Clone, PartialEq)]
pub struct InheritedAuthInfo {
    pub source_name: String,
    /// `None` when the auth comes from the workspace scope.
    pub source_folder_id: Option<String>,
    pub auth: Auth,
}

/// Human-readable label for an `Auth` variant, shared by banner/card renderers.
pub fn auth_kind_label(auth: &Auth) -> &'static str {
    match auth {
        Auth::None => "No Auth",
        other => other.display_label(),
    }
}

impl CollectionState {
    pub fn get_folder_hierarchy(&self, folder_id: Option<&str>) -> Vec<&Folder> {
        let mut chain = Vec::new();
        let mut cur_id = folder_id.map(|s| s.to_string());
        while let Some(id) = cur_id {
            if let Some(folder) = self.folders.iter().find(|f| f.id == id) {
                chain.push(folder);
                cur_id = folder.parent_id.clone();
            } else {
                break;
            }
        }
        chain.reverse();
        chain
    }

    pub fn resolve_request_hierarchy(&self, req: &RequestItem) -> (Vec<KeyValuePair>, Auth) {
        let ws_headers = self.workspace_scope.headers.as_slice();
        let ws_auth = &self.workspace_scope.auth;

        let folder_chain_refs = self.get_folder_hierarchy(req.folder_id.as_deref());
        let folder_chain: Vec<(&[KeyValuePair], &Auth)> = folder_chain_refs
            .iter()
            .map(|f| (f.headers.as_slice(), &f.auth))
            .collect();

        zest_core::transport::resolve_hierarchical_headers_and_auth(
            ws_headers,
            ws_auth,
            &folder_chain,
            &req.headers,
            &req.auth,
        )
    }

    /// Projects the headers a request inherits from workspace/folders using the exact
    /// send-time merge semantics (nearest ancestor wins per key, case-insensitive).
    pub fn inherited_headers_for_request(&self, req: &RequestItem) -> Vec<InheritedHeader> {
        let ws_name = if self.workspace_scope.name.is_empty() {
            "Workspace"
        } else {
            self.workspace_scope.name.as_str()
        };
        let ws_label = format!("workspace \"{}\"", ws_name);
        let ws_headers = self.workspace_scope.headers.as_slice();
        let folder_chain_refs = self.get_folder_hierarchy(req.folder_id.as_deref());
        let chain_owned: Vec<(String, &[KeyValuePair])> = folder_chain_refs
            .iter()
            .map(|f| (format!("folder \"{}\"", f.name), f.headers.as_slice()))
            .collect();
        let chain: Vec<(&str, &[KeyValuePair])> = chain_owned
            .iter()
            .map(|(label, headers)| (label.as_str(), *headers))
            .collect();
        resolve_inherited_headers(&ws_label, ws_headers, &chain, &req.headers)
    }

    /// Resolves the auth a request inherits when its own auth is `Auth::Inherit`:
    /// nearest non-inherit folder auth, else workspace auth.
    pub fn inherited_auth_source(&self, folder_id: Option<&str>) -> Option<InheritedAuthInfo> {
        let folder_chain_refs = self.get_folder_hierarchy(folder_id);
        for f in folder_chain_refs.iter().rev() {
            if !f.auth.is_inherit() {
                return Some(InheritedAuthInfo {
                    source_name: f.name.clone(),
                    source_folder_id: Some(f.id.clone()),
                    auth: f.auth.clone(),
                });
            }
        }
        let ws_auth = &self.workspace_scope.auth;
        if ws_auth.is_inherit() {
            return None;
        }
        let ws_name = if self.workspace_scope.name.is_empty() {
            "Workspace".to_string()
        } else {
            self.workspace_scope.name.clone()
        };
        Some(InheritedAuthInfo {
            source_name: ws_name,
            source_folder_id: None,
            auth: ws_auth.clone(),
        })
    }

    pub fn get_inherited_auth_description(&self, folder_id: Option<&str>) -> String {
        match self.inherited_auth_source(folder_id) {
            Some(info) if info.source_folder_id.is_some() => format!(
                "Inherited from folder \"{}\" ({})",
                info.source_name,
                auth_kind_label(&info.auth)
            ),
            Some(info) => format!(
                "Inherited from workspace \"{}\" ({})",
                info.source_name,
                auth_kind_label(&info.auth)
            ),
            None => "No auth configured in parent hierarchy".to_string(),
        }
    }
}
