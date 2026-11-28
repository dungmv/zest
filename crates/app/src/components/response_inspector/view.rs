use super::format::{
    build_cached_format, FormattedResponseBody, ImageZoomMode, ResponseTabKind, ResponseViewMode,
};
use std::sync::Arc;
use zest_core::models::{HttpResponseRecord, ResponsePreviewKind};

pub struct ResponseView {
    pub active_tab: ResponseTabKind,
    pub mode: ResponseViewMode,
    /// Zoom preset for the image preview canvas (reset on response switch).
    pub image_zoom: ImageZoomMode,
    /// Active JSONPath query for the response Body tab (empty = no filter).
    pub jsonpath_query: String,
    /// Whether the JSONPath filter bar below the response toolbar is visible.
    pub show_jsonpath_bar: bool,
    pub show_examples_dropdown: bool,
    pub show_timing_popover: bool,
    pub formatted_body: Option<FormattedResponseBody>,
    pub scroll_handle: gpui_kit::UniformListScrollHandle,
    pub wrap_lines: bool,
}

impl Default for ResponseView {
    fn default() -> Self {
        Self {
            active_tab: ResponseTabKind::Response,
            mode: ResponseViewMode::Pretty,
            image_zoom: ImageZoomMode::Fit,
            jsonpath_query: String::new(),
            show_jsonpath_bar: false,
            show_examples_dropdown: false,
            show_timing_popover: false,
            formatted_body: None,
            scroll_handle: gpui_kit::UniformListScrollHandle::new(),
            wrap_lines: false,
        }
    }
}

impl ResponseView {
    pub(crate) fn refresh_formatted_body(&mut self, response: Option<&Arc<HttpResponseRecord>>) {
        if let Some(resp) = response {
            let resp_ptr = Arc::as_ptr(resp) as usize;

            let cached_format = match &self.formatted_body {
                Some(current) => match &current.cached_format {
                    Some(cf) if cf.response_ptr == resp_ptr => cf.clone(),
                    _ => Arc::new(build_cached_format(resp)),
                },
                None => Arc::new(build_cached_format(resp)),
            };

            let (mut display_text, mut lines) = if matches!(self.mode, ResponseViewMode::Pretty) {
                (
                    cached_format.pretty_text.clone(),
                    cached_format.pretty_lines.clone(),
                )
            } else {
                (
                    cached_format.raw_text.clone(),
                    cached_format.raw_lines.clone(),
                )
            };

            // Apply the JSONPath query. The result is always pretty JSON
            // because the query runs on parsed data rather than the raw text
            // view; incomplete or invalid expressions silently keep the base
            // display so typing (e.g. `$.`) is never disrupted.
            let mut match_count = None;
            if !self.jsonpath_query.is_empty() {
                if let Some(parsed) = &cached_format.parsed_json {
                    if let Ok(result) =
                        zest_core::json_path::run_json_path(parsed, &self.jsonpath_query)
                    {
                        match_count = Some(result.match_count);
                        display_text = Arc::new(result.result_text);
                        lines = Arc::new(result.lines);
                    }
                }
            }

            self.formatted_body = Some(FormattedResponseBody {
                display_text,
                lines,
                is_json: cached_format.is_json,
                is_xml: cached_format.is_xml,
                is_binary: cached_format.is_binary,
                is_truncated: cached_format.is_truncated,
                preview_kind: cached_format.preview_kind,
                preview_image: cached_format.preview_image.clone(),
                image_dimensions: cached_format.image_dimensions,
                match_count,
                cached_format: Some(cached_format),
            });
        } else {
            self.formatted_body = None;
        }
    }

    /// Reconciles `view_mode` and resets `image_zoom` whenever the active
    /// response changes:
    /// - Image responses auto-switch to `Preview`.
    /// - Non-previewable responses fall back from `Preview` to `Pretty`.
    pub(crate) fn reconcile_view_mode_for_response(&mut self) {
        self.image_zoom = ImageZoomMode::Fit;
        let preview_kind = self.formatted_body.as_ref().and_then(|f| f.preview_kind);
        match preview_kind {
            Some(ResponsePreviewKind::Image(_)) => {
                self.mode = ResponseViewMode::Preview;
            }
            None => {
                if self.mode == ResponseViewMode::Preview {
                    self.mode = ResponseViewMode::Pretty;
                }
            }
        }
    }

    pub(crate) fn reset_workspace(&self) -> Self {
        Self {
            wrap_lines: self.wrap_lines,
            active_tab: self.active_tab,
            mode: self.mode,
            ..Self::default()
        }
    }

    pub(crate) fn reset_body(&mut self) {
        self.jsonpath_query.clear();
        self.reset_scroll();
    }

    pub(crate) fn reset_scroll(&mut self) {
        self.scroll_handle = gpui_kit::UniformListScrollHandle::new();
    }
}
