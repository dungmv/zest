//! GPUI styling, tooltip, and modal-navigation bindings for `zest_core::env`.

use crate::state::{EnvironmentState, ModalKind, ModalPayload, ModalState};
use crate::theme::theme;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{App, Entity, Hsla, Styled as _};
use zest_core::env::{
    list_candidates, resolve_variable, ResolvedVariable, VariableCandidate, VariableScope,
};

/// Foreground/border and tinted background colors for rendering a variable chip or decoration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvVarStyle {
    pub text: Hsla,
    pub bg_tint: Hsla,
}

/// Resolves a variable name against `EnvironmentState` using `zest_core::env::resolve_variable`.
pub fn resolve_in_state(env: &EnvironmentState, var_name: &str) -> ResolvedVariable {
    resolve_variable(env.active_environment(), &env.global_environment, var_name)
}

/// Lists matching autocomplete candidates from `EnvironmentState` using `zest_core::env::list_candidates`.
pub fn list_candidates_in_state(env: &EnvironmentState, query: &str) -> Vec<VariableCandidate> {
    list_candidates(env.active_environment(), &env.global_environment, query)
}

/// Maps a domain [`VariableScope`] to theme-consistent foreground and pill background colors.
pub fn variable_style(scope: &VariableScope) -> EnvVarStyle {
    let t = theme();
    let text = match scope {
        VariableScope::Active(_) => t.tag_variable,
        VariableScope::Global => t.json_boolean,
        VariableScope::Dynamic => t.tag_dynamic,
        VariableScope::Missing => t.var_unresolved,
    };
    EnvVarStyle {
        text,
        bg_tint: Hsla { a: 0.14, ..text },
    }
}

/// Builds a compact hover tooltip showing the resolved value (or missing notice) of a variable.
pub fn variable_value_tooltip(resolved: &ResolvedVariable) -> Tooltip {
    let style = variable_style(&resolved.scope);
    if resolved.is_resolved() {
        let text = if resolved.value.is_empty() {
            "\"\"".to_string()
        } else {
            resolved.value.clone()
        };
        Tooltip::new(text).font_family(".SystemMonoFont").text_xs()
    } else {
        Tooltip::new("Variable not found")
            .text_color(style.text)
            .text_xs()
    }
}

/// Opens the Environment modal focused on `var_name`:
/// 1. For dynamic variables (`$timestamp`, `$guid`, etc.): opens the environment modal without targeting a row.
/// 2. For environment variables: resolves the owning environment (or creates the key in the active/target environment if missing).
/// 3. Opens the Environment modal, switches to the target environment, scrolls to the row, and focuses its value input.
pub fn open_variable_in_modal(
    env_state: &Entity<EnvironmentState>,
    modal_state: &Entity<ModalState>,
    var_name: &str,
    cx: &mut App,
) {
    if var_name.starts_with('$') {
        modal_state.update(cx, |st, cx| {
            st.open(ModalKind::Environment, None, cx);
        });
        return;
    }

    let target_env_id = env_state.update(cx, |env_st, cx| {
        let env_id = env_st.resolve_variable_env_id(var_name);
        if !env_st.has_variable(&env_id, var_name) {
            env_st.add_variable_with_key(&env_id, var_name.to_string(), cx);
        }
        env_id
    });

    modal_state.update(cx, |modal_st, cx| {
        modal_st.open(
            ModalKind::Environment,
            Some(ModalPayload::Env {
                env_id: Some(target_env_id),
                target_variable: Some(var_name.to_string()),
            }),
            cx,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::AppContext as _;

    #[gpui_kit::test]
    fn test_open_variable_in_modal(cx: &mut gpui_kit::TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let store: std::sync::Arc<dyn zest_core::storage::WorkspaceStore> =
            std::sync::Arc::new(zest_core::storage::FsWorkspaceStore::new(dir.path()));
        store.init_workspace("TokenClickWS").unwrap();

        let env_state =
            cx.new(|_| EnvironmentState::new(store.clone(), "TokenClickWS".to_string(), None));
        let modal_state = cx.new(|_| ModalState::default());

        // 1. Dynamic variable ($guid) opens modal without changing target_variable
        cx.update(|cx| {
            open_variable_in_modal(&env_state, &modal_state, "$guid", cx);
        });
        assert!(modal_state.read_with(cx, |m, _| m.has_any_modal_open()));
        assert_eq!(
            modal_state.read_with(cx, |m, _| m.env_modal.target_variable.clone()),
            None
        );

        // 2. Custom/missing variable creates the variable in active environment ("local") and targets it
        cx.update(|cx| {
            open_variable_in_modal(&env_state, &modal_state, "API_KEY", cx);
        });
        assert!(modal_state.read_with(cx, |m, _| m.has_any_modal_open()));
        assert_eq!(
            modal_state.read_with(cx, |m, _| m.env_modal.target_variable.clone()),
            Some("API_KEY".to_string())
        );
        assert_eq!(
            modal_state.read_with(cx, |m, _| m.env_modal.selected_id.clone()),
            Some("local".to_string())
        );
        assert!(env_state.read_with(cx, |e, _| e.has_variable("local", "API_KEY")));
    }
}
