# Kế hoạch: Điều phối Git discard và đồng bộ thay đổi từ công cụ bên ngoài

## Goal

Làm cho trạng thái workspace và Git trong Zest hội tụ đúng với disk sau các thao tác discard/checkout/pull của app hoặc thay đổi do Git CLI/công cụ khác tạo ra. Bảo vệ request đang được sửa khỏi việc bị reload hoặc ghi đè âm thầm, báo lỗi discard đúng kết quả, và tránh đọc/quét disk theo từng lần gõ hoặc từng event watcher trùng lặp.

## Context

- `WorkspaceWatcher` gom filesystem events, phân loại request/environment/tree, bỏ qua `.git`, và debounce ở core; `PersistenceState::start_watcher` tiếp tục gom batch trước khi gọi `handle_fs_events_batch`.
- `PersistenceState` đã phát hiện request dirty khi watcher báo thay đổi ngoài và đặt conflict. Tuy nhiên các Git operation từ `GitService` chưa dùng một barrier chung với write-behind persistence.
- `GitService::discard_file` chạy nền nhưng bỏ qua kết quả `git_discard_file`, không dùng guard `loading`/`is_syncing`, và refresh status sau đó. Core `git_discard_file` có thể trả `Ok(())` dù restore/clean thất bại.
- Watcher không quan sát Git metadata. External `git add`, `reset`, `commit`, branch switch hoặc cập nhật ref có thể không làm đổi file workspace nhưng Git panel vẫn giữ status cũ.
- Không đọc disk sau mỗi keystroke hoặc mỗi lần app ghi. Editor tiếp tục cập nhật state và write-behind tiếp tục coalesce theo target. Disk chỉ được đọc khi xử lý batch event external hoặc barrier Git cần reconcile.
- Hai kế hoạch liên quan đã có: `20261027-discard-change-sidebar-vanish-fix.md` xử lý delete-vs-replace và ngữ nghĩa untracked discard; `20261027-unified-write-behind-persistence.md` định hướng coordinator/barrier persistence tổng quát. Kế hoạch này bổ sung phần Git reconciliation, thao tác discard và thay đổi Git ngoài app; không lặp lại các fix đã được đánh dấu implemented.

## Architectural Decisions

1. **Filesystem/Git là trạng thái quan sát được, event là tín hiệu invalidate.** Gộp event theo path và kiểm tra trạng thái mới nhất khi xử lý batch. Không suy luận trạng thái cuối từ thứ tự Create/Remove/Rename riêng lẻ.
2. **Giữ conflict semantics cho target dirty.** Khi external change trùng request/environment còn dirty hoặc có write in-flight, không reload đè lên editor và không cho writer cũ âm thầm ghi đè external content. Đánh dấu conflict và để luồng resolve conflict hiện có quyết định reload hoặc giữ bản local.
3. **Không thêm disk read vào đường gõ/lưu thông thường.** Nhiều edit cùng request tiếp tục coalesce thành snapshot mới nhất. Một batch watcher chỉ đọc mỗi target cần reconcile tối đa một lần; tree scan tối đa một lần khi cấu trúc thực sự thay đổi.
4. **Git mutation của app đi qua coordinator/barrier dùng chung.** Discard/checkout/pull và các operation thay working tree phải tuần tự với writer persistence liên quan. Barrier xác định policy từng operation: hủy pending local edits khi người dùng chọn discard, hoặc flush/drain trước operation cần nội dung local mới nhất.
5. **Trả kết quả Git chính xác.** Core discard phải trả `Err` nếu thao tác khôi phục/xóa thất bại. GitService chỉ refresh và báo success sau `Ok`; lỗi được đưa vào `GitState::error` và thông báo UI phù hợp.
6. **Refresh Git status khi có thay đổi từ ngoài app.** Bổ sung tín hiệu metadata Git (ít nhất index/HEAD/ref phù hợp) và tín hiệu file workspace đã đổi. Coalesce/debounce trước khi chạy `git status`; khi Git panel không hoạt động, có thể trì hoãn refresh đến lần mở panel.
7. **Giới hạn phạm vi watcher `.git`.** Không recursively theo dõi và phản ứng với mọi file trong `.git` (object database/log churn). Lọc metadata có ý nghĩa hoặc dùng refresh định kỳ nhẹ chỉ khi Git panel đang mở để bao phủ ref/worktree edge cases.
8. **An toàn path và thao tác xóa.** Xác minh path discard là path tương đối hợp lệ bên trong repo; không cho path tuyệt đối hoặc `..` tác động ngoài workspace. `delete_untracked` tiếp tục chỉ được bật từ Git panel có chủ ý.

