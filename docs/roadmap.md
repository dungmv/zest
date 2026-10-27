# Kế hoạch phát triển tính năng (Roadmap to Full Parity — 7 Phases: Phase 0 to Phase 6)

Tài liệu này xác định lộ trình phát triển toàn diện của **Zest** qua 7 giai đoạn (từ Phase 0 đến Phase 6), bắt đầu từ nền tảng gửi/nhận HTTP cốt lõi, tiếp nối bằng Scripting, Đa giao thức, Xác thực, DevTools, Khả năng tương thích Import/Export và Bộ công cụ Kiểm thử & Tự động hóa toàn diện (Testing & Automation Suite).

---

## 🎯 Giai đoạn 0: Nền Tảng Cốt Lõi — Gửi HTTP Request & Xem Phản Hồi (Core HTTP Foundation)
*Mục tiêu: Xây dựng nền móng vững chắc cho ứng dụng: bộ soạn thảo request đa định dạng, tầng mạng async decoupled từ luồng UI, đo lường thời gian chính xác, trình xem phản hồi đa năng và hệ thống lưu trữ file phẳng Git-native.*

1. **Bộ Soạn Thảo HTTP Request Đa Dạng (Request Builder):**
   - Hỗ trợ đầy đủ phương thức: `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`.
   - Bảng Query Parameters & Headers động kèm checkbox bật/tắt từng dòng, tự động encode URL.
   - Hỗ trợ các định dạng Request Body: None, JSON (đánh số dòng, format, syntax highlight), Form URL-Encoded, Multipart Form-Data, Raw Text, Binary File.
   - Cấu hình request: Network Timeout, Follow Redirects, Validate TLS Certificates.
2. **Tầng Mạng Tokio Decoupled & Huỷ Yêu Cầu In-Flight:**
   - Thực thi request hoàn toàn trên luồng Tokio riêng biệt, không block UI 120 FPS của GPUI.
   - Nút **Cancel** ngắt kết nối socket ngay lập tức khi request đang chạy.
   - Stream thông báo trạng thái kết nối liên tục về giao diện.
3. **Đo Lường Thời Gian Chính Xác Cao (High-Precision Timing Breakdown):**
   - Đo lường chi tiết: DNS Lookup, TCP Connect, TLS Handshake, TTFB (Time to First Byte), Content Download, và Total Duration.
   - Hiển thị thanh đo trực quan dạng popover khi di chuột vào thời gian phản hồi.
4. **Trình Xem Phản Hồi Đa Năng (Response Viewer & Inspector):**
   - Thanh trạng thái trực quan: Status Code phân màu (2xx/3xx/4xx/5xx), Thời gian ms, Dung lượng byte/KB/MB.
   - Các tab kiểm tra: Response Body (`Pretty` / `Raw`), Request Sent (request thực tế gửi qua mạng), Headers, Cookies.
   - Xử lý mượt mà payload lớn (10MB+) bằng cơ chế dòng ảo hóa (virtualized line rendering), không gây đơ lag giao diện.
5. **Hệ Thống Lưu Trữ File Phẳng Git-Native (Storage Foundation):**
   - Lưu trữ collections và requests dưới dạng file YAML phẳng (Git-friendly, không lock-in).
   - Biến môi trường cơ bản với cơ chế thay thế `{{var}}` trong URL, headers, params, auth, body.
   - Lưu response cache vào file `.loc` cạnh file request.
6. **Tự Động Nhận Diện & Parse cURL Khi Dán (cURL Auto-Parse on Paste) — ✅ Đã hoàn thành:**
   - Tự động nhận diện lệnh cURL dán vào ô URL input (`is_curl_command`).
   - Hỗ trợ định dạng cURL đa nền tảng (Bash `\`, Windows CMD `^`, PowerShell ``` ` ```, loại bỏ `$ ` / `> `).
   - Tự động bóc tách: Method, URL, Query params, Headers, Auth (Bearer / Basic base64), Body (JSON, Form-urlencoded, Multipart, Raw text).
   - Tự động kích hoạt tab phù hợp và đồng bộ toàn bộ giao diện kèm Toast thông báo.

---

