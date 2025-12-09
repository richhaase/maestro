use std::collections::BTreeMap;

use uuid::Uuid;
use zellij_tile::prelude::*;

use crate::agent::{AgentPane, PaneStatus};
use crate::error::MaestroError;
use crate::model::Model;
use crate::utils::{build_command, workspace_basename};

#[derive(Debug, Clone, PartialEq)]
pub enum TabChoice {
    Existing(String),
    New,
}

pub(super) fn derive_tab_name_from_workspace(input: &str) -> Option<String> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    if let Some(resolved) = crate::utils::resolve_workspace_path(trimmed) {
        let normalized = resolved.to_string_lossy().to_string();
        if !normalized.is_empty() {
            return Some(normalized);
        }
    }

    Some(trimmed.to_string())
}

pub fn spawn_agent_pane(
    model: &mut Model,
    workspace_path: String,
    agent_name: String,
    tab_choice: TabChoice,
) {
    if !model.permissions_granted() {
        *model.error_message_mut() = MaestroError::PermissionsNotGranted.to_string();
        return;
    }
    // Extract what we need from the agent before any mutable borrows
    let cmd = match model.agents().iter().find(|a| a.name == agent_name) {
        Some(a) => build_command(a),
        None => {
            *model.error_message_mut() = MaestroError::AgentNotFound(agent_name).to_string();
            return;
        }
    };
    let workspace_label = workspace_basename(&workspace_path);
    let title_label = if workspace_label.is_empty() {
        agent_name.clone()
    } else {
        workspace_label
    };
    let title = format!("{}:{}", title_label, Uuid::new_v4());
    let tab_name = match &tab_choice {
        TabChoice::Existing(name) => name.clone(),
        TabChoice::New => model
            .custom_tab_name()
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| crate::utils::default_tab_name(&workspace_path)),
    };

    let resolved_workspace = crate::utils::resolve_workspace_path(&workspace_path);

    let (tab_target, _is_new_tab) = match tab_choice {
        TabChoice::Existing(name) => (name, false),
        TabChoice::New => {
            let cwd_for_tab = resolved_workspace
                .as_ref()
                .map(|p| p.to_string_lossy().to_string());
            new_tab(Some(tab_name.clone()), cwd_for_tab);
            if !model.tab_names().contains(&tab_name) {
                model.tab_names_mut().push(tab_name.clone());
            }
            (tab_name, true)
        }
    };
    go_to_tab_name(&tab_target);
    let mut ctx = BTreeMap::new();
    ctx.insert("pane_title".to_string(), title.clone());
    if let Some(ref resolved) = resolved_workspace {
        ctx.insert("cwd".to_string(), resolved.to_string_lossy().to_string());
    }
    ctx.insert("agent".to_string(), agent_name.clone());
    ctx.insert("tab_name".to_string(), tab_target.clone());

    let mut command_to_run = if cmd.len() > 1 {
        CommandToRun::new_with_args(cmd[0].clone(), cmd[1..].to_vec())
    } else {
        CommandToRun::new(cmd.first().cloned().unwrap_or_default())
    };
    if let Some(ref resolved) = resolved_workspace {
        command_to_run.cwd = Some(resolved.clone());
    }
    open_command_pane(command_to_run, ctx);

    model.agent_panes_mut().push(AgentPane {
        pane_title: title.clone(),
        tab_name: tab_target,
        pending_tab_index: None,
        pane_id: None,
        workspace_path,
        agent_name,
        status: PaneStatus::Running,
    });
    model.error_message_mut().clear();
    *model.custom_tab_name_mut() = None;
    *model.wizard_agent_filter_mut() = String::new();
}

pub fn focus_selected(model: &mut Model, selected_idx: usize) {
    if !model.permissions_granted() {
        *model.error_message_mut() = MaestroError::PermissionsNotGranted.to_string();
        return;
    }

    if selected_idx >= model.agent_panes().len() {
        *model.error_message_mut() = MaestroError::NoAgentPanes.to_string();
        return;
    }
    let pane = &model.agent_panes()[selected_idx];
    go_to_tab_name(&pane.tab_name);
    if let Some(pid) = pane.pane_id {
        focus_terminal_pane(pid, false);
        model.error_message_mut().clear();
    } else {
        *model.error_message_mut() = MaestroError::PaneIdUnavailable.to_string();
    }
}

pub fn kill_selected(model: &mut Model, selected_idx: usize) {
    if !model.permissions_granted() {
        *model.error_message_mut() = MaestroError::PermissionsNotGranted.to_string();
        return;
    }

    if selected_idx >= model.agent_panes().len() {
        *model.error_message_mut() = MaestroError::NoAgentPanes.to_string();
        return;
    }
    let pane = &model.agent_panes()[selected_idx];
    if let Some(pid) = pane.pane_id {
        close_terminal_pane(pid);
        model.agent_panes_mut().retain(|p| p.pane_id != Some(pid));
        model.error_message_mut().clear();
        model.clamp_selections();
    } else {
        *model.error_message_mut() = MaestroError::PaneIdUnavailable.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WASI_HOST_MOUNT;

    #[test]
    fn test_derive_tab_name_from_workspace_relative() {
        let derived = derive_tab_name_from_workspace("src/maestro");
        assert_eq!(derived, Some("src/maestro".to_string()));
    }

    #[test]
    fn test_derive_tab_name_from_workspace_host_prefix() {
        let derived = derive_tab_name_from_workspace(&format!("{}/src/maestro", WASI_HOST_MOUNT));
        assert_eq!(derived, Some("src/maestro".to_string()));
    }
}
