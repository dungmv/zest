use zest_core::models::Protocol;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestTabKind {
    #[default]
    Body,
    Message,
    /// GraphQL query + variables dual-pane editor.
    Query,
    Params,
    Headers,
    Auth,
    Proto,
    PreRequestScript,
    Tests,
}

impl RequestTabKind {
    /// Tab strip for a request of the given protocol:
    /// - HTTP/SSE: Body, Params, Headers, Auth, scripts
    /// - WebSocket: Message (outgoing frame), Params, Headers, Auth, pre-request
    /// - gRPC: Message (JSON payload), Proto (schema source), Headers, Auth, pre-request
    /// - GraphQL: Query (query + variables), Params, Headers, Auth, scripts
    pub fn tabs_for(protocol: Protocol) -> &'static [RequestTabKind] {
        match protocol {
            Protocol::Http | Protocol::Sse => &[
                RequestTabKind::Body,
                RequestTabKind::Params,
                RequestTabKind::Headers,
                RequestTabKind::Auth,
                RequestTabKind::PreRequestScript,
                RequestTabKind::Tests,
            ],
            Protocol::WebSocket => &[
                RequestTabKind::Message,
                RequestTabKind::Params,
                RequestTabKind::Headers,
                RequestTabKind::Auth,
                RequestTabKind::PreRequestScript,
            ],
            Protocol::Grpc => &[
                RequestTabKind::Message,
                RequestTabKind::Proto,
                RequestTabKind::Headers,
                RequestTabKind::Auth,
                RequestTabKind::PreRequestScript,
            ],
            Protocol::GraphQL => &[
                RequestTabKind::Query,
                RequestTabKind::Params,
                RequestTabKind::Headers,
                RequestTabKind::Auth,
                RequestTabKind::PreRequestScript,
                RequestTabKind::Tests,
            ],
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RequestTabKind::Body => "Body",
            RequestTabKind::Message => "Message",
            RequestTabKind::Query => "Query",
            RequestTabKind::Params => "Params",
            RequestTabKind::Headers => "Headers",
            RequestTabKind::Auth => "Auth",
            RequestTabKind::Proto => "Proto",
            RequestTabKind::PreRequestScript => "Pre-request",
            RequestTabKind::Tests => "Post-response",
        }
    }

    /// Returns `kind` if it is valid for `protocol`, else the first valid tab.
    pub fn clamp_for(kind: RequestTabKind, protocol: Protocol) -> RequestTabKind {
        let tabs = Self::tabs_for(protocol);
        if tabs.contains(&kind) {
            kind
        } else {
            tabs[0]
        }
    }
}
