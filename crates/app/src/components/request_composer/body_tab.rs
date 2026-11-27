use crate::app::AppView;
use crate::components::*;
use gpui_kit::{div, AnyElement, IntoElement, ParentElement, Styled};

pub(super) fn render_body_tab(av: &AppView) -> AnyElement {
    div()
        .size_full()
        .child(
            BodyEditor::new(
                av.state.session.clone(),
                av.composer_inputs.json_editor.clone(),
                av.composer_inputs.xml_editor.clone(),
                av.composer_inputs.raw_textarea.clone(),
            )
            .workspace_state(av.state.collection.clone())
            .env_state(av.state.env.clone())
            .modal_state(av.state.modal.clone()),
        )
        .into_any_element()
}
