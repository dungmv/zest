use crate::theme::theme;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, px, AnyElement, IntoElement, ParentElement, Styled};
use zest_core::models::HttpResponseRecord;

pub fn render_timing_popover(
    resp: &HttpResponseRecord,
    show_timing: bool,
    is_sending: bool,
) -> Option<AnyElement> {
    if !show_timing || is_sending {
        return None;
    }

    let t = theme();
    let timing = &resp.timing;
    let total_ms = timing.total_ms.max(1);

    // Proportions for visual bar
    let dns_pct = (timing.dns_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);
    let tcp_pct =
        (timing.connect_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);
    let tls_pct = (timing.tls_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);
    let ttfb_pct = (timing.ttfb_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);
    let dl_pct =
        (timing.download_ms.unwrap_or(0) as f32 / total_ms as f32 * 100.0).clamp(0.0, 100.0);

    Some(
        div()
            .p(px(10.))
            .mx(px(12.))
            .mt(px(4.))
            .rounded(px(6.))
            .bg(t.bg_card)
            .border_1()
            .border_color(t.border)
            .flex()
            .flex_col()
            .gap(px(6.))
            .text_xs()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(t.text_primary)
                            .child("High-Precision Timing Breakdown"),
                    )
                    .child(
                        div()
                            .font_weight(gpui_kit::FontWeight::BOLD)
                            .text_color(t.primary)
                            .child(format!("Total: {} ms", timing.total_ms)),
                    ),
            )
            // Visual Waterfall bar
            .child(
                div()
                    .h(px(8.))
                    .w_full()
                    .rounded(px(4.))
                    .bg(t.bg_input)
                    .overflow_hidden()
                    .flex()
                    .child(div().w(px(dns_pct * 2.5)).bg(t.timing_dns)) // Cyan for DNS
                    .child(div().w(px(tcp_pct * 2.5)).bg(t.timing_tcp)) // Blue for TCP
                    .child(div().w(px(tls_pct * 2.5)).bg(t.timing_tls)) // Purple for TLS
                    .child(div().w(px(ttfb_pct * 2.5)).bg(t.timing_ttfb)) // Yellow for TTFB
                    .child(div().w(px(dl_pct * 2.5)).bg(t.timing_download)), // Green for Download
            )
            // Detail rows
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_color(t.text_secondary)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child("● DNS Lookup:")
                            .text_color(t.timing_dns),
                    )
                    .child(format!("{} ms", timing.dns_ms.unwrap_or(0))),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_color(t.text_secondary)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child("● TCP Connect:")
                            .text_color(t.timing_tcp),
                    )
                    .child(format!("{} ms", timing.connect_ms.unwrap_or(0))),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_color(t.text_secondary)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child("● TLS Handshake:")
                            .text_color(t.timing_tls),
                    )
                    .child(format!("{} ms", timing.tls_ms.unwrap_or(0))),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_color(t.text_secondary)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child("● TTFB (Wait):")
                            .text_color(t.timing_ttfb),
                    )
                    .child(format!("{} ms", timing.ttfb_ms.unwrap_or(0))),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_color(t.text_secondary)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child("● Content Download:")
                            .text_color(t.timing_download),
                    )
                    .child(format!("{} ms", timing.download_ms.unwrap_or(0))),
            )
            .when(timing.connection_reused, |this| {
                this.child(
                    div()
                        .px(px(4.))
                        .py(px(2.))
                        .rounded(px(3.))
                        .bg(t.primary.opacity(0.15))
                        .text_color(t.primary)
                        .child("⚡ Connection Reused (HTTP Keep-Alive)"),
                )
            })
            .into_any_element(),
    )
}
