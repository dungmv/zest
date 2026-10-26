# Kế hoạch: Đồng bộ UI và YAML bằng Write-Behind Persistence dùng chung

## Goal

Thiết kế và triển khai một cơ chế persistence thống nhất cho các chỉnh sửa từ UI: cập nhật state trong bộ nhớ ngay lập tức, coalesce nhiều thay đổi thành snapshot mới nhất theo file, rồi ghi YAML ở background theo đúng thứ tự.

Phạm vi là toàn bộ dữ liệu YAML-backed mà UI có thể chỉnh sửa, không chỉ Environment Modal:

- Request: URL, method, params, headers, auth, body (raw/JSON/form/multipart), scripts, settings và examples.
- Workspace headers/auth/default environment và metadata workspace.
- Folder metadata, headers/auth/inheritance và thứ tự request/folder.
- Environments, bao gồm biến thường, biến secret, thao tác thêm/xóa/đổi tên và trạng thái secret.
- Globals nếu chúng là dữ liệu người dùng mong được giữ sau khi mở lại workspace: xác định schema/path YAML và tích hợp vào load/save thay vì để thay đổi chỉ tồn tại trong RAM.
- Các dữ liệu cấu hình khác đang được UI ghi xuống YAML (ví dụ pins hoặc local state) cần được kiểm kê và đưa vào cùng mô hình nếu phù hợp.

Không áp dụng write-behind cho dữ liệu vốn không phải cấu hình YAML hoặc cho cache response binary/JSON nếu lifecycle hiện tại yêu cầu ghi theo cơ chế riêng. Những ngoại lệ phải được liệt kê rõ trong inventory.

## Context

- Request input cập nhật `WorkspaceDataState`, gọi `RequestSessionState::mark_dirty`, rồi `PersistenceState` debounce và ghi request YAML.
- Environment Modal hiện clone danh sách state cũ cho mỗi lần sửa và debounce các snapshot riêng; các thay đổi liên tiếp có thể ghi đè lẫn nhau.
- Một số thao tác metadata/CRUD gọi storage trực tiếp; một số đường ghi bỏ qua lỗi.
- Request auto-save và environment save có cơ chế task riêng; cần hợp nhất semantics để coalesce, quản lý revision, lỗi và thứ tự ghi nhất quán.
- Storage đã có `atomic_write_str` và file watcher suppression cho một số loại file. Từng file được ghi nguyên tử, nhưng nhiều file thuộc cùng một logical update không mặc nhiên là một transaction.
- `EnvironmentFile::from_environment` tách dữ liệu secret khỏi YAML public. Secret file cần được tính trong cùng snapshot và trạng thái dirty của environment.

## Architectural Decisions

1. **Các domain state hiện có vẫn là nguồn dữ liệu chuẩn.** Không tạo một bản cache toàn cục thứ hai chứa bản sao lâu dài của toàn workspace. UI cập nhật entity tương ứng ngay; persistence chỉ giữ snapshot immutable đang chờ ghi và metadata revision.
2. **Tạo persistence coordinator dùng chung.** Coordinator nhận `PersistTarget`/file identity cùng snapshot đã serialize hoặc typed snapshot, revision và workspace generation. Có thể dùng service/state mới hoặc mở rộng `PersistenceState` nếu giữ được ranh giới trách nhiệm rõ ràng.
3. **Coalesce theo file đích.** Mỗi file chỉ có tối đa một snapshot pending mới nhất. Các thay đổi tiếp theo thay snapshot đang chờ thay vì thêm một tác vụ ghi cho mỗi phím.
4. **Snapshot đầy đủ, ghi thay thế nguyên tử theo file.** Worker serialize toàn bộ entity hiện tại thành nội dung file mới rồi dùng storage API/atomic write hiện hữu. Không patch text YAML thủ công.
5. **Ghi có thứ tự và revision.** Mỗi target có revision đơn điệu. Tối đa một writer hoạt động cho một file; snapshot revision cũ không được hoàn tất sau và ghi đè revision mới. Nếu có update trong lúc write, flush tiếp revision mới nhất sau khi write hiện tại kết thúc.
6. **Hỗ trợ nhóm file thuộc cùng logical entity.** Ví dụ environment public YAML và secrets YAML lấy từ cùng một snapshot. Chỉ đánh dấu revision clean khi mọi file của nhóm ghi thành công; retry phải ghi lại toàn nhóm từ snapshot hiện hành.
7. **Debounce có thể cấu hình theo target/domain.** Gõ liên tục được gộp; các thao tác cấu trúc như rename/delete/import/reorder cần barrier/flush hoặc command tuần tự hóa, tránh tạo lại file bị xóa/đổi tên.
8. **Lỗi persistence có trạng thái rõ ràng.** Trả kết quả worker về UI; giữ dirty snapshot khi lỗi; hiển thị lỗi hoặc trạng thái retry. Không reload state cũ từ disk sau write failure.
9. **Giữ watcher suppression và conflict detection.** Mọi write app-owned phải đi qua storage layer đăng ký suppressed paths; external changes vẫn theo conflict policy hiện hành.
10. **Không I/O trong render hoặc UI thread.** Render chỉ dựng element tree. Serialization nặng và filesystem I/O chạy trên background executor; callback cập nhật model và enqueue công việc ngắn gọn.

