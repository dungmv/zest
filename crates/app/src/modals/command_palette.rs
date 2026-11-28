use crate::components::status_badge::MethodBadge;
use crate::state::{
    CollectionState, EnvironmentState, ModalKind, ModalState, PersistenceState,
    RequestSessionState, SidebarState,
};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::separator::Separator;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, AppContext as _, Entity, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, StatefulInteractiveElement as _, Styled, Window,
};

#[derive(IntoElement)]
pub struct CommandPalette {
    workspace_state: Entity<CollectionState>,
    env_state: Entity<EnvironmentState>,
    persistence_state: Entity<PersistenceState>,
    modal_state: Entity<ModalState>,
    sidebar_state: Entity<SidebarState>,
    editor_state: Entity<RequestSessionState>,
}

impl CommandPalette {
    pub fn new(
        workspace_state: Entity<CollectionState>,
        env_state: Entity<EnvironmentState>,
        persistence_state: Entity<PersistenceState>,
        modal_state: Entity<ModalState>,
        sidebar_state: Entity<SidebarState>,
        editor_state: Entity<RequestSessionState>,
    ) -> Self {
        Self {
            workspace_state,
            env_state,
            persistence_state,
            modal_state,
            sidebar_state,
            editor_state,
        }
    }
}

/// One filtered request row: (rank, id, name, full_path, folder_path, method).
type PaletteHit = (
    usize,
    String,
    String,
    String,
    String,
    zest_core::models::HttpMethod,
);

/// Cached palette filter results, keyed by (query, tree_version,
/// url_version, recent_len). The expensive folder-path resolution is
/// memoized inside each rebuild instead of re-walking the folder chain for
/// every request on every keystroke.
#[derive(Default)]
struct PaletteCache {
    key: Option<(String, usize, usize, usize)>,
    requests: Vec<PaletteHit>,
}

fn rebuild_palette_cache(
    cache: &mut PaletteCache,
    ws: &CollectionState,
    per: &PersistenceState,
    q: &str,
) {
    let by_id: std::collections::HashMap<&str, &zest_core::models::Folder> =
        ws.folders().iter().map(|f| (f.id.as_str(), f)).collect();

    // Memoized folder-id -> display path; the depth cap also guards against
    // corrupted parent chains forming cycles.
    fn resolve_path(
        fid: &str,
        by_id: &std::collections::HashMap<&str, &zest_core::models::Folder>,
        memo: &mut std::collections::HashMap<String, String>,
        depth: usize,
    ) -> String {
        if depth > 32 {
            return String::new();
        }
        if let Some(path) = memo.get(fid) {
            return path.clone();
        }
        let path = match by_id.get(fid) {
            Some(f) => match &f.parent_id {
                Some(pid) => format!("{} / {}", resolve_path(pid, by_id, memo, depth + 1), f.name),
                None => f.name.clone(),
            },
            None => String::new(),
        };
        memo.insert(fid.to_string(), path.clone());
        path
    }

    let mut folder_paths: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for f in ws.folders() {
        resolve_path(&f.id, &by_id, &mut folder_paths, 0);
    }

    let recent_len = per.local_state.recent.len();
    let recent_index: std::collections::HashMap<&str, usize> = per
        .local_state
        .recent
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    cache.requests = ws
        .requests()
        .iter()
        .filter_map(|r| {
            let folder_path = r
                .folder_id
                .as_ref()
                .and_then(|fid| folder_paths.get(fid))
                .cloned()
                .unwrap_or_default();
            let req_name = crate::components::common::request_label::effective_request_name(r);
            let full_path = if folder_path.is_empty() {
                req_name.clone()
            } else {
                format!("{} / {}", folder_path, req_name)
            };

            let matches = q.is_empty()
                || full_path.to_lowercase().contains(q)
                || r.url.to_lowercase().contains(q);
            if !matches {
                return None;
            }

            let rank = recent_index
                .get(r.id.as_str())
                .copied()
                .unwrap_or(recent_len + 100);
            Some((
                rank,
                r.id.clone(),
                req_name,
                full_path,
                folder_path,
                r.method,
            ))
        })
        .collect();
    cache.requests.sort_by_key(|(rank, _, _, _, _, _)| *rank);
    cache.requests.truncate(50);
}

impl RenderOnce for CommandPalette {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();

        let env_clone = self.env_state.clone();
        let modal_clone = self.modal_state.clone();
        let sb_clone = self.sidebar_state.clone();
        let ed_clone = self.editor_state.clone();

