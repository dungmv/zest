mod rows;

use crate::state::{EnvironmentEvent, EnvironmentState, ModalEvent, ModalState};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, Render, ScrollHandle, Subscription, Window,
};
use rows::render_var_table;
use zest_core::models::EnvironmentVariable;

struct EnvVarRowInputs {
    key_input: Entity<InputState>,
    init_input: Entity<InputState>,
    curr_input: Entity<InputState>,
    secret: bool,
    _subscriptions: Vec<Subscription>,
}

/// Dedicated GPUI entity managing the `InputState` pool for the Environment
/// Modal's variable table. Synchronizes exclusively through typed events
/// (`ModalEvent`, `EnvironmentEvent`) so `Render::render` remains 100% pure.
pub struct EnvVarTable {
    env_state: Entity<EnvironmentState>,
    modal_state: Entity<ModalState>,
    bound_env_id: String,
    rows: Vec<EnvVarRowInputs>,
    scroll_handle: ScrollHandle,
    target_variable: Option<String>,
    in_local_change: bool,
    _subscriptions: Vec<Subscription>,
}

impl EnvVarTable {
    pub fn new(
        window: &mut Window,
        env_state: Entity<EnvironmentState>,
        modal_state: Entity<ModalState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_id = modal_state
            .read(cx)
            .env_modal
            .selected_id
            .clone()
            .unwrap_or_else(|| "globals".to_string());

        let mut this = Self {
            env_state: env_state.clone(),
            modal_state: modal_state.clone(),
            bound_env_id: initial_id,
            rows: Vec::new(),
            scroll_handle: ScrollHandle::new(),
            target_variable: None,
            in_local_change: false,
            _subscriptions: Vec::new(),
        };

        this.reconcile_full(window, cx);

        let sub_modal = cx.subscribe_in(
            &modal_state,
            window,
            |this, _modal, event: &ModalEvent, window, cx| match event {
                ModalEvent::EnvModalSelectionChanged => {
                    let next_id = this.current_selected_env_id(cx);
                    if next_id != this.bound_env_id {
                        this.rebuild_for_env(next_id, window, cx);
                    } else {
                        this.reconcile_full(window, cx);
                    }
                    this.focus_target_variable(window, cx);
                    cx.notify();
                }
                ModalEvent::ThemeChanged => {}
            },
        );

        let sub_env = cx.subscribe_in(
            &env_state,
            window,
            |this, _env, event: &EnvironmentEvent, window, cx| match event {
                EnvironmentEvent::EnvironmentsReloaded => {
                    let next_id = this.current_selected_env_id(cx);
                    if next_id != this.bound_env_id {
                        this.rebuild_for_env(next_id, window, cx);
                    } else {
                        this.reconcile_full(window, cx);
                    }
                    cx.notify();
                }
                EnvironmentEvent::EnvironmentModified(name) => {
                    if !this.in_local_change && this.bound_env_id == *name {
                        this.reconcile_structure(window, cx);
                        this.focus_target_variable(window, cx);
                        cx.notify();
                    }
                }
                EnvironmentEvent::GlobalsModified => {
                    if !this.in_local_change && this.bound_env_id == "globals" {
                        this.reconcile_structure(window, cx);
                        this.focus_target_variable(window, cx);
                        cx.notify();
                    }
                }
                EnvironmentEvent::ActiveEnvironmentChanged(_) => {}
            },
        );

        this._subscriptions = vec![sub_modal, sub_env];
        this
    }

    fn focus_target_variable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.modal_state.read(cx).env_modal.target_variable.clone();
        self.target_variable = target.clone();

        let Some(var_name) = target else {
            return;
        };

