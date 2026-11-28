# Mở workspace từ Git repository

## Goal

Cho phép clone workspace Zest từ URL Git qua HTTPS hoặc SSH, kiểm tra metadata rồi mở bản clone cục bộ trong app.

## Context

[Đặc tả Git](../specs/05-git-sync-and-collaboration.md#4-mở-workspace-từ-git-repository) yêu cầu `workspace.yaml` hợp lệ ở gốc repo. Luồng mở thư mục hiện tại có thể khởi tạo metadata khi tệp này vắng mặt. Luồng Git cần kiểm tra trước khi chuyển workspace.

## Architectural Decisions

- Dùng modal trong registry GPUI, đặt lệnh cạnh **Open Workspace…**. Người dùng chọn hoặc nhập đường dẫn đích.
- `GitService` điều phối tác vụ nền. Core chạy Git, theo dõi timeout và yêu cầu hủy, kiểm tra metadata trong thư mục tạm cạnh đích rồi đưa bản clone vào vị trí cuối.
- Phát `WorkspaceSwitchRequested` cho bản clone đã xác thực để dùng chung hàng rào flush của `PersistenceCoordinator`, file watcher, Git state và danh sách workspace gần đây.

## Step-by-Step Implementation Tasks

1. Thêm lệnh clone có thể hủy, kiểm tra URL, metadata và thư mục đích.
2. Thêm hộp thoại, bộ chọn thư mục, trạng thái tiến trình và lỗi có thể thử lại.
3. Kết nối kết quả clone với luồng chuyển workspace và cập nhật trạng thái trong đặc tả.
4. Kiểm tra các trường hợp bản clone hợp lệ, metadata lỗi, đích có dữ liệu và hủy.

## Risk Mitigation

- Chỉ dọn thư mục tạm của tác vụ. Kiểm tra lại đích trước khi đặt bản clone và chỉ chấp nhận hủy trước khi bắt đầu bước này.
- Dùng đối số Git tách biệt và giới hạn giao thức. Hiển thị lỗi đã phân loại để tránh đưa credential hoặc đầu ra thô của Git vào UI.
- Chỉ chấp nhận metadata là tệp thường. Kiểm tra symlink và dữ liệu `.local/` trong bản checkout trước khi giao cho tầng lưu trữ.

## Verification Steps

- Chạy các test hồi quy của luồng clone qua Git và các test toàn workspace với `cargo test`.
- Chạy `cargo fmt --check` và `cargo clippy --all-targets -- -D warnings`.
- Kiểm tra từ menu workspace đến hộp thoại, hoàn tất clone và mở lại từ danh sách gần đây khi có môi trường kiểm tra UI.
