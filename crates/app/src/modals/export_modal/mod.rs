use crate::modals::Modal;
use crate::services::openapi_export::{build_openapi_export_for, openapi_export_stats_for};
use crate::state::{CollectionState, ModalKind, ModalState, WorkspaceState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement,
    RenderOnce, Styled, Window,
};
use zest_core::exporters::OpenApiFileFormat;

/// Per-open UI state of the export modal (output format choice).
pub struct ExportModalState {
    pub format: OpenApiFileFormat,
}

impl ExportModalState {
    pub fn new() -> Self {
        Self {
            format: OpenApiFileFormat::Yaml,
        }
    }

    pub fn set_format(&mut self, format: OpenApiFileFormat, cx: &mut Context<Self>) {
        self.format = format;
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct ExportModal {
    workspace_state: Entity<WorkspaceState>,
    collection_state: Entity<CollectionState>,
    modal_state: Entity<ModalState>,
}

impl ExportModal {
    pub fn new(
        workspace_state: Entity<WorkspaceState>,
        collection_state: Entity<CollectionState>,
        modal_state: Entity<ModalState>,
    ) -> Self {
        Self {
            workspace_state,
            collection_state,
            modal_state,
        }
    }
}

impl RenderOnce for ExportModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let ws_entity = self.workspace_state.clone();
        let col_entity = self.collection_state.clone();
        let modal_entity_ref = self.modal_state.clone();

        let modal_entity = window
            .use_keyed_state(
                gpui_kit::SharedString::from("export-modal-state-key"),
                cx,
                |_window, cx| cx.new(|_cx| ExportModalState::new()),
            )
            .read(cx)
            .clone();

        let format = modal_entity.read(cx).format;
        let export_modal_ui = &self.modal_state.read(cx).export_modal;
        let target_folder_id = export_modal_ui.target_folder_id.clone();
        let target_folder_name = export_modal_ui
            .target_folder_name
            .clone()
            .unwrap_or_else(|| "Folder".to_string());
        let (exportable, folders, skipped) =
            openapi_export_stats_for(col_entity.read(cx), target_folder_id.as_deref());
        let (source_label, source_name) = match &target_folder_id {
            Some(_) => ("Folder", target_folder_name.as_str()),
            None => ("Workspace", ws_entity.read(cx).workspace_name()),
        };
        let default_file_name = format!(
            "{}.openapi.{}",
            zest_core::storage::slug::slugify(source_name),
            format.extension()
        );

        Modal::for_kind("export-modal", ModalKind::Export, &self.modal_state)
            .w(px(560.))
            .h(px(320.))
            .title("Export OpenAPI 3.0")
            .icon(IconName::Upload)
            // Content Body
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    // Source summary row
                    .child(
                        div()
                            .w_full()
                            .p(px(10.))
                            .rounded(px(6.))
                            .bg(t.bg_app)
                            .border_1()
                            .border_color(t.border)
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.text_primary)
                                    .child(format!("{}: {}", source_label, source_name)),
                            )
                            .child(div().text_xs().text_color(t.text_secondary).child(format!(
                                "{} requests across {} folders will be exported.",
                                exportable, folders
                            )))
                            .when(skipped > 0, |this| {
                                this.child(div().text_xs().text_color(t.text_muted).child(format!(
                                    "{} non-HTTP request(s) (WebSocket/gRPC) will be skipped.",
                                    skipped
                                )))
                            }),
                    )
                    // Format selection row (mirrors the import format row)
                    .child(
                        div()
                            .w_full()
                            .p(px(10.))
                            .rounded(px(6.))
                            .bg(t.bg_app)
                            .border_1()
                            .border_color(t.border)
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(16.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(t.text_primary)
                                            .child("Output Format"),
                                    )
                                    .child(
                                        div().text_xs().text_color(t.text_muted).child(
                                            "Serialization of the exported OpenAPI document.",
                                        ),
                                    ),
                            )
                            .child({
                                let format_label = match format {
                                    OpenApiFileFormat::Yaml => "YAML",
                                    OpenApiFileFormat::Json => "JSON",
                                };
                                let st_formats = modal_entity.clone();

                                div().flex_shrink_0().child(
                                    Button::new("btn-export-format-dropdown")
                                        .ghost()
                                        .compact()
                                        .px(px(10.))
                                        .py(px(4.))
                                        .h(px(28.))
                                        .rounded(px(5.))
                                        .bg(t.bg_input)
                                        .border_1()
                                        .border_color(t.border)
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(
                                            Icon::new(IconName::FileText)
                                                .size(px(13.))
                                                .text_color(t.text_secondary),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(t.text_primary)
                                                .child(format_label),
                                        )
                                        .child(
                                            Icon::new(IconName::ChevronDown)
                                                .size(px(12.))
                                                .text_color(t.text_muted),
                                        )
                                        .dropdown_menu(move |menu, _window, _cx| {
                                            let st_yaml = st_formats.clone();
                                            let st_json = st_formats.clone();
                                            menu.item(
                                                PopupMenuItem::new("YAML (.yaml)")
                                                    .checked(format == OpenApiFileFormat::Yaml)
                                                    .on_click(move |_, _, cx| {
                                                        st_yaml.update(cx, |s, cx| {
                                                            s.set_format(
                                                                OpenApiFileFormat::Yaml,
                                                                cx,
                                                            );
                                                        });
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("JSON (.json)")
                                                    .checked(format == OpenApiFileFormat::Json)
                                                    .on_click(move |_, _, cx| {
                                                        st_json.update(cx, |s, cx| {
                                                            s.set_format(
                                                                OpenApiFileFormat::Json,
                                                                cx,
                                                            );
                                                        });
                                                    }),
                                            )
                                        }),
                                )
                            }),
                    ),
            )
            // Footer Actions
            .footer(
                DialogFooter::new()
                    .h(px(52.))
                    .px(px(16.))
                    .border_t_1()
                    .border_color(t.border)
                    .bg(t.bg_panel)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.text_muted)
                            .child(if exportable == 0 {
                                "No HTTP requests to export in this workspace".to_string()
                            } else {
                                format!("Ready to export {} requests", exportable)
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Button::new("btn-cancel-export")
                                    .ghost()
                                    .small()
                                    .label("Cancel")
                                    .on_click({
                                        let modal_c = modal_entity_ref.clone();
                                        move |_, _, cx| {
                                            modal_c
                                                .update(cx, |m, cx| m.close(ModalKind::Export, cx));
                                        }
                                    }),
                            )
                            .child({
                                let is_valid = exportable > 0;
                                let file_name = default_file_name;
                                let ws_exec = ws_entity.clone();
                                let col_exec = col_entity.clone();
                                let modal_exec = modal_entity_ref.clone();
                                let st_exec = modal_entity.clone();
                                let scope_exec = target_folder_id.clone();

                                Button::new("btn-execute-export")
                                    .primary()
                                    .small()
                                    .disabled(!is_valid)
                                    .label("Export...")
                                    .on_click(move |_, _, cx: &mut App| {
                                        if !is_valid {
                                            return;
                                        }
                                        let format = st_exec.read(cx).format;
                                        let file_name = file_name.clone();
                                        let ws_exec = ws_exec.clone();
                                        let col_exec = col_exec.clone();
                                        let scope = scope_exec.clone();
                                        let modal_exec = modal_exec.clone();

                                        cx.spawn(async move |cx| {
                                            if let Some(file) = rfd::AsyncFileDialog::new()
                                                .set_file_name(&file_name)
                                                .add_filter("OpenAPI 3.0", &["yaml", "json"])
                                                .save_file()
                                                .await
                                            {
                                                let path = file.path().to_path_buf();
                                                let (ws_name, ws_desc) =
                                                    ws_exec.read_with(cx, |ws, _| {
                                                        (
                                                            ws.workspace_name().to_string(),
                                                            ws.workspace_description()
                                                                .map(str::to_string),
                                                        )
                                                    });
                                                let result = col_exec.update(cx, |col, _cx| {
                                                    let export = build_openapi_export_for(
                                                        col,
                                                        scope.as_deref(),
                                                        &ws_name,
                                                        ws_desc.as_deref(),
                                                    );
                                                    let warnings = export.warnings.clone();
                                                    zest_core::exporters::serialize_openapi(
                                                        &export, format,
                                                    )
                                                    .map(|content| (content, warnings))
                                                });
                                                match result {
                                                    Ok((content, warnings)) => {
                                                        let write_path = path.clone();
                                                        let write_res = cx
                                                            .background_executor()
                                                            .spawn(async move {
                                                                std::fs::write(write_path, content)
                                                            })
                                                            .await;
                                                        match write_res {
                                                            Ok(()) => {
                                                                modal_exec.update(cx, |m, cx| {
                                                                    m.close(ModalKind::Export, cx);
                                                                });
                                                                cx.update(|cx| {
                                                                    crate::notify::success(
                                                                        "Export Complete",
                                                                        format!(
                                                                            "Saved {}",
                                                                            path.display()
                                                                        ),
                                                                        cx,
                                                                    );
                                                                    if !warnings.is_empty() {
                                                                        crate::notify::info(
                                                                            "Export Warnings",
                                                                            warnings.join(" · "),
                                                                            cx,
                                                                        );
                                                                    }
                                                                });
                                                            }
                                                            Err(e) => {
                                                                cx.update(|cx| {
                                                                    crate::notify::error(
                                                                        "Export Failed",
                                                                        e.to_string(),
                                                                        cx,
                                                                    );
                                                                });
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        cx.update(|cx| {
                                                            crate::notify::error(
                                                                "Export Failed",
                                                                e.to_string(),
                                                                cx,
                                                            );
                                                        });
                                                    }
                                                }
                                            }
                                        })
                                        .detach();
                                    })
                            }),
                    ),
            )
    }
}
