# Plan 2026112302 — Response Preview Renderer (Image & Sandboxed HTML)

> **Note:** This plan document is tracked in `docs/plans/` per AGENTS.md § 4.

## Goal

Implement the **Preview Renderer** from Phase 4 of `docs/roadmap.md` inside the Response Body tab (`ResponseInspector`), split into **2 independent tasks**:

1. **Task 1 — Image Preview Renderer**: Render PNG, JPEG, SVG, WebP, GIF (plus BMP/ICO/TIFF) responses natively via `gpui_kit::img(...)` and `gpui_kit::Image::from_bytes(...)`, with zoom/fit controls, dimension & size metadata, `.loc` binary-cache handling, and decode-error fallback.
2. **Task 2 — HTML Preview Renderer (Sandboxed)**: Render `text/html` and `application/xhtml+xml` responses natively via `gpui_kit::component::text::{TextView, TextViewState}` (`html5ever`-backed), completely sandboxed (no JS execution, `<script>`/`<style>` stripped, external image URLs blocked).

**Out of scope:** PDF viewer (skipped to reduce complexity) and Hex viewer (deferred).

## Context

- `HttpResponseRecord` already stores raw `body: Vec<u8>`, and `ResponseViewMode::Preview` already exists in `crates/app/src/state/response_state.rs`, but is not yet wired in the UI.
- `gpui-kit 0.10.0` already bundles:
  - `gpui_kit::Image` / `ImageFormat` / `img(...)` supporting PNG, JPEG, animated GIF, WebP, SVG (via `resvg`), BMP, ICO, TIFF.
  - `gpui_kit::component::text::{TextView, TextViewState}` supporting `html5ever`-based HTML rendering with `.selectable(true)`, `.scrollable(true)`, and `.image_source(...)`.
- Zero new Cargo dependencies are required for either task.
- `crates/app/src/components/response_inspector/body_tab.rs` is at 601 LOC, so preview UIs must live in dedicated sibling modules (`preview_image.rs`, `preview_html.rs`) to stay under the 650 LOC modularity limit.

## Architectural Decisions

| Decision | Choice | Rationale |
| :--- | :--- | :--- |
| Classification & dimension probing | Pure `crates/core/src/models/preview.rs` in `zest-core` | Core owns pure domain logic; 100% unit-tested without GPUI. |
| Image rendering | `Arc<gpui_kit::Image>` cached in `CachedResponseFormat` + `gpui_kit::img(...)` | Built once per response (`response_ptr`), zero per-frame decoding, zero extra crates. |
| HTML rendering | `InspectorDisplays::html_preview: Entity<TextViewState>` synced on `ResponseEvent::ContentChanged` | Preserves Dual-Channel Event Architecture (`PERF-006`) and pure `render()`. |
| HTML sandboxing | `html5ever` drops `<script>`/`<style>`; `.image_source(...)` blocks non-`data:` URIs | Prevents JS execution, tracking pixels, and local file reads. |
| Auto view-mode switching | Auto-switch to `Preview` on Image responses; fallback to `Pretty` on non-previewable responses; HTML stays `Pretty` with `Preview` 1 click away | Binary images are unreadable in `Pretty`/`Raw`; HTML error pages are often inspected as source first. |
| Size caps | 25 MB for Image preview; 2 MB for HTML preview | Protects UI thread and memory while keeping "Save to File" available. |
| `.loc` binary cache edge case | Detect `body.is_empty() && size_bytes > 0` | `.loc` files omit binary bodies (`storage/models.rs`); show a clear resend prompt. |

## Step-by-Step Implementation Tasks

### Task 1 — Image Preview Renderer