## 🎯 Giai đoạn 1: Scripting, Tự động hóa & Request Chaining (Scripting & Automation Engine) — ✅ Đã hoàn thành (Implemented & Tested)
*Mục tiêu: Đạt được tính năng cốt lõi tạo nên sức mạnh của Postman: chạy script tiền xử lý, kiểm thử response và tự động truyền token giữa các request. (Xem chi tiết tại [specs/09-phase1-implementation-notes.md](./specs/09-phase1-implementation-notes.md))*

1. **Embedded Scripting Runtime (`rquickjs` QuickJS C bindings):**
   - Tích hợp engine QuickJS siêu nhẹ qua `rquickjs`, khởi tạo context < 0.5ms độc lập cho mỗi request.
   - Hỗ trợ đầy đủ cú pháp ES2023, async-safe, cô lập bộ nhớ, không crash UI.
2. **Pre-request Scripts:**
   - Cho phép viết code JS chạy trước khi gửi request.
   - Cung cấp API `z.environment.set(key, val)`, `z.environment.get/unset`, `z.variables.get/set/has(key)`.
   - Tính toán dynamic signatures (`z.crypto.hmacSha256`, `md5`, `base64Encode`, `base64Decode`, timestamp, nonce).
3. **Test Scripts & Assertions (Request Chaining):**
   - Viết test assertions sau khi nhận response:
     ```javascript
     z.test("Status code is 200", () => {
         z.assert(z.response.status === 200, "Expected status 200");
     });
     // Request Chaining: Trích xuất access token từ response và lưu vào Environment
     const data = z.response.json();
     z.environment.set("access_token", data.token);
     ```
   - Hiển thị tab "Tests & Console" hợp nhất trong Response Inspector với badge kết quả (`Passed 3/3` màu xanh lá hoặc `Failed 1/3` màu đỏ), test execution duration và console logs có cấu trúc.
4. **Dynamic Fake Variables (Faker built-in):**
   - Tích hợp các macro biến ngẫu nhiên trực tiếp trong URL/Headers/Body:
     - `{{$uuid}}` hoặc `{{$guid}}`: Sinh UUID v4
     - `{{$timestamp}}`: Timestamp unix epoch hiện tại
     - `{{$isoTimestamp}}`: ISO 8601 UTC string
     - `{{$randomInt}}`, `{{$randomEmail}}`, `{{$randomName}}`, `{{$randomPhoneNumber}}`.

---

## 🎯 Giai đoạn 2: Mở rộng Đa Giao thức (Protocols: GraphQL, WebSocket, SSE, gRPC)
*Mục tiêu: Đưa Zest trở thành API client toàn diện không chỉ giới hạn ở REST thông thường.*

1. **GraphQL Client Chuyên Dụng:**
   - Editor GraphQL 2 pane: Query/Mutation bên trái, Variables JSON bên phải.
   - Nút **Introspect Schema**: Gửi introspection query tới server để tải schema về.
   - Gợi ý từ khóa (Autocomplete) query, fields, types, directives dựa trên schema đã introspect.
2. **WebSocket Client:**
   - Khởi tạo kết nối WebSocket trực tiếp (`ws://` và `wss://`).
   - Quản lý trạng thái: Connecting, Connected, Disconnected.
   - Khung gửi tin nhắn: Text, JSON, Binary.
   - Message Stream Viewer: Lịch sử tin nhắn Real-time (inbound/outbound) kèm timestamp, lọc tin nhắn, clear log.
3. **Server-Sent Events (SSE) & Streaming Responses:**
   - Hỗ trợ nhận stream HTTP (chunked transfer encoding, SSE `text/event-stream`).
   - Hiển thị từng event nhận được theo thời gian thực (rất quan trọng khi test các LLM API như OpenAI, Anthropic, Gemini).
4. **gRPC Client (Unary & Streaming):**
   - Hỗ trợ nạp file `.proto` hoặc Server Reflection.
   - Chọn Service và RPC Method từ dropdown.
   - Sinh payload JSON tương ứng với protobuf message.

---

## 🎯 Giai đoạn 3: Xác thực Nâng cao & Kế thừa Quyền (Auth & Inheritance)
*Mục tiêu: Tối ưu quy trình đăng nhập phức tạp trong doanh nghiệp.*

1. **OAuth 2.0 Hoàn Chỉnh:**
   - Hỗ trợ các Grant Types:
     - Authorization Code (kèm **PKCE** - Proof Key for Code Exchange)
     - Client Credentials
     - Resource Owner Password Credentials
     - Implicit
   - Tự động bắt Authorization Code qua Local HTTP Callback Server (`http://localhost:random_port`).
   - Tự động Refresh Token khi token hết hạn dựa trên expiry header hoặc status code 401.
