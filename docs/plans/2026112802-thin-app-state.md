# Thu gọn state của ứng dụng Zest

## Goal

Giữ `crates/app/src/state/` là nơi sở hữu dữ liệu hiện tại của ứng dụng, áp dụng các chuyển đổi trạng thái và phát sự kiện GPUI. Chuyển quy tắc nghiệp vụ dùng chung về `zest-core`, điều phối I/O về `services`, và dữ liệu trình bày cục bộ về component hoặc modal tương ứng.

## Context

Repo đã có chín GPUI entity và đã tách các nhóm lớn như `WriteQueue`, `GitOperation`, `ResponseConsole` và `RequestSend`. Phần còn tăng nhanh đến từ code vượt qua ranh giới của entity.

- `state/persistence/coordinator.rs` quản lý thứ tự revision, retry và chuyển snapshot thành thao tác ghi qua `WorkspaceStore`.
- `state/collection/` vừa giữ cây hiện tại, vừa đọc workspace, xử lý đường dẫn và xác định scope kế thừa. Watcher còn sửa trực tiếp danh sách request và lỗi parse.
- `state/environment/` tự gộp quy tắc ưu tiên biến, đọc YAML và secrets, đồng thời thực hiện một số thao tác ghi từ callback UI.
- `state/response/format.rs`, `state/session/naming.rs` và cache cây trong `SidebarState` quyết định cách hiển thị dữ liệu. `ModalState` cũng giữ tùy chọn toàn app bên cạnh dữ liệu hộp thoại.

## Architectural Decisions

| Lớp | Quyền sở hữu |
| --- | --- |
| `zest-core` | Quy tắc nghiệp vụ thuần, kiểu dữ liệu dùng chung, đọc/ghi và quy tắc nhất quán qua `WorkspaceStore`. Dùng lại `core::env`, `core::transport` và `core::storage`. |
| `app/services` | Điều phối thao tác nhiều bước, chạy I/O hoặc tính toán nặng trên background executor, quản lý vòng đời tác vụ, rồi trả kết quả có kiểu và thông tin để xác thực tác vụ. |
| `app/state` | Snapshot đang dùng chung, lựa chọn của người dùng, trạng thái tương tác và các bất biến khi áp dụng kết quả. Entity dùng `cx.emit` để phối hợp và `cx.notify` để yêu cầu vẽ lại. |
| `components` và `modals` | Nhãn, tab khả dụng, trạng thái trình bày cục bộ và projection dành riêng cho UI. Các view cần phối hợp trao đổi qua entity đang sở hữu lựa chọn dùng chung. |

Giữ chín entity hiện có và dùng các API cụ thể của từng entity làm đường cập nhật chung. `AppState` tiếp tục chỉ khởi tạo và giữ handle. `app/workspace_event_handler.rs` tiếp tục điều phối giữa các entity qua typed events.

### Dữ liệu còn lại trong từng entity

| Entity | Dữ liệu và thao tác giữ lại |
| --- | --- |
| `WorkspaceState` | Workspace hiện tại, metadata chuẩn, recent workspaces và snapshot cookie dùng chung. Kích hoạt workspace và áp dụng kết quả nạp hoặc cập nhật metadata. |
| `CollectionState` | Cây đang mở, scratch request, lỗi parse, pins, index và revision phục vụ subscriber. Các API mutation bảo đảm dữ liệu và revision thay đổi cùng nhau. |
| `EnvironmentState` | Danh sách environment, globals, ID đang chọn và cache biến hiệu lực. Áp dụng snapshot và thay đổi biến, rồi phát sự kiện. |
| `PersistenceState` | Dirty/conflict quan sát được trên UI, trạng thái flush dùng khi đổi workspace và local selection đã lưu. Nhận kết quả từ luồng lưu hoặc watcher. |
| `RequestSessionState` | ID request đang mở, tab dùng chung và trạng thái của lượt gửi hiện tại, gồm tiến độ và khả năng hủy. |
| `ResponseState` | Response hiện tại, ID hoặc example đang hiển thị, cache theo request và dữ liệu console/realtime phục vụ các view. |
| `GitState` | Snapshot repo hiện tại, trạng thái lệnh và lựa chọn đang hiển thị. Áp dụng status, history hoặc diff đã xác thực theo repo và truy vấn. |
| `SidebarState` | Lựa chọn, lịch sử điều hướng và trạng thái cây cần chia sẻ với action toàn app. |
| `ModalState` | Hộp thoại đang mở và dữ liệu tương tác cần chia sẻ khi mở, đóng hoặc đổi modal. |

## Step-by-Step Implementation Tasks

