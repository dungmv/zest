# Đặc tả Kỹ thuật: Xác thực Nâng cao & Kế thừa Quyền (Authentication & Inheritance)

## 1. Vấn đề Thực Tế trong Postman & Insomnia
1. **Lặp lại cấu hình:** Trong một API collection có 50 requests, việc phải dán token Bearer hoặc API Key vào từng request là một trải nghiệm cực kỳ bất tiện.
2. **Hết hạn Token:** OAuth 2.0 Access Token thường hết hạn sau 15 đến 60 phút. Người dùng phải liên tục thực hiện lại các bước đăng nhập bằng tay.
3. **Môi trường Doanh nghiệp khắt khe:** Các hệ thống tài chính, ngân hàng và nội bộ thường yêu cầu mTLS (Mutual TLS với Client Certificate) hoặc AWS IAM Signature v4.

---

## 2. Kế thừa Xác thực (Auth Inheritance Hierarchy)

### 2.1 Cây Kế thừa Quyền
Mọi request có thể chọn sử dụng cấu hình Auth riêng hoặc thừa kế từ cấp cha:

```
Workspace Auth (e.g. Bearer Token hoặc OAuth 2.0)
  └── Folder "Billing API" (Inherit from Workspace)
        ├── Request "Get Invoices" (Inherit from Parent -> dùng Workspace Auth)
        └── Request "Webhook Test" (Auth: None -> ghi đè, không dùng auth)
  └── Folder "Admin API" (Auth: API Key riêng của Admin)
        └── Request "Delete User" (Inherit from Parent -> dùng Admin API Key)
```

### 2.2 Model Cập Nhật (`crates/core/src/models.rs`)
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Auth {
    Inherit, // Kế thừa từ folder cha hoặc workspace cha
    None,
    Bearer { token: String },
    Basic { username: String, password: String },
    ApiKey { key: String, value: String, add_to: ApiKeyLocation },
    OAuth2(OAuth2Config),
    AwsSigV4(AwsSigV4Config),
}
```

---

## 3. OAuth 2.0 Tự Động Hóa (OAuth 2.0 Engine)

### 3.1 Các Luồng Hỗ Trợ (Grant Types):
1. **Authorization Code (kèm PKCE):**
   - Tạo `code_verifier` (chuỗi ngẫu nhiên cryptographically secure) và `code_challenge = BASE64URL(SHA256(code_verifier))`.
   - Khởi chạy một **Local HTTP Server mini** tại cổng ngẫu nhiên (ví dụ: `http://localhost:54321/callback`).
   - Mở trình duyệt mặc định của hệ thống với URL Authorization.
   - Khi người dùng đăng nhập xong, Identity Provider redirect về `http://localhost:54321/callback?code=xyz`.
   - Local server nhận mã `code`, trao đổi với Token URL bằng `code_verifier` để lấy `access_token` và `refresh_token`.
   - Lưu trữ token vào Vault an toàn của Workspace.
2. **Client Credentials:**
   - Dùng Client ID và Client Secret gửi POST trực tiếp tới Token Endpoint.
   - Hoàn toàn không cần tương tác trình duyệt, thích hợp cho Server-to-Server API.
3. **Tự Động Refresh Token (Auto-Refresh Lifecycle):**
   - Lưu trữ trường `expires_at = now() + expires_in`.
   - Trước khi gửi bất kỳ request nào, nếu `now() >= expires_at - 60s`, client tự động gửi POST với `grant_type=refresh_token` để lấy token mới mà người dùng không hề nhận thấy sự gián đoạn.

---

## 4. Client Certificates & Mutual TLS (mTLS)

### 4.1 Cấu hình mTLS theo Host:
Trong Cài đặt Workspace (`WorkspaceSettings`), thêm mục Client Certificates:

```rust
pub struct ClientCertificate {
    pub host: String, // ví dụ: "*.internal.bank.com" hoặc "api.partner.io"
    pub cert_path: String, // đường dẫn tới file cert (.pem, .crt)
    pub key_path: String, // đường dẫn tới private key (.key)
    pub passphrase: Option<String>,
}
```

### 4.2 Tích hợp vào `reqwest` Client:
- Khi khởi tạo `reqwest::ClientBuilder`:
  - Đọc file PEM certificate và private key qua `reqwest::Identity::from_pkcs8_pem(...)`.
  - Nạp CA Root tùy chỉnh qua `reqwest::Certificate::from_pem(...)` để kết nối an toàn với các API tự ký (Self-signed).

---

## 5. AWS Signature Version 4 (AWS SigV4)

- Tự động sinh header:
  ```http
  Authorization: AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20261013/us-east-1/s3/aws4_request, SignedHeaders=host;range;x-amz-date, Signature=fe5f80f779dad...
  x-amz-date: 20261013T083000Z
  ```
- Sử dụng các crate mật mã chuẩn của Rust (`hmac`, `sha2`, `hex`).