2. **Kế Thừa Xác Thực & Headers (Auth & Headers Inheritance) — ✅ Đã hoàn thành:**
   - Cấu hình Auth và Headers một lần tại cấp độ **Workspace** hoặc **Folder** (`ScopeSettingsModal`).
   - Mọi request con bên trong tự động kế thừa headers (hiển thị read-only qua `InheritedHeaders`) và dùng `Auth: Inherit from parent` mà không cần cấu hình lại từng request.
3. **Client Certificates & Mutual TLS (mTLS):**
   - Cấu hình file Certificate (`.pem`, `.pfx`, `.crt`) và Private Key (`.key`) tương ứng cho từng Host/Domain.
   - Tùy chọn Custom CA Certificate bundle cho môi trường nội bộ doanh nghiệp có SSL Proxy.
4. **AWS Signature v4:**
   - Nhập Access Key ID, Secret Access Key, AWS Region, Service Name, Session Token.
   - Tự động tính toán Canonical Request và ký header `Authorization: AWS4-HMAC-SHA256...`.

---

## 🎯 Giai đoạn 4: Công cụ Developer & Nâng Cấp Giao Diện (DevTools & UI Polish)
*Mục tiêu: Vượt trội hơn Postman/Insomnia về trải nghiệm lập trình viên và tốc độ.*

1. **Code Snippet Generator (16+ targets):**
   - Sinh mã nguồn từ request sang các ngôn ngữ phổ biến:
     - Rust (`reqwest`, `ureq`)
     - Python (`requests`, `httpx`, `aiohttp`)
     - JavaScript / TypeScript (`fetch`, `axios`)
     - Node.js (`http`, `node-fetch`)
     - Go (`net/http`)
     - Java (`HttpClient`, `OkHttp`)
     - C# (`HttpClient`, `RestSharp`)
     - PHP (`curl`, `Guzzle`)
     - cURL, HTTPie, Wget.
2. **Response Inspector Đa Dạng (Previews & Diff):**
   - **JSONPath / XPath Query Bar:** Gõ `$.data.items[*].id` để lọc nhanh dữ liệu JSON hiển thị ngay lập tức.
   - **Preview Renderer:**
     - Ảnh (PNG, JPEG, SVG, WebP, GIF).
     - HTML Web Preview (sandboxed).
     - PDF viewer & Hex viewer.
   - **Response Diff:** So sánh 2 response từ History xem trường nào vừa thay đổi.
3. **Mô Hình Phiên Làm Việc Single-Active Request, Write-Behind Persistence & History Navigation — ✅ Đã hoàn thành (SL-001):**
   - Áp dụng mô hình gọn nhẹ chuẩn Insomnia/Yaak: `RequestSessionState` chỉ trỏ tới active request, dữ liệu mượn on-demand từ `WorkspaceDataState`.
   - History Navigation: ngăn xếp lịch sử chuyển đổi request với phím tắt `NavigateBack` (`Cmd+[`), `NavigateForward` (`Cmd+]`).
   - Tự động lưu ngầm hợp nhất qua `PersistenceCoordinator` (coalesced write-behind theo `PersistTarget` với quản lý `revision` và `workspace_gen`).
4. **Tích hợp Git Trực Quan trong Ứng dụng — ✅ Đã hoàn thành:**
   - Panel quản lý Git (`GitModal` & `GitService`): Xem danh sách file bị sửa đổi (`git status`), lịch sử commit, quản lý nhánh và remotes.
   - Visual Diff: So sánh trực quan thay đổi của file với commit trước.
   - Nút Commit, Discard Changes, Fetch, Pull, Push trực tiếp không cần mở terminal, tự động đồng bộ với file watcher.

---

## 🎯 Giai đoạn 5: Khả năng Tương Thích & Nhập/Xuất (Import/Export & Migration)
*Mục tiêu: Giúp người dùng chuyển ngay lập tức toàn bộ dữ liệu đang có từ Postman, Insomnia, OpenAPI và cURL sang Zest mà không mất bất kỳ cấu hình nào.*