        let cmd_holder = window.use_keyed_state(
            gpui_kit::SharedString::from("cmd-palette-search-holder"),
            cx,
            |window, cx| {
                let inp = cx.new(|cx| {
                    InputState::new(window, cx).placeholder("Search requests or environments...")
                });
                let sub = cx.subscribe_in(&inp, window, {
                    let modal_s = modal_clone.clone();
                    move |_, _, event, _, cx| {
                        if let InputEvent::Change = event {
                            // Scope the redraw through the modal state
                            // (observed by AppView) instead of refreshing
                            // every window on each keystroke.
                            modal_s.update(cx, |_, cx| cx.notify());
                        }
                    }
                });
                (inp, sub, std::cell::RefCell::new(PaletteCache::default()))
            },
        );

        let search_input = cmd_holder.read(cx).0.clone();
        search_input.update(cx, |s, cx| s.focus(window, cx));
        let q = search_input.read(cx).value().to_lowercase();

        let matched_requests: Vec<PaletteHit> = {
            let ws = self.workspace_state.read(cx);
            let per = self.persistence_state.read(cx);
            let cache = &cmd_holder.read(cx).2;
            let mut cache = cache.borrow_mut();
            let key = (
                q.clone(),
                ws.tree_version,
                ws.url_version,
                per.local_state.recent.len(),
            );
            if cache.key.as_ref() != Some(&key) {
                rebuild_palette_cache(&mut cache, ws, per, &q);
                cache.key = Some(key);
            }
            cache.requests.clone()
        };

        let filtered_envs: Vec<(String, String)> = {
            let env = self.env_state.read(cx);
            env.environments
                .iter()
                .filter(|e| q.is_empty() || e.name.to_lowercase().contains(&q))
                .map(|e| (e.id.clone(), e.name.clone()))
                .collect()
        };

        let has_matched_requests = !matched_requests.is_empty();
        let has_filtered_envs = !filtered_envs.is_empty();
        let is_empty = !has_matched_requests && !has_filtered_envs;

