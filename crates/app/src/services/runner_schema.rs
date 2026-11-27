use super::Runner;
use gpui_kit::App;
use std::sync::Arc;
use zest_core::models::Protocol;

impl Runner {
    /// Resolves the gRPC schema (reflection or .proto) for the picker UI.
    pub fn fetch_grpc_schema(&self, cx: &mut App) {
        let req_id = self.session_state.read(cx).active_request_id().to_string();
        let Some(req) = self
            .collection_state
            .read(cx)
            .find_request(&req_id)
            .cloned()
        else {
            return;
        };
        if req.protocol != Protocol::Grpc {
            return;
        }
        let Some(payload) = req.grpc.clone() else {
            return;
        };

        let session_state = self.session_state.clone();
        let workspace_state = self.workspace_state.clone();
        let origin = workspace_state.read(cx).store.clone();

        let rx = self.engines.grpc.fetch_schema(
            req.url.clone(),
            payload.proto_path.clone(),
            payload.use_reflection,
        );
        cx.spawn(async move |cx| {
            let result = rx
                .await
                .unwrap_or_else(|_| Err("Schema fetch task dropped".to_string()));
            cx.update(|cx| {
                if Arc::ptr_eq(&origin, &workspace_state.read(cx).store)
                    && session_state.read(cx).active_request_id() == req_id
                {
                    match result {
                        Ok(summary) => {
                            session_state.update(cx, |ed, cx| {
                                ed.set_grpc_schema(Some(std::sync::Arc::new(summary)), cx);
                            });
                        }
                        Err(err) => {
                            crate::notify::error("Schema Fetch Failed", err, cx);
                        }
                    }
                }
            });
        })
        .detach();
    }

    /// Fetches the GraphQL schema via an introspection query for the
    /// autocomplete provider. Uses the request's URL/headers/auth; runs on
    /// the tokio runtime and lands the parsed summary on the session.
    pub fn fetch_graphql_schema(&self, cx: &mut App) {
        let req_id = self.session_state.read(cx).active_request_id().to_string();
        let Some(req) = self
            .collection_state
            .read(cx)
            .find_request(&req_id)
            .cloned()
        else {
            return;
        };
        if req.protocol != Protocol::GraphQL {
            return;
        }
        let env = Some(self.env_state.read(cx).effective_environment());

        let session_state = self.session_state.clone();
        let workspace_state = self.workspace_state.clone();
        let origin = workspace_state.read(cx).store.clone();

        let rx = self.engines.http.fetch_graphql_schema(req, env);
        cx.spawn(async move |cx| {
            let result = rx
                .await
                .unwrap_or_else(|_| Err("Schema fetch task dropped".to_string()));
            cx.update(|cx| {
                if Arc::ptr_eq(&origin, &workspace_state.read(cx).store)
                    && session_state.read(cx).active_request_id() == req_id
                {
                    match result {
                        Ok(summary) => {
                            session_state.update(cx, |ed, cx| {
                                ed.set_graphql_schema(Some(summary), cx);
                            });
                        }
                        Err(err) => {
                            crate::notify::error("Schema Fetch Failed", err, cx);
                        }
                    }
                }
            });
        })
        .detach();
    }
}
