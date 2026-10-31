use crate::components::ModalDialog;
use crate::state::{ModalState, ToastState, WorkspaceDataState};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Disableable as _;
use gpui_kit::component::Icon;
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Context, Entity, FontWeight, InteractiveElement as _,
    IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};
use std::path::PathBuf;
use zest_core::importers::{parse_content, ImportFormat, ImportOptions, ImportResult};
use zest_core::models::HttpMethod;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSourceTab {
    File,
    Paste,
}

pub struct ImportModalState {
    pub format: ImportFormat,
    pub source_tab: ImportSourceTab,
    pub selected_file: Option<PathBuf>,
    pub loaded_content: Option<String>,
    pub create_enclosing_folder: bool,
    pub import_environments: bool,
    pub parse_result: Option<Result<ImportResult, String>>,
    pub paste_input: Entity<InputState>,
}

impl ImportModalState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let paste_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(
                "Paste OpenAPI (JSON/YAML) or Postman Collection (JSON) content here...",
            )
        });

        Self {
            format: ImportFormat::OpenApi3,
            source_tab: ImportSourceTab::File,
            selected_file: None,
            loaded_content: None,
            create_enclosing_folder: true,
            import_environments: true,
            parse_result: None,
            paste_input,
        }
    }

    pub fn set_format(&mut self, format: ImportFormat, cx: &mut Context<Self>) {
        self.format = format;
        self.reparse(cx);
    }

    pub fn set_source_tab(&mut self, tab: ImportSourceTab, cx: &mut Context<Self>) {
        self.source_tab = tab;
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
        let content_to_parse = match self.source_tab {
            ImportSourceTab::File => self.loaded_content.clone(),
            ImportSourceTab::Paste => {
                let text = self.paste_input.read(cx).value().to_string();
                if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                }
            }
        };

        if let Some(content) = content_to_parse {
            match parse_content(&content, self.format) {
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
                |window, cx| cx.new(|cx| ImportModalState::new(window, cx)),
            )
            .read(cx)
            .clone();

        let (
            format,
            source_tab,
            selected_file,
            create_enclosing,
            import_env,
            parse_result,
            paste_inp,
        ) = {
            let s = modal_entity.read(cx);
            (
                s.format,
                s.source_tab,
                s.selected_file.clone(),
                s.create_enclosing_folder,
                s.import_environments,
                s.parse_result.clone(),
                s.paste_input.clone(),
            )
        };

        let target_folder_rel = modal_entity_ref.read(cx).import_target_folder_rel.clone();

        ModalDialog::new("import-modal")
            .w(px(700.))
            .max_h(px(600.))
            .title("Import into Workspace")
            .description("OpenAPI 3.0/3.1, Postman v2.0/v2.1, or cURL")
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
                            // Source Tab Bar
                            .child(
                                div()
                                    .flex()
                                    .border_b_1()
                                    .border_color(t.border)
                                    .gap(px(16.))
                                    .child(
                                        render_tab_header(
                                            "btn-tab-file",
                                            "From File",
                                            source_tab == ImportSourceTab::File,
                                            {
                                                let st = modal_entity.clone();
                                                move |_, _, cx| {
                                                    st.update(cx, |s, cx| s.set_source_tab(ImportSourceTab::File, cx));
                                                }
                                            },
                                            t,
                                        ),
                                    )
                                    .child(
                                        render_tab_header(
                                            "btn-tab-paste",
                                            "Paste Content",
                                            source_tab == ImportSourceTab::Paste,
                                            {
                                                let st = modal_entity.clone();
                                                move |_, _, cx| {
                                                    st.update(cx, |s, cx| s.set_source_tab(ImportSourceTab::Paste, cx));
                                                }
                                            },
                                            t,
                                        ),
                                    ),
                            )
                            // Source Input Views
                            .child(
                                match source_tab {
                                    ImportSourceTab::File => render_file_picker_view(
                                        selected_file.as_ref(),
                                        modal_entity.clone(),
                                        t,
                                    ).into_any_element(),
                                    ImportSourceTab::Paste => render_paste_view(
                                        paste_inp.clone(),
                                        modal_entity.clone(),
                                        t,
                                    ).into_any_element(),
                                }
                            )
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
                                            t,
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
                                            t,
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

fn render_tab_header<F>(
    id: &'static str,
    label: &'static str,
    active: bool,
    on_click: F,
    t: &crate::theme::AppTheme,
) -> impl IntoElement
where
    F: Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static,
{
    div()
        .id(id)
        .pb(px(6.))
        .cursor_pointer()
        .text_xs()
        .font_weight(if active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(if active { t.text_primary } else { t.text_muted })
        .when(active, |s| s.border_b_2().border_color(t.primary))
        .child(label)
        .on_click(on_click)
}

fn render_file_picker_view(
    selected_file: Option<&PathBuf>,
    modal_state: Entity<ImportModalState>,
    t: &crate::theme::AppTheme,
) -> impl IntoElement {
    let st_pick = modal_state.clone();

    div().flex().flex_col().gap(px(8.)).child(
        div()
            .p(px(16.))
            .rounded(px(8.))
            .border_1()
            .border_color(t.border)
            .bg(t.bg_panel)
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text_primary)
                            .child(match selected_file {
                                Some(f) => f
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("Selected File")
                                    .to_string(),
                                None => "No file selected".to_string(),
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.text_muted)
                            .child(match selected_file {
                                Some(f) => f.to_string_lossy().to_string(),
                                None => "Supports .json, .yaml, .yml files".to_string(),
                            }),
                    ),
            )
            .child(
                div()
                    .id("btn-browse-import-file")
                    .px(px(12.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.bg_card)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.bg_hover).border_color(t.primary))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text_primary)
                    .child("Browse File...")
                    .on_click(move |_, _, cx| {
                        let st_pick = st_pick.clone();
                        cx.spawn(async move |cx| {
                            if let Some(file) = rfd::AsyncFileDialog::new()
                                .add_filter(
                                    "API Specification & Collections",
                                    &["json", "yaml", "yml"],
                                )
                                .pick_file()
                                .await
                            {
                                let path = file.path().to_path_buf();
                                let read_res = cx
                                    .background_executor()
                                    .spawn(async move {
                                        std::fs::read_to_string(&path).map(|c| (path, c))
                                    })
                                    .await;
                                st_pick.update(cx, |s, cx| match read_res {
                                    Ok((path, content)) => s.set_loaded_file(path, content, cx),
                                    Err(e) => s.set_file_read_error(e.to_string(), cx),
                                });
                            }
                        })
                        .detach();
                    }),
            ),
    )
}

