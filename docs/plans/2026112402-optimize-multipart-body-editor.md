# Plan: Optimize Multipart Body Editor Component

- **Date**: 2026-11-24
- **Sequence**: 02
- **Artifact**: `docs/plans/2026112402-optimize-multipart-body-editor.md`
- **Scope**: `crates/app/src/components/body_editor/multipart/`

---

## 1. Goal
Optimize the multipart form-data body editor component, eliminating responsibility overlap between `state.rs` and `row.rs`, reducing boilerplate, improving modularity, and consolidating state definitions into `mod.rs` (following the cohesive pattern of `kv_table.rs`):
- `mod.rs`: Consolidates `MpTableState`, `MpRowInputs`, `reconcile`, and `render_multipart_editor`.
- `row.rs`: Pure row presentation element dispatching UI actions via `RowHandles::mutate`.
- `file_picker.rs`: Focused native async file dialog widget.
- `state.rs`: Removed by merging into `mod.rs`.

---

## 2. Context & Root Cause Analysis
Previously:
- `row.rs` and `state.rs` both held handles to `WorkspaceDataState` and `RequestSessionState`.
- `state.rs` was a standalone file replicating the `KvTableState` state management pattern.
- Consolidating `state.rs` into `mod.rs` matches the pattern used by `kv_table.rs`, keeping the state model directly adjacent to the editor entrypoint while maintaining row presentation modularity.

---

## 3. Architectural Decisions
1. **Merge `state.rs` into `mod.rs`**:
   - `mod.rs` now contains `MpRowSnapshot`, `RowMeta`, `MpRowInputs`, and `MpTableState` alongside `render_multipart_editor`.
   - `row.rs` and `file_picker.rs` remain modular child submodules (`super::MpTableState`, `super::MpRowSnapshot`).
   - Remove `crates/app/src/components/body_editor/multipart/state.rs`.

2. **Clean Modularity & LOC Balance**:
   - `mod.rs`: ~370 LOC (well below the ~650 LOC ceiling in AGENTS.md).
   - `row.rs`: ~208 LOC.
   - `file_picker.rs`: ~95 LOC.

---

## 4. Step-by-Step Implementation Tasks
1. Move `MpRowSnapshot`, `RowMeta`, `MpRowInputs`, and `MpTableState` from `state.rs` into `crates/app/src/components/body_editor/multipart/mod.rs`.
2. Update imports in `row.rs` and `file_picker.rs` to refer to `super::{MpRowSnapshot, MpTableState}` instead of `super::state`.
3. Delete `crates/app/src/components/body_editor/multipart/state.rs`.
4. Verify compilation, clippy (`-D warnings`), formatting, and test suite.
