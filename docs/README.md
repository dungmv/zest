# Zest — Postman & Insomnia Parity Architecture & Roadmap

> **Sứ mệnh (Mission):** Xây dựng một Desktop REST & API Client nguồn mở thế hệ mới bằng **Rust** và **GPUI** (Zed GUI engine), có hiệu năng siêu tốc (< 50MB RAM, khởi động < 0.2s), trải nghiệm cao cấp tương đương hoặc vượt trội **Postman** và **Insomnia**, nhưng tuân thủ triệt để triết lý:
> - **100% Offline-First / Local-First:** Không ép buộc tạo tài khoản cloud, không thu phí lưu trữ, dữ liệu hoàn toàn nằm trên máy người dùng.
> - **Git-Native Storage:** Collections và Environments được lưu dưới dạng file plain text (JSON/YAML ổn định key), dễ dàng commit, push/pull, review PR và giải quyết conflict trực tiếp qua Git.
> - **High-Performance Native GUI:** Sử dụng GPUI (framework đồ họa GPU-accelerated của Zed) mang lại tốc độ phản hồi 120 FPS, không dùng Electron cồng kềnh ngốn RAM.

---

## 📑 Mục lục tài liệu thiết kế (Documentation Index)

Thư mục `docs/` này bao gồm toàn bộ thiết kế chi tiết, kiến trúc kỹ thuật và kế hoạch thực thi để đưa Zest đạt mức tương đương Postman & Insomnia:

| Tài liệu | Nội dung chính |
|---|---|
| [**roadmap.md**](./roadmap.md) | Lộ trình phát triển 7 giai đoạn từ Phase 0 (Foundation) đến Phase 6 (Automation Suite) |
| [**technical-debt.md**](./technical-debt.md) | Danh mục nợ kỹ thuật tập trung: SL-001..SL-006, PERF, CC, TD, Kiến trúc LRU Cache, State decoupling |
| [**specs/00-core-http-foundation.md**](./specs/00-core-http-foundation.md) | Giai đoạn 0: Nền tảng cốt lõi — Gửi HTTP Request, timing breakdown, xem response, File storage |
| [**specs/01-import-export-compatibility.md**](./specs/01-import-export-compatibility.md) | Chuẩn tương thích: Import/Export Postman v2.1, Insomnia v4, OpenAPI 3.0/3.1, HAR, Bruno |
| [**specs/02-scripting-and-testing-engine.md**](./specs/02-scripting-and-testing-engine.md) | Pre-request Script, Test Script, Request Chaining (lấy token nhét vào env), JS Sandbox |
| [**specs/03-protocols-graphql-websocket-grpc.md**](./specs/03-protocols-graphql-websocket-grpc.md) | Mở rộng giao thức: GraphQL (Introspection, Explorer), WebSocket (real-time stream), SSE, gRPC |
| [**specs/04-advanced-auth-and-inheritance.md**](./specs/04-advanced-auth-and-inheritance.md) | OAuth 2.0 (PKCE, Refresh Token), AWS SigV4, mTLS Certs, Kế thừa Auth từ Folder/Workspace |
| [**specs/05-git-sync-and-collaboration.md**](./specs/05-git-sync-and-collaboration.md) | Tích hợp Git UI trong app: Branching, Commit, Push/Pull, Visual Diff cho API changes |
| [**specs/06-developer-tools-codegen-diff-docs.md**](./specs/06-developer-tools-codegen-diff-docs.md) | Sinh Code (15+ ngôn ngữ), JSONPath Filter, Dynamic Fake Vars (`{{$uuid}}`), Response Diff |
| [**specs/07-ui-ux-enhancements.md**](./specs/07-ui-ux-enhancements.md) | Single-active Request & History Navigation, Preview ảnh/HTML, Cookie Manager nâng cao, Theme |
| [**specs/08-collection-runner-and-testing-suite.md**](./specs/08-collection-runner-and-testing-suite.md) | Collection Runner, Data-Driven CSV/JSON, Headless CLI (Newman alt), Mock Server, Benchmark |
| [**specs/09-phase1-implementation-notes.md**](./specs/09-phase1-implementation-notes.md) | Giai đoạn 1 (Hoàn thành): Scripting QuickJS (`rquickjs`), Pre-request/Tests, Request Chaining, Faker |

---

## 📊 Bảng so sánh tính năng (Feature Parity Matrix)

| Nhóm tính năng | Postman | Insomnia | Zest (Hiện tại) | Zest (Mục tiêu Parity) |
|---|:---:|:---:|:---:|:---:|
| **Nền tảng & RAM** | Electron (~800MB - 1.5GB) | Electron (~600MB - 1.2GB) | **Rust + GPUI (~40MB)** | **Rust + GPUI (~50MB)** |
| **Khởi động** | 3s - 8s | 2s - 5s | **< 0.2s** | **< 0.2s** |
| **Bắt buộc Cloud/Login** | Có (từ 2023) | Có (từ v8) | **Không (100% Offline)** | **Không (100% Offline)** |
| **Lưu trữ Git-friendly** | Kém (Cloud JSON blob) | Kém | **Tuyệt vời (1 file / req)** | **Tuyệt vời (1 file / req)** |
| **HTTP Methods & Headers** | Đầy đủ | Đầy đủ | Đầy đủ | Đầy đủ |
| **Biến môi trường `{{var}}`** | Đầy đủ | Đầy đủ | **Đầy đủ + Dynamic Fake Vars (`{{$uuid}}`, `{{$timestamp}}`,...)** | **Đầy đủ + Dynamic Fake Vars** |
| **Timing Breakdown** | DNS, TLS, TTFB | TTFB, Total | DNS, TCP, TLS, TTFB, Download | DNS, TCP, TLS, TTFB, Download |
| **Response Cache & Examples** | Cloud sync | Local Storage | Local Response Cache (.loc) | Examples & Response Cache |
| **Import Postman / Insomnia** | N/A | Postman | **Postman v2.0/v2.1 (Đã hoàn thành)** | **1-Click Import hoàn chỉnh** |
| **Import cURL khi dán (URL bar)** | Có | Có | **Đầy đủ (Auto-detect & parse đa nền tảng)** | **Đầy đủ** |
| **Import OpenAPI 3.0 / 3.1** | Có | Có | **Đầy đủ (JSON/YAML + Cycle-safe `$ref`)** | **Full Spec Tree Generator** |
| **Code Generator** | 15+ languages | 12+ languages | cURL | **16+ languages & frameworks** |
| **Pre-request & Test Scripts** | JavaScript sandbox | JavaScript sandbox | **Embedded QuickJS (`rquickjs`) Sandbox** | **Embedded QuickJS (`rquickjs`) Sandbox** |
| **Request Chaining** | `pm.environment.set` | Tag filters | **Tự động qua `z.environment.set`** | **Tự động qua Test Script** |
| **GraphQL Client** | Có (Schema explorer) | Có | Gửi qua POST JSON | **Dedicated GraphQL IDE** |
| **WebSocket / SSE** | Có | Có | Chưa có | **Realtime Messages Frame Log** |
| **OAuth 2.0 PKCE / Auto-Refresh** | Có | Có | Bearer / Basic / API Key + Kế thừa Folder/Workspace | **Tự động lấy & refresh token** |
| **Collection Runner (Testing)** | Có | Inso CLI | Chưa có | **UI Runner + Headless CLI** |
| **In-app Git Client** | Fork/PR Cloud | Insomnia Git sync | **Đầy đủ (Branch, Commit, Diff, Push/Pull, Remotes)** | **Branch switch, Diff, Commit** |