        if let Some(idx) = self
            .rows
            .iter()
            .position(|r| r.key_input.read(cx).value().as_ref() == var_name)
        {
            self.scroll_handle.scroll_to_item(idx);
            let row = &self.rows[idx];
            row.curr_input.update(cx, |inp, cx| {
                inp.select_all(window, cx);
                inp.focus(window, cx);
            });
        }
    }

    fn current_selected_env_id(&self, cx: &Context<Self>) -> String {
        self.modal_state
            .read(cx)
            .env_modal
            .selected_id
            .clone()
            .unwrap_or_else(|| "globals".to_string())
    }

    fn target_variables(&self, cx: &Context<Self>) -> Vec<EnvironmentVariable> {
        let env_read = self.env_state.read(cx);
        if self.bound_env_id == "globals" {
            env_read.global_environment.variables.clone()
        } else if let Some(custom) = env_read
            .environments
            .iter()
            .find(|e| e.id == self.bound_env_id || e.name == self.bound_env_id)
        {
            custom.variables.clone()
        } else {
            Vec::new()
        }
    }

    fn rebuild_for_env(&mut self, next_id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.bound_env_id = next_id;
        self.rows.clear();
        let vars = self.target_variables(cx);
        for var in &vars {
            let row = self.create_row_inputs(var, window, cx);
            self.rows.push(row);
        }
    }

    /// Full reconciliation: used on environment switch, modal open, or external
    /// disk reload (`EnvironmentsReloaded`). Updates row count, secret flags,
    /// and input values when they differ from `EnvironmentState`.
    pub fn reconcile_full(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let vars = self.target_variables(cx);
        if self.rows.len() > vars.len() {
            self.rows.truncate(vars.len());
        }

        for (idx, var) in vars.iter().enumerate() {
            if idx < self.rows.len() {
                self.rows[idx].secret = var.secret;
                if self.rows[idx].key_input.read(cx).value().as_ref() != var.key {
                    let next_key = var.key.clone();
                    self.rows[idx]
                        .key_input
                        .update(cx, |inp, cx| inp.set_value(next_key, window, cx));
                }
                if self.rows[idx].init_input.read(cx).value().as_ref() != var.initial_value {
                    let next_init = var.initial_value.clone();
                    self.rows[idx]
                        .init_input
                        .update(cx, |inp, cx| inp.set_value(next_init, window, cx));
                }
                if self.rows[idx].curr_input.read(cx).value().as_ref() != var.current_value {
                    let next_curr = var.current_value.clone();
                    self.rows[idx]
                        .curr_input
                        .update(cx, |inp, cx| inp.set_value(next_curr, window, cx));
                }
            } else {
                let row = self.create_row_inputs(var, window, cx);
                self.rows.push(row);
            }
        }
    }

    /// Structural reconciliation: used when a variable is added/removed/toggled
    /// in the currently bound environment without calling `set_value` on
    /// existing inputs (never interferes with active user typing).
    fn reconcile_structure(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let vars = self.target_variables(cx);
        if self.rows.len() > vars.len() {
            self.rows.truncate(vars.len());
        }
        for (idx, var) in vars.iter().enumerate() {
            if idx < self.rows.len() {
                self.rows[idx].secret = var.secret;
            } else {
                let row = self.create_row_inputs(var, window, cx);
                self.rows.push(row);
            }
        }
    }

    fn create_row_inputs(
        &self,
        var: &EnvironmentVariable,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> EnvVarRowInputs {
        let init_k = var.key.clone();
        let init_val_init = var.initial_value.clone();
        let init_val_curr = var.current_value.clone();

        let k_in = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Variable key...");
            s.set_value(init_k, window, cx);
            s
        });
        let init_in = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Initial value (git)...");
            s.set_value(init_val_init, window, cx);
            s
        });
        let curr_in = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder("Current value (local)...");
            s.set_value(init_val_curr, window, cx);
            s
        });

        let k_ent = k_in.clone();
        let sub1 = cx.subscribe_in(&k_in, window, move |this, _, ev, _, cx| {
            if let InputEvent::Change = ev {
                let nk = k_ent.read(cx).value().to_string();
                let cid = this.bound_env_id.clone();
                if let Some(idx) = this.rows.iter().position(|r| r.key_input == k_ent) {
                    this.in_local_change = true;
                    let env_state = this.env_state.clone();
                    env_state.update(cx, |state, cx| {
                        state.update_variable_key(&cid, idx, nk, cx);
                    });
                    this.in_local_change = false;
                }
            }
        });

        let init_ent = init_in.clone();
        let sub2 = cx.subscribe_in(&init_in, window, move |this, _, ev, _, cx| {
            if let InputEvent::Change = ev {
                let n_init = init_ent.read(cx).value().to_string();
                let cid = this.bound_env_id.clone();
                if let Some(idx) = this.rows.iter().position(|r| r.init_input == init_ent) {
                    this.in_local_change = true;
                    let env_state = this.env_state.clone();
                    env_state.update(cx, |state, cx| {
                        state.update_variable_initial_value(&cid, idx, n_init, cx);
                    });
                    this.in_local_change = false;
                }
            }
        });

        let curr_ent = curr_in.clone();
        let sub3 = cx.subscribe_in(&curr_in, window, move |this, _, ev, _, cx| {
            if let InputEvent::Change = ev {
                let n_curr = curr_ent.read(cx).value().to_string();
                let cid = this.bound_env_id.clone();
                if let Some(idx) = this.rows.iter().position(|r| r.curr_input == curr_ent) {
                    this.in_local_change = true;
                    let env_state = this.env_state.clone();
                    env_state.update(cx, |state, cx| {
                        state.update_variable_current_value(&cid, idx, n_curr, cx);
                    });
                    this.in_local_change = false;
                }
            }
        });

        EnvVarRowInputs {
            key_input: k_in,
            init_input: init_in,
            curr_input: curr_in,
            secret: var.secret,
            _subscriptions: vec![sub1, sub2, sub3],
        }
    }

    fn toggle_row_secret_by_key(&mut self, key_input: &Entity<InputState>, cx: &mut Context<Self>) {
        let Some(pos) = self.rows.iter().position(|r| &r.key_input == key_input) else {
            return;
        };
        self.rows[pos].secret = !self.rows[pos].secret;
        let cid = self.bound_env_id.clone();
        self.in_local_change = true;
        let env_state = self.env_state.clone();
        env_state.update(cx, |state, cx| {
            state.toggle_variable_secret(&cid, pos, cx);
        });
        self.in_local_change = false;
        cx.notify();
    }

    fn delete_row_by_key(&mut self, key_input: &Entity<InputState>, cx: &mut Context<Self>) {
        let Some(pos) = self.rows.iter().position(|r| &r.key_input == key_input) else {
            return;
        };
        self.rows.remove(pos);
        let cid = self.bound_env_id.clone();
        self.in_local_change = true;
        let env_state = self.env_state.clone();
        env_state.update(cx, |state, cx| {
            state.delete_variable(&cid, pos, cx);
        });
        self.in_local_change = false;
        cx.notify();
    }

    #[cfg(test)]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    #[cfg(test)]
    pub fn row_current_value(&self, idx: usize, cx: &gpui_kit::App) -> Option<String> {
        self.rows
            .get(idx)
            .map(|r| r.curr_input.read(cx).value().to_string())
    }

    #[cfg(test)]
    pub fn target_variable(&self) -> Option<String> {
        self.target_variable.clone()
    }
}

