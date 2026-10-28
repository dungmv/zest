# Đặc tả Kỹ thuật: Nhập / Xuất & Tương Thích Hệ Sinh Thái (Import & Export Compatibility)

## 1. Bối cảnh & Mục tiêu
Khi Postman chuyển đổi sang chính sách lưu trữ đám mây bắt buộc (Cloud-only) vào năm 2023 và Insomnia đưa ra yêu cầu đăng nhập tài khoản bắt buộc ở phiên bản 8, hàng triệu kỹ sư trên thế giới đã tìm kiếm giải pháp thay thế mã nguồn mở, lưu trữ cục bộ (Local-first / Git-friendly).

Để Zest trở thành sự thay thế khả dĩ nhất, rào cản di chuyển dữ liệu (Migration Barrier) phải bằng **0**. Người dùng phải có khả năng:
1. Nhập nguyên vẹn Collections, Folders, Requests và Environments từ Postman (v2.0 & v2.1).
2. Nhập Workspaces từ Insomnia (v4 export).
3. Nhập từ tài liệu kỹ thuật OpenAPI 3.0 / 3.1 & Swagger 2.0.
4. Nhập từ tệp log trình duyệt HAR (HTTP Archive).
5. Xuất ngược lại ra Postman v2.1 hoặc OpenAPI để chia sẻ cho các đối tác hoặc thành viên nhóm vẫn đang sử dụng các công cụ cũ.

---

## 2. Kiến trúc Module (`zest-core`)

```
crates/core/src/
├── curl/
│   └── mod.rs            # cURL parser (is_curl_command, curl_to_request) & exporter (request_to_curl)
└── importers/
    ├── mod.rs            # Trait CollectionImporter, CollectionExporter
    ├── postman.rs        # Postman v2.0 & v2.1 JSON parser & converter
    ├── insomnia.rs       # Insomnia v4 YAML/JSON parser
    ├── openapi.rs        # OpenAPI 3.0, 3.1 & Swagger 2.0 parser
    ├── har.rs            # HAR 1.2 log parser
    └── bruno.rs          # Bruno .bru parser
```

### 2.1 Trait Định Nghĩa Core

```rust
use crate::models::{Workspace, Folder, HttpRequestItem, Environment};

pub struct ImportedData {
    pub workspace_name: String,
    pub folders: Vec<Folder>,
    pub requests: Vec<HttpRequestItem>,
    pub environments: Vec<Environment>,
}

pub trait CollectionImporter {
    fn name(&self) -> &'static str;
    fn can_handle(&self, content: &str) -> bool;
    fn parse(&self, content: &str) -> Result<ImportedData, ImportError>;
}

pub trait CollectionExporter {
    fn name(&self) -> &'static str;
    fn export(
        &self,
        workspace: &Workspace,
        folders: &[Folder],
        requests: &[HttpRequestItem],
        environments: &[Environment],
    ) -> Result<String, ExportError>;
}
```

---

## 3. Chi tiết chuyển đổi Postman v2.1 (`postman.rs`)

### 3.1 Sơ đồ cấu trúc Postman Schema
Postman v2.1 có cấu trúc lồng nhau dạng đệ quy (`item` có thể là Request hoặc Folder chứa các `item` con):
```json
{
  "info": {
    "name": "My API Collection",
    "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
  },
  "item": [
    {
      "name": "Auth Folder",
      "item": [
        {
          "name": "Login",
          "request": {
            "method": "POST",
            "header": [{"key": "Content-Type", "value": "application/json"}],
            "url": {
              "raw": "{{base_url}}/api/login",
              "query": [{"key": "redirect", "value": "true", "disabled": false}]
            },
            "body": {
              "mode": "raw",
              "raw": "{\"user\": \"test\"}"
            }
          }
        }
      ]
    }
  ]
}
```

