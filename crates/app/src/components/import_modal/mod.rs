mod preview;

use crate::components::ModalDialog;
use crate::state::{ModalState, ToastState, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Context, Entity, FontWeight, IntoElement, ParentElement,
    RenderOnce, Styled, Window,
};
use preview::{render_checkbox, render_file_picker_view, render_preview_card};
use std::path::PathBuf;
use zest_core::importers::{parse_content, ImportFormat, ImportOptions, ImportResult};

pub struct ImportModalState {
    pub format: ImportFormat,
    pub selected_file: Option<PathBuf>,
    pub loaded_content: Option<String>,
    pub create_enclosing_folder: bool,
    pub import_environments: bool,
    pub parse_result: Option<Result<ImportResult, String>>,
}

impl ImportModalState {
    pub fn new() -> Self {
        Self {
            format: ImportFormat::OpenApi3,
            selected_file: None,
            loaded_content: None,
            create_enclosing_folder: true,
            import_environments: true,
            parse_result: None,
        }
    }

    pub fn set_format(&mut self, format: ImportFormat, cx: &mut Context<Self>) {
        self.format = format;
        self.reparse(cx);
    }

    pub fn set_loaded_file(&mut self, path: PathBuf, content: String, cx: &mut Context<Self>) {
        self.selected_file = Some(path);
        self.loaded_content = Some(content);
        self.reparse(cx);
    }

