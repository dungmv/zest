use super::*;

#[test]
fn test_openapi_circular_ref_safety() {
    let json_spec = r##"{
            "openapi": "3.0.0",
            "info": { "title": "Circular API", "version": "1.0.0" },
            "paths": {
                "/test": {
                    "get": {
                        "responses": {
                            "200": {
                                "description": "OK",
                                "content": {
                                    "application/json": {
                                        "schema": { "$ref": "#/components/schemas/NodeA" }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "components": {
                "schemas": {
                    "NodeA": {
                        "type": "object",
                        "properties": {
                            "next": { "$ref": "#/components/schemas/NodeB" }
                        }
                    },
                    "NodeB": {
                        "type": "object",
                        "properties": {
                            "next": { "$ref": "#/components/schemas/NodeA" }
                        }
                    }
                }
            }
        }"##;

    let res = parse_openapi(json_spec).unwrap();
    assert_eq!(res.requests.len(), 1);
}

#[test]
fn test_openapi_3_multipart_form_data_with_binary_file() {
    let yaml_spec = r#"
openapi: 3.0.0
info:
  title: Upload API
  version: 1.0.0
paths:
  /upload:
    post:
      summary: Upload user document
      requestBody:
        required: true
        content:
          multipart/form-data; charset=utf-8:
            schema:
              type: object
              required:
                - document
              properties:
                document:
                  type: string
                  format: binary
                  description: The file to upload
                caption:
                  type: string
                  default: "My document"
                  description: Document caption
      responses:
        '200':
          description: OK
"#;
    let res = parse_openapi(yaml_spec).unwrap();
    assert_eq!(res.requests.len(), 1);
    let req = &res.requests[0];
    assert_eq!(req.name, "Upload user document");

    match &req.body {
        crate::models::RequestBody::Multipart { items } => {
            assert_eq!(items.len(), 2);
            let doc_item = items.iter().find(|i| i.key == "document").unwrap();
            assert!(
                doc_item.is_file,
                "Document field should be recognized as a file"
            );

            let caption_item = items.iter().find(|i| i.key == "caption").unwrap();
            assert!(
                !caption_item.is_file,
                "Caption field should be recognized as text"
            );
            assert_eq!(caption_item.value, "My document");
        }
        other => panic!("Expected RequestBody::Multipart, got {:?}", other),
    }

    assert!(req
        .headers
        .iter()
        .any(|h| h.key.eq_ignore_ascii_case("content-type")
            && h.value.contains("multipart/form-data")));
}

#[test]
fn test_openapi_3_url_encoded_with_all_of() {
    let json_spec = r##"{
            "openapi": "3.0.0",
            "info": { "title": "Auth API", "version": "1.0.0" },
            "paths": {
                "/login": {
                    "post": {
                        "summary": "User Login",
                        "requestBody": {
                            "content": {
                                "application/x-www-form-urlencoded": {
                                    "schema": {
                                        "allOf": [
                                            { "$ref": "#/components/schemas/Credentials" },
                                            {
                                                "type": "object",
                                                "properties": {
                                                    "remember_me": { "type": "boolean", "default": true }
                                                }
                                            }
                                        ]
                                    }
                                }
                            }
                        },
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            },
            "components": {
                "schemas": {
                    "Credentials": {
                        "type": "object",
                        "required": ["username", "password"],
                        "properties": {
                            "username": { "type": "string", "example": "alice" },
                            "password": { "type": "string", "example": "secret123" }
                        }
                    }
                }
            }
        }"##;

    let res = parse_openapi(json_spec).unwrap();
    assert_eq!(res.requests.len(), 1);
    let req = &res.requests[0];

    match &req.body {
        crate::models::RequestBody::FormUrlEncoded { params } => {
            assert_eq!(params.len(), 3);
            let user_param = params.iter().find(|p| p.key == "username").unwrap();
            assert_eq!(user_param.value, "alice");

            let pass_param = params.iter().find(|p| p.key == "password").unwrap();
            assert_eq!(pass_param.value, "secret123");

            let rem_param = params.iter().find(|p| p.key == "remember_me").unwrap();
            assert_eq!(rem_param.value, "true");
        }
        other => panic!("Expected RequestBody::FormUrlEncoded, got {:?}", other),
    }
}

#[test]
fn test_swagger_2_form_data_multipart_and_urlencoded() {
    let json_spec = r#"{
            "swagger": "2.0",
            "info": { "title": "Legacy API", "version": "1.0.0" },
            "host": "api.example.com",
            "paths": {
                "/upload": {
                    "post": {
                        "summary": "Upload avatar",
                        "consumes": ["multipart/form-data"],
                        "parameters": [
                            {
                                "name": "avatar",
                                "in": "formData",
                                "type": "file",
                                "required": true,
                                "description": "Profile image"
                            },
                            {
                                "name": "user_id",
                                "in": "formData",
                                "type": "integer",
                                "default": 42
                            }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                },
                "/token": {
                    "post": {
                        "summary": "Obtain Token",
                        "consumes": ["application/x-www-form-urlencoded"],
                        "parameters": [
                            {
                                "name": "grant_type",
                                "in": "formData",
                                "type": "string",
                                "default": "password"
                            },
                            {
                                "name": "scope",
                                "in": "formData",
                                "type": "string",
                                "default": "read write"
                            }
                        ],
                        "responses": { "200": { "description": "OK" } }
                    }
                }
            }
        }"#;

    let res = parse_openapi(json_spec).unwrap();
    assert_eq!(res.requests.len(), 2);

    let upload_req = res
        .requests
        .iter()
        .find(|r| r.name == "Upload avatar")
        .unwrap();
    match &upload_req.body {
        crate::models::RequestBody::Multipart { items } => {
            assert_eq!(items.len(), 2);
            let avatar = items.iter().find(|i| i.key == "avatar").unwrap();
            assert!(avatar.is_file);
            let uid = items.iter().find(|i| i.key == "user_id").unwrap();
            assert!(!uid.is_file);
            assert_eq!(uid.value, "42");
        }
        other => panic!("Expected Multipart for upload, got {:?}", other),
    }

    let token_req = res
        .requests
        .iter()
        .find(|r| r.name == "Obtain Token")
        .unwrap();
    match &token_req.body {
        crate::models::RequestBody::FormUrlEncoded { params } => {
            assert_eq!(params.len(), 2);
            let gt = params.iter().find(|p| p.key == "grant_type").unwrap();
            assert_eq!(gt.value, "password");
            let sc = params.iter().find(|p| p.key == "scope").unwrap();
            assert_eq!(sc.value, "read write");
        }
        other => panic!("Expected FormUrlEncoded for token, got {:?}", other),
    }
}

#[test]
fn test_openapi_3_form_fallback_to_example() {
    let yaml_spec = r#"
openapi: 3.0.0
info:
  title: Example Form API
  version: 1.0.0
paths:
  /contact:
    post:
      summary: Send contact message
      requestBody:
        content:
          application/x-www-form-urlencoded:
            example:
              email: "support@zest.app"
              subject: "Bug Report"
      responses:
        '200':
          description: OK
"#;
    let res = parse_openapi(yaml_spec).unwrap();
    assert_eq!(res.requests.len(), 1);
    let req = &res.requests[0];

    match &req.body {
        crate::models::RequestBody::FormUrlEncoded { params } => {
            assert_eq!(params.len(), 2);
            let email_param = params.iter().find(|p| p.key == "email").unwrap();
            assert_eq!(email_param.value, "support@zest.app");
            let subj_param = params.iter().find(|p| p.key == "subject").unwrap();
            assert_eq!(subj_param.value, "Bug Report");
        }
        other => panic!("Expected FormUrlEncoded from example, got {:?}", other),
    }
}

#[test]
fn test_openapi_3_yaak_style_form_array_example() {
    let json_spec = r##"{
        "openapi": "3.0.0",
        "info": { "title": "Login API", "version": "1.0.0" },
        "paths": {
            "/login": {
                "post": {
                    "summary": "login",
                    "parameters": [
                        {
                            "name": "Content-Type",
                            "in": "header",
                            "required": false,
                            "schema": { "type": "string" },
                            "example": "application/x-www-form-urlencoded"
                        }
                    ],
                    "requestBody": {
                        "required": false,
                        "content": {
                            "application/x-www-form-urlencoded": {
                                "schema": { "type": "string" },
                                "example": {
                                    "form": [
                                        {
                                            "enabled": true,
                                            "name": "id",
                                            "value": "FB::310087480809153"
                                        },
                                        {
                                            "enabled": true,
                                            "name": "",
                                            "value": ""
                                        }
                                    ]
                                }
                            }
                        }
                    },
                    "responses": { "200": { "description": "OK" } }
                }
            }
        }
    }"##;

    let res = parse_openapi(json_spec).unwrap();
    assert_eq!(res.requests.len(), 1);
    let req = &res.requests[0];

    match &req.body {
        crate::models::RequestBody::FormUrlEncoded { params } => {
            assert_eq!(params.len(), 1, "Empty name row should be ignored");
            let id_param = &params[0];
            assert_eq!(id_param.key, "id");
            assert_eq!(id_param.value, "FB::310087480809153");
            assert!(id_param.enabled);
        }
        other => panic!("Expected FormUrlEncoded, got {:?}", other),
    }
}
