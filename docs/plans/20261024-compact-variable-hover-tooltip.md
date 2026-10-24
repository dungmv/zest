# Plan: Compact Variable Hover Tooltip (chỉ hiển thị value)

- **Ngày**: 2026-10-24
- **Trạng thái**: Đã triển khai xong (same-day)
- **Phạm vi**: `crates/app/src/components/` (UI only, không đổi core)

---

## Goal

Khi di chuột (hover) vào một variable token (`{{var_name}}`) trong các input, tooltip chỉ hiển thị **giá trị đã resolve** của variable — bỏ toàn bộ thông tin thừa để giao diện gọn hơn.

## Context

Tooltip hover variable trước đây gồm 4 phần, bị trùng lặp cấu trúc ở 2 nơi render độc lập:

1. Tên variable in đậm `{{var_name}}` (đã hiển thị sẵn trong token).
2. Badge scope ("Active: X" / "Global").
3. Dòng giá trị `= "value"` hoặc thông báo "Variable not found in active or global environments" (quá dài).
4. Hint tĩnh "Click variable to manage environments" (nhiễu, chiếm chỗ).

Hai nơi render trùng lặp:

- `request_bar.rs` (URL bar).
- `env_input.rs` (`render_env_input`, dùng chung cho header table, query params, auth tab, multipart body, scope settings).

## Architectural Decisions

- **Một helper dùng chung** `variable_value_tooltip(val, scope, is_res) -> Div` đặt trong `env_autocomplete.rs` — cạnh `get_variable_details` / `get_variable_kind_and_color` mà nó phụ thuộc. Tránh duplicate 2 block tooltip giống hệt nhau, tuân thủ nguyên tắc modularity.
- **Không đổi logic resolve** (`get_variable_details`) và màu (`get_variable_kind_and_color`): helper chỉ là lớp render. Secret vẫn được che `••••••••` ở tầng resolve.
- **Giữ tín hiệu trạng thái bằng màu viền** (xanh dương = active env, tím = global, vàng = dynamic, đỏ = unresolved) thay vì text/badge — giảm text mà không mất thông tin.
- **Unresolved vẫn hiện phản hồi lỗi** "Variable not found" (rút gọn) màu đỏ — đây là feedback thiết yếu, không bỏ.
- **Không đổi cơ chế tương tác**: hover detection (`on_token_hover`), click mở env modal, inline token rendering, autocomplete `{{` giữ nguyên.

## Step-by-Step Implementation Tasks

1. **`env_autocomplete.rs`** — thêm `variable_value_tooltip()`:
   - Một dòng value mono `text_xs`, viền màu theo kind, `bg_card`, `occlude()`.
   - Width tự co theo nội dung, `max_w(480px)` + `truncate()` cho value dài.
   - Value rỗng hiển thị `""` để tooltip không biến mất; unresolved hiển thị "Variable not found" màu đỏ.
2. **`request_bar.rs`** — thay block tooltip URL bar (trước đây ~70 dòng) bằng gọi helper; giữ nguyên định vị `deferred().absolute().top(42px).left(80px)` và `.with_priority(90)`; xóa biến không còn dùng (`scope`, `bg_tint`, `FontWeight` trong block).
3. **`env_input.rs`** — thay block tooltip của `render_env_input` bằng gọi helper; giữ định vị `top(34px).left(0px)`.

Kết quả: −116 dòng trùng lặp, +42 dòng helper.

## Risk Mitigation

- **Tooltip trống với value rỗng** → hiển thị `""` literal.
- **Value dài làm vỡ layout** → `max_w(480px)` + `truncate()` (ellipsis).
- **Mất khả năng click-through khi tooltip che nội dung** → giữ `occlude()` như cũ (hành vi không đổi).
- **Regression nhiều input cùng lúc** (env_input dùng chung 5+ nơi) → helper là thay đổi thuần render, không đụng state; chạy full `cargo test` workspace.

## Verification Steps

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Kết quả đã chạy: cả 3 pass (clippy 0 warning, toàn bộ test pass).

Kiểm chứng thủ công:

- Hover variable trong URL bar → tooltip chỉ hiện value, viền màu theo kind.
- Hover variable trong header value / auth / multipart → cùng hành vi.
- Hover variable không tồn tại → "Variable not found" màu đỏ.
- Hover variable secret → `••••••••`.
- Click variable → vẫn mở env modal.