fn render_paste_view(
    paste_input: Entity<InputState>,
    modal_state: Entity<ImportModalState>,
    t: &crate::theme::AppTheme,
) -> impl IntoElement {
    let st_parse = modal_state.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .h(px(140.))
                .rounded(px(6.))
                .border_1()
                .border_color(t.border)
                .bg(t.bg_panel)
                .overflow_hidden()
                .child(Input::new(&paste_input)),
        )
        .child(
            div().flex().justify_end().child(
                div()
                    .id("btn-analyze-paste-content")
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.bg_card)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.bg_hover).border_color(t.primary))
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text_primary)
                    .child("Analyze & Preview")
                    .on_click(move |_, _, cx| {
                        st_parse.update(cx, |s, cx| {
                            s.reparse(cx);
                        });
                    }),
            ),
        )
}

fn render_preview_card(res: &ImportResult, t: &crate::theme::AppTheme) -> impl IntoElement {
    div()
        .p(px(12.))
        .rounded(px(8.))
        .bg(t.bg_panel)
        .border_1()
        .border_color(t.border)
        .flex()
        .flex_col()
        .gap(px(10.))
        // Summary row
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(t.text_primary)
                                .child(res.title.clone()),
                        )
                        .when_some(res.description.as_ref(), |this, desc| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child(format!("— {}", desc)),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(render_badge(
                            &format!("{} requests", res.requests.len()),
                            t.primary,
                        ))
                        .child(render_badge(
                            &format!("{} folders", res.folders.len()),
                            t.text_secondary,
                        ))
                        .when(!res.environments.is_empty(), |this| {
                            this.child(render_badge(
                                &format!("{} env", res.environments.len()),
                                t.status_3xx,
                            ))
                        }),
                ),
        )
        // Requests preview list (scrollable)
        .child(
            div()
                .max_h(px(140.))
                .overflow_y_scrollbar()
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(res.requests.iter().take(20).map(|r| {
                    div()
                        .px(px(8.))
                        .py(px(4.))
                        .rounded(px(4.))
                        .bg(t.bg_card)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .overflow_hidden()
                                .child(render_method_pill(r.method, t))
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(t.text_primary)
                                        .child(r.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_muted)
                                        .truncate()
                                        .child(r.url.clone()),
                                ),
                        )
                        .when(!r.folder_path.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(t.text_muted)
                                    .child(r.folder_path.join(" / ")),
                            )
                        })
                }))
                .when(res.requests.len() > 20, |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(t.text_muted)
                            .py(px(4.))
                            .child(format!("... and {} more requests", res.requests.len() - 20)),
                    )
                }),
        )
}

fn render_badge(label: &str, color: gpui_kit::Hsla) -> impl IntoElement {
    div()
        .px(px(6.))
        .py(px(2.))
        .rounded(px(4.))
        .bg(color.opacity(0.12))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(label.to_string())
}

fn render_method_pill(method: HttpMethod, t: &crate::theme::AppTheme) -> impl IntoElement {
    let color = match method {
        HttpMethod::Get => t.method_get,
        HttpMethod::Post => t.method_post,
        HttpMethod::Put => t.method_put,
        HttpMethod::Delete => t.method_delete,
        HttpMethod::Patch => t.method_patch,
        HttpMethod::Head => t.method_head,
        HttpMethod::Options => t.method_options,
    };

    div()
        .w(px(44.))
        .flex()
        .items_center()
        .justify_center()
        .py(px(1.))
        .text_xs()
        .font_weight(FontWeight::NORMAL)
        .text_color(color)
        .child(method.as_str())
}

fn render_checkbox<F>(
    id: &'static str,
    label: &'static str,
    checked: bool,
    on_click: F,
    _t: &crate::theme::AppTheme,
) -> impl IntoElement
where
    F: Fn(&bool, &mut Window, &mut App) + 'static,
{
    Checkbox::new(id)
        .label(label)
        .checked(checked)
        .on_click(on_click)
}