impl Render for EnvVarTable {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_var_table(self, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ModalKind, ModalPayload};
    use gpui_kit::{div, ParentElement, TestAppContext, VisualTestContext};
    use std::sync::Arc;
    use tempfile::tempdir;
    use zest_core::storage::{FsWorkspaceStore, WorkspaceStore};

    struct HostView {
        table: Entity<EnvVarTable>,
    }

    impl Render for HostView {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div().child(self.table.clone())
        }
    }

    #[gpui_kit::test]
    fn test_env_var_table_event_driven_sync(cx: &mut TestAppContext) {
        let dir = tempdir().unwrap();
        let store: Arc<dyn WorkspaceStore> = Arc::new(FsWorkspaceStore::new(dir.path()));
        store.init_workspace("VarTableWS").unwrap();

        let env_state =
            cx.new(|_| EnvironmentState::new(store.clone(), "VarTableWS".to_string(), None));
        let modal_state = cx.new(|_| ModalState::default());

        let window = cx.update(|cx| {
            gpui_kit::component::init(cx);
            let env_s = env_state.clone();
            let mod_s = modal_state.clone();
            cx.open_window(Default::default(), move |window, cx| {
                let table = cx.new(|cx| EnvVarTable::new(window, env_s, mod_s, cx));
                cx.new(|_| HostView { table })
            })
            .unwrap()
        });
        let mut vcx = VisualTestContext::from_window(window.into(), cx);
        let table = window
            .read_with(&vcx, |host, _| host.table.clone())
            .unwrap();

        assert_eq!(table.read_with(&vcx, |t, _| t.row_count()), 0);

        // 1. Adding a variable in globals appends a row via GlobalsModified -> reconcile_structure
        env_state.update(&mut vcx, |env, cx| {
            env.add_variable("globals", cx);
            env.update_variable_key("globals", 0, "BASE_URL".to_string(), cx);
            env.update_variable_current_value(
                "globals",
                0,
                "https://v1.example.com".to_string(),
                cx,
            );
        });
        vcx.run_until_parked();
        assert_eq!(table.read_with(&vcx, |t, _| t.row_count()), 1);

        // 2. EnvironmentsReloaded synchronizes input values from EnvironmentState
        env_state.update(&mut vcx, |_, cx| {
            cx.emit(EnvironmentEvent::EnvironmentsReloaded);
        });
        vcx.run_until_parked();
        assert_eq!(
            table.read_with(&vcx, |t, cx| t.row_current_value(0, cx)),
            Some("https://v1.example.com".to_string())
        );

        // 3. Switching modal selection to "local" rebuilds rows for "local" (0 vars initially)
        modal_state.update(&mut vcx, |m, cx| {
            m.select_modal_environment(Some("local".to_string()), cx);
        });
        vcx.run_until_parked();
        assert_eq!(table.read_with(&vcx, |t, _| t.row_count()), 0);

        // 4. Target variable focus sets target_variable and rebuilds for target environment
        modal_state.update(&mut vcx, |m, cx| {
            m.open(
                ModalKind::Environment,
                Some(ModalPayload::Env {
                    env_id: Some("globals".to_string()),
                    target_variable: Some("BASE_URL".to_string()),
                }),
                cx,
            );
        });
        vcx.run_until_parked();
        assert_eq!(table.read_with(&vcx, |t, _| t.row_count()), 1);
        assert_eq!(
            table.read_with(&vcx, |t, _| t.target_variable()),
            Some("BASE_URL".to_string())
        );
    }
}
