# Implementation Plan: Index-Based Table State Architecture (No Hidden IDs)

**Date**: 2026-10-26  
**Status**: Proposed  
**Category**: Architecture Refactoring / Index-Based Table State  

---

## 1. Goal Description

Implement an index-based synchronization architecture for all dynamic tables (`KvTable` for Params, Headers, Form URL-Encoded, Scope Headers; `Multipart`; `EnvModal`) that keeps array indices as keys without adding hidden/synthetic `id` fields to `zest-core` domain models.

---

## 2. Architecture & Data Flow

```mermaid
flowchart TD
    subgraph Data ["Single Source of Truth (zest-core)"]
        Pairs["pairs: Vec<KeyValuePair> (No hidden IDs, pure index-based)"]
    end

    subgraph UIState ["Table-Level State (Entity<KvTableState>)"]
        Rows["rows: Vec<KvRowInputs> [Slot 0, Slot 1, Slot 2...]"]
    end

    subgraph Action ["User Action: Delete Row i"]
        Btn["Click Delete on Row i"]
        Btn -->|1. st.rows.remove(i)| Rows
        Btn -->|2. ws.remove_param(i)| Pairs
    end

    subgraph DynamicLookup ["Dynamic Index Resolution"]
        Typing["User types in Input"] --> Pos["st.rows.iter().position(...)"]
        Pos --> CurrentIdx["Current Array Index i (Always Accurate)"]
        CurrentIdx --> OnUp["on_update(i, ...)"]
    end
```

---

## 3. Key Design Decisions

1. **Table-Level Input State Entity (`KvTableState`)**:
   - Instead of calling `window.use_keyed_state` per row slot (`row-{idx}`), `KvTable` holds a single table entity `KvTableState` via `window.use_keyed_state("{id_prefix}-inputs-state")`.
   - `KvTableState` owns `rows: Vec<KvRowInputs>`. Index $i$ in `st.rows` corresponds directly to index $i$ in `self.pairs`.
2. **Synchronized Removal**:
   - When row $i$ is deleted:
     - `st.rows.remove(idx)` drops row $i$'s `InputState` from memory immediately.
     - `on_remove(idx)` removes index $i$ from the domain model.
     - Both arrays shift together in lockstep.
3. **Dynamic Index Resolution on Input Change**:
   - `InputEvent::Change` does not use a stale captured index. It queries `st.rows.iter().position(|r| r.key_input == entity)` to discover its actual current position in the array.
4. **100% Pure `render()`**:
   - No `set_value` calls inside `render()`.
   - Conforms strictly to AGENTS.md Rule 2.3.
5. **No Domain Model Churn**:
   - `zest-core` domain models (`KeyValuePair`, `MultipartItem`, `EnvironmentVariable`) remain 100% untouched.

---

## 4. Implementation Steps

1. **`crates/app/src/components/kv_table.rs`**:
   - Introduce `KvTableState` and `KvRowInputs`.
   - Replace per-row `use_keyed_state` with single `use_keyed_state` for `KvTableState`.
   - Wire delete button to `st.rows.remove(idx)` and `on_remove(idx)`.
   - Wire add button to `st.rows.push(...)` and `on_add()`.
   - Use dynamic position lookup for typing.
   - Clean up render-time `set_value` syncing logic and `Cell<bool>` hacks.
2. **`crates/app/src/components/body_editor/multipart.rs`**:
   - Implement `MpTableState` with synchronized removals and dynamic position lookups.
   - Remove render-time `set_value` syncing logic.
3. **`crates/app/src/components/env_modal.rs`**:
   - Implement `EnvVarTableState` with synchronized removals.
   - Remove render-time `set_value` syncing logic.

---

## 5. Verification Plan

- `cargo fmt --check`
- `cargo check --all-targets`
- `cargo test` (all 165+ tests pass)
- `cargo clippy --all-targets -- -D warnings` (0 warnings)
- Manual check: verify delete and edit across Params, Headers, Body Form, Multipart, and EnvModal.