## Step-by-Step Implementation Tasks

1. **Lập inventory mọi dữ liệu YAML-backed và đường ghi UI.**
   - Liệt kê target file/schema, entity nguồn, các callback/UI actions, storage API hiện tại và lifecycle.
   - Bao gồm request child fields như headers/params/multipart, workspace/folder metadata, environments/secrets, Globals, pins/local state và CRUD/import flows.
   - Đánh dấu các file/cache nằm ngoài phạm vi và ghi lý do.

2. **Chốt API và khóa định danh persistence target.**
   - Định nghĩa target ổn định có workspace generation/path và file identity (request id/path, env id/name, folder path, workspace metadata, v.v.).
   - Định nghĩa payload snapshot bất biến, revision, debounce policy, trạng thái pending/in-flight/failed/saved.
   - Tránh giữ Entity/UI handles trong background worker.

3. **Triển khai coordinator/queue.**
   - Coalesce snapshot pending mới nhất theo file target.
   - Đảm bảo một writer có thứ tự cho mỗi target; xử lý update đến khi write đang chạy.
   - Báo kết quả thành công/lỗi về app state; giữ dirty state cho retry.
   - Thêm flush/drain/barrier để caller có thể đợi trước rename, delete, workspace switch, app shutdown hoặc thao tác Git yêu cầu nội dung mới nhất.
   - Hạn chế queue memory bằng cách chỉ giữ một pending snapshot mỗi target và giải phóng sau khi hoàn tất.

4. **Tích hợp request persistence.**
   - URL, method, params, headers, auth, body, multipart, scripts, settings và examples cập nhật `WorkspaceDataState` ngay.
   - Mọi `RequestModified` enqueue snapshot request đầy đủ; các edits của cùng request trong debounce được gộp thành một lần ghi.
   - Thay/đơn giản hóa `request_save_tasks` hiện tại nếu coordinator đảm nhiệm revision, debounce và thứ tự tốt hơn.
   - Giữ scratch request không ghi file cho đến khi có thao tác promote/save rõ ràng.

5. **Tích hợp environment và secret persistence.**
   - Sửa callback `env_modal.rs` cập nhật `EnvironmentState` trực tiếp trước khi enqueue snapshot; không clone state cũ rồi chỉ lưu bản clone.
   - Gộp sửa key/initial/current/secret/add/delete theo environment.
   - Ghi public YAML và secret YAML từ cùng snapshot, với retry nhóm file khi một ghi thất bại.
   - Thêm persistence cho Globals bằng schema/path YAML xác định; định nghĩa load, save, migration/default behavior và secret splitting tương ứng. Không để chỉnh sửa Globals chỉ nằm trong RAM.

6. **Tích hợp workspace, folder và các YAML-backed preferences.**
   - Chuyển sửa workspace metadata/headers/auth và folder metadata/headers/auth/order sang cùng coordinator.
   - Đưa pins/local state hoặc target YAML khác vào nếu chúng thực sự là file UI chỉnh sửa và lợi ích coalescing phù hợp; giữ các file cần ghi tức thời theo policy riêng nhưng vẫn dùng thứ tự/revision chung.
   - Đảm bảo thao tác reorder nhiều phần tử enqueue snapshot đầy đủ, không ghi một lần cho từng phần tử.

