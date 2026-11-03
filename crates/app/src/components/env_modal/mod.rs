mod detail_header;
mod env_list;
mod var_table;

pub use var_table::EnvVarTable;

use crate::components::ModalDialog;
use crate::state::{EnvironmentState, ModalState, PersistenceState, SidebarState};
use crate::theme::theme;
use detail_header::{render_detail_header, DetailHeaderProps};
use env_list::{render_env_list_sidebar, EnvListProps};
use gpui_kit::assets::IconName;
use gpui_kit::{div, px, App, Entity, IntoElement, ParentElement, RenderOnce, Styled, Window};

#[derive(IntoElement)]
pub struct EnvironmentModal {
    env_state: Entity<EnvironmentState>,
    persistence_state: Entity<PersistenceState>,
    modal_state: Entity<ModalState>,
    sidebar_state: Entity<SidebarState>,
    var_table: Entity<EnvVarTable>,
}

impl EnvironmentModal {
    pub fn new(
        env_state: Entity<EnvironmentState>,
        persistence_state: Entity<PersistenceState>,
        modal_state: Entity<ModalState>,
        sidebar_state: Entity<SidebarState>,
        var_table: Entity<EnvVarTable>,
    ) -> Self {
        Self {
            env_state,
            persistence_state,
            modal_state,
            sidebar_state,
            var_table,
        }
    }
}

impl RenderOnce for EnvironmentModal {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = theme();
        let ws = self.env_state.read(cx);
        let modal = self.modal_state.read(cx);
        let sidebar = self.sidebar_state.read(cx);
        let per = self.persistence_state.read(cx);

        let envs = ws.environments.clone();
        let global_var_count = ws.global_environment.variables.len();
        let selected_id = modal
            .selected_env_modal_id
            .clone()
            .unwrap_or_else(|| "globals".to_string());
        let is_global_selected = selected_id == "globals";
        let selected_custom_env = envs.iter().find(|e| e.id == selected_id).cloned();

        let conflict_lookup_key = if is_global_selected {
            "globals"
        } else {
            selected_custom_env
                .as_ref()
                .map(|e| e.name.as_str())
                .unwrap_or(selected_id.as_str())
        };
        let active_conflict = per.env_conflicts.get(conflict_lookup_key).cloned();

        let renaming_env_id = sidebar.renaming_env_id().map(ToString::to_string);
        let rename_env_input = sidebar.rename_env_input().cloned();
        let modal_close = self.modal_state.clone();

        ModalDialog::new("env-modal")
            .w(px(980.))
            .h(px(580.))
            .title("Environments & Variables")
            .icon(IconName::Globe)
            .on_close(move |_, cx| {
                modal_close.update(cx, |state, cx| {
                    state.close_env_modal(cx);
                });
            })
            .child(
                div()
                    .flex_1()
                    .flex()
                    .overflow_hidden()
                    .child(render_env_list_sidebar(EnvListProps {
                        env_state: self.env_state.clone(),
                        modal_state: self.modal_state.clone(),
                        sidebar_state: self.sidebar_state.clone(),
                        selected_id: selected_id.clone(),
                        is_global_selected,
                        global_var_count,
                        environments: envs,
                        renaming_env_id,
                        rename_env_input,
                    }))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .bg(t.bg_panel)
                            .flex()
                            .flex_col()
                            .child(render_detail_header(DetailHeaderProps {
                                env_state: self.env_state,
                                persistence_state: self.persistence_state,
                                selected_id,
                                is_global_selected,
                                selected_custom_env,
                                active_conflict,
                            }))
                            .child(self.var_table),
                    ),
            )
    }
}
