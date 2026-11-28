use crate::state::{ImageZoomMode, ResponseState, MAX_PREVIEW_IMAGE_BYTES};
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, img, px, AnyElement, App, Entity, InteractiveElement as _, IntoElement, ObjectFit,
    ParentElement, StatefulInteractiveElement as _, Styled, StyledImage as _,
};
use std::sync::Arc;
use zest_core::models::{HttpResponseRecord, ImagePreviewFormat};

/// Renders the native image preview canvas, metadata bar, and zoom controls.
pub(super) fn render_image_preview(
    response_state: &Entity<ResponseState>,
    resp: &Arc<HttpResponseRecord>,
    format: ImagePreviewFormat,
    cx: &App,
) -> AnyElement {
    let t = theme();
    let resp_s = response_state.read(cx);
    let zoom = resp_s.view().image_zoom;
    let preview_image = resp_s
        .view()
        .formatted_body
        .as_ref()
        .and_then(|f| f.preview_image.clone());
    let dimensions = resp_s
        .view()
        .formatted_body
        .as_ref()
        .and_then(|f| f.image_dimensions);

    // Case 1: Binary image restored from .loc cache where body bytes were not persisted.
    if resp.body.is_empty() && resp.size_bytes > 0 {
        return render_notice_state(
            IconName::Info,
            "Binary image not cached on disk",
            format!(
                "{} ({}) is omitted from .loc disk cache. Resend the request to preview.",
                format.label(),
                resp.formatted_size()
            ),
        );
    }

    // Case 2: Image exceeds the in-app decode safety cap.
    if resp.body.len() > MAX_PREVIEW_IMAGE_BYTES {
        return render_notice_state(
            IconName::Info,
            "Image too large for inline preview",
            format!(
                "{} payload ({}) exceeds the 25 MB preview limit. Use Save to File to view externally.",
                format.label(),
                resp.formatted_size()
            ),
        );
    }

    let Some(image) = preview_image else {
        return render_notice_state(
            IconName::CircleX,
            "Unable to preview image",
            "The response body is empty or could not be prepared for rendering.".to_string(),
        );
    };

    let mut image_el = img(image)
        .object_fit(ObjectFit::Contain)
        .with_fallback(move || {
            let t = theme();
            div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .p(px(24.))
                .child(
                    Icon::new(IconName::CircleX)
                        .size(px(20.))
                        .text_color(t.text_muted),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_secondary)
                        .child("Failed to decode image data"),
                )
                .into_any_element()
        });

    image_el = match zoom {
        ImageZoomMode::Fit => image_el.max_w_full().max_h_full(),
        ImageZoomMode::Scale(pct) => {
            let scale = f32::from(pct) / 100.0;
            if let Some((w, h)) = dimensions {
                image_el.w(px(w as f32 * scale)).h(px(h as f32 * scale))
            } else {
                // Vector SVGs or formats without probed pixel headers scale against
                // a sensible 320px base box when zoomed explicitly.
                image_el.w(px(320.0 * scale)).h(px(320.0 * scale))
            }
        }
    };

    let rs_out = response_state.clone();
    let rs_reset = response_state.clone();
    let rs_in = response_state.clone();
    let is_fit = matches!(zoom, ImageZoomMode::Fit);
    let reset_target = if is_fit {
        ImageZoomMode::Scale(100)
    } else {
        ImageZoomMode::Fit
    };
    let reset_tooltip = if is_fit {
        "Switch to 100%"
    } else {
        "Reset to Fit"
    };

    let meta_summary = match dimensions {
        Some((w, h)) => format!(
            "{}  ·  {} × {} px  ·  {}",
            format.label(),
            w,
            h,
            resp.formatted_size()
        ),
        None => format!("{}  ·  {}", format.label(), resp.formatted_size()),
    };

    div()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        // Scrollable image viewport
        .child(
            div()
                .id("response-image-preview-canvas")
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .overflow_scroll()
                .p(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .when(is_fit, |d| d.max_w_full().max_h_full())
                        .p(px(12.))
                        .rounded(px(6.))
                        .bg(t.bg_card)
                        .border_1()
                        .border_color(t.border)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(image_el),
                ),
        )
        // Bottom metadata & zoom bar
        .child(
            div()
                .h(px(30.))
                .px(px(10.))
                .border_t_1()
                .border_color(t.border)
                .bg(t.bg_editor)
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_secondary)
                        .child(meta_summary),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .gap(px(2.))
                        .child(
                            Button::new("btn-img-zoom-out")
                                .ghost()
                                .xsmall()
                                .label("−")
                                .tooltip("Zoom out")
                                .on_click(move |_, _, cx: &mut App| {
                                    rs_out.update(cx, |state, cx| {
                                        state.zoom_out_image(cx);
                                    });
                                }),
                        )
                        .child(
                            div()
                                .w(px(48.))
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Button::new("btn-img-zoom-reset")
                                        .ghost()
                                        .xsmall()
                                        .w_full()
                                        .label(zoom.label())
                                        .tooltip(reset_tooltip)
                                        .on_click(move |_, _, cx: &mut App| {
                                            rs_reset.update(cx, |state, cx| {
                                                state.set_image_zoom(reset_target, cx);
                                            });
                                        }),
                                ),
                        )
                        .child(
                            Button::new("btn-img-zoom-in")
                                .ghost()
                                .xsmall()
                                .label("+")
                                .tooltip("Zoom in")
                                .on_click(move |_, _, cx: &mut App| {
                                    rs_in.update(cx, |state, cx| {
                                        state.zoom_in_image(cx);
                                    });
                                }),
                        ),
                ),
        )
        .into_any_element()
}

fn render_notice_state(icon: IconName, title: &str, description: String) -> AnyElement {
    let t = theme();
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .px(px(24.))
        .child(Icon::new(icon).size(px(20.)).text_color(t.text_muted))
        .child(
            div()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(t.text_secondary)
                .child(title.to_string()),
        )
        .child(div().text_xs().text_color(t.text_muted).child(description))
        .into_any_element()
}
