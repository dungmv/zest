# Implementation Plan: Tối ưu UI Git History Modal

## Goal Description
Tối ưu hóa giao diện hiển thị danh sách commit trong Git History modal (`GitHistoryModal`) theo yêu cầu:
1. **Cấu trúc 2 hàng cố định cho mỗi commit item**:
   - **Hàng 1**: Nội dung commit message (subject), sử dụng font medium và `truncate()` để đảm bảo chiếm đúng 1 dòng duy nhất.
   - **Hàng 2**: Hàng thông tin chi tiết:
     - Bên trái: Username (`author_name`) và commit hash ngắn (`short_hash`) dạng badge monospace.
     - Bên phải: Thời gian (`relative_date`).
2. **Loại bỏ thông tin tổng số commit**:
   - Bỏ nhãn `({} commits)` trên thanh header của modal để giữ header sạch sẽ, tối giản.

---

## User Review Required
> [!NOTE]
> **Hiển thị Username & Hash**:
> - Username sẽ được hiển thị dạng chữ gọn gàng (`item.author_name`), màu `text_secondary`, tự động co giãn / cắt nếu tên tác giả dài (`truncate()`).
> - Hash (`item.short_hash`) hiển thị dạng chip/badge nhỏ monospace với border tinh tế.
> - Thiết kế đề xuất mặc định hiển thị tên trực tiếp để giao diện hiện đại và thanh thoát.

---

## Proposed Changes

### Component: `crates/app`

#### [MODIFY] [`history_modal.rs`](crates/app/src/components/git/history_modal.rs)

1. **Cập nhật hàm `render_history_view`**:
   - Thay đổi danh sách con của mỗi commit item thành 2 hàng:
     ```rust
     div()
         .id(format!("commit-item-{}", item.hash))
         .px(px(12.))
         .py(px(7.))
         .border_b_1()
         .border_color(t.border)
         .cursor_pointer()
         .when(is_selected, |s| s.bg(t.bg_hover))
         .hover(|s| s.bg(t.bg_hover))
         .on_click(move |_, _, cx| {
             git_sel.update(cx, |git, cx| {
                 git.select_commit(&h, cx);
             });
         })
         .flex()
         .flex_col()
         .gap(px(4.))
         // Hàng 1: Commit message (1 dòng, truncate)
         .child(
             div()
                 .text_xs()
                 .font_weight(gpui::FontWeight::MEDIUM)
                 .text_color(t.text_primary)
                 .truncate()
                 .child(item.message),
         )
         // Hàng 2: Username & hash bên trái, time bên phải
         .child(
             div()
                 .flex()
                 .items_center()
                 .justify_between()
                 .gap(px(8.))
                 .child(
                     div()
                         .flex()
                         .items_center()
                         .gap(px(6.))
                         .min_w(px(0.))
                         .child(
                             div()
                                 .text_xs()
                                 .text_color(t.text_secondary)
                                 .truncate()
                                 .child(item.author_name),
                         )
                         .child(
                             div()
                                 .px(px(4.))
                                 .py(px(0.5))
                                 .rounded(px(3.))
                                 .bg(t.bg_card)
                                 .border_1()
                                 .border_color(t.border)
                                 .text_xs()
                                 .font_family(".SystemMonoFont")
                                 .text_color(t.primary)
                                 .flex_shrink_0()
                                 .child(item.short_hash),
                         ),
                 )
                 .child(
                     div()
                         .text_xs()
                         .text_color(t.text_muted)
                         .flex_shrink_0()
                         .child(item.relative_date),
                 ),
         )
     ```

2. **Cập nhật hàm `GitHistoryModal::render`**:
   - Loại bỏ hiển thị `({} commits)` ở header của dialog:
     ```rust
     // Xóa bỏ:
     // .child(
     //     div()
     //         .text_xs()
     //         .text_color(t.text_muted)
     //         .child(format!("({} commits)", history.len())),
     // )
     ```

---

## Verification Plan

### Automated Tests
1. Chạy toàn bộ test suites của workspace:
   ```bash
   cargo test
   ```
2. Kiểm tra linter nghiêm ngặt không phát sinh warning:
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```
3. Kiểm tra formatting:
   ```bash
   cargo fmt --check
   ```

### Manual Verification
1. Mở ứng dụng Zest trong một Git repository.
2. Mở Git History modal (qua footer menu hoặc command palette: `Git: View Commit History`).
3. Xác nhận trên header chỉ hiển thị biểu tượng `◷`, tiêu đề `Git History` và badge tên branch (không còn `({} commits)`).
4. Xác nhận mỗi commit trong danh sách bên trái chỉ có đúng 2 hàng:
   - Hàng 1: Commit message.
   - Hàng 2: Tên author và hash ở bên trái, relative time ở bên phải.
   - Thao tác click chọn commit xem diff bên phải vẫn hoạt động mượt mà.
