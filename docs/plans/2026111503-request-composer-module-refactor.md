# 2026111503 — Request Composer Module Refactor

## Goal

Refactor `crates/app/src/app/request_composer.rs` (509 LOC, một method `AppView::render_request_composer` duy nhất) thành folder module `crates/app/src/app/request_composer/` với tổ chức tương đương `crates/app/src/components/response_inspector/`, đồng thời **di chuyển toàn bộ file liên quan đến composer** vào chung một module:

- `ws_message_tab.rs`, `grpc_message_tab.rs`, `grpc_proto_tab.rs`, `auth_tab.rs` — các tab renderer do chính request composer sử dụng (consumer duy nhất).
- `composer_inputs.rs` — bundle editor handles của composer.

Không thay đổi behavior/UI — move verbatim, chỉ re-thread data access.

## Context

- `request_composer.rs` là một `impl AppView` chứa: 3 banner (conflict / parse-error / corrupt-secrets), request bar, tab strip với badges, và 8 tab content arm (Body, Message, Proto, Params, Headers, Auth, PreRequestScript, Tests).
- Khảo sát consumer: 4 file tab **chỉ** được dùng bởi `request_composer.rs` (qua `super::`) và **không có import `use super::`** bên trong → di chuyển sạch. `composer_inputs.rs` chỉ được tham chiếu bởi `app/mod.rs` (`pub use composer_inputs::*`), `subscriptions.rs` (1 dòng import), `workspace_event_handler.rs` (chỉ gọi method trên field).
- `ComposerSyncScope` nằm ở `state/workspace_events.rs` — không nằm trong `composer_inputs.rs`, state layer không bị ảnh hưởng.

## Architectural Decisions

1. **1:1 per-tab layout** (đã chốt với user): mỗi tab một file, mirrors `response_inspector` & `RequestTabKind`.
2. **Free render functions**, không tách inherent impl: mỗi file export `pub(super) fn render_x_tab(av: &AppView, ...) -> AnyElement` — pattern của `response_inspector` (tab fns nhận `&ResponseInspector`). Child module của `app` đọc được field `pub(crate)` của `AppView`.
3. **`window` param chỉ ở nơi dùng thật** (message/proto) để tránh clippy unused param.
4. **Re-derive snapshot cục bộ**: fn mới tự đọc `av.state.workspace.read(cx)` / `active_request_id()` — render đồng bộ, semantics không đổi; không giữ borrow xuyên fn boundary.
5. **Visibility**: tab files đổi `pub` → `pub(super)`; `composer_inputs` là `pub(crate) mod` + item `pub`, re-export từ `app/mod.rs`.
6. **Auth arm gọi trực tiếp** `auth_tab::render_auth_tab` từ dispatch (việc move auth_tab.rs vào module tự giải quyết xung đột tên).
7. **Dedup closure append-snippet** 12 dòng bị lặp giữa 2 arm script vào 1 helper trong `scripts_tab.rs`.

## Cấu trúc đích

```
crates/app/src/app/request_composer/
├── mod.rs              (~150) orchestrator: prelude, banners wiring, request bar, match dispatch
├── composer_inputs.rs  (341)  git mv
├── auth_tab.rs         (629)  git mv, pub → pub(super)
├── ws_message_tab.rs   (147)  git mv, pub → pub(super)
├── grpc_message_tab.rs (204)  git mv, pub → pub(super)
├── grpc_proto_tab.rs   (173)  git mv, pub → pub(super)
├── banners.rs          (~140) extract 3 banner (dòng 60–178)
├── tab_strip.rs        (~85)  extract TabBar + badges (dòng 201–260)
├── body_tab.rs         (~35)  extract arm Body
├── message_tab.rs      (~80)  extract arm Message (WS/gRPC/Http)
├── proto_tab.rs        (~40)  extract arm Proto
├── params_tab.rs       (~60)  extract arm Params (KvTable)
├── headers_tab.rs      (~65)  extract arm Headers (KvTable + inherited)
└── scripts_tab.rs      (~75)  extract 2 arm script + dedup snippet closure
```

## Step-by-Step Implementation Tasks

1. Ghi plan artifact này.
2. `mkdir app/request_composer/` + `git mv` 6 file (request_composer.rs → `mod.rs`).
3. Sửa `app/mod.rs`: bỏ 5 khai báo `pub mod {auth_tab, composer_inputs, grpc_message_tab, grpc_proto_tab, ws_message_tab};`, đổi `pub use composer_inputs::*;` → `pub use request_composer::composer_inputs::*;`. Sửa 1 dòng import trong `subscriptions.rs`.
4. Khai báo 13 submodule trong `mod.rs`; chỉnh visibility các file moved; sửa đường dẫn sibling (`super::ws_message_tab::…` → sibling path).
5. Extract `banners.rs`, `tab_strip.rs` verbatim.
6. Extract 6 per-tab files verbatim.
7. Viết lại `mod.rs` làm orchestrator mỏng.
8. Verification pipeline.

## Risk Mitigation

- `auth_tab.rs` 629 LOC sát limit 650 — move nguyên vẹn, không tách (out of scope).
- Borrow `ws_read` xuyên match trong bản cũ → các fn mới tự đọc lại state; banner fns nhận entity clone + msg owned.
- Diff review: style calls trong `git diff` phải nguyên văn 1-1 (memory: refactor must not restyle).

## Verification Steps

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

- LOC mỗi file < 650.
- Smoke test (cargo run): đủ 8 tab render đúng, badge counts đúng, 3 banner hiện đúng, WS/gRPC Message + Proto tab hoạt động.
