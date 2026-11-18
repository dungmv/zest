# Đặc tả Kỹ thuật: Bộ Công Cụ Developer — Sinh Mã Nguồn, So Sánh Response & Biến Ảo (Developer Tools Specification)

## 1. Trình Sinh Mã Nguồn Đa Ngôn Ngữ (Code Snippet Generator) — ⚠️ [ĐÃ LOẠI BỎ KHỎI SCOPE]

> **Lưu ý:** Tính năng Code Snippet Generator đa ngôn ngữ đã được loại bỏ khỏi lộ trình phát triển của Zest để giữ ứng dụng gọn nhẹ và tối ưu hiệu năng. Thay vào đó, Zest tập trung hỗ trợ tính năng xuất lệnh **"Copy as cURL"** chất lượng cao và cURL auto-paste.

---

## 2. Dynamic Fake Variables (Macro Biến Động `$random*`)

### 2.1 Cú pháp Sử Dụng
Tương thích hoàn toàn với Postman, hỗ trợ chèn trực tiếp trong URL, Headers, hoặc JSON Body:

- **ID & Thời gian:**
  - `{{$guid}}` hoặc `{{$uuid}}`: Sinh ngẫu nhiên UUID v4 (ví dụ: `8f7b3a41-2b99-4d1e-8e33-728b9c6142e0`).
  - `{{$timestamp}}`: Unix timestamp (ví dụ: `1791827400`).
  - `{{$isoTimestamp}}`: Thời gian chuẩn ISO 8601 (ví dụ: `2026-10-13T08:30:00Z`).
- **Dữ liệu giả lập (Faker):**
  - `{{$randomInt}}`: Số nguyên ngẫu nhiên từ 1 đến 1000.
  - `{{$randomEmail}}`: Email ngẫu nhiên (ví dụ: `user_8492@example.org`).
  - `{{$randomFirstName}}`, `{{$randomLastName}}`, `{{$randomFullName}}`.
  - `{{$randomPhoneNumber}}`.
  - `{{$randomIpv4}}`, `{{$randomCity}}`, `{{$randomCountry}}`.

---

## 3. Lọc & Tìm Kiếm Response Nâng Cao (JSONPath Filter Bar)

### 3.1 Vấn đề
Khi server trả về một mảng JSON khổng lồ (vài nghìn phần tử hoặc vài megabytes), việc cuộn và tìm đúng trường dữ liệu rất tốn thời gian.

### 3.2 Giải pháp
Phía trên cửa sổ Response Body, bổ sung một thanh nhập liệu **JSONPath Filter Bar**:
- Nhập cú pháp: `$.items[*].id` hoặc `$.users[?(@.age > 20)].email`.
- Ứng dụng lọc dữ liệu ngay lập tức trong bộ nhớ và chỉ hiển thị tập kết quả khớp.
- Có nút **"Copy Filtered JSON"** để sao chép kết quả đã lọc ra clipboard.

---

## 4. So Sánh Response (Response Diff Viewer) — ⚠️ [ĐÃ LOẠI BỎ KHỎI SCOPE]

> **Lưu ý:** Tính năng so sánh 2 response từ History (Response Diff) đã được loại bỏ khỏi phạm vi phát triển để giảm độ phức tạp giao diện và tối ưu tài nguyên lưu trữ bộ nhớ của hệ thống History/LRU Cache.

---

## 5. Trình Tạo Tài Liệu API Tự Động (API Documentation Generator)

- Nút **"Generate Documentation"** ở cấp độ Collection hoặc Workspace.
- Chế độ xem trang đơn (Single-page view) bao gồm:
  - Tên và mô tả API.
  - Mục lục danh sách endpoint theo từng thư mục.
  - Chi tiết từng endpoint: Method, URL, bảng Parameters, Headers, ví dụ Request Body và ví dụ Response JSON mẫu tương ứng.
  - Cho phép xuất ra tệp `API_DOCS.md` hoặc xuất tệp HTML hoàn chỉnh có thể mở trực tiếp trong trình duyệt web.