1. **Import cURL Tự Động Khi Dán Trực Tiếp (Inline cURL Paste) — ✅ Đã hoàn thành (100% Implemented & Tested):**
   - Tự động nhận diện lệnh cURL khi dán vào thanh URL input.
   - Trích xuất toàn bộ Method, URL, Query params, Headers, Auth (Bearer/Basic), Body (JSON/Form/Multipart/Raw).
   - Đồng bộ ngay lập tức toàn bộ giao diện và cập nhật request.
2. **Import Postman Collections & Environments — ✅ Đã hoàn thành:**
   - Hỗ trợ chuẩn Postman Collection v2.0 và v2.1 format (`importers/postman.rs`).
   - Chuyển đổi chính xác: Folders, Requests, URL params, Headers, Body, Auth, Scripts, Environments.
3. **Import Insomnia Export:**
   - Hỗ trợ Insomnia Export v4 (YAML & JSON).
   - Mapping Workspaces, Request Groups (Folders), Requests, Environments.
4. **Import OpenAPI / Swagger (v3.0, v3.1) — ✅ Đã hoàn thành:**
   - Đọc OpenAPI specs (YAML/JSON) kèm bộ phân giải `$ref` chống đệ quy vòng lặp (`importers/openapi.rs`).
   - Tự động sinh cây request theo Tag/Path kèm query parameters, headers, auth và sample JSON body từ schema.
5. **Export đa định dạng:**
   - Xuất Collection sang Postman v2.1 JSON (để chia sẻ cho đồng nghiệp vẫn dùng Postman).
   - Xuất sang OpenAPI 3.0 YAML/JSON.
   - Xuất sang định dạng Bruno (`.bru`) và cURL (đã hỗ trợ "Copy as cURL").

---

## 🎯 Giai đoạn 6: Bộ Công Cụ Kiểm Thử & Tự Động Hóa Nâng Cao (Testing & Automation Suite)
*Mục tiêu: Cung cấp sức mạnh tương đương Postman Collection Runner, Newman CLI, và Mock Server cho quy trình CI/CD.*

1. **Collection Runner (Giao Diện UI):**
   - Chạy hàng loạt toàn bộ request trong một Folder hoặc Workspace theo thứ tự.
   - Tùy chọn cấu hình:
     - Số lần lặp (Iterations).
     - Độ trễ giữa các request (Delay ms).
     - Dừng khi gặp lỗi (Stop on failure toggle).
     - Tự động chia sẻ state và biến môi trường giữa các request xuyên suốt runner run.
   - Giao diện giám sát tiến độ thời gian thực (Progress bar, danh sách request đang chạy, kết quả Test pass/fail từng request).
2. **Data-Driven Testing (Kiểm thử dựa trên tệp dữ liệu CSV / JSON):**
   - Nạp file `test-data.csv` hoặc `data.json`.
   - Mỗi hàng trong file tương ứng với 1 iteration của Collection Runner.
   - Các cột trong CSV tự động ánh xạ vào biến `{{column_name}}` trong URL, Header, Body và script `client.iterationData.get("column_name")`.
3. **Headless CLI Test Runner (Tương đương Newman / Bruno CLI):**
   - Tạo sub-crate CLI nhị phân riêng: `crates/cli` (`zest-cli`).
   - Lệnh chạy trong CI/CD pipeline (GitHub Actions, GitLab CI, Docker):
     ```bash
     zest-cli run ./my-api-collection --env staging --data users.csv --reporters cli,json,html
     ```
   - Trả về exit code 0 nếu tất cả test đỗ, exit code 1 nếu có test trượt.
4. **Báo Cáo Kiểm Thử Tự Động (Visual Test Reports):**
   - Xuất báo cáo kết quả kiểm thử ra tệp HTML đẹp mắt hoặc JSON summary để đính kèm vào Artifacts của CI/CD.
5. **Local Mock Server (Trình Giả Lập API Cục Bộ):**
   - Khởi chạy HTTP server giả lập trên `localhost:port` từ các Response Examples đã lưu trong collection hoặc từ OpenAPI spec.
   - Hỗ trợ frontend developers phát triển trước khi backend APIs hoàn thành mà không cần kết nối internet hay server thật.
6. **Stress & Benchmark Testing Cơ Bản:**
   - Chạy kiểm thử tải nhẹ (Concurrency 1..50, Total Requests 100..10,000).
   - Đo lường và vẽ biểu đồ phân phối độ trễ: Min, Max, Mean, P50, P90, P99, Throughput (req/s), tỷ lệ lỗi Error Rate.
