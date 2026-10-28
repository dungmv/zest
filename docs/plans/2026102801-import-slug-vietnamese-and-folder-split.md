# Import: chuẩn hoá slug (mọi ký tự đặc biệt) & tách "/" thành thư mục lồng nhau

## Goal
Khi import OpenAPI/Postman:
1. Slug của thư mục/file được chuẩn hoá hoàn toàn sang ASCII an toàn cho filesystem: bỏ dấu tiếng Việt (`Thông quan` → `thong-quan`, `đ` → `d`), chuyển các ký tự Latin/Unicode khác (`é`, `ü`, `ß`, `ø`, CJK, Cyrillic...) sang dạng ASCII tương ứng, ký tự đặc biệt còn lại thành `-`.
2. Tên **folder** chứa `/` (vd `merchant/Thông quan`) được tách thành cây thư mục `merchant/` → `thong-quan/`.
3. Tên hiển thị gốc (`Thông quan`) giữ nguyên trong `_folder.yaml`; chỉ tên thư mục/file trên đĩa bị slug.

## Quyết định đã chốt với user
| # | Quyết định |
|---|---|
| 1 | Chuẩn hoá slug cho **tất cả** ký tự đặc biệt, không chỉ tiếng Việt |
| 2 | Chỉ tách `/` ở **tên folder**; tên request không tách |
| 3 | Tên hiển thị folder giữ nguyên bản; chỉ slug tên thư mục trên đĩa |

## Context (kết quả khảo sát)
- [`slugify`](file:///Users/dungmv/Documents/macos-native-apps/zest/crates/core/src/storage/slug.rs) dùng `char::is_alphanumeric()` nên giữ nguyên ký tự Unicode (`ô`, `đ`...) và biến `/` thành `-`. Được dùng bởi `fs_ops::create_group/rename_group`, `create_request/rename_request` và import environment.
- OpenAPI ([openapi.rs](file:///Users/dungmv/Documents/macos-native-apps/zest/crates/core/src/importers/openapi.rs) ~L139-155): lấy `tags[0]` làm 1 folder, `path = vec![tag]`.
- Postman ([postman.rs](file:///Users/dungmv/Documents/macos-native-apps/zest/crates/core/src/importers/postman.rs) ~L107-125): mỗi item có `item[]` là 1 folder, `name` giữ nguyên.
- [`apply_import_to_workspace`](file:///Users/dungmv/Documents/macos-native-apps/zest/crates/core/src/importers/mod.rs) tạo folder theo `folder.path` (sort theo độ sâu, parent trước) → importer chỉ cần sinh `path` nhiều cấp là có thư mục lồng nhau.
- `create_group` đã lưu `name` gốc vào `_folder.yaml` → yêu cầu #3 đã thoả sẵn, không cần đổi.
- Hiện không có crate transliteration trong `zest-core`.

## Architectural Decisions
- **Thêm dependency `deunicode`** (nhỏ, pure Rust, không phụ thuộc) vào `crates/core/Cargo.toml` để transliterate toàn bộ Unicode → ASCII một cách tổng quát (thay vì bảng thủ công chỉ phủ vài ký tự). `đ/Đ` → `d/D` được `deunicode` hỗ trợ; sẽ có test khẳng định.
- `slugify` quy trình mới: `deunicode(name)` → lowercase → mọi ký tự không phải `[a-z0-9]` thành `-` → gộp `-` → trim → fallback `"request"`. Ký tự không có ánh xạ ASCII (deunicode trả `[?]` hoặc rỗng) được lọc ra, không để lọt chuỗi `[?]` vào slug.
- **Tách `/` ở tầng importer**, không ở `slugify` (slugify xử lý 1 segment). Helper `split_folder_segments(name) -> Vec<String>`: split theo `/`, trim, bỏ segment rỗng; nếu rỗng toàn bộ thì giữ tên gốc.
- Mỗi segment giữ **tên gốc** làm `ImportedFolder.name` (hiển thị), slug do `create_group` tự tạo.
- Folder path nhiều cấp: với mỗi tiền tố của path đảm bảo có một `ImportedFolder` (dedupe bằng `HashSet<Vec<String>>`); `description` chỉ gán cho folder lá.

## Proposed Changes

### zest-core / dependency
#### [MODIFY] crates/core/Cargo.toml
- Thêm `deunicode = "1"`.

### zest-core / storage
#### [MODIFY] slug.rs
- Viết lại thân `slugify` theo quy trình ở trên; cập nhật doc comment (thêm bullet "Transliterates Unicode to ASCII").
- Tests mới: `Thông quan`→`thong-quan`, `Đăng nhập`→`dang-nhap`, `ĐƯỜNG`→`duong`, dạng NFD (ký tự + combining mark), `Café Müller`→`cafe-muller`, `Straße`→`strasse`, `merchant/Thông quan`→`merchant-thong-quan`, `日本語` → slug ASCII không rỗng/không chứa `?`, `!!!`→`request`; giữ nguyên các test cũ.

### zest-core / importers
#### [MODIFY] mod.rs
- Thêm `pub(crate) fn split_folder_segments(name: &str) -> Vec<String>` và `push_folder_chain(parent_prefix, segments, description, folders, seen)`.
- Tests cho split: `"a/b"`, `" a / b "`, `"a//b"`, `"/"`, tên không có `/`.

#### [MODIFY] openapi.rs
- Khối tag: `segments = split_folder_segments(first_tag)`; `folder_path = segments`; đăng ký mọi cấp bằng `push_folder_chain` (thay `folder_names_set` theo tên phẳng bằng set theo `Vec<String>`).

#### [MODIFY] postman.rs
- Nhánh folder: `segments = split_folder_segments(&name)`; `current_path = parent_path + segments`; đăng ký từng cấp trung gian; `parse_items` đệ quy dùng `current_path`.
- Tên **request** không tách `/` (chỉ slug thành `-` khi tạo file).

### Docs
#### [NEW] docs/plans/2026102801-import-slug-vietnamese-and-folder-split.md
- Bản sao plan này (AGENTS.md §4).

## Risk Mitigation
- Đổi `slugify` chỉ ảnh hưởng thư mục/file tạo mới hoặc đổi tên; dữ liệu cũ trên đĩa không bị đổi tên. `unique_slug` / vòng lặp trong `create_group` xử lý đụng tên (`Đăng nhập` & `Dang nhap` → `-2`).
- Slug luôn ASCII nên an toàn hơn trên các filesystem; fallback `request` cho trường hợp rỗng.
- Dependency mới nhỏ, không kéo thêm transitive crate; kiểm tra `cargo tree` sau khi thêm.

## Verification Plan
### Automated Tests
```bash
cargo test -p zest-core slug
cargo test -p zest-core importers
cargo test
cargo clippy --all-targets -- -D warnings
```
Thêm integration test import OpenAPI (tag `merchant/Thông quan`) và Postman (folder cùng tên) vào tempdir; assert tồn tại `collections/merchant/thong-quan/_folder.yaml`, và `_folder.yaml` của lá có `name: Thông quan`, của cha có `name: merchant`.

### Manual Verification
Import spec OpenAPI/Postman thực có tag `merchant/Thông quan`; sidebar hiển thị cây `merchant › Thông quan`, thư mục trên đĩa toàn ASCII.
