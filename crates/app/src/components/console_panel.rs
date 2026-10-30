use crate::state::ResponseState;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyMediaVariant, EmptyTitle,
};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::Icon;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, App, Entity, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StatefulInteractiveElement as _, Styled, Window,
};
use zest_core::models::{ConsoleLogEntry, ConsoleLogLevel, ConsoleLogSource, TestResult};

#[derive(IntoElement)]
pub struct ConsolePanel {
    response_state: Entity<ResponseState>,
}

impl ConsolePanel {
    pub fn new(response_state: Entity<ResponseState>) -> Self {
        Self { response_state }
    }
}

impl RenderOnce for ConsolePanel {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let resp_s = self.response_state.read(cx);
        let all_tests = resp_s.test_results.clone();
        let all_logs = resp_s.console_logs.clone();
        let total_items = all_tests.len() + all_logs.len();

        div().size_full().flex().flex_col().bg(t.bg_response).child(
            div().flex_1().overflow_y_scrollbar().p(px(12.)).child({
                if total_items == 0 {
                    render_empty_state()
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .children(all_tests.into_iter().map(render_test_card))
                        .children(
                            all_logs
                                .into_iter()
                                .enumerate()
                                .map(|(idx, entry)| render_log_card(idx, entry)),
                        )
                        .into_any_element()
                }
            }),
        )
    }
}

fn render_empty_state() -> gpui_kit::AnyElement {
    let t = theme();
    Empty::new()
        .header(
            EmptyHeader::new()
                .media(
                    EmptyMedia::new()
                        .with_variant(EmptyMediaVariant::Icon)
                        .child(Icon::new(IconName::Terminal).size(px(24.))),
                )
                .title(EmptyTitle::new().child("No Tests or Console Output"))
                .description(EmptyDescription::new().child(
                    "Test assertions from z.test(...) and logs from console.log() will appear here.",
                )),
        )
        .content(
            EmptyContent::new().child(
                div()
                    .w_full()
                    .p(px(12.))
                    .rounded(px(6.))
                    .bg(t.bg_card)
                    .border_1()
                    .border_color(t.border)
                    .font_family(".SystemMonoFont")
                    .text_xs()
                    .text_color(t.text_muted)
                    .text_left()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child("// In Pre-request or Post-response Script:")
                    .child("console.log(\"Response status:\", z.response.status);")
                    .child("z.test(\"Status is 200\", () => {")
                    .child("    z.expect(z.response.status).toBe(200);")
                    .child("});"),
            ),
        )
        .into_any_element()
}

fn render_test_card(res: TestResult) -> gpui_kit::Div {
    let t = theme();
    let status_color = if res.passed {
        t.method_post
    } else {
        t.method_delete
    };

    div()
        .p(px(8.))
        .rounded(px(6.))
        .bg(if res.passed {
            t.bg_card
        } else {
            gpui_kit::Hsla {
                a: 0.08,
                ..status_color
            }
        })
        .border_1()
        .border_color(if res.passed {
            t.border
        } else {
            gpui_kit::Hsla {
                a: 0.35,
                ..status_color
            }
        })
        .flex()
        .flex_col()
        .gap(px(4.))
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
                                .px(px(6.))
                                .py(px(1.))
                                .rounded(px(3.))
                                .bg(gpui_kit::Hsla {
                                    a: 0.14,
                                    ..status_color
                                })
                                .border_1()
                                .border_color(gpui_kit::Hsla {
                                    a: 0.3,
                                    ..status_color
                                })
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(status_color)
                                .child(if res.passed { "✓ PASS" } else { "✗ FAIL" }),
                        )
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(t.text_primary)
                                .child(res.name),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .font_family(".SystemMonoFont")
                        .text_color(t.text_muted)
                        .child(format!("{}ms", res.duration_ms)),
                ),
        )
        .when_some(res.error_message, |this, err| {
            this.child(
                div()
                    .pl(px(12.))
                    .pt(px(2.))
                    .text_xs()
                    .font_family(".SystemMonoFont")
                    .text_color(status_color)
                    .child(format!("↳ {}", err)),
            )
        })
}

fn render_log_card(idx: usize, entry: ConsoleLogEntry) -> gpui_kit::Div {
    let t = theme();
    let is_err = entry.level == ConsoleLogLevel::Error;
    let is_warn = entry.level == ConsoleLogLevel::Warn;
    let is_info = entry.level == ConsoleLogLevel::Info;

    let msg_copy = entry.message.clone();

    let (src_label, src_color) = match entry.source {
        ConsoleLogSource::PreRequest => ("Pre-request", t.log_pre_request),
        ConsoleLogSource::PostResponse => ("Post-response", t.log_post_response),
        ConsoleLogSource::System => ("System", t.text_secondary),
    };

    let (lvl_label, lvl_color) = match entry.level {
        ConsoleLogLevel::Log => ("LOG", t.text_secondary),
        ConsoleLogLevel::Info => ("INFO", t.log_info),
        ConsoleLogLevel::Warn => ("WARN", t.log_warn),
        ConsoleLogLevel::Error => ("ERROR", t.log_error),
    };

    let card_border = if is_err {
        gpui_kit::Hsla {
            a: 0.35,
            ..t.log_error
        }
    } else if is_warn {
        gpui_kit::Hsla {
            a: 0.35,
            ..t.log_warn
        }
    } else {
        t.border
    };

    let card_bg = if is_err {
        gpui_kit::Hsla {
            a: 0.08,
            ..t.log_error
        }
    } else if is_warn {
        gpui_kit::Hsla {
            a: 0.06,
            ..t.log_warn
        }
    } else {
        t.bg_card
    };

    div()
        .p(px(8.))
        .rounded(px(6.))
        .bg(card_bg)
        .border_1()
        .border_color(card_border)
        .flex()
        .flex_col()
        .gap(px(4.))
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
                                .text_xs()
                                .font_family(".SystemMonoFont")
                                .text_color(t.text_muted)
                                .child(entry.timestamp),
                        )
                        .child(
                            div()
                                .px(px(6.))
                                .py(px(1.))
                                .rounded(px(3.))
                                .bg(gpui_kit::Hsla {
                                    a: 0.12,
                                    ..src_color
                                })
                                .border_1()
                                .border_color(gpui_kit::Hsla {
                                    a: 0.25,
                                    ..src_color
                                })
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                                .text_color(src_color)
                                .child(src_label),
                        )
                        .child(
                            div()
                                .px(px(5.))
                                .py(px(1.))
                                .rounded(px(3.))
                                .bg(gpui_kit::Hsla {
                                    a: 0.12,
                                    ..lvl_color
                                })
                                .border_1()
                                .border_color(gpui_kit::Hsla {
                                    a: 0.25,
                                    ..lvl_color
                                })
                                .text_xs()
                                .font_weight(gpui_kit::FontWeight::BOLD)
                                .text_color(lvl_color)
                                .child(lvl_label),
                        ),
                )
                .child(
                    div()
                        .id(format!("btn-copy-log-{}", idx))
                        .cursor_pointer()
                        .text_color(t.text_muted)
                        .hover(|s| s.text_color(t.text_primary))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(
                                msg_copy.clone(),
                            ));
                        })
                        .child(Icon::new(IconName::Copy).size(px(14.))),
                ),
        )
        .child(
            div()
                .pl(px(4.))
                .text_xs()
                .font_family(".SystemMonoFont")
                .text_color(if is_err {
                    t.log_error
                } else if is_warn {
                    t.log_warn
                } else if is_info {
                    t.log_info
                } else {
                    t.text_primary
                })
                .child(entry.message),
        )
}
