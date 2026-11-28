---
name: request-authoring
description: >-
  Use this skill when generating, creating, or editing Zest API requests (HTTP, WebSocket,
  gRPC, GraphQL, SSE), folder metadata, collections, environment variables, and workspace
  directory structures.
---

# Zest Request Authoring & Workspace Directory Guide

This skill provides the exact specifications, YAML schemas, folder conventions, and scripting patterns needed for any AI agent to generate valid, conflict-free Zest requests and workspaces across all supported protocols (**HTTP**, **WebSocket**, **gRPC**, **GraphQL**, and **SSE**).

---

## 1. Workspace Directory Structure

A Zest workspace is local-first, stored as clean, human-readable YAML files. When creating requests or folders, always adhere to this structure:

```
my-workspace/
├── workspace.yaml                 # Root workspace metadata, shared headers, and auth
├── environments/                  # Environment variable definitions
│   ├── local.yaml                 # Public environment variables (committed)
│   ├── production.yaml
│   └── globals.yaml               # Shared global variables (optional)
├── collections/                   # Root collections directory
│   ├── health-check.yaml          # Root-level request
│   └── auth/                      # Collection / Folder
│       ├── _folder.yaml           # Folder metadata (name, order, headers, auth)
│       ├── login.yaml             # Request definition
│       └── refresh-token.yaml
└── .local/                        # Local runtime state & secrets (STRICTLY GITIGNORED)
    ├── state.yaml
    └── secrets/
        ├── local.yaml             # Secret values for local environment
        └── globals.yaml
```

> [!IMPORTANT]
> - Never manually create `.loc` files. Sidecar response caches (`*.loc`) are managed automatically by Zest at runtime and are gitignored.
> - Never store unencrypted secret values in `environments/*.yaml` files. Secrets belong in `.local/secrets/<env>.yaml`.

---

## 2. File & Directory Naming (Slugification Rules)

Files and folders in `collections/` must use ASCII slug names derived from their display names:

1. **Transliterate Unicode / Diacritics**: Accented characters are converted to ASCII (e.g. `Đăng nhập` → `dang-nhap`, `Thông quan` → `thong-quan`).
2. **Lowercase & Hyphenated**: Alphanumeric characters only; replace spaces and punctuation with `-`.
3. **Trim & Collapse**: Multiple consecutive hyphens collapse into one; strip leading and trailing hyphens.
4. **Collision Resolution**: If a slug already exists, append `-2`, `-3` (e.g. `login.yaml` → `login-2.yaml`).
5. **Extension**: Always use `.yaml` for request and folder files.

---

## 3. Request File Schema (`RequestFile`)

Zest rejects unrecognized top-level request fields during deserialization. Use the documented fields for nested objects and tagged variants as well.

### 3.1 Field Order & Specification

Use plain YAML strings when possible. Quote values that start with `{{`, such as
`token: '{{auth_token}}'`, because YAML treats a leading `{` as the start of a
flow mapping. A value like `Bearer {{auth_token}}` can stay unquoted. Quote
numeric-looking values such as `'1'` when the field stores a string.

Use this field order to match Zest's YAML output. The loader accepts fields in any order.

1. `name` (string, required)
2. `method` (required string: `GET` | `POST` | `PUT` | `DELETE` | `PATCH` | `HEAD` | `OPTIONS`)
3. `url` (string, required; supports interpolation like `{{base_url}}/api/v1/resource`)
4. `protocol` (optional string: `http` | `websocket` | `grpc` | `graphql` | `sse`, defaults to `http`, which Zest omits when saving)
5. `grpc` (object, optional; **only** when `protocol: grpc`)
6. `graphql` (object, optional; **only** when `protocol: graphql`)
7. `ws_message_format` (string: `text` | `json`, optional; defaults to `text` and is **omitted** when `text`; used when `protocol: websocket`)
8. `params` (array of KeyValuePair, optional)
9. `headers` (array of KeyValuePair, optional)
10. `auth` (optional tagged enum object, defaults to `type: inherit`)
11. `body` (optional tagged enum object, defaults to `type: none`)
12. `settings` (object, optional)
13. `script` (object, optional)
14. `examples` (array of saved response examples, optional)

### 3.2 Protocols & Protocol-Specific Fields

- **HTTP (`http`, default):** Omit `protocol` to use the default HTTP transport.
- **WebSocket (`websocket`):**
  - Set `protocol: websocket`, `method: GET`, and a `ws://` or `wss://` URL.
  - Optional `ws_message_format`: `text` (default, omit if `text`) or `json`.
  ```yaml
  protocol: websocket
  ws_message_format: json
  ```
