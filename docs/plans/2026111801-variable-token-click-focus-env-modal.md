# Plan: Chuyển trỏ chuột vào đúng variable khi click vào variable token trong input

- **Ngày**: 2026-11-18
- **Trạng thái**: Đang triển khai
- **Phạm vi**: `crates/app/src/` (`environment_state`, `modal_state`, `env_autocomplete`, `env_input`, `request_bar`, `env_modal/var_table`)

---

## 1. Goal

Khi người dùng click vào một variable token (`{{var_name}}`) trong thanh URL (`request_bar.rs`) hoặc trong bất kỳ input hỗ trợ biến môi trường nào (`env_input.rs` - header, params, auth, multipart body...):
1. Tự động xác định môi trường chứa biến đó (ưu tiên Active Environment, rồi đến Globals, rồi đến các Custom Environment khác). Nếu biến chưa tồn tại, tự động thêm biến vào Active Environment (hoặc Globals nếu không có active env).
2. Mở Environment Modal và chọn đúng tab môi trường tương ứng (`selected_id`).
3. Tự động cuộn danh sách (`ScrollHandle::scroll_to_item`) đến hàng chứa variable được click.
4. Chuyển focus / trỏ chuột (`focus(window, cx)`) vào input của variable (ô `CURRENT VALUE`), đồng thời bôi đen toàn bộ giá trị (`select_all`) để người dùng có thể xem hoặc gõ giá trị mới ngay lập tức.
5. Highlight viền hàng của variable được chọn (`target_variable`) bằng màu primary để người dùng nhận diện trực quan ngay lập tức.

## 2. Context & Root Cause

Hiện tại, trong `env_input.rs` và `request_bar.rs`:
```rust
.on_token_click(move |_, _window, cx| {
    if let Some(modal) = &modal_for_click {
        modal.update(cx, |st, cx| {
            st.toggle_env_modal(cx);
        });
    }
})
```
- Sự kiện `.on_token_click` chỉ gọi `toggle_env_modal`, hoàn toàn bỏ qua `InlineTokenClickEvent` (bỏ qua tên token `ev.token().id()`).
- Không xác định môi trường nào sở hữu biến, dẫn đến việc modal mở ra tab mặc định (hoặc tab trước đó) thay vì tab có biến đó.
- `EnvVarTable` trong `components/env_modal/var_table.rs` không có `ScrollHandle` và không có cơ chế bắt `target_variable` để focus hay scroll.
- Con trỏ chuột vẫn giữ nguyên ở input bên ngoài, người dùng phải tự tìm biến và click bằng tay.

## 3. Architectural Decisions

1. **State Extensions (`EnvironmentState` & `ModalState`)**:
   - `EnvironmentState`:
     - Thêm `resolve_variable_env_id(&self, var_name: &str) -> String`: tìm env id theo thứ tự active -> globals -> custom envs -> fallback.
     - Thêm `has_variable(&self, env_id: &str, var_name: &str) -> bool`: kiểm tra biến đã tồn tại trong env hay chưa.
     - Thêm `add_variable_with_key(&mut self, env_id: &str, key: String, cx: &mut Context<Self>)`: tạo biến mới với key cho trước nếu chưa có.
   - `ModalState`:
     - Thêm `target_variable: Option<String>` vào `EnvModalUi`.
     - Thêm `open_env_modal_for_variable(&mut self, env_id: Option<String>, var_name: String, cx: &mut Context<Self>)`: mở modal, đặt `selected_id` và `target_variable`, emit `ModalEvent::EnvModalSelectionChanged`.
     - Reset `target_variable = None` khi đóng modal (`close_env_modal`) hoặc mở thông thường (`open_env_modal`).

2. **Unified Variable Click Handler (`handle_variable_token_click`)**:
   - Đặt trong `components/completion/env_autocomplete/mod.rs` (dùng chung cho cả `render_env_input` và `request_bar`).
   - Với dynamic fake variables (`$timestamp`, `$uuid`...): mở modal thông thường.
   - Với environment variables: tìm `env_id`, thêm biến nếu chưa có, gọi `open_env_modal_for_variable`.

3. **`EnvVarTable` Integration**:
   - Bổ sung `scroll_handle: gpui_kit::ScrollHandle` và `target_variable: Option<String>` vào struct `EnvVarTable`.
   - Trong `subscribe_in(&modal_state, ...)` và `subscribe_in(&env_state, ...)`: sau khi rebuild/reconcile, gọi `focus_target_variable(window, cx)`.
   - `focus_target_variable`: tìm vị trí `idx` của biến trong `self.rows`, gọi `scroll_handle.scroll_to_item(idx)`, gọi `curr_input.update(cx, |inp, cx| { inp.select_all(window, cx); inp.focus(window, cx); })`.
   - Trong `render()`: gắn `.track_scroll(&self.scroll_handle)` vào container danh sách hàng; khi một hàng là `target_variable`, thêm border màu primary và background highlight.

## 4. Step-by-Step Implementation Tasks

1. **Task 1: Cập nhật `EnvironmentState`** (`crates/app/src/state/environment_state.rs`):
   - Thêm `resolve_variable_env_id`, `has_variable`, `add_variable_with_key`.
2. **Task 2: Cập nhật `ModalState`** (`crates/app/src/state/modal_state.rs`):
   - Thêm field `target_variable: Option<String>` vào `EnvModalUi`.
   - Thêm `open_env_modal_for_variable(...)` và clear `target_variable` ở `close_env_modal`, `open_env_modal`.
3. **Task 3: Cập nhật `EnvVarTable`** (`crates/app/src/components/env_modal/var_table.rs`):
   - Thêm `scroll_handle` và `target_variable`.
   - Xử lý focus và scroll tới đúng biến trong `focus_target_variable`.
   - Render highlight cho hàng được target và track scroll trên container.
4. **Task 4: Thêm `handle_variable_token_click` và kết nối với UI components**:
   - Viết `handle_variable_token_click` trong `env_autocomplete/mod.rs`.
   - Cập nhật `.on_token_click` trong `crates/app/src/components/env_input.rs`.
   - Cập nhật `.on_token_click` trong `crates/app/src/components/request_bar.rs`.
5. **Task 5: Kiểm thử & Xác minh**:
   - Viết unit test cho `resolve_variable_env_id`, `has_variable`, `add_variable_with_key`, `open_env_modal_for_variable`.
   - Chạy `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.

## 5. Verification Steps

- `cargo fmt --check` pass.
- `cargo test` pass 100%.
- `cargo clippy --all-targets -- -D warnings` không có warning nào.
- Thao tác click variable:
  - Click vào biến đã có trong Active Env -> Modal mở đúng Active Env, cuộn tới biến, viền highlight, con trỏ ở ô Current Value (chữ được select_all).
  - Click vào biến trong Globals -> Modal mở đúng Globals.
  - Click vào biến chưa có (Missing) -> Tự động thêm biến vào Active Env/Globals, mở modal và focus con trỏ vào để nhập giá trị.