## Step-by-Step Implementation Tasks

1. **Kiểm kê luồng và API hiện hành.** Đối chiếu `git_discard_file`, các caller sidebar/Git panel, `GitService`, write-behind targets/barrier API, watcher classification và conflict resolution. Ghi rõ semantics của từng caller và xác nhận fix trong plan discard sidebar đã hiện diện.

2. **Sửa kết quả discard ở core.**
   - Tách trường hợp tracked, staged-added và untracked theo status/index/HEAD thay vì dùng duy nhất `cat-file -e HEAD:path` nếu cần để quyết định restore/clean chính xác.
   - Không bỏ qua lỗi restore/checkout/clean/remove. Chỉ fallback khi có lý do hợp lệ; nếu mọi cách thất bại, trả lỗi có ngữ cảnh path và operation.
   - Validate path tương đối, từ chối traversal/absolute path; giữ `--` trước pathspec.
   - Bảo toàn ngữ nghĩa `delete_untracked = false` của sidebar và `true` của Git panel.

3. **Điều phối discard với persistence.**
   - Thêm API barrier/cancel theo `PersistTarget::Request` trong coordinator hiện hành hoặc tích hợp đúng API nếu plan unified write-behind đã được triển khai.
   - Sidebar discard: hủy/cancel snapshot pending theo chính sách revert đã xác định, ngăn completion cũ cập nhật dirty state sai, chạy Git restore, rồi reconcile request từ disk. Với request untracked mà core discard là no-op, giữ hành vi revert về snapshot đã lưu theo semantics hiện tại.
   - Git panel discard: áp dụng chính sách Git panel đã chọn (kể cả xóa untracked có chủ đích), sau đó đồng bộ workspace/sidebar nếu file thuộc dữ liệu workspace.
   - Không giữ lock hoặc block UI trong thời gian I/O; command và reads chạy background.

4. **Đồng bộ kết quả discard vào state.**
   - Thành công: reconcile request sau khi command hoàn tất, cập nhật workspace event/version để editor inputs nhận snapshot mới, dọn conflict/dirty state chỉ khi policy operation xác nhận đã loại bỏ local edits.
   - Thất bại: giữ dữ liệu/state an toàn, không giả lập reload thành công; báo lỗi actionable trong Git UI/toast.
   - Chống callback cũ ghi đè kết quả mới bằng operation/revision token hoặc cơ chế serialization theo repo/target.

5. **Bổ sung quan sát thay đổi Git từ công cụ bên ngoài.**
   - Thiết kế loại `FsEvent`/invalidation riêng cho Git metadata và xử lý path `.git` mà không để lẫn với dữ liệu workspace.
   - Xác định path có ích như index, HEAD và refs; xử lý `.git` file của linked worktree/submodule nếu repo hỗ trợ. Tránh assumptions rằng `.git` luôn là directory.
   - Khi event working-tree thuộc `collections`, `environments` hoặc `workspace.yaml`, đồng bộ domain data và đánh dấu Git status cần refresh.
   - Debounce/coalesce nhiều event thành một lần `git status`; nếu Git panel đang mở thì refresh kịp thời, còn đóng thì refresh khi panel được mở.
   - Cân nhắc timer refresh status khi panel mở để bắt ref/branch updates không được backend watcher báo đầy đủ; dừng timer khi panel đóng.

