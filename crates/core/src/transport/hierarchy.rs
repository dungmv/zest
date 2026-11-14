//! Hierarchy resolution for the Workspace -> Folders -> Request inheritance chain.
//!
//! Both send-time merging ([`resolve_hierarchical_headers_and_auth`]) and the read-only
//! ancestor projection used by the composer UI ([`resolve_inherited_headers`]) share the
//! same merge machinery so the UI always mirrors what is actually sent.

use crate::models::{Auth, KeyValuePair};
use std::collections::{HashMap, HashSet};

/// A single ancestor header projected for read-only display: the merged pair, the name of
/// the scope (workspace or folder) that supplied the winning value, and whether the
/// request itself declares the same key (case-insensitive) and therefore overrides it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InheritedHeader {
    pub pair: KeyValuePair,
    pub source: String,
    pub overridden_by_request: bool,
}

/// Merges key/value pairs across scopes. Child overrides parent with case-insensitive key
/// matching; first-seen position is preserved. Empty/whitespace keys are skipped.
struct HeaderMerger<'a> {
    order: Vec<String>,
    map: HashMap<String, MergedEntry<'a>>,
}

struct MergedEntry<'a> {
    pair: &'a KeyValuePair,
    source: &'a str,
}

impl<'a> HeaderMerger<'a> {
    fn new() -> Self {
        Self {
            order: Vec::new(),
            map: HashMap::new(),
        }
    }

    fn apply(&mut self, kv: &'a KeyValuePair, source: &'a str) {
        let trimmed_key = kv.key.trim();
        if trimmed_key.is_empty() {
            return;
        }
        let lower = trimmed_key.to_lowercase();
        match self.map.get_mut(&lower) {
            Some(entry) => {
                entry.pair = kv;
                entry.source = source;
            }
            None => {
                self.order.push(lower.clone());
                self.map.insert(lower, MergedEntry { pair: kv, source });
            }
        }
    }

    fn apply_all(&mut self, pairs: &'a [KeyValuePair], source: &'a str) {
        for kv in pairs {
            self.apply(kv, source);
        }
    }

    /// Entries in first-seen order (workspace headers first, then folder chain order).
    fn into_entries(self) -> Vec<(&'a KeyValuePair, &'a str)> {
        let HeaderMerger { order, map } = self;
        order
            .into_iter()
            .filter_map(|k| map.get(&k).map(|e| (e.pair, e.source)))
            .collect()
    }
}

/// Merges headers and resolves Auth across Workspace -> Folders -> Request hierarchy.
/// - Headers: Child overrides parent with case-insensitive key matching. Preserves order.
/// - Auth: Child inherits parent's auth if `Auth::Inherit`. If none found, defaults to `Auth::None`.
pub fn resolve_hierarchical_headers_and_auth<'a>(
    workspace_headers: &'a [KeyValuePair],
    workspace_auth: &'a Auth,
    folder_chain: &'a [(&'a [KeyValuePair], &'a Auth)],
    request_headers: &'a [KeyValuePair],
    request_auth: &'a Auth,
) -> (Vec<KeyValuePair>, Auth) {
    // 1. Resolve Auth
    let mut resolved_auth = request_auth.clone();
    if resolved_auth.is_inherit() {
        let mut found_auth = None;
        for (_, f_auth) in folder_chain.iter().rev() {
            if !f_auth.is_inherit() {
                found_auth = Some((*f_auth).clone());
                break;
            }
        }
        resolved_auth = found_auth.unwrap_or_else(|| {
            if workspace_auth.is_inherit() {
                Auth::None
            } else {
                workspace_auth.clone()
            }
        });
    }

    // 2. Resolve Headers
    let mut merger = HeaderMerger::<'a>::new();
    merger.apply_all(workspace_headers, "Workspace");
    for (f_headers, _) in folder_chain {
        merger.apply_all(f_headers, "Folder");
    }
    merger.apply_all(request_headers, "Request");
    let merged_headers: Vec<KeyValuePair> = merger
        .into_entries()
        .into_iter()
        .map(|(pair, _)| pair.clone())
        .collect();

    (merged_headers, resolved_auth)
}

