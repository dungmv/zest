use crate::app::AppView;
use crate::components::*;
use crate::state::*;
use crate::theme::theme;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{px, Context, Styled};
use zest_core::models::Protocol;

pub(super) fn render_tab_strip(
    av: &AppView,
    active_tab_kind: RequestTabKind,
    cx: &mut Context<AppView>,
) -> TabBar {
    let t = theme();
    let req_id_str = av.state.session.read(cx).active_request_id().to_string();
    let ws_read = av.state.workspace.read(cx);
    let active_req = ws_read.find_request(&req_id_str);
    let protocol = active_req.map(|r| r.protocol).unwrap_or(Protocol::Http);
    let tabs = RequestTabKind::tabs_for(protocol);
    let active_idx = tabs.iter().position(|k| *k == active_tab_kind).unwrap_or(0);
    let active_req_info = active_req.map(|r| {
        (
            r.params.len(),
            r.headers.len(),
            r.script
                .as_ref()
                .and_then(|s| s.pre_request.as_ref())
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            r.script
                .as_ref()
                .and_then(|s| s.post_response.as_ref())
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
        )
    });
    let param_count = active_req_info.as_ref().map(|info| info.0).unwrap_or(0);
    let header_count = active_req_info.as_ref().map(|info| info.1).unwrap_or(0);
    let has_pre = active_req_info.as_ref().map(|info| info.2).unwrap_or(false);
    let has_test = active_req_info.as_ref().map(|info| info.3).unwrap_or(false);
    let ed_c = av.state.session.clone();

    let mut bar = TabBar::new("request-composer-tabs")
        .underline()
        .px(px(8.))
        .selected_index(active_idx)
        .on_click(move |&idx, _, cx| {
            let tabs = RequestTabKind::tabs_for(protocol);
            let kind = tabs.get(idx).copied().unwrap_or(tabs[0]);
            ed_c.update(cx, |state, cx| {
                state.set_active_tab_kind(kind, cx);
            });
        });
    for kind in tabs {
        let mut tab = Tab::new().label(kind.label());
        tab = match kind {
            RequestTabKind::Params => tab.when(param_count > 0, |tab| {
                tab.suffix(render_tab_badge(
                    format!("{param_count}"),
                    active_tab_kind == RequestTabKind::Params,
                    t,
                ))
            }),
            RequestTabKind::Headers => tab.when(header_count > 0, |tab| {
                tab.suffix(render_tab_badge(
                    format!("{header_count}"),
                    active_tab_kind == RequestTabKind::Headers,
                    t,
                ))
            }),
            RequestTabKind::PreRequestScript => tab.when(has_pre, |tab| {
                tab.suffix(render_tab_badge(
                    "*",
                    active_tab_kind == RequestTabKind::PreRequestScript,
                    t,
                ))
            }),
            RequestTabKind::Tests => tab.when(has_test, |tab| {
                tab.suffix(render_tab_badge(
                    "*",
                    active_tab_kind == RequestTabKind::Tests,
                    t,
                ))
            }),
            _ => tab,
        };
        bar = bar.child(tab);
    }
    bar
}