6. **Giữ conflict policy đúng cho edit liên tục.**
   - Mỗi batch external event chỉ kiểm tra một lần trạng thái dirty/in-flight của target và đọc file tối đa một lần nếu cần reload.
   - Nếu target dirty/in-flight: giữ state editor, đặt conflict; không schedule save snapshot cũ cho đến khi conflict được resolve.
   - Nếu target clean: đọc disk mới nhất, cập nhật domain state và phát event để composer/table input đồng bộ.
   - Nếu request bị xóa thật: xác minh tồn tại tại thời điểm xử lý; xóa row và hủy target persistence. Atomic replace hoặc file được tạo lại phải thành change/rescan như plan discard sidebar.

7. **Tối ưu watcher và refresh cost.**
   - Dùng một cửa sổ coalesce hợp lý cho watcher và receiver; tránh cộng dồn debounce 200 ms + 100 ms + nhiều lần refresh status.
   - Chuẩn hóa path lúc đăng ký suppression để lookup hash thay vì scan toàn bộ `HashSet` cho mọi event. Không mở rộng suppression để che external changes cùng path.
   - Với batch thay đổi nhiều request, đọc/update theo path; chỉ `reload_tree()` một lần nếu có thay đổi cấu trúc, folder metadata, workspace metadata hoặc request cần được tái thêm.
   - Bỏ qua event lỗi có chủ đích nhưng ghi nhận watcher failure và cung cấp recovery/rescan khi watcher không hoạt động.

8. **Giữ ranh giới kiến trúc.** Git CLI, filesystem scan/read và serialization chạy background. Render thuần. Không lưu bản sao lâu dài của toàn request trong `RequestSessionState`; domain request vẫn thuộc `WorkspaceDataState`. Giữ module dưới khoảng 650 LOC bằng cách tách helper/coordinator khi cần.

## Risk Mitigation

- **Mất edit đang gõ:** conflict check trước khi apply external snapshot; barrier và revision ngăn writer cũ ghi sau discard/checkout.
- **Disk reads quá thường xuyên:** chỉ đọc trên event batch đã coalesce; không đọc theo keystroke hay từng app-owned write; tree scan tối đa một lần mỗi batch.
- **Git status refresh storm:** lọc metadata, debounce, chỉ refresh panel đang mở hoặc khi user mở panel.
- **Watcher bỏ sót metadata:** kết hợp path watch với refresh khi mở panel và timer nhẹ khi panel mở; test linked worktree `.git` file.
- **Discard báo success sai:** core trả lỗi thật; app chỉ apply success transition khi command thành công.
- **Xóa nhầm file:** giữ opt-in `delete_untracked`, validate path, test staged-added/untracked/tracked/deleted/conflicted cases.
- **Stale async callback:** dùng revision/operation generation hoặc serialize theo repo; callback cũ không được thay state mới hơn.
- **Event replace bị diễn giải thành deletion:** giữ kiểm tra filesystem ở watcher và handler như kế hoạch `20261027-discard-change-sidebar-vanish-fix.md`.
- **Phạm vi trùng với write-behind plan:** tích hợp barrier vào coordinator được chọn; không tạo một queue persistence song song.

## Verification Steps

1. Core Git tests: discard tracked modified/staged/deleted file; staged-added và untracked với cả hai giá trị `delete_untracked`; restore/clean failure; path traversal/absolute path; xác nhận lỗi được trả về.
2. App persistence tests: gõ URL/params/headers liên tục tạo một snapshot cuối; external edit cùng request khi clean được reload; external edit khi dirty/in-flight tạo conflict và không bị ghi đè.
3. Discard race tests: có write pending/in-flight rồi discard; xác nhận file/state cuối cùng đúng semantics caller, không có save cũ ghi đè sau restore, conflict/dirty indicator được dọn đúng.
4. Watcher tests: nhiều event modify/remove/create cùng path trong một batch hội tụ theo filesystem hiện tại; deletion thật vẫn xóa request; event từ `.git/index` hoặc `HEAD` yêu cầu status refresh nhưng không gây tree reload.
5. Manual external-tool scenarios: `git add`, `git reset`, `git commit`, `git checkout`/branch switch và `git restore` từ terminal khi Git panel mở/đóng; xác nhận status, branch, diff, sidebar và editor hội tụ.
6. Performance checks: nhiều event cùng batch chỉ gây một `git status`; update editor không thêm disk read; bulk restore không kích hoạt nhiều tree scan.
7. Chạy validation repository: `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`.
