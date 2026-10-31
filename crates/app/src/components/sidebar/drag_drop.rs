use crate::components::MethodBadge;
use crate::theme::theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::{div, px, Context, IntoElement, ParentElement, Render, Styled, Window};
use zest_core::models::HttpMethod;

#[derive(Clone, Debug)]
pub struct DragPinnedItem {
    pub id: String,
    pub index: usize,
}

impl Render for DragPinnedItem {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(t.bg_card)
            .border_1()
            .border_color(t.border_focus)
            .shadow_md()
            .text_xs()
            .text_color(t.text_primary)
            .child(format!("📌 {}", self.id))
    }
}

#[derive(Clone, Debug)]
pub struct DragRequestItem {
    pub id: String,
    pub name: String,
    pub method: HttpMethod,
}

impl Render for DragRequestItem {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(t.bg_card)
            .border_1()
            .border_color(t.border_focus)
            .shadow_md()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(MethodBadge::new(self.method))
            .child(
                div()
                    .text_xs()
                    .font_family(".SystemMonoFont")
                    .font_weight(gpui_kit::FontWeight::NORMAL)
                    .text_color(t.text_primary)
                    .child(self.name.clone()),
            )
    }
}

#[derive(Clone, Debug)]
pub struct DragFolderItem {
    pub id: String,
    pub name: String,
}

impl Render for DragFolderItem {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(t.bg_card)
            .border_1()
            .border_color(t.border_focus)
            .shadow_md()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                Icon::new(IconName::Folder)
                    .size(px(14.))
                    .text_color(t.text_secondary),
            )
            .child(
                div()
                    .text_xs()
                    .font_family(".SystemMonoFont")
                    .font_weight(gpui_kit::FontWeight::NORMAL)
                    .text_color(t.text_primary)
                    .child(self.name.clone()),
            )
    }
}

#[derive(Clone, Debug)]
pub struct DragMultiItem {
    pub request_ids: Vec<String>,
    pub folder_ids: Vec<String>,
}

impl DragMultiItem {
    pub fn new(request_ids: Vec<String>, folder_ids: Vec<String>) -> Self {
        Self {
            request_ids,
            folder_ids,
        }
    }

    pub fn total_count(&self) -> usize {
        self.request_ids.len() + self.folder_ids.len()
    }
}

impl Render for DragMultiItem {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        let total = self.total_count();
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(t.bg_card)
            .border_1()
            .border_color(t.border_focus)
            .shadow_md()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                Icon::new(IconName::Folder)
                    .size(px(14.))
                    .text_color(t.text_secondary),
            )
            .child(
                div()
                    .text_xs()
                    .font_family(".SystemMonoFont")
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(t.text_primary)
                    .child(format!("{} items", total)),
            )
    }
}
