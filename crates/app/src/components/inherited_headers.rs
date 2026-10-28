use crate::components::kv_table::KvReadonlyRow;
use crate::state::WorkspaceDataState;
use zest_core::models::{ApiKeyLocation, Auth, HttpRequestItem};

/// Masks an inherited secret for read-only display with a fixed-length placeholder so the
/// value length is not leaked. Values that reference an environment variable (contain
/// `{{`) are shown verbatim: they reference configuration rather than embed the secret.
pub(crate) fn mask_secret(value: &str) -> String {
    if value.trim().is_empty() || value.contains("{{") {
        value.to_string()
    } else {
        "••••••••".to_string()
    }
}

/// Builds the read-only rows appended to the Headers `KvTable`:
/// - ancestor headers, using the exact send-time merge semantics. Rows the request
///   overrides (case-insensitive key match) are excluded — they are not sent, so
///   showing them as inherited values would be misleading.
/// - a preview of the header contributed by inherited auth (e.g. Bearer ->
///   `Authorization: Bearer ***`), when the request auth is `Inherit`.
pub fn inherited_readonly_rows(
    ws: &WorkspaceDataState,
    req: &HttpRequestItem,
) -> Vec<KvReadonlyRow> {
    let mut rows: Vec<KvReadonlyRow> = ws
        .inherited_headers_for_request(req)
        .into_iter()
        .filter(|h| !h.overridden_by_request)
        .map(|h| KvReadonlyRow {
            key: h.pair.key,
            value: h.pair.value,
            enabled: h.pair.enabled,
            source_detail: format!("Inherited from {}", h.source),
        })
        .collect();

    if req.auth.is_inherit() {
        if let Some(info) = ws.inherited_auth_source(req.folder_id.as_deref()) {
            let scope = if info.source_folder_id.is_some() {
                format!("folder \"{}\"", info.source_name)
            } else {
                format!("workspace \"{}\"", info.source_name)
            };
            let source_detail = format!("Added by inherited auth from {scope}");
            match &info.auth {
                Auth::Bearer { token } => rows.push(KvReadonlyRow {
                    key: "Authorization".to_string(),
                    value: format!("Bearer {}", mask_secret(token)),
                    enabled: true,
                    source_detail,
                }),
                Auth::Basic { username, password } => rows.push(KvReadonlyRow {
                    key: "Authorization".to_string(),
                    value: format!("Basic {}:{}", username, mask_secret(password)),
                    enabled: true,
                    source_detail,
                }),
                Auth::ApiKey {
                    key,
                    value,
                    add_to: ApiKeyLocation::Header,
                } => rows.push(KvReadonlyRow {
                    key: key.clone(),
                    value: mask_secret(value),
                    enabled: true,
                    source_detail,
                }),
                // None adds no header; API key in query params is not a header.
                _ => {}
            }
        }
    }

    rows
}
