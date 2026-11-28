use super::{ImageZoomMode, ResponseState, ResponseTabKind, ResponseViewMode};
use crate::state::ResponseEvent;
use gpui_kit::Context;

impl ResponseState {
    pub fn refresh_formatted_body(&mut self) {
        self.view
            .refresh_formatted_body(self.current_response.as_ref());
    }

    pub(crate) fn reconcile_view_mode_for_response(&mut self) {
        self.view.reconcile_view_mode_for_response();
    }

    pub fn set_active_tab(&mut self, tab: ResponseTabKind, cx: &mut Context<Self>) {
        self.view.active_tab = tab;
        cx.notify();
    }

    pub fn set_view_mode(&mut self, mode: ResponseViewMode, cx: &mut Context<Self>) {
        if self.view.mode == mode {
            return;
        }
        self.view.mode = mode;
        self.refresh_formatted_body();
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: false,
        });
        cx.notify();
    }

    pub fn set_image_zoom(&mut self, zoom: ImageZoomMode, cx: &mut Context<Self>) {
        if self.view.image_zoom == zoom {
            return;
        }
        self.view.image_zoom = zoom;
        cx.notify();
    }

    pub fn zoom_in_image(&mut self, cx: &mut Context<Self>) {
        let next = self.view.image_zoom.zoom_in();
        self.set_image_zoom(next, cx);
    }

    pub fn zoom_out_image(&mut self, cx: &mut Context<Self>) {
        let prev = self.view.image_zoom.zoom_out();
        self.set_image_zoom(prev, cx);
    }

    pub fn toggle_examples_dropdown(&mut self, cx: &mut Context<Self>) {
        self.view.show_examples_dropdown = !self.view.show_examples_dropdown;
        cx.notify();
    }

    pub fn toggle_timing_popover(&mut self, cx: &mut Context<Self>) {
        self.view.show_timing_popover = !self.view.show_timing_popover;
        cx.notify();
    }

    pub fn toggle_wrap_lines(&mut self, cx: &mut Context<Self>) {
        self.view.wrap_lines = !self.view.wrap_lines;
        cx.notify();
    }

    /// Updates the JSONPath query used to filter the response Body tab.
    ///
    /// Re-emits `ContentChanged` so the shared response editor re-syncs with
    /// the filtered display text through the standard event channel.
    pub fn set_jsonpath_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.view.jsonpath_query = query;
        self.refresh_formatted_body();
        cx.emit(ResponseEvent::ContentChanged {
            sync_sent_preview: false,
        });
        cx.notify();
    }

    /// Toggles the JSONPath filter bar below the response toolbar. Closing the
    /// bar clears any active query so the full response body is restored.
    /// Returns the new visibility state.
    pub fn toggle_jsonpath_bar(&mut self, cx: &mut Context<Self>) -> bool {
        if self.view.show_jsonpath_bar {
            self.close_jsonpath_bar(cx);
            false
        } else {
            self.view.show_jsonpath_bar = true;
            cx.notify();
            true
        }
    }

    /// Closes the JSONPath filter bar and clears any active query.
    pub fn close_jsonpath_bar(&mut self, cx: &mut Context<Self>) {
        self.view.show_jsonpath_bar = false;
        if !self.view.jsonpath_query.is_empty() {
            self.view.jsonpath_query.clear();
            self.refresh_formatted_body();
            cx.emit(ResponseEvent::ContentChanged {
                sync_sent_preview: false,
            });
        }
        cx.notify();
    }
}
