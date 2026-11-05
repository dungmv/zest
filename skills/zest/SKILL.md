---
name: request-authoring
description: >-
  Use this skill when generating, creating, or editing Zest API requests, folder metadata,
  collections, environment variables, and workspace directory structures.
---

# Zest Request Authoring & Workspace Directory Guide

This skill provides the exact specifications, YAML schemas, folder conventions, and scripting patterns needed for any AI agent to generate valid, conflict-free Zest requests and workspaces.

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

Zest uses strict Rust deserialization with `#[serde(deny_unknown_fields)]`. **Any unrecognized field will fail to load.**

### 3.1 Field Order & Specification

Order your YAML fields as follows:
1. `name` (string, required)
2. `method` (string: `GET` | `POST` | `PUT` | `DELETE` | `PATCH` | `HEAD` | `OPTIONS`, default `GET`)
3. `url` (string, required; supports interpolation like `{{base_url}}/api/v1/resource`)
4. `params` (array of KeyValuePair, optional)
5. `headers` (array of KeyValuePair, optional)
6. `auth` (tagged enum object, optional)
7. `body` (tagged enum object, optional)
8. `settings` (object, optional)
9. `script` (object, optional)
10. `examples` (**MUST be last** if present, array of example objects)

### 3.2 KeyValuePair Object
Headers, query parameters, and url-encoded forms use this structure:
```yaml
name: "Content-Type"       # or 'key'
value: "application/json"
enabled: true              # optional, defaults to true
description: "Optional notes" # optional
```

### 3.3 Auth Types
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
    token: "{{auth_token}}"
  ```
- **Basic Auth**:
  ```yaml
  auth:
    type: basic
    username: "{{api_user}}"
    password: "{{api_password}}"
  ```
- **API Key**:
  ```yaml
  auth:
    type: api_key
    key: "X-API-Key"
    value: "{{api_key}}"
    add_to: header       # "header" or "query" (default: "header")
  ```

### 3.4 Request Body Types
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
        value: "{{username}}"
        enabled: true
  ```
- **Multipart Form**:
  ```yaml
  body:
    type: multipart
    items:
      - name: description
        value: "Profile photo"
        enabled: true
        is_file: false
      - name: avatar
        value: "avatar.png"
        enabled: true
        is_file: true
        file_path: "/path/to/avatar.png"
  ```
- **Raw**:
  ```yaml
  body:
    type: raw
    content: "raw payload"
    content_type: "text/plain" # default: "text/plain"
  ```
- **Binary**:
  ```yaml
  body:
    type: binary
    file_path: "/path/to/archive.zip"
  ```

### 3.5 Request Settings
```yaml
settings:
  timeout_ms: 30000        # default: 30000 (30 seconds)
  follow_redirects: true   # default: true
  validate_tls: true       # default: true
```

### 3.6 Script Config (Pre-Request & Post-Response)
Pre-request scripts run before transmission; post-response scripts run after response arrival. Evaluated in a QuickJS sandbox.
```yaml
script:
  pre_request: |
    // Generate timestamp or dynamic data
    z.setEnv("request_timestamp", Date.now().toString());
  post_response: |
    z.assert(res.status === 200, "Expected HTTP 200 OK");
    if (res.status === 200) {
      z.setEnv("token", res.body.token);
    }
```

### 3.7 Saved Examples
Examples must be placed at the bottom of the request file:
```yaml
examples:
  - name: "Successful Login"
    status: 200
    headers:
      - name: Content-Type
        value: application/json
        enabled: true
    body: '{"status":"ok","token":"jwt-token-xyz"}'
```

---

## 4. Folder Metadata File (`_folder.yaml`)

Every directory inside `collections/` can contain a `_folder.yaml` defining folder-level properties:

```yaml
name: "Users & Accounts"
order:
  - get-profile
  - update-profile
  - change-password
headers:
  - name: "X-App-Version"
    value: "2.0.0"
    enabled: true
auth:
  type: inherit
```

- `name`: Display name of the folder in the sidebar.
- `order`: List of request/subfolder file stems determining display order.
- `headers`: Headers inherited by all child requests.
- `auth`: Shared auth (default `type: inherit`).

---

## 5. Environment Files (`environments/*.yaml`)

Variables can be interpolated in URLs, headers, params, and bodies using `{{variable_name}}`.

```yaml
# environments/development.yaml
variables:
  - name: base_url
    value: "https://dev-api.example.com"
    secret: false
    enabled: true
  - name: api_key
    value: null
    secret: true           # Private value goes into .local/secrets/development.yaml
    enabled: true
```

---

## 6. Complete End-to-End Examples

### Example 1: Standard GET Request with Query Params
File: `collections/users/list-users.yaml`

```yaml
name: List Users
method: GET
url: "{{base_url}}/api/v1/users"
params:
  - name: page
    value: "1"
    enabled: true
  - name: limit
    value: "20"
    enabled: true
  - name: search
    value: "john"
    enabled: false
headers:
  - name: Accept
    value: application/json
    enabled: true
auth:
  type: bearer
  token: "{{auth_token}}"
```

### Example 2: POST Request with JSON Body, Script & Assertions
File: `collections/auth/login.yaml`

```yaml
name: Login
method: POST
url: "{{base_url}}/api/v1/auth/login"
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
    z.assert(res.status === 200, "Login must return 200 OK");
    if (res.body && res.body.token) {
      z.setEnv("auth_token", res.body.token);
    }
examples:
  - name: "Success Response"
    status: 200
    headers:
      - name: Content-Type
        value: application/json
        enabled: true
    body: '{"token": "sample-jwt-token-value"}'
```
