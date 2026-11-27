# Xử lý nợ kỹ thuật TD-501 đến TD-508

## Mục tiêu

Cách ly dữ liệu giữa các workspace, bảo toàn chỉnh sửa, giới hạn bộ nhớ khi nhận dữ liệu và giảm I/O trên luồng GPUI. Khắc phục lỗi lưu cache UTF-8 và chia các module lớn theo chức năng.

## Bối cảnh

Đợt rà soát tại commit `9da7964` ghi nhận tám vấn đề trong [sổ nợ kỹ thuật](../technical-debt.md). Lỗi cache UTF-8 và giới hạn event SSE đã được kiểm chứng riêng.

## Quyết định kiến trúc

- Mỗi lượt thực thi và mỗi kết quả I/O bất đồng bộ thuộc về một workspace xác định.
- Áp dụng snapshot trên GPUI sau khi kiểm tra workspace và chỉnh sửa hiện tại. Tiếp tục dùng coordinator cho autosave và AtomicWriter cho thao tác file.
- Giới hạn kích thước từng payload và số event chờ, đồng thời giữ đường truyền sự kiện kết thúc.
- Giữ schema lưu trữ và điểm gọi hiện có khi chia module.

## Các bước thực hiện

1. TD-501 và TD-502. Cách ly executor, cache và phiên realtime, rồi kiểm tra workspace và revision khi áp dụng kết quả scan.
2. TD-503. Hoàn tất snapshot đang chờ vào store cũ theo thứ tự revision trước khi chấp nhận chuyển workspace.
3. TD-504. Giới hạn body HTTP, event SSE và hàng đợi realtime.
4. TD-505 và TD-506. Truyền lỗi CRUD, thống nhất lưu pins và cắt cache theo biên ký tự.
5. TD-507. Chuyển I/O watcher, tải environment và import sang background executor.
6. TD-508. Tách form auth, bảng màu hoặc test theme, và nhóm các bộ test lớn theo chức năng.

## Giảm rủi ro

- Kiểm tra hai workspace có ID request giống nhau và các task hoàn tất muộn.
- Kiểm tra autosave khi còn revision đang ghi hoặc gặp lỗi. Giữ dữ liệu chưa lưu để người dùng có thể thử lại.
- Kiểm tra payload đa byte, stream vượt ngân sách và khả năng nhận trạng thái cuối.
- Giữ các mục chưa đạt trong sổ nợ.

## Kiểm chứng

Chạy test hồi quy cho từng lỗi đã sửa, sau đó chạy `cargo fmt --check`, `cargo test --workspace` và `cargo clippy --workspace --all-targets -- -D warnings`. Các test loopback và watcher cần quyền chạy ngoài sandbox trên máy này.

## Kết quả

| Mục | Thay đổi và kiểm chứng |
| :--- | :--- |
| TD-501 | Executor, cookie jar và các engine realtime được tạo theo workspace. Cache response được đặt lại khi chuyển. Test sử dụng hai workspace có cùng ID request để kiểm tra kết quả HTTP và thay đổi environment đến muộn. |
| TD-502 | Kết quả rescan được kiểm tra theo store. Kết quả hợp nhất với bản request trong bộ nhớ tại thời điểm áp dụng. Test bao phủ rescan hoàn tất sau khi chuyển và chỉnh sửa trong lúc hai lượt quét chồng nhau. |
| TD-503 | Quá trình chuyển đợi coordinator ghi revision cuối vào store cũ. Nếu ghi lỗi, workspace hiện tại giữ snapshot để thử lại. Test bao phủ write đang chạy, revision mới và lỗi ghi. |
| TD-504 | Body HTTP giới hạn 64 MiB. Payload realtime giới hạn 4 MiB, hàng đợi có 8 vị trí mỗi chặng, lịch sử riêng mỗi request giữ tối đa 8 MiB hoặc 1.000 frame. Test xác nhận lỗi giới hạn HTTP, giới hạn event SSE nhiều dòng, loại frame cũ theo byte và nhận đủ frame trước sự kiện kết thúc khi hàng đợi đầy. |
| TD-505 | Nhân bản dùng một lần tạo file đầy đủ. Xoá chỉ sửa cây sau khi store xác nhận. Examples và pins dùng coordinator. Test chặn lỗi xoá và nhân bản làm thay đổi cây hoặc pins. |
| TD-506 | Cache cắt theo biên UTF-8. Test kiểm tra cả đường chuyển response và đường ghi cache tại mốc 512 KiB. |
| TD-507 | Watcher đọc và parse snapshot trong background executor. Scan workspace tải local state và environment cùng lúc. Import ghi trong worker và yêu cầu watcher cập nhật dữ liệu về GPUI. Test chạy đường reconcile sau import. |
| TD-508 | Tách form chọn auth, bảng màu, test theme và các nhóm test dài. File Rust dài nhất còn 647 dòng. |

`cargo fmt --check`, `cargo clippy --offline --workspace --all-targets -- -D warnings` và `cargo test --offline --workspace --no-fail-fast` đều đạt. Suite chạy ngoài sandbox có **392 test** gồm 152 app và 240 core. Các test hồi quy kiểm tra thứ tự xử lý và tính toàn vẹn dữ liệu. Thời gian phản hồi giao diện với workspace lớn chưa được đo trên ứng dụng chạy thực tế.