7. **Phối hợp mutation cấu trúc và multi-file operations.**
   - Với rename/move, drain hoặc invalidate target cũ trước khi chuyển path; remap queued targets theo ID mapping.
   - Với delete, hủy snapshot pending và serialize delete sau mọi write in-flight; không cho writer tái tạo file.
   - Với import/bulk update, batch nhiều target rồi flush theo transaction boundary hợp lý; không chặn UI.
   - Với workspace switch, tăng generation và drain/cancel có kiểm soát để task workspace cũ không ghi nhầm store/path.
   - Với Git checkout/discard/pull, phối hợp barrier với writer và conflict handling để không ghi đè trạng thái được checkout hoặc external edit.

8. **Bổ sung trạng thái lưu và lỗi cho UI.**
   - Cho biết target còn pending, đang ghi, đã lưu hoặc lỗi nếu UI hiện có chỗ phù hợp (unsaved indicator/toast/status).
   - Retry không yêu cầu người dùng gõ lại.
   - Không làm lộ secret trong log, toast, diagnostics hoặc debug output.

9. **Giữ rendering thuần và giới hạn trách nhiệm component.**
   - Component chỉ chuyển user intent thành state mutation/event; không gọi filesystem hoặc serialize YAML trong render.
   - Reconciliation InputState không được phát sinh vòng lặp autosave ngoài ý muốn.
   - Tách coordinator/serializer thành module phù hợp, giữ file dưới giới hạn khoảng 650 LOC.

## Risk Mitigation

- **Mất snapshot do stale clone:** cập nhật domain state chuẩn trước khi snapshot; regression test cập nhật nhiều field của cùng entity trong debounce window.
- **Ghi đè sai thứ tự:** revision + một writer per file; không dựa vào drop/cancel task để bảo đảm thứ tự I/O.
- **Rename/delete tái tạo file cũ:** barrier và invalidation/remap queue target trước khi thao tác filesystem cấu trúc.
- **Ghi nhầm workspace:** snapshot mang workspace generation và store/path identity; bỏ qua kết quả stale nhưng phải xử lý pending data theo policy rõ ràng.
- **Partial write nhiều file:** chỉ clean khi cả nhóm thành công; retry ghi lại toàn nhóm từ một snapshot; ghi rõ giới hạn transaction nguyên tử của filesystem.
- **Watcher echo:** mọi write đi qua storage API hỗ trợ suppression; xác nhận đầy đủ các path YAML và secrets được watcher quan sát.
- **External edits/conflict:** không cho write-behind âm thầm ghi đè external change; tích hợp dirty target với conflict detection hiện hành.
- **Secrets exposure:** tách public/private đúng schema, không log payload; kiểm tra file public không chứa giá trị secret.
- **Globals compatibility:** xác định schema/path và behavior cho workspace hiện hữu; load thiếu file bằng default; không thay đổi format âm thầm.
- **Queue phình bộ nhớ:** coalesce theo target, giới hạn một pending snapshot mỗi file, giải phóng snapshot sau completion.
- **Render/typing latency:** mọi I/O và công việc serialization nặng trên background executor; UI cập nhật model đồng bộ và nhanh.

## Verification Steps

1. Kiểm thử coordinator: nhiều update cùng target trong debounce window chỉ flush snapshot revision mới nhất.
2. Kiểm thử race: update mới đến khi snapshot cũ đang ghi; xác nhận revision cuối cùng trên disk là mới nhất.
3. Kiểm thử target độc lập: thay đổi hai request/environment khác nhau không làm coalesce nhầm file.
4. Kiểm thử request end-to-end: sửa liên tục URL, headers, params, auth, body và multipart; reload YAML xác nhận snapshot cuối có đủ mọi thay đổi.
5. Kiểm thử environment end-to-end: sửa key/initial/current/secret nhanh; xác nhận state không mất update, public YAML và secrets file khớp snapshot.
6. Kiểm thử Globals: restart/reload workspace và xác nhận Globals được load từ schema/path mới; kiểm tra migration/default cho workspace chưa có file.
7. Kiểm thử workspace/folder: cập nhật headers/auth/order nhanh và xác nhận YAML metadata chứa snapshot cuối.
8. Mô phỏng lỗi ghi một target và một file trong nhóm secret/public; xác nhận dirty state còn lại, lỗi được báo và retry thành công.
9. Kiểm thử rename/move/delete, workspace switch, import và Git operations khi có write pending/in-flight; xác nhận không ghi lại path cũ hoặc ghi sang workspace sai.
10. Kiểm tra watcher suppression và external-edit conflict sau write.
11. Xác nhận không có filesystem I/O trong render/UI thread và không có autosave loop từ InputState reconciliation.
12. Chạy validation repository: `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`.