1. **`zest-core` preview classifier & dimension prober (`crates/core/src/models/preview.rs`)**:
   - Define `ImagePreviewFormat` (`Png`, `Jpeg`, `Webp`, `Gif`, `Svg`, `Bmp`, `Ico`, `Tiff`) and `ResponsePreviewKind` (`Image(ImagePreviewFormat)`, `Html`).
   - Implement `detect_preview_kind(headers, body)` with Content-Type priority + magic-byte sniffing fallback.
   - Implement bounds-checked `probe_image_dimensions(format, bytes)` for PNG, GIF, JPEG, WebP, BMP.
   - Expose `HttpResponseRecord::preview_kind(&self)` and enhance `suggested_filename()` fallback extension.
2. **`ResponseState` image cache & zoom state (`crates/app/src/state/response_state.rs`)**:
   - Extend `CachedResponseFormat` with `preview_kind`, `preview_image: Option<Arc<gpui_kit::Image>>`, and `image_dimensions: Option<(u32, u32)>`.
   - Skip multi-megabyte `from_utf8_lossy` formatting for binary raster images.
   - Add `ImageZoomMode` (`Fit`, `Scale(u16)`) with zoom methods + `reconcile_view_mode_for_response()`.
3. **Image Preview UI (`crates/app/src/components/response_inspector/preview_image.rs`)**:
   - Render `.loc`-uncached binary state, >25 MB oversized state, or framed `img(Arc<Image>)` with `ObjectFit::Contain` / scaled zoom, decode-error `.with_fallback(...)`, and bottom metadata + zoom toolbar.
4. **Toolbar integration (`crates/app/src/components/response_inspector/body_tab.rs`, `mod.rs`)**:
   - Show `Preview` button alongside `Pretty`/`Raw` when `preview_kind.is_some()`.
   - Hide text-only toolbar buttons (`Toggle line wrap`, `JSONPath`) in `Preview` mode; dispatch Image responses to `preview_image::render_image_preview`.

### Task 2 — HTML Preview Renderer (Sandboxed)

5. **HTML detection in `zest-core` (`crates/core/src/models/preview.rs`)**:
   - Match `text/html`, `application/xhtml+xml`, and `<!doctype html` / `<html` prefix fallback; add `HttpResponseRecord::is_html()`.
6. **Event-driven `TextViewState` sync (`crates/app/src/app/inspector_displays.rs`, `workspace_event_handler.rs`, `app/mod.rs`)**:
   - Add `pub html_preview: Entity<TextViewState>` to `InspectorDisplays` (`.selectable(true).scrollable(true)`).
   - Sync on `ResponseEvent::ContentChanged` via `sync_html_preview` (with pointer guard to avoid re-parsing on `Pretty`↔`Raw` toggles, and clearing to `""` for non-HTML responses).
   - Pass `html_preview` entity handle into `ResponseInspector::new`.
7. **Sandboxed HTML Preview UI (`crates/app/src/components/response_inspector/preview_html.rs`)**:
   - Render subtle sandbox notice bar + >2 MB guard + `TextView::new(&ri.html_preview)` with `.image_source(...)` blocking non-`data:` URIs.
   - Wire `Some(ResponsePreviewKind::Html)` in `body_tab.rs` `Preview` dispatch.
8. **Roadmap update (`docs/roadmap.md`)**:
   - Mark Image Preview and Sandboxed HTML Preview completed and note PDF viewer is skipped.

## Risk Mitigation

- Pure `render()`: zero per-frame image decoding or HTML parsing.
- Bounded memory: 25 MB image cap, 2 MB HTML cap, `html_preview` cleared when inactive.
- Sandboxed HTML: zero JS execution, `<script>`/`<style>` stripped, external image URLs blocked.
- Graceful fallbacks: corrupt images caught by `.with_fallback(...)`, `.loc` binary misses handled cleanly.

## Verification Steps

1. `cargo fmt --check`
2. `cargo test` (including unit tests for `preview.rs` and `response_state_tests.rs`)
3. `cargo clippy --all-targets -- -D warnings`
4. Manual checks: PNG/JPEG/WebP/GIF/SVG previews + zoom + dark/light theme + `.loc` reload; HTML preview + script/external-image sandboxing + switching back to JSON.
