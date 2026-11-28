use super::SidebarState;
use crate::state::{CollectionState, EnvironmentState};
use gpui_kit::component::input::InputState;
use gpui_kit::{AppContext, Context, Entity, Window};

pub struct RenamingState<T> {
    pub id: T,
    pub input: Entity<InputState>,
    pub _subscription: gpui_kit::Subscription,
}

/// Per-entity inline-rename sessions of the sidebar, grouped so
/// [`SidebarState`] stays under 15 fields.
#[derive(Default)]
pub struct SidebarRenaming {
    pub request: Option<RenamingState<String>>,
    pub folder: Option<RenamingState<String>>,
    pub env: Option<RenamingState<String>>,
    pub example: Option<RenamingState<(String, String)>>,
}

impl SidebarState {
    pub fn cancel_renaming(&mut self, cx: &mut Context<Self>) {
        self.cancel_renaming_request(cx);
    }

    /// Creates the shared inline-rename input: prefilled, fully selected, focused,
    /// and committed on Enter/Blur through `on_commit`. Cancelling always goes
    /// through [`Self::cancel_any_renaming`], which drops the subscription together
    /// with the input so a cancelled rename never commits.
    fn spawn_renaming<T: 'static>(
        &mut self,
        id: T,
        initial_value: String,
        on_commit: impl Fn(&str, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> RenamingState<T> {
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_value(initial_value, window, cx);
            s.select_all(window, cx);
            s.focus(window, cx);
            s
        });

        let sub = cx.subscribe(
            &input,
            move |this, input, event: &gpui_kit::component::input::InputEvent, cx| match event {
                gpui_kit::component::input::InputEvent::PressEnter { .. }
                | gpui_kit::component::input::InputEvent::Blur => {
                    let value = input.read(cx).value().to_string();
                    this.cancel_any_renaming(cx);
                    let trimmed = value.trim();
                    if !trimmed.is_empty() {
                        on_commit(trimmed, cx);
                    }
                }
                _ => {}
            },
        );

        RenamingState {
            id,
            input,
            _subscription: sub,
        }
    }

    pub fn start_renaming_request(
        &mut self,
        req_id: String,
        initial_name: String,
        ws: Entity<CollectionState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_any_renaming(cx);
        let r_id = req_id.clone();
        self.renaming.request = Some(self.spawn_renaming(
            req_id,
            initial_name,
            move |name, cx| {
                ws.update(cx, |ws, cx| ws.rename_request(&r_id, name, cx));
            },
            window,
            cx,
        ));
        cx.notify();
    }

    pub fn start_renaming_folder(
        &mut self,
        folder_id: String,
        initial_name: String,
        ws: Entity<CollectionState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_any_renaming(cx);
        let f_id = folder_id.clone();
        self.renaming.folder = Some(self.spawn_renaming(
            folder_id,
            initial_name,
            move |name, cx| {
                ws.update(cx, |ws, cx| ws.rename_folder(&f_id, name.to_string(), cx));
            },
            window,
            cx,
        ));
        cx.notify();
    }

    pub fn start_renaming_example(
        &mut self,
        req_id: String,
        example_name: String,
        ws: Entity<CollectionState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_any_renaming(cx);
        let r_id = req_id.clone();
        let old_ex_name = example_name.clone();
        self.renaming.example = Some(self.spawn_renaming(
            (req_id, example_name.clone()),
            example_name,
            move |name, cx| {
                if name != old_ex_name {
                    ws.update(cx, |ws, cx| {
                        ws.rename_example(&r_id, &old_ex_name, name, cx)
                    });
                }
            },
            window,
            cx,
        ));
        cx.notify();
    }

    pub fn start_renaming_environment(
        &mut self,
        env_id: String,
        initial_name: String,
        env_state: Entity<EnvironmentState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_any_renaming(cx);
        let e_id = env_id.clone();
        self.renaming.env = Some(self.spawn_renaming(
            env_id,
            initial_name,
            move |name, cx| {
                env_state.update(cx, |env, cx| {
                    env.rename_environment(&e_id, name.to_string(), cx)
                });
            },
            window,
            cx,
        ));
        cx.notify();
    }

    pub fn renaming_request_id(&self) -> Option<&str> {
        self.renaming.request.as_ref().map(|r| r.id.as_str())
    }

    pub fn rename_input(&self) -> Option<&Entity<InputState>> {
        self.renaming.request.as_ref().map(|r| &r.input)
    }

    pub fn renaming_folder_id(&self) -> Option<&str> {
        self.renaming.folder.as_ref().map(|r| r.id.as_str())
    }

    pub fn rename_folder_input(&self) -> Option<&Entity<InputState>> {
        self.renaming.folder.as_ref().map(|r| &r.input)
    }

    pub fn renaming_env_id(&self) -> Option<&str> {
        self.renaming.env.as_ref().map(|r| r.id.as_str())
    }

    pub fn rename_env_input(&self) -> Option<&Entity<InputState>> {
        self.renaming.env.as_ref().map(|r| &r.input)
    }

    pub fn renaming_example(&self) -> Option<&(String, String)> {
        self.renaming.example.as_ref().map(|r| &r.id)
    }

    pub fn rename_example_input(&self) -> Option<&Entity<InputState>> {
        self.renaming.example.as_ref().map(|r| &r.input)
    }

    pub fn cancel_renaming_request(&mut self, cx: &mut Context<Self>) {
        if self.renaming.request.take().is_some() {
            cx.notify();
        }
    }

    pub fn cancel_renaming_folder(&mut self, cx: &mut Context<Self>) {
        if self.renaming.folder.take().is_some() {
            cx.notify();
        }
    }

    pub fn cancel_renaming_environment(&mut self, cx: &mut Context<Self>) {
        if self.renaming.env.take().is_some() {
            cx.notify();
        }
    }

    pub fn cancel_renaming_example(&mut self, cx: &mut Context<Self>) {
        if self.renaming.example.take().is_some() {
            cx.notify();
        }
    }

    pub fn cancel_any_renaming(&mut self, cx: &mut Context<Self>) -> bool {
        let mut cancelled = false;
        if self.renaming.request.take().is_some() {
            cancelled = true;
        }
        if self.renaming.folder.take().is_some() {
            cancelled = true;
        }
        if self.renaming.env.take().is_some() {
            cancelled = true;
        }
        if self.renaming.example.take().is_some() {
            cancelled = true;
        }
        if cancelled {
            cx.notify();
        }
        cancelled
    }
}
