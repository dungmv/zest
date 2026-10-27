---
name: interop-spec-converters
description: >-
  Use this skill when developing, refactoring, or testing API specification importers,
  exporters (OpenAPI, Postman, Insomnia, cURL, HAR), and code generation in core and app.
---

# Interoperability, Importers & Spec Converters Guide

This skill governs data migration and conversion between external formats and Zest canonical domain models in `zest-core::importers` and `zest-core::curl`.

---

## 1. Format Support Matrix

| Format | Specification / Version | Status | Module / Spec Reference |
| :--- | :--- | :---: | :--- |
| **OpenAPI / Swagger** | OpenAPI 3.0, 3.1 (JSON & YAML) | ✅ Implemented | `crates/core/src/importers/openapi.rs` |
| **Postman Collection** | Postman v2.0, v2.1 (JSON) | ✅ Implemented | `crates/core/src/importers/postman.rs` |
| **cURL** | Bash, PowerShell (`` ` ``), CMD (`^`) parser & exporter | ✅ Implemented | `crates/core/src/curl/mod.rs` |
| **Insomnia** | Insomnia Export v4 (JSON & YAML) | 📋 Planned (Spec 01) | [`docs/specs/01-import-export-compatibility.md`](docs/specs/01-import-export-compatibility.md) |
| **HAR** | HTTP Archive 1.2 | 📋 Planned (Spec 01) | [`docs/specs/01-import-export-compatibility.md`](docs/specs/01-import-export-compatibility.md) |
| **Multi-Language Codegen** | Python, JS/TS, Rust, Go, Java, C#, PHP, HTTPie | 📋 Planned (Spec 06) | [`docs/specs/06-developer-tools-codegen-diff-docs.md`](docs/specs/06-developer-tools-codegen-diff-docs.md) |

---

## 2. OpenAPI 3.0 / 3.1 Importer Principles (`importers/openapi.rs`)

### 2.1 Iterative Cycle-Safe `$ref` Resolution (TD-014)
When resolving `$ref` pointers (e.g., `#/components/schemas/Pet`), never use unbounded recursion. Use an iterative loop with a `visited` set and a hard hop bound (max 32):
```rust
// ✅ MANDATORY: Iterative cycle-safe $ref resolution
fn resolve_ref<'a>(mut val: &'a Value, root: &'a Value) -> &'a Value {
    let mut visited = HashSet::new();
    for _ in 0..32 {
        if let Some(ref_str) = val.get("$ref").and_then(|v| v.as_str()) {
            if !visited.insert(ref_str) {
                break; // Circular reference detected
            }
            if let Some(stripped) = ref_str.strip_prefix("#/") {
                let mut curr = root;
                let mut found = true;
                for part in stripped.split('/') {
                    let unescaped = part.replace("~1", "/").replace("~0", "~");
                    if let Some(next) = curr.get(&unescaped) {
                        curr = next;
                    } else {
                        found = false;
                        break;
                    }
                }
                if found {
                    val = curr;
                    continue;
                }
            }
        }
        break;
    }
    val
}
```

### 2.2 Content-Type Extraction (TD-01)
Extract the first media type entry via `.into_iter().next()` rather than looping with an unconditional `break` (which triggers `clippy::never_loop`).

---

## 3. Workspace Import Application (`importers/mod.rs`)

`apply_import_to_workspace(store, result, options)` writes imported collections to disk safely:
- **Folder Ordering**: Sorts `result.folders` by path depth (`f.path.len()`) so parent folders are always created on disk before nested subfolders.
- **Enclosing Folder**: Supports wrapping the entire import inside a root collection folder when `options.create_enclosing_folder` is enabled.
- **Event Notification**: After running an import in `WorkspaceDataState::execute_import_current_workspace`, emit `WorkspaceEvent::RequestsReloaded { req_ids: None }` so the sidebar and active composer inputs synchronize immediately.

---

## 4. cURL Command Parser & Generator (`crates/core/src/curl/mod.rs`)

- **Auto-Detection on Paste**: `is_curl_command(text)` detects cURL commands pasted directly into the URL input bar (`subscriptions.rs`) or the Import Modal.
- **Cross-Shell Normalization**: Strips leading `$ ` / `> ` prompts and joins multiline continuations across Bash (`\`), PowerShell (`` ` ``), and Windows CMD (`^`).
- **Flag & Auth Mapping**: Parses `-X`/`--request`, `-H`/`--header`, `-d`/`--data`/`--data-raw`/`--data-binary`/`--data-urlencode`, `-F`/`--form`, `-u`/`--user` (`Auth::Basic`), and `Authorization: Bearer` (`Auth::Bearer`) directly into `HttpRequestItem`.
- **Exporter (`to_curl_command`)**: Reuses a single `VariableInterpolator` across URL, headers, auth, and body to generate clean multiline Bash cURL snippets.

---

## 5. Verification & Testing

```bash
# 1. Run OpenAPI and Postman importer tests
cargo test --package zest-core --lib importers::

# 2. Run cURL parser and exporter tests
cargo test --package zest-core --lib curl::

# 3. Verify with strict clippy
cargo clippy --package zest-core --all-targets -- -D warnings
```
