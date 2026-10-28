# Plan: Chuyển variable hover từ custom div sang native Tooltip

- **Ngày**: 2026-10-28
- **Trạng thái**: Đã hoàn thành (same-day)
- **Phạm vi**: `crates/app/src/components/` (`env_autocomplete`, `env_input`, `request_bar`)

---

## Goal

Chuyển cơ chế hiển thị giá trị resolved khi hover vào variable token (`{{var}}`) từ custom deferred absolute `div` sang native `gpui_component::tooltip::Tooltip`, giúp UI gọn nhẹ, loại bỏ boilerplate state và giảm re-render không cần thiết.

## Context

Hiện tại, khi hover vào một variable token trong URL bar (`request_bar.rs`) hoặc các input hỗ trợ môi trường (`env_input.rs`):
- Ứng dụng phải duy trì `hover_token_state` qua `window.use_keyed_state(...)`.
- Lắng nghe sự kiện qua `.on_token_hover(...)` và gọi `cx.notify()`, kích hoạt re-render toàn bộ component chỉ để bật/tắt popup.
- Render một custom div bằng `deferred(div().absolute().top(...).left(...).child(variable_value_tooltip(...)))` với toạ độ cứng (`top: 42px, left: 80px` ở URL bar và `top: 34px, left: 0px` ở env_input), dẫn đến việc tooltip không bám theo vị trí thực của token và che các thành phần bên dưới.
- `variable_value_tooltip` trong `env_autocomplete/mod.rs` trả về một `Div` với shadow nặng và kiểu dáng dạng panel nổi thay vì tooltip native gọn nhẹ.

## Architectural Decisions

1. **Sử dụng GPUI Native Tooltip**:
   - Tận dụng phương thức `.tooltip(...)` trên `div().id(...)` của token element bên trong closure `.token(...)`.
   - Chuyển `variable_value_tooltip(val, scope, is_res)` trong `env_autocomplete/mod.rs` sang trả về `gpui_component::tooltip::Tooltip`.
   - Tooltip hiển thị gọn gàng giá trị resolved (hoặc `"Variable not found"` nếu chưa resolve), font monospace (`.SystemMonoFont`), kích thước chữ `text_xs`.
2. **Loại bỏ Keyed State và Re-render Boilerplate**:
   - Xóa `hover_token_state` trong `request_bar.rs` và `env_input.rs`.
   - Xóa `.on_token_hover(...)` và block `.when_some(hovered_token, ...)`.
   - Xóa quét và tạo `var_map` thừa trong `request_bar.rs` (trước đây chỉ phục vụ riêng cho hover deferred div).
3. **Giữ nguyên hành vi tương tác**:
   - Token vẫn mở Environment Modal khi click qua `.on_token_click(...)`.
   - Che giấu bí mật (`••••••••`) và hiển thị `"Variable not found"` cho biến chưa resolve được giữ nguyên.

## Step-by-Step Implementation Tasks

1. **`crates/app/src/components/env_autocomplete/mod.rs`**:
   - Cập nhật `variable_value_tooltip(val: &str, scope: &str, is_res: bool) -> gpui_component::tooltip::Tooltip`.
   - Khi `is_res == true`: hiển thị `val` (hoặc `""` nếu chuỗi rỗng), monospace `text_xs`.
   - Khi `is_res == false`: hiển thị `"Variable not found"`, chữ đỏ (`0xef4444`), `text_xs`.
2. **`crates/app/src/components/request_bar.rs`**:
   - Gắn `.tooltip(...)` trực tiếp vào token element trong closure `.token(...)`.
   - Xóa `hover_token_state`, `h_token`, `.on_token_hover(...)`, `.when_some(hovered_token, ...)`.
   - Xóa `var_map`, `var_spans`, và import `scan_variables`.
3. **`crates/app/src/components/env_input.rs`**:
   - Gắn `.tooltip(...)` trực tiếp vào token element trong closure `.token(...)`.
   - Xóa `hover_token_state`, `h_token`, `env_for_hover`, `.on_token_hover(...)`, `.when_some(hovered_token, ...)`.
4. **Kiểm thử và xác minh**:
   - Chạy `cargo test` để bảo đảm các unit test không bị hồi quy.
   - Chạy `cargo clippy --all-targets -- -D warnings` đảm bảo 0 warning.

## Risk Mitigation

- **Hit testing của token trong GPUI input**: `Input` của `gpui-component` paint token như một `AnyElement` đầy đủ trong prepaint, cho phép interactivity và tooltip hoạt động tự nhiên.
- **Biến rỗng hoặc chưa resolve**: Giữ feedback rõ ràng với `""` hoặc `"Variable not found"`.

## Verification Steps

1. `cargo fmt --check`
2. `cargo test`
3. `cargo clippy --all-targets -- -D warnings`
