use crate::state::{RequestSessionState, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, Styled, Window};
use zest_core::models::GrpcPayload;

/// Inputs for the gRPC Proto tab.
pub struct GrpcProtoTabContext<'a> {
    pub workspace_state: &'a Entity<WorkspaceDataState>,
    pub editor_state: &'a Entity<RequestSessionState>,
    /// Invokes `Runner::fetch_grpc_schema` for the active request.
    pub on_fetch_schema: std::rc::Rc<dyn Fn(&mut App)>,
    pub proto_path_input: &'a Entity<InputState>,
}

/// Schema source tab for gRPC requests: reflection vs .proto file, plus the
/// Service/Method picker fed by a resolved schema.
pub fn render_grpc_proto_tab(
    payload: &GrpcPayload,
    ctx: GrpcProtoTabContext,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let _ = window;
    let t = theme();
    let ws = ctx.workspace_state;
    let ed = ctx.editor_state;
    let req_id = ed.read(cx).active_request_id().to_string();
    let schema = ed.read(cx).grpc_schema.clone();

    let ws_reflect = ws.clone();
    let ed_reflect = ed.clone();
    let req_reflect = req_id.clone();
    let on_fetch_schema = ctx.on_fetch_schema.clone();
    let ws_fetch = ws.clone();
    let ed_fetch = ed.clone();
    let req_fetch = req_id.clone();

    let use_reflection = payload.use_reflection;
    let selected_service = payload.service.clone();
    let selected_method = payload.method.clone();

    // Service / method lists from the resolved schema
    let services: Vec<String> = schema
        .as_ref()
        .map(|s| s.services.iter().map(|(name, _)| name.clone()).collect())
        .unwrap_or_default();
    let methods = schema.as_ref().and_then(|s| {
        s.services
            .iter()
            .find(|(name, _)| *name == selected_service)
            .map(|(_, methods)| methods.clone())
    });

    div()
        .size_full()
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(12.))
        .overflow_y_scrollbar()
        // Schema source toggle
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(t.text_secondary)
                        .child("Schema source"),
                )
                .child(
                    Button::new("btn-grpc-src-reflection")
                        .when(use_reflection, |b| b.primary())
                        .when(!use_reflection, |b| b.ghost())
                        .small()
                        .label("Reflection")
                        .on_click(move |_, _, cx| {
                            ws_reflect.update(cx, |ws, _| {
                                ws.update_active_request(&req_reflect, |r| {
                                    r.grpc_mut().use_reflection = true;
                                });
                            });
                            ed_reflect.update(cx, |ed, cx| ed.mark_dirty(cx));
                        }),
                )
                .child(
                    Button::new("btn-grpc-src-proto")
                        .when(!use_reflection, |b| b.primary())
                        .when(use_reflection, |b| b.ghost())
                        .small()
                        .label(".proto file")
                        .on_click(move |_, _, cx| {
                            ws_fetch.update(cx, |ws, _| {
                                ws.update_active_request(&req_fetch, |r| {
                                    r.grpc_mut().use_reflection = false;
                                });
                            });
                            ed_fetch.update(cx, |ed, cx| ed.mark_dirty(cx));
                        }),
                )
                .child(div().flex_1())
                .child(
                    Button::new("btn-grpc-fetch-schema")
                        .ghost()
                        .small()
                        .label("Fetch Schema")
                        .on_click(move |_, _, cx| {
                            (on_fetch_schema)(cx);
                        }),
                ),
        )
        // Proto file picker (only in .proto mode)
        .when(!use_reflection, |this| {
            let ws_path = ctx.workspace_state.clone();
            let ed_path = ctx.editor_state.clone();
            let req_path = req_id.clone();
            let input = ctx.proto_path_input.clone();
            this.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .h(px(28.))
                            .px(px(6.))
                            .rounded(px(6.))
                            .bg(t.bg_input)
                            .border_1()
                            .border_color(t.border)
                            .flex()
                            .items_center()
                            .child(Input::new(&input).appearance(false).bordered(false)),
                    )
                    .child(
                        Button::new("btn-grpc-browse-proto")
                            .ghost()
                            .small()
                            .label("Browse…")
                            .on_click(move |_, _, cx| {
                                let ws_c = ws_path.clone();
                                let ed_c = ed_path.clone();
                                let req_c = req_path.clone();
                                let _ = input;
                                cx.spawn(async move |cx| {
                                    if let Some(file) =
                                        rfd::AsyncFileDialog::new().pick_file().await
                                    {
                                        let path = file.path().to_string_lossy().to_string();
                                        cx.update(|cx| {
                                            ws_c.update(cx, |ws, _| {
                                                ws.update_active_request(&req_c, |r| {
                                                    r.grpc_mut().proto_path = Some(path);
                                                });
                                            });
                                            // Re-sync editors (incl. the path input)
                                            // and flag the request dirty.
                                            ed_c.update(cx, |ed, cx| {
                                                ed.sync_composer_inputs(
                                                    crate::state::ComposerSyncScope::All,
                                                    cx,
                                                );
                                                ed.mark_dirty(cx);
                                            });
                                        });
                                    }
                                })
                                .detach();
                            }),
                    ),
            )
        })
        // Service picker
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(t.text_secondary)
                        .child("Service"),
                )
                .child(render_service_dropdown(
                    ws.clone(),
                    ed.clone(),
                    req_id.clone(),
                    services,
                    selected_service.clone(),
                )),
        )
        // Method picker
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(t.text_secondary)
                        .child("Method"),
                )
                .child(render_method_dropdown(
                    ws.clone(),
                    ed.clone(),
                    req_id.clone(),
                    methods.unwrap_or_default(),
                    selected_method.clone(),
                )),
        )
        .when(schema.is_none(), |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(
                        "No schema loaded. Use Reflection (server must support it) or pick a .proto file, then Fetch Schema.",
                    ),
            )
        })
}