    pub fn set_file_read_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.parse_result = Some(Err(format!("Could not read file: {}", error)));
        cx.notify();
    }

    pub fn reparse(&mut self, cx: &mut Context<Self>) {
        if let Some(content) = &self.loaded_content {
            match parse_content(content, self.format) {
                Ok(res) => {
                    self.parse_result = Some(Ok(res));
                }
                Err(e) => {
                    self.parse_result = Some(Err(e.to_string()));
                }
            }
        } else {
            self.parse_result = None;
        }
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct ImportModal {
    workspace_state: Entity<WorkspaceDataState>,
    modal_state: Entity<ModalState>,
    toast_state: Entity<ToastState>,
}

impl ImportModal {
    pub fn new(
        workspace_state: Entity<WorkspaceDataState>,
        modal_state: Entity<ModalState>,
        toast_state: Entity<ToastState>,
    ) -> Self {
        Self {
            workspace_state,
            modal_state,
            toast_state,
        }
    }
}

impl RenderOnce for ImportModal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let ws_entity = self.workspace_state.clone();
        let modal_entity_ref = self.modal_state.clone();
        let toast_entity = self.toast_state.clone();

        let modal_entity = window
            .use_keyed_state(
                gpui_kit::SharedString::from("import-modal-state-key"),
                cx,
                |_window, cx| cx.new(|_cx| ImportModalState::new()),
            )
            .read(cx)
            .clone();

        let (format, selected_file, create_enclosing, import_env, parse_result) = {
            let s = modal_entity.read(cx);
            (
                s.format,
                s.selected_file.clone(),
                s.create_enclosing_folder,
                s.import_environments,
                s.parse_result.clone(),
            )
        };

        let target_folder_rel = modal_entity_ref
            .read(cx)
            .import_modal
            .target_folder_rel
            .clone();

        ModalDialog::new("import-modal")
            .w(px(640.))
            .h(px(520.))
            .title("Import into Workspace")
            .icon(IconName::Download)
            .on_close({
                let modal_c = modal_entity_ref.clone();
                move |_, cx| {
                    modal_c.update(cx, |m, cx| m.close_import_modal(cx));
                }
            })
            // Content Body (Scrollable)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scrollbar()
                    .p(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    // Format Selection Row (Left: Text, Right: Dropdown)
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
                                            .child("Specification Format"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(t.text_muted)
                                            .child("Choose the specification or collection format to parse."),
                                    ),
                            )
                            .child({
                                let format_label = match format {
                                    ImportFormat::OpenApi3 => "OpenAPI 3.1 / 3.0",
                                    ImportFormat::PostmanCollection => "Postman Collection v2.1",
                                };
                                let st = modal_entity.clone();

                                div().flex_shrink_0().child(
                                    Button::new("btn-import-format-dropdown")
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
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(t.text_primary)
                                                .child(format_label),
                                        )
                                        .child(
                                            Icon::new(IconName::ChevronDown)
                                                .size(px(12.))
                                                .text_color(t.text_muted),
                                        )
                                        .dropdown_menu(move |menu, _window, _cx| {
                                            let st_openapi = st.clone();
                                            let st_postman = st.clone();
                                            menu.item(
                                                PopupMenuItem::new("OpenAPI 3.1 / 3.0 (YAML/JSON)")
                                                    .checked(format == ImportFormat::OpenApi3)
                                                    .on_click(move |_, _, cx| {
                                                        st_openapi.update(cx, |s, cx| {
                                                            s.set_format(ImportFormat::OpenApi3, cx);
                                                        });
                                                    }),
                                            )
                                            .item(
                                                PopupMenuItem::new("Postman Collection v2.1 (JSON)")
                                                    .checked(format == ImportFormat::PostmanCollection)
                                                    .on_click(move |_, _, cx| {
                                                        st_postman.update(cx, |s, cx| {
                                                            s.set_format(ImportFormat::PostmanCollection, cx);
                                                        });
                                                    }),
                                            )
                                        }),
                                )
                            }),
                    )
                    // File Picker
                    .child(render_file_picker_view(
                        selected_file.as_ref(),
                        modal_entity.clone(),
                        t,
                    ))
                            // Parse & Preview Section
                            .when_some(parse_result.clone(), |this, res| {
                                match res {
                                    Ok(spec) => this.child(render_preview_card(&spec, t)),
                                    Err(err) => this.child(
                                        Alert::error(
                                            "import-parse-error",
                                            format!("Parsing Error: {}", err),
                                        )
                                        .with_size(gpui_kit::component::Size::Small),
                                    ),
                                }
                            })
                            // Options Section (Always Current Workspace)
                            .child(
                                div()
                                    .p(px(10.))
                                    .rounded(px(6.))
                                    .bg(t.bg_panel)
                                    .border_1()
                                    .border_color(t.border)
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(t.text_secondary)
                                            .child(format!(
                                                "Target Location: {}",
                                                target_folder_rel.as_deref().unwrap_or("Root Workspace Collection")
                                            )),
                                    )
                                    .child(
                                        render_checkbox(
                                            "chk-create-enclosing",
                                            "Create enclosing folder with Collection / Spec title",
                                            create_enclosing,
                                            {
                                                let st = modal_entity.clone();
                                                move |_, _, cx| {
                                                    st.update(cx, |s, cx| {
                                                        s.create_enclosing_folder = !s.create_enclosing_folder;
                                                        cx.notify();
                                                    });
                                                }
                                            },
                                        ),
                                    )
                                    .child(
                                        render_checkbox(
                                            "chk-import-env-current",
                                            "Import environment variables if present",
                                            import_env,
                                            {
                                                let st = modal_entity.clone();
                                                move |_, _, cx| {
                                                    st.update(cx, |s, cx| {
                                                        s.import_environments = !s.import_environments;
                                                        cx.notify();
                                                    });
                                                }
                                            },
                                        ),
                                    ),
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
                            .child(match &parse_result {
                                Some(Ok(res)) => format!("Ready to import {} requests", res.requests.len()),
                                Some(Err(_)) => "Please resolve parse error".to_string(),
                                None => "Select a file or paste content to preview".to_string(),
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                Button::new("btn-cancel-import")
                                    .ghost()
                                    .small()
                                    .label("Cancel")
                                    .on_click({
                                        let modal_c = modal_entity_ref.clone();
                                        move |_, _, cx| {
                                            modal_c.update(cx, |m, cx| m.close_import_modal(cx));
                                        }
                                    }),
                            )
                            .child({
                                let is_valid = matches!(&parse_result, Some(Ok(r)) if !r.requests.is_empty());
                                let ws_exec = ws_entity.clone();
                                let modal_exec = modal_entity_ref.clone();
                                let toast_exec = toast_entity.clone();
                                let st_exec = modal_entity.clone();

                                Button::new("btn-execute-import")
                                    .primary()
                                    .small()
                                    .disabled(!is_valid)
                                    .label("Import Now")
                                    .on_click(move |_, _, cx: &mut App| {
                                        if !is_valid {
                                            return;
                                        }
                                        let state = st_exec.read(cx);
                                        let Some(Ok(result)) = state.parse_result.clone() else {
                                            return;
                                        };

                                        let options = ImportOptions {
                                            target_folder_rel: target_folder_rel.clone(),
                                            create_enclosing_folder: state.create_enclosing_folder,
                                            enclosing_folder_name: Some(result.title.clone()),
                                            import_environments: state.import_environments,
                                        };

                                        let res = ws_exec.update(cx, |ws, cx| {
                                            ws.execute_import_current_workspace(result, options, cx)
                                        });

                                        match res {
                                            Ok(summary) => {
                                                modal_exec.update(cx, |m, cx| {
                                                    m.close_import_modal(cx);
                                                });
                                                toast_exec.update(cx, |ts, cx| {
                                                    ts.push_success(
                                                        "Import Complete",
                                                        format!(
                                                            "Imported {} requests and {} folders into workspace",
                                                            summary.total_requests, summary.total_folders
                                                        ),
                                                        cx,
                                                    );
                                                });
                                            }
                                            Err(e) => {
                                                toast_exec.update(cx, |ts, cx| {
                                                    ts.push_error("Import Failed", e.to_string(), cx);
                                                });
                                            }
                                        }
                                    })
                            }),
                    ),
            )
    }
}