        div()
            .id("cmd-palette-overlay")
            .absolute()
            .inset_0()
            .bg(gpui_kit::Hsla { a: 0.6, ..t.bg_app })
            .flex()
            .items_start()
            .justify_center()
            .pt(px(100.))
            .on_click({
                let modal_s = modal_clone.clone();
                move |_, _, cx| {
                    modal_s.update(cx, |state, cx| {
                        state.close(ModalKind::CommandPalette, cx);
                    });
                }
            })
            .on_key_down({
                let modal_s = modal_clone.clone();
                move |event, _, cx| {
                    let is_escape = event.keystroke.key.as_str() == "escape";
                    let is_ctrl_or_cmd_k = (event.keystroke.modifiers.control
                        || event.keystroke.modifiers.platform)
                        && event.keystroke.key.eq_ignore_ascii_case("k");
                    if is_escape || is_ctrl_or_cmd_k {
                        modal_s.update(cx, |state, cx| {
                            state.close(ModalKind::CommandPalette, cx);
                        });
                        cx.stop_propagation();
                    }
                }
            })
            .child(
                div()
                    .id("cmd-palette-dialog")
                    .occlude()
                    .w(px(560.))
                    .max_h(px(400.))
                    .rounded(px(8.))
                    .bg(t.bg_card)
                    .border_1()
                    .border_color(t.border)
                    .shadow_2xl()
                    .flex()
                    .flex_col()
                    .on_key_down({
                        let modal_s = modal_clone.clone();
                        move |event, _, cx| {
                            let is_escape = event.keystroke.key.as_str() == "escape";
                            let is_ctrl_or_cmd_k = (event.keystroke.modifiers.control
                                || event.keystroke.modifiers.platform)
                                && event.keystroke.key.eq_ignore_ascii_case("k");
                            if is_escape || is_ctrl_or_cmd_k {
                                modal_s.update(cx, |state, cx| {
                                    state.close(ModalKind::CommandPalette, cx);
                                });
                                cx.stop_propagation();
                            }
                        }
                    })
                    // Search bar
                    .child(
                        div()
                            .px(px(12.))
                            .py(px(8.))
                            .border_b_1()
                            .border_color(t.border)
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(Icon::new(IconName::Search).size(px(14.)))
                            .child(
                                div()
                                    .flex_1()
                                    .on_key_down({
                                        let modal_s = modal_clone.clone();
                                        move |event, _, cx| {
                                            let is_escape = event.keystroke.key.as_str() == "escape";
                                            let is_ctrl_or_cmd_k = (event
                                                .keystroke
                                                .modifiers
                                                .control
                                                || event.keystroke.modifiers.platform)
                                                && event.keystroke.key.eq_ignore_ascii_case("k");
                                            if is_escape || is_ctrl_or_cmd_k {
                                                modal_s.update(cx, |state, cx| {
                                                    state.close(ModalKind::CommandPalette, cx);
                                                });
                                                cx.stop_propagation();
                                            }
                                        }
                                    })
                                    .child(
                                        Input::new(&search_input)
                                            .id("cmd-palette-input")
                                            .appearance(false)
                                            .bordered(false)
                                            .focus_bordered(false)
                                            .cleanable(true)
                                            .px(px(0.))
                                            .text_sm(),
                                    ),
                            )
                            .child({
                                let modal_s = modal_clone.clone();
                                div()
                                    .id("close-cmd-pal")
                                    .cursor_pointer()
                                    .px(px(4.))
                                    .py(px(1.))
                                    .rounded(px(3.))
                                    .bg(t.bg_input)
                                    .border_1()
                                    .border_color(t.border)
                                    .text_color(t.text_muted)
                                    .font_family(".SystemMonoFont")
                                    .text_size(px(10.))
                                    .on_click(move |_, _, cx| {
                                        modal_s.update(cx, |state, cx| {
                                            state.close(ModalKind::CommandPalette, cx);
                                        });
                                    })
                                    .child("Esc")
                            }),
                    )
                    // Items List: Switch Request & Switch Environment
                    .child(
                        div()
                            .flex_1()
                            .overflow_y_scrollbar()
                            .p(px(6.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .when(
                                is_empty,
                                |this| {
                                    this.child(
                                        div()
                                            .px(px(12.))
                                            .py(px(16.))
                                            .text_center()
                                            .text_xs()
                                            .text_color(t.text_muted)
                                            .child("No matching requests or environments found."),
                                    )
                                },
                            )
                            // List Requests
                            .when(has_matched_requests, |this| {
                                this.child(
                                    div()
                                        .px(px(8.))
                                        .pt(px(2.))
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(t.text_muted)
                                        .child("SWITCH REQUEST"),
                                )
                                .children(matched_requests.into_iter().map(
                                    |(_, req_id, req_name, full_path, folder_path, req_method)| {
                                        let modal_s = modal_clone.clone();
                                        let sb_s = sb_clone.clone();
                                        let ed_s = ed_clone.clone();
                                        let r_id = req_id.clone();

                                        div()
                                            .id(format!("cmd-req-{}", req_id))
                                            .px(px(8.))
                                            .py(px(6.))
                                            .rounded(px(4.))
                                            .hover(|s| s.bg(t.bg_hover))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .gap(px(8.))
                                            .on_click(move |_, _, cx| {
                                                sb_s.update(cx, |state, _cx| {
                                                    state.record_navigation(&r_id);
                                                });
                                                modal_s.update(cx, |state, cx| {
                                                    state.close(ModalKind::CommandPalette, cx);
                                                });
                                                ed_s.update(cx, |state, cx| {
                                                    state.set_active_request_id(&r_id, cx);
                                                });
                                            })
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .min_w(px(0.))
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .child(MethodBadge::new(req_method))
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_family(".SystemMonoFont")
                                                            .truncate()
                                                            .child(if folder_path.is_empty() {
                                                                div()
                                                                    .text_color(t.text_primary)
                                                                    .child(full_path)
                                                                    .into_any_element()
                                                            } else {
                                                                div()
                                                                    .flex()
                                                                    .items_center()
                                                                    .gap(px(4.))
                                                                    .child(
                                                                        div()
                                                                            .text_color(
                                                                                t.text_muted,
                                                                            )
                                                                            .child(format!(
                                                                                "{} /",
                                                                                folder_path
                                                                            )),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_color(
                                                                                t.text_primary,
                                                                            )
                                                                            .font_weight(
                                                                                gpui_kit::FontWeight::MEDIUM,
                                                                            )
                                                                            .child(req_name),
                                                                    )
                                                                    .into_any_element()
                                                            }),
                                                    ),
                                            )
                                    },
                                ))
                            })
                            // Switch Environment
                            .when(has_filtered_envs, |this| {
                                this.when(has_matched_requests, |this| {
                                    this.child(Separator::horizontal().my(px(4.)))
                                })
                                .child(
                                    div()
                                        .px(px(8.))
                                        .pt(px(2.))
                                        .text_xs()
                                        .font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(t.text_muted)
                                        .child("SWITCH ENVIRONMENT"),
                                )
                                .children(filtered_envs.into_iter().map(|(env_id, env_name)| {
                                    let env_s = env_clone.clone();
                                    let modal_s = modal_clone.clone();

                                    div()
                                        .id(format!("cmd-env-{}", env_id))
                                        .px(px(8.))
                                        .py(px(6.))
                                        .rounded(px(4.))
                                        .hover(|s| s.bg(t.bg_hover))
                                        .cursor_pointer()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .on_click(move |_, _, cx| {
                                            env_s.update(cx, |state, cx| {
                                                state.set_active_environment(
                                                    Some(env_id.clone()),
                                                    cx,
                                                );
                                            });
                                            modal_s.update(cx, |state, cx| {
                                                state.close(ModalKind::CommandPalette, cx);
                                            });
                                        })
                                        .child("●")
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(t.text_primary)
                                                .child(format!("Switch to: {}", env_name)),
                                        )
                                }))
                            }),
                    ),
            )
    }
}