- **gRPC (`grpc`):**
  - Set `protocol: grpc`, `method: POST`, and a `grpc://` or `grpcs://` (or `http://`/`https://`) URL.
  - Include the `grpc` payload object (`#[serde(deny_unknown_fields)]`):
    - `proto_path` (optional string): Path to a `.proto` file. Omit when using Server Reflection.
    - `use_reflection` (boolean, default `true`): Fetch schema via gRPC Server Reflection.
    - `service` (string): Fully-qualified service name (e.g. `helloworld.Greeter`).
    - `method` (string): RPC method name within `service` (e.g. `SayHello`).
    - `message_json` (string): Request message as raw JSON string.
  ```yaml
  protocol: grpc
  grpc:
    use_reflection: true
    service: helloworld.Greeter
    method: SayHello
    message_json: '{"name": "Zest"}'
  ```
- **GraphQL (`graphql`):**
  - Set `protocol: graphql` and `method: POST`.
  - Include the `graphql` payload object (`#[serde(deny_unknown_fields)]`):
    - `query` (string): GraphQL query or mutation document.
    - `variables` (string, optional/default `""`): Variables as a raw JSON object string. Empty string means no variables.
    - `operation_name` (optional string): Operation name when the document defines multiple operations.
  ```yaml
  protocol: graphql
  graphql:
    query: |
      query GetCountry($code: ID!) {
        country(code: $code) {
          name
          capital
        }
      }
    variables: '{"code": "VN"}'
    operation_name: GetCountry
  ```
- **Server-Sent Events (`sse`):**
  - Set `protocol: sse` (uses HTTP transport; typically `method: GET` or `POST`, with optional `params`, `headers`, `auth`, `body`).
  ```yaml
  protocol: sse
  ```

### 3.3 KeyValuePair Object
Headers, query parameters, and url-encoded forms use this structure:
```yaml
name: Content-Type         # or 'key'
value: application/json
enabled: true              # optional, defaults to true
description: Optional notes # optional
```

### 3.4 Auth Types
Auth is a tagged object using `type`:
- **Inherit from parent folder/workspace**:
  ```yaml
  auth:
    type: inherit
  ```
- **No Auth**:
  ```yaml
  auth:
    type: none
  ```
- **Bearer Token**:
  ```yaml
  auth:
    type: bearer
    token: '{{auth_token}}'
  ```
- **Basic Auth**:
  ```yaml
  auth:
    type: basic
    username: '{{api_user}}'
    password: '{{api_password}}'
  ```
- **API Key**:
  ```yaml
  auth:
    type: apikey         # 'api_key' is also accepted
    key: X-API-Key
    value: '{{api_key}}'
    add_to: header       # "header" or "query" (default: "header")
  ```

### 3.5 Request Body Types
Body is a tagged object using `type`:
- **None**:
  ```yaml
  body:
    type: none
  ```
- **JSON**:
  ```yaml
  body:
    type: json
    content: '{"email": "{{user_email}}", "password": "{{user_password}}"}'
  ```
- **Form URL Encoded**:
  ```yaml
  body:
    type: form
    params:
      - name: grant_type
        value: password
        enabled: true
      - name: username
        value: '{{username}}'
        enabled: true
  ```
- **Multipart Form**:
  ```yaml
  body:
    type: multipart
    items:
      - name: description
        value: Profile photo
        enabled: true
        is_file: false
      - name: avatar
        value: /path/to/avatar.png
        enabled: true
        is_file: true
        file_path: /path/to/avatar.png
  ```
- **Raw**:
  ```yaml
  body:
    type: raw
    content: raw payload
    content_type: text/plain # default: "text/plain"
  ```
- **Binary**:
  ```yaml
  body:
    type: binary
    file_path: /path/to/archive.zip
  ```

### 3.6 Request Settings
```yaml
settings:
  timeout_ms: 30000        # default: 30000 (30 seconds)
  follow_redirects: true   # default: true
  validate_tls: true       # default: true
```

### 3.7 Script Config (Pre-Request & Post-Response)
Pre-request scripts run before transmission; post-response scripts run after response arrival. Evaluated in a QuickJS sandbox.
```yaml
script:
  pre_request: |
    // Generate timestamp or dynamic data
    z.environment.set("request_timestamp", Date.now().toString());
  post_response: |
    z.test("Response contains a token", function () {
      z.assert(z.response.status === 200, "Expected HTTP 200 OK");
      const body = z.response.json();
      z.assert(typeof body?.token === "string" && body.token.length > 0, "Expected a token");
      z.environment.set("auth_token", body.token);
    });
```

### 3.8 Saved Examples
Each saved response requires `name` and `status`. `headers` and `body` are optional. Store `body` as text, including for JSON responses.

```yaml
examples:
  - name: Successful Login
    status: 200
    headers:
      - name: Content-Type
        value: application/json
        enabled: true
    body: '{"status":"ok","token":"jwt-token-xyz"}'
```

---

## 4. Workspace Metadata File (`workspace.yaml`)

The root `workspace.yaml` defines workspace-level metadata, shared headers, and default auth (`#[serde(deny_unknown_fields)]`):