fn render_service_dropdown(
    ws: Entity<WorkspaceDataState>,
    ed: Entity<RequestSessionState>,
    req_id: String,
    services: Vec<String>,
    selected: String,
) -> impl IntoElement {
    let label = if selected.is_empty() {
        "Select service…".to_string()
    } else {
        selected.clone()
    };
    Button::new("btn-grpc-service-dropdown")
        .ghost()
        .small()
        .label(label)
        .dropdown_menu(move |menu, _window, _cx| {
            services.iter().fold(menu, |m, svc| {
                let ws_set = ws.clone();
                let ed_set = ed.clone();
                let req = req_id.clone();
                let svc_name = svc.clone();
                m.item(
                    PopupMenuItem::new(svc.clone())
                        .checked(*svc == selected)
                        .on_click(move |_, _, cx| {
                            ws_set.update(cx, |ws, _| {
                                ws.update_active_request(&req, |r| {
                                    r.grpc_mut().service = svc_name.clone();
                                });
                            });
                            ed_set.update(cx, |ed, cx| ed.mark_dirty(cx));
                        }),
                )
            })
        })
}

fn render_method_dropdown(
    ws: Entity<WorkspaceDataState>,
    ed: Entity<RequestSessionState>,
    req_id: String,
    methods: Vec<zest_core::grpc::GrpcMethodInfo>,
    selected: String,
) -> impl IntoElement {
    let label = if selected.is_empty() {
        "Select method…".to_string()
    } else {
        selected.clone()
    };
    Button::new("btn-grpc-method-dropdown")
        .ghost()
        .small()
        .label(label)
        .dropdown_menu(move |menu, _window, _cx| {
            methods.iter().fold(menu, |m, info| {
                let ws_set = ws.clone();
                let ed_set = ed.clone();
                let req = req_id.clone();
                let method_name = info.name.clone();
                let label = info.label();
                m.item(
                    PopupMenuItem::new(label)
                        .checked(info.name == selected)
                        .on_click(move |_, _, cx| {
                            ws_set.update(cx, |ws, _| {
                                ws.update_active_request(&req, |r| {
                                    r.grpc_mut().method = method_name.clone();
                                });
                            });
                            ed_set.update(cx, |ed, cx| ed.mark_dirty(cx));
                        }),
                )
            })
        })
}