/// Projects the headers a request inherits from its ancestors (workspace + folder chain,
/// excluding the request's own rows) using the exact same merge semantics as
/// [`resolve_hierarchical_headers_and_auth`]:
/// - duplicate keys collapse case-insensitively, nearest ancestor wins and is credited
///   as the source;
/// - rows whose key is re-declared by the request are kept but flagged
///   `overridden_by_request` so the UI can dim them out;
/// - disabled rows are kept (they still shadow ancestor duplicates at send time and are
///   only dropped later by the executor);
/// - empty/whitespace keys are skipped.
pub fn resolve_inherited_headers(
    workspace_name: &str,
    workspace_headers: &[KeyValuePair],
    folder_chain: &[(&str, &[KeyValuePair])],
    request_headers: &[KeyValuePair],
) -> Vec<InheritedHeader> {
    let mut merger = HeaderMerger::new();
    merger.apply_all(workspace_headers, workspace_name);
    for (name, headers) in folder_chain {
        merger.apply_all(headers, name);
    }

    let request_keys: HashSet<String> = request_headers
        .iter()
        .filter(|h| !h.key.trim().is_empty())
        .map(|h| h.key.trim().to_lowercase())
        .collect();

    merger
        .into_entries()
        .into_iter()
        .map(|(pair, source)| InheritedHeader {
            overridden_by_request: request_keys.contains(&pair.key.trim().to_lowercase()),
            pair: pair.clone(),
            source: source.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_hierarchical_headers_and_auth() {
        use crate::models::Auth;

        let ws_headers = vec![
            KeyValuePair::new("X-Workspace", "ws-val"),
            KeyValuePair::new("X-Override-Me", "from-workspace"),
        ];
        let ws_auth = Auth::Bearer {
            token: "ws-token".to_string(),
        };

        let folder_headers = [
            KeyValuePair::new("X-Folder", "f-val"),
            KeyValuePair::new("X-Override-Me", "from-folder"),
        ];
        let folder_auth = Auth::Inherit;

        let req_headers = vec![
            KeyValuePair::new("X-Req", "req-val"),
            KeyValuePair::new("x-override-me", "from-request"), // Case-insensitive override
        ];
        let req_auth = Auth::Inherit;

        let folder_chain = [(&folder_headers[..], &folder_auth)];
        let (merged_headers, resolved_auth) = resolve_hierarchical_headers_and_auth(
            &ws_headers,
            &ws_auth,
            &folder_chain,
            &req_headers,
            &req_auth,
        );

        // Auth should inherit from workspace through folder
        assert_eq!(
            resolved_auth,
            Auth::Bearer {
                token: "ws-token".to_string()
            }
        );

        // Headers should be merged and overridden correctly
        assert_eq!(merged_headers.len(), 4);
        let override_hdr = merged_headers
            .iter()
            .find(|h| h.key.eq_ignore_ascii_case("x-override-me"))
            .unwrap();
        assert_eq!(override_hdr.value, "from-request");

        // Folder auth override
        let folder_auth_override = Auth::Basic {
            username: "admin".to_string(),
            password: "secret".to_string(),
        };
        let folder_chain_2 = [(&folder_headers[..], &folder_auth_override)];
        let (_, resolved_auth_2) = resolve_hierarchical_headers_and_auth(
            &ws_headers,
            &ws_auth,
            &folder_chain_2,
            &req_headers,
            &req_auth,
        );
        assert_eq!(
            resolved_auth_2,
            Auth::Basic {
                username: "admin".to_string(),
                password: "secret".to_string()
            }
        );

        // Request explicit None auth override
        let req_none_auth = Auth::None;
        let (_, resolved_auth_3) = resolve_hierarchical_headers_and_auth(
            &ws_headers,
            &ws_auth,
            &folder_chain_2,
            &req_headers,
            &req_none_auth,
        );
        assert_eq!(resolved_auth_3, Auth::None);
    }

    fn kv(key: &str, value: &str) -> KeyValuePair {
        KeyValuePair::new(key, value)
    }

    #[test]
    fn test_inherited_headers_sources_and_override_flag() {
        let ws_headers = vec![kv("X-Workspace", "ws-val"), kv("X-Shared", "from-ws")];
        let folder_root = [kv("X-Shared", "from-root"), kv("X-Root", "root-val")];
        let folder_nested = [kv("X-Nested", "nested-val")];
        // Root-first chain, mirroring WorkspaceDataState::get_folder_hierarchy.
        let chain: Vec<(&str, &[KeyValuePair])> =
            vec![("Root", &folder_root), ("Nested", &folder_nested)];
        let req_headers = vec![kv("x-shared", "from-request")];

        let rows = resolve_inherited_headers("My Workspace", &ws_headers, &chain, &req_headers);

        assert_eq!(rows.len(), 4);
        let by_key = |k: &str| {
            rows.iter()
                .find(|r| r.pair.key.eq_ignore_ascii_case(k))
                .unwrap()
        };

        let ws_row = by_key("X-Workspace");
        assert_eq!(ws_row.source, "My Workspace");
        assert_eq!(ws_row.pair.value, "ws-val");
        assert!(!ws_row.overridden_by_request);

        // Nearest ancestor (folder "Root") wins over workspace for the shared key.
        let shared = by_key("X-Shared");
        assert_eq!(shared.pair.value, "from-root");
        assert_eq!(shared.source, "Root");
        assert!(shared.overridden_by_request);

        assert_eq!(by_key("X-Root").source, "Root");
        assert_eq!(by_key("X-Nested").source, "Nested");
    }

    #[test]
    fn test_inherited_headers_skips_empty_keys_and_keeps_disabled() {
        let mut disabled = kv("X-Disabled", "off");
        disabled.enabled = false;
        let ws_headers = vec![kv("  ", "empty-key"), disabled];

        let rows = resolve_inherited_headers("Workspace", &ws_headers, &[], &[]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pair.key, "X-Disabled");
        assert!(!rows[0].pair.enabled);
        assert!(!rows[0].overridden_by_request);
    }

    #[test]
    fn test_inherited_headers_disabled_row_still_shadows_ancestor() {
        // Send-time semantics: a disabled child row still masks an ancestor row with the
        // same key; the executor drops it later. The projection must match that.
        let ws_headers = vec![kv("X-Masked", "from-ws")];
        let mut disabled = kv("X-Masked", "from-folder");
        disabled.enabled = false;
        let folder = [disabled];

        let rows = resolve_inherited_headers("Workspace", &ws_headers, &[("F", &folder)], &[]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pair.value, "from-folder");
        assert!(!rows[0].pair.enabled);
        assert_eq!(rows[0].source, "F");
    }

    #[test]
    fn test_inherited_headers_empty_hierarchy() {
        assert!(resolve_inherited_headers("Workspace", &[], &[], &[]).is_empty());
    }
}