```yaml
version: 1
name: My API Workspace
headers:
  - name: X-Client
    value: zest
    enabled: true
auth:
  type: none
```

- `version`: Schema version (integer, default `1`).
- `name`: Workspace display name.
- `headers`: Optional headers inherited by all collections and requests.
- `auth`: Optional workspace auth. Omitting it loads `type: inherit`, which adds no authentication at the workspace level.

---

## 5. Folder Metadata File (`_folder.yaml`)

Every directory inside `collections/` can contain a `_folder.yaml` defining folder-level properties:

```yaml
name: Users & Accounts
order:
  - get-profile
  - update-profile
  - change-password
headers:
  - name: X-App-Version
    value: 2.0.0
    enabled: true
auth:
  type: inherit
```

- `name`: Display name of the folder in the sidebar.
- `order`: List of request file stems determining the order of requests in this folder.
- `headers`: Headers inherited by all child requests.
- `auth`: Shared auth (default `type: inherit`).

---

## 6. Environment Files (`environments/*.yaml`)

Variables can be interpolated in URLs, headers, params, and bodies using `{{variable_name}}`.

```yaml
# environments/development.yaml
variables:
  - name: base_url
    value: https://dev-api.example.com
    secret: false
    enabled: true
  - name: api_key
    secret: true           # Store the value in .local/secrets/development.yaml
    enabled: true
```

---

## 7. Complete End-to-End Examples

### Example 1: Standard GET Request with Query Params
File: `collections/users/list-users.yaml`

```yaml
name: List Users
method: GET
url: '{{base_url}}/api/v1/users'
params:
  - name: page
    value: '1'
    enabled: true
  - name: limit
    value: '20'
    enabled: true
  - name: search
    value: john
    enabled: false
headers:
  - name: Accept
    value: application/json
    enabled: true
auth:
  type: bearer
  token: '{{auth_token}}'
```

### Example 2: POST Request with JSON Body, Script & Assertions
File: `collections/auth/login.yaml`

```yaml
name: Login
method: POST
url: '{{base_url}}/api/v1/auth/login'
headers:
  - name: Content-Type
    value: application/json
    enabled: true
auth:
  type: none
body:
  type: json
  content: |
    {
      "email": "{{user_email}}",
      "password": "{{user_password}}"
    }
settings:
  timeout_ms: 15000
  follow_redirects: true
  validate_tls: true
script:
  post_response: |
    z.test("Login returns a token", function () {
      z.assert(z.response.status === 200, "Login must return 200 OK");
      const body = z.response.json();
      z.assert(typeof body?.token === "string" && body.token.length > 0, "Expected a token");
      z.environment.set("auth_token", body.token);
    });
examples:
  - name: Success Response
    status: 200
    headers:
      - name: Content-Type
        value: application/json
        enabled: true
    body: '{"token": "sample-jwt-token-value"}'
```

### Example 3: WebSocket Request
File: `collections/realtime/echo-socket.yaml`

```yaml
name: Echo Socket
method: GET
url: wss://echo.websocket.events
protocol: websocket
ws_message_format: json
headers:
  - name: Origin
    value: https://example.com
    enabled: true
auth:
  type: inherit
```

### Example 4: gRPC Request (Server Reflection)
File: `collections/grpc/say-hello.yaml`

```yaml
name: Say Hello
method: POST
url: grpc://localhost:50051
protocol: grpc
grpc:
  use_reflection: true
  service: helloworld.Greeter
  method: SayHello
  message_json: '{"name": "Zest"}'
headers:
  - name: x-request-id
    value: '{{$guid}}'
    enabled: true
auth:
  type: inherit
```

*(For a local `.proto` file, set `use_reflection: false` and give its absolute path in `proto_path`, for example `proto_path: /path/to/protos/helloworld.proto`.)*

### Example 5: GraphQL Query with Variables
File: `collections/graphql/get-country.yaml`

```yaml
name: Get Country
method: POST
url: https://countries.trevorblades.com/graphql
protocol: graphql
graphql:
  query: |
    query GetCountry($code: ID!) {
      country(code: $code) {
        name
        capital
        currency
      }
    }
  variables: '{"code": "VN"}'
  operation_name: GetCountry
auth:
  type: none
```

### Example 6: Server-Sent Events (SSE) Stream
File: `collections/streaming/chat-completions-stream.yaml`

```yaml
name: Chat Completions Stream
method: POST
url: '{{base_url}}/v1/chat/completions'
protocol: sse
headers:
  - name: Content-Type
    value: application/json
    enabled: true
  - name: Accept
    value: text/event-stream
    enabled: true
auth:
  type: bearer
  token: '{{api_key}}'
body:
  type: json
  content: |
    {
      "model": "gpt-4o-mini",
      "stream": true,
      "messages": [
        { "role": "user", "content": "Hello!" }
      ]
    }
```