### 3.2 Quy tắc Mapping:
1. **URL & Variables:**
   - Postman sử dụng cú pháp `{{variable_name}}` tương thích 100% với Zest.
   - Trường hợp `url` là String hoặc Object `{ raw: "..." }`, parser tự động chuẩn hóa về String URL và bảng Query KeyValuePair.
2. **Body:**
   - `mode: "raw"` -> `RequestBody::Json` (nếu Header có `application/json`) hoặc `RequestBody::Raw`.
   - `mode: "urlencoded"` -> `RequestBody::FormUrlEncoded`.
   - `mode: "formdata"` -> `RequestBody::Multipart`.
3. **Authentication:**
   - `type: "bearer"` -> `Auth::Bearer { token: ... }`.
   - `type: "basic"` -> `Auth::Basic { username, password }`.
   - `type: "apikey"` -> `Auth::ApiKey { key, value, add_to }`.
4. **Pre-request & Tests Script:**
   - Postman lưu trong mảng `event`:
     - `listen: "prerequest"` -> map sang `request.pre_request_script`.
     - `listen: "test"` -> map sang `request.test_script`.

---

## 4. Chi tiết chuyển đổi OpenAPI 3.0 / 3.1 (`openapi.rs`)

### 4.1 Quy tắc sinh cây Request:
1. **Phân nhóm theo Tags:**
   - Mỗi `tag` trong OpenAPI spec được tạo thành một `Folder` tương ứng trong Zest.
   - Các endpoints không có tag sẽ được đưa vào thư mục `General` hoặc root level.
2. **Method & URL Path:**
   - Lấy URL từ `servers[0].url` (chuyển thành biến môi trường `{{base_url}}`).
   - Đường dẫn `{user_id}` tự động chuyển đổi thành cú pháp biến `{{user_id}}` hoặc path parameter trong bảng Params.
3. **Mẫu JSON Body tự động (Mock Example Generator):**
   - Đọc `requestBody.content["application/json"].schema`.
   - Nếu có `example` thì lấy trực tiếp.
   - Nếu không có, duyệt đệ quy `properties` để sinh JSON khung mẫu (ví dụ: `string` -> `"string"`, `integer` -> `0`, `boolean` -> `true`).
4. **Query & Header Parameters:**
   - Duyệt `parameters` với `in: "query"` và `in: "header"`.
   - Tự động điền giá trị `required`, `example` hoặc `default`.

---

## 5. Trải nghiệm người dùng trên UI (UI UX Flow)

1. **Dán cURL Trực Tiếp Vào Thanh URL (Inline cURL Paste & Auto-Import) — ✅ Đã Triển Khai:**
   - Người dùng chỉ cần sao chép câu lệnh cURL từ bất kỳ đâu (Chrome DevTools, Postman, Terminal, Docs) và nhấn `Ctrl + V` vào ô nhập URL.
   - Hệ thống tự động nhận diện tiền tố `curl` / `curl.exe` / `$ curl` qua `is_curl_command`.
   - Toàn bộ method, url, query parameters, headers, auth và body được trích xuất ngay lập tức, thanh URL hiển thị URL sạch, các tab được cập nhật đồng bộ và Toast thông báo hiển thị kết quả.
2. **Kéo-Thả (Drag & Drop):** Người dùng có thể kéo trực tiếp file `.json` (Postman/OpenAPI) hoặc `.yaml` vào cửa sổ Zest để hiển thị dialog preview import.
3. **Import Modal (`Ctrl + I` hoặc Menu `⋮` -> Import):**
   - Hỗ trợ 3 tab:
     - **File:** Chọn file từ ổ đĩa.
     - **Raw Text / Clipboard:** Dán JSON/YAML hoặc lệnh cURL.
     - **URL:** Tải từ link công khai (ví dụ: `https://api.example.com/swagger/v1/swagger.json`).
4. **Conflict Resolution:** Khi import phát hiện trùng tên Workspace hoặc Request, cung cấp tùy chọn:
   - Ghi đè (Overwrite).
   - Đổi tên (Keep both, append `_copy`).
   - Bỏ qua (Skip).