1. **Tách quy tắc persistence và environment.** Chuyển `PersistenceCoordinator`, `PersistTarget` và logic áp dụng payload qua `WorkspaceStore` vào `core::storage`. Giữ thời gian debounce, vòng đời `Task` và tín hiệu flush ở app. Chuyển quy tắc gộp biến vào `core::env`, dùng chung thứ tự ưu tiên và first-wins với resolver hiện có. Cung cấp một đường đọc environment trong `core::storage` trả dữ liệu và cảnh báo secrets cùng nhau.
2. **Bảo vệ dữ liệu khi tác vụ hoàn thành.** Xử lý xóa ngoài app như conflict khi bản trong bộ nhớ đang dirty, trước khi watcher được tách khỏi state. Định danh từng lượt gửi để session chỉ áp dụng progress và kết quả khớp lượt hiện tại. Ghi response theo request ID vào cache và chỉ áp dụng lên inspector khi lựa chọn khớp. Dùng lại kiểm tra workspace generation, revision và lựa chọn hiện có ở persistence và Git.
3. **Đưa I/O workspace và collection qua service.** Dùng lại `WorkspaceStore` cho thao tác đĩa, chuyển phần duyệt cây, ánh xạ ID và tìm scope vào API thuần có thể kiểm thử trong core. Đưa mở/scan workspace, CRUD đĩa và đọc dữ liệu watcher vào service. Trả về kết quả có kiểu để entity áp dụng qua API của chính nó. Hợp nhất workspace metadata quanh `WorkspaceMeta` và một đường lưu, với tên thư mục làm giá trị dự phòng khi metadata chưa có tên.
4. **Chuyển phần trình bày về UI.** Đưa nhãn request về helper dùng chung của UI, tab theo protocol về composer/inspector, và ảnh GPUI, zoom, placeholder cùng projection định dạng về response inspector. Đặt projection và cache cây sidebar cùng component sidebar, cập nhật theo thay đổi dữ liệu hoặc bộ lọc. Đưa theme, network preferences, layout và trạng thái kiểm tra cập nhật về lớp settings hoặc service tương ứng. Các component tiếp tục dùng trạng thái chung khi cần đồng bộ giữa nhiều view.
5. **Thu hẹp API của state sau khi di chuyển.** Tìm các caller còn sửa trực tiếp `requests`, `pins`, lỗi parse, collapsed folders và metadata. Thay bằng API theo ý định như áp dụng thay đổi từ watcher, đổi thứ tự pins hoặc mở rộng folder. Các phép cập nhật phải tự duy trì dữ liệu, cache và revision liên quan. Đặt test cạnh module sở hữu hành vi.

## Risk Mitigation

- Tách commit chuyển code khỏi commit sửa hành vi để dễ đối chiếu thứ tự sự kiện và kết quả. Mỗi đợt giữ cùng một đường thực thi cho các caller đang dùng.
- Quy tắc persistence cần giữ thứ tự theo target, snapshot mới nhất, một lượt ghi đang chạy trên mỗi target và flush trước khi đổi workspace. Các thao tác đổi ID tiếp tục remap các bản ghi đang chờ lưu.
- Lượt đọc đĩa hoặc truy vấn nền mang danh tính workspace và revision phù hợp. Các kết quả hoàn thành sau khi đổi workspace, đổi request hoặc đổi lựa chọn được kiểm tra trước khi áp dụng.
- Giữ format YAML, phân tách secrets, giới hạn response cache, quy tắc ghi đè header/auth và thứ tự biến hiện có. Với thao tác có thể lỗi một phần, kiểm tra riêng sự nhất quán giữa snapshot, file và hàng đợi.

## Verification Steps

- Kiểm thử core cho revision và retry, thứ tự ưu tiên biến, cảnh báo secrets lỗi, traversal và remap ID qua cây.
- Kiểm thử service/state cho xóa ngoài app khi request dirty, đổi tên environment trước autosave, chuyển workspace khi còn lượt ghi, hủy lượt gửi A rồi gửi B, và đổi request trước khi response A về.
- Kiểm tra các đường UI chính cho đổi request, sidebar filter/collapse, tab theo protocol, response preview và đóng/mở modal.
- Kiểm tra ranh giới bằng diff. Entity còn dữ liệu, chuyển đổi và sự kiện. Lệnh ghi/đọc file hoặc Git chạy qua service hoặc background executor. UI-specific formatting và projection nằm gần nơi hiển thị. Mỗi phép cập nhật dữ liệu có một đường API chịu trách nhiệm cho revision của nó.
- Chạy `cargo fmt --check`, `cargo test --workspace` và `cargo clippy --workspace --all-targets -- -D warnings`.
