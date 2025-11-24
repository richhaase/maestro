use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow;
use uuid::Uuid;
use zellij_tile::prelude::*;
use zellij_tile::prelude::{
    BareKey, KeyModifier, KeyWithModifier, PaneId, PaneManifest, PermissionStatus, TabInfo,
};

use crate::agent::{Agent, AgentPane, PaneStatus};
use crate::agent::{default_config_path, is_default_agent, save_agents};
use crate::error::{MaestroError, MaestroResult};
use crate::model::Model;
use crate::ui::{AgentFormField, Mode, Section, next_field, prev_field};
use crate::utils::{build_command_with_env, find_agent_by_command, is_maestro_tab, parse_env_input, parse_title_hint, workspace_basename, workspace_tab_name};

#[derive(Debug, Clone, PartialEq)]
pub enum TabChoice {
    Existing(String),
    New,
}

fn selected_tab_choice(model: &Model) -> TabChoice {
    if model.wizard_tab_idx() < model.tab_names().len() {
        TabChoice::Existing(model.tab_names()[model.wizard_tab_idx()].to_string())
    } else {
        TabChoice::New
    }
}

fn handle_text_edit(target: &mut String, key: &KeyWithModifier) -> bool {
    match key.bare_key {
        BareKey::Backspace => {
            target.pop();
            true
        }
        BareKey::Delete => {
            target.clear();
            true
        }
        BareKey::Char(c) => {
            target.push(c);
            true
        }
        _ => false,
    }
}

fn handle_form_text(model: &mut Model, key: &KeyWithModifier) -> bool {
    match model.agent_form_field() {
        AgentFormField::Name => handle_text_edit(model.agent_name_input_mut(), key),
        AgentFormField::Command => handle_text_edit(model.agent_command_input_mut(), key),
        AgentFormField::Env => handle_text_edit(model.agent_env_input_mut(), key),
        AgentFormField::Note => handle_text_edit(model.agent_note_input_mut(), key),
    }
}

pub fn handle_permission_result(model: &mut Model, status: PermissionStatus) {
    match status {
        PermissionStatus::Granted => {
            *model.permissions_granted_mut() = true;
            *model.permissions_denied_mut() = false;
        }
        PermissionStatus::Denied => {
            *model.permissions_granted_mut() = false;
            *model.permissions_denied_mut() = true;
        }
    }
}

pub fn apply_tab_update(model: &mut Model, tabs: Vec<TabInfo>) {
    let tab_names: Vec<String> = tabs.iter().map(|t| t.name.clone()).collect();
    let tab_names_ref = &tab_names;
    *model.tab_names_mut() = tab_names.clone();
    
    model.agent_panes_mut()
        .retain(|p| p.pane_id.is_some() || tab_names_ref.contains(&p.tab_name));
    model.clamp_selections();
}

pub fn apply_pane_update(model: &mut Model, update: PaneManifest) {
    for (tab_idx, pane_list) in update.panes {
        let tab_name = model.tab_names().get(tab_idx).cloned().unwrap_or_default();

        let tab_names_ref = model.tab_names().to_vec();
        for pane in pane_list {
            if let Some(existing) = model
                .agent_panes_mut()
                .iter_mut()
                .find(|p| p.pane_id == Some(pane.id))
            {
                if (existing.tab_name.is_empty()
                    || (!tab_name.is_empty() && !tab_names_ref.contains(&existing.tab_name)))
                    && !tab_name.is_empty()
                {
                    existing.tab_name = tab_name.clone();
                }
                existing.status = if pane.exited {
                    PaneStatus::Exited(pane.exit_status)
                } else {
                    PaneStatus::Running
                };
                continue;
            }

            let title = pane.title.clone();

            if is_maestro_tab(&title) {
                let (agent_name, workspace_path) = parse_title_hint(&title)
                    .unwrap_or_default();
                model.agent_panes_mut().push(AgentPane {
                    pane_title: title,
                    tab_name: tab_name.clone(),
                    pane_id: Some(pane.id),
                    workspace_path,
                    agent_name,
                    status: if pane.exited {
                        PaneStatus::Exited(pane.exit_status)
                    } else {
                        PaneStatus::Running
                    },
                });
                continue;
            }

            if !pane.is_plugin {
                if let Some(agent) = find_agent_by_command(model.agents(), &title) {
                    let agent_name = agent.name.clone();
                    let reconstructed_title = format!("maestro:{agent_name}::recovered");
                    model.agent_panes_mut().push(AgentPane {
                        pane_title: reconstructed_title,
                        tab_name: tab_name.clone(),
                        pane_id: Some(pane.id),
                        workspace_path: String::new(),
                        agent_name,
                        status: if pane.exited {
                            PaneStatus::Exited(pane.exit_status)
                        } else {
                            PaneStatus::Running
                        },
                    });
                }
            }
        }
    }
    model.clamp_selections();
}

pub fn handle_command_pane_opened(model: &mut Model, pane_id: u32, ctx: BTreeMap<String, String>) {
    let title = ctx
        .get("pane_title")
        .cloned()
        .unwrap_or_else(|| format!("maestro:{pane_id}"));
    let workspace_path = ctx.get("cwd").cloned().unwrap_or_default();
    let agent_name = ctx.get("agent").cloned().unwrap_or_default();

    let first_tab = model.tab_names().first().cloned();
    let entry = model
        .agent_panes_mut()
        .iter_mut()
        .find(|p| p.pane_id == Some(pane_id) || (p.pane_id.is_none() && p.pane_title == title));

    if let Some(existing) = entry {
        existing.pane_id = Some(pane_id);
        existing.pane_title = title.clone();

        if existing.tab_name.is_empty() {
            if let Some(tab_name) = ctx.get("tab_name").cloned() {
                existing.tab_name = tab_name;
            } else if let Some(first_tab) = first_tab {
                existing.tab_name = first_tab;
            }
        }
        if !workspace_path.is_empty() {
            existing.workspace_path = workspace_path.clone();
        }
        if !agent_name.is_empty() {
            existing.agent_name = agent_name.clone();
        }
        existing.status = PaneStatus::Running;
    } else {
        let tab_name = ctx
            .get("tab_name")
            .cloned()
            .or_else(|| model.tab_names().first().cloned())
            .unwrap_or_default();
        model.agent_panes_mut().push(AgentPane {
            pane_title: title,
            tab_name,
            pane_id: Some(pane_id),
            workspace_path,
            agent_name,
            status: PaneStatus::Running,
        });
    }
    model.clamp_selections();
}

fn rebuild_from_session_infos(model: &mut Model, session_infos: &[SessionInfo]) {
    let has_tab_names = !model.tab_names().is_empty();

    for session in session_infos {
        let session_name = session.name.clone();
        if let Some(current) = model.session_name() {
            if &session_name != current {
                continue;
            }
        } else {
            *model.session_name_mut() = Some(session_name.clone());
        }
        for (tab_idx, pane_list) in session.panes.clone().panes {
            let tab_name = if has_tab_names {
                model.tab_names().get(tab_idx).cloned().unwrap_or_default()
            } else {
                String::new()
            };

            let mut unmatched_in_tab: Vec<usize> = if has_tab_names {
                model
                    .agent_panes()
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.pane_id.is_none() && p.tab_name == tab_name)
                    .map(|(idx, _)| idx)
                    .collect()
            } else {
                Vec::new()
            };

                let tab_names_ref = model.tab_names().to_vec();
                for pane in pane_list {
                    if let Some(existing) = model
                        .agent_panes_mut()
                        .iter_mut()
                        .find(|p| p.pane_id == Some(pane.id))
                    {
                        existing.status = if pane.exited {
                            PaneStatus::Exited(pane.exit_status)
                        } else {
                            PaneStatus::Running
                        };

                        if (existing.tab_name.is_empty()
                            || (!tab_name.is_empty()
                                && !tab_names_ref.contains(&existing.tab_name)))
                            && !tab_name.is_empty()
                        {
                            existing.tab_name = tab_name.clone();
                        }
                    continue;
                }

                if is_maestro_tab(&pane.title) {
                    let (agent_name, workspace_path) = parse_title_hint(&pane.title)
                        .unwrap_or_default();
                    model.agent_panes_mut().push(AgentPane {
                        pane_title: pane.title.clone(),
                        tab_name: tab_name.clone(),
                        pane_id: Some(pane.id),
                        workspace_path,
                        agent_name,
                        status: if pane.exited {
                            PaneStatus::Exited(pane.exit_status)
                        } else {
                            PaneStatus::Running
                        },
                    });
                    continue;
                }

                if let Some(unmatched_idx) = unmatched_in_tab.pop() {
                    let existing = &mut model.agent_panes_mut()[unmatched_idx];
                    existing.pane_id = Some(pane.id);
                    existing.status = if pane.exited {
                        PaneStatus::Exited(pane.exit_status)
                    } else {
                        PaneStatus::Running
                    };
                    if !tab_name.is_empty() {
                        existing.tab_name = tab_name.clone();
                    }
                    continue;
                }

                if !pane.is_plugin {
                    if let Some(agent) = find_agent_by_command(model.agents(), &pane.title) {
                        let agent_name = agent.name.clone();
                        let reconstructed_title = format!("maestro:{agent_name}::recovered");
                        model.agent_panes_mut().push(AgentPane {
                            pane_title: reconstructed_title,
                            tab_name: tab_name.clone(),
                            pane_id: Some(pane.id),
                            workspace_path: String::new(),
                            agent_name,
                            status: if pane.exited {
                                PaneStatus::Exited(pane.exit_status)
                            } else {
                                PaneStatus::Running
                            },
                        });
                    }
                }
            }
        }
    }
    model.clamp_selections();
}

pub fn handle_command_pane_exited(
    model: &mut Model,
    pane_id: u32,
    exit_status: Option<i32>,
    ctx: BTreeMap<String, String>,
) {
    let title = ctx
        .get("pane_title")
        .cloned()
        .unwrap_or_else(|| format!("maestro:{pane_id}"));
    if let Some(pane) = model
        .agent_panes_mut()
        .iter_mut()
        .find(|p| p.pane_id == Some(pane_id) || p.pane_title == title)
    {
        pane.status = PaneStatus::Exited(exit_status);
    }
    model.clamp_selections();
}

pub fn handle_command_pane_rerun(model: &mut Model, pane_id: u32, ctx: BTreeMap<String, String>) {
    let title = ctx
        .get("pane_title")
        .cloned()
        .unwrap_or_else(|| format!("maestro:{pane_id}"));
    if let Some(pane) = model
        .agent_panes_mut()
        .iter_mut()
        .find(|p| p.pane_id == Some(pane_id) || p.pane_title == title)
    {
        pane.status = PaneStatus::Running;
    }
    model.clamp_selections();
}

pub fn handle_session_update(model: &mut Model, sessions: Vec<SessionInfo>) {
    let current_session_name = sessions
        .iter()
        .find(|s| s.is_current_session)
        .map(|s| s.name.clone());
    
    if let Some(new_session_name) = current_session_name {
        if let Some(old_session_name) = model.session_name() {
            if old_session_name != &new_session_name {
                model.agent_panes_mut().clear();
            }
        }
        *model.session_name_mut() = Some(new_session_name);
    }
    
    rebuild_from_session_infos(model, &sessions);
}

pub fn handle_pane_closed(model: &mut Model, pane_id: PaneId) {
    let pid = match pane_id {
        PaneId::Terminal(id) | PaneId::Plugin(id) => id,
    };
    model.agent_panes_mut().retain(|p| p.pane_id != Some(pid));
    model.clamp_selections();
}

pub fn spawn_agent_pane(
    model: &mut Model,
    workspace_path: String,
    agent_name: String,
    tab_choice: TabChoice,
) {
    if !model.permissions_granted() {
        *model.error_message_mut() = "permissions not granted".to_string();
        return;
    }
    let agent = match model.agents().iter().find(|a| a.name == agent_name) {
        Some(a) => a.clone(),
        None => {
            *model.error_message_mut() = "agent not found".to_string();
            return;
        }
    };
    let title = format!(
        "maestro:{}:{}:{}",
        agent.name,
        workspace_basename(&workspace_path),
        Uuid::new_v4()
    );
    let (tab_target, _is_new_tab) = match tab_choice {
        TabChoice::Existing(name) => (name, false),
        TabChoice::New => {
            let name = workspace_tab_name(&workspace_path);
            new_tab(Some(name.clone()), Some(workspace_path.clone()));
            if !model.tab_names().contains(&name) {
                model.tab_names_mut().push(name.clone());
            }
            (name, true)
        }
    };
    go_to_tab_name(&tab_target);
    let cmd = build_command_with_env(&agent);
    let mut ctx = BTreeMap::new();
    ctx.insert("pane_title".to_string(), title.clone());
    if !workspace_path.is_empty() {
        ctx.insert("cwd".to_string(), workspace_path.clone());
    }
    ctx.insert("agent".to_string(), agent.name.clone());

    let mut command_to_run = CommandToRun::new(cmd.join(" "));
    if !workspace_path.is_empty() {
        command_to_run.cwd = Some(PathBuf::from(workspace_path.clone()));
    }
    open_command_pane(command_to_run, ctx);

    model.agent_panes_mut().push(AgentPane {
        pane_title: title.clone(),
        tab_name: tab_target.clone(),
        pane_id: None,
        workspace_path,
        agent_name,
        status: PaneStatus::Running,
    });
    model.error_message_mut().clear();
    if model.status_message().is_empty() {
        *model.status_message_mut() = "Agent pane launched".to_string();
    } else {
        *model.status_message_mut() = format!("{}; Agent pane launched", model.status_message());
    }
}

pub fn focus_selected(model: &mut Model, selected_idx: usize) {
    if !model.permissions_granted() {
        *model.error_message_mut() = "permissions not granted".to_string();
        return;
    }

    let filter_lower = model.filter_text().to_lowercase();
    let panes: Vec<&AgentPane> = if filter_lower.is_empty() {
        model.agent_panes().iter().collect()
    } else {
        model
            .agent_panes()
            .iter()
            .filter(|p| {
                p.agent_name.to_lowercase().contains(&filter_lower)
                    || p.tab_name.to_lowercase().contains(&filter_lower)
            })
            .collect()
    };

    if selected_idx >= panes.len() {
        *model.error_message_mut() = "no agent panes".to_string();
        return;
    }
    let pane = panes[selected_idx];
    go_to_tab_name(&pane.tab_name);
    if let Some(pid) = pane.pane_id {
        focus_terminal_pane(pid, false);
        model.error_message_mut().clear();
        *model.status_message_mut() = "Focused agent pane".to_string();
    } else {
        *model.error_message_mut() = "Pane ID not available yet".to_string();
    }
}

pub fn kill_selected(model: &mut Model, selected_idx: usize) {
    if !model.permissions_granted() {
        *model.error_message_mut() = "permissions not granted".to_string();
        return;
    }

    let filter_lower = model.filter_text().to_lowercase();
    let panes: Vec<&AgentPane> = if filter_lower.is_empty() {
        model.agent_panes().iter().collect()
    } else {
        model
            .agent_panes()
            .iter()
            .filter(|p| {
                p.agent_name.to_lowercase().contains(&filter_lower)
                    || p.tab_name.to_lowercase().contains(&filter_lower)
            })
            .collect()
    };

    if selected_idx >= panes.len() {
        *model.error_message_mut() = "no agent panes".to_string();
        return;
    }
    let pane = panes[selected_idx];
    if let Some(pid) = pane.pane_id {
        close_terminal_pane(pid);
        model.agent_panes_mut().retain(|p| p.pane_id != Some(pid));
        model.error_message_mut().clear();
        *model.status_message_mut() = "Killed agent pane".to_string();
        model.clamp_selections();
    } else {
        *model.error_message_mut() = "no valid target to kill".to_string();
    }
}

fn build_agent_from_inputs(model: &Model) -> MaestroResult<Agent> {
    let name = model.agent_name_input().trim().to_string();
    if name.is_empty() {
        return Err(MaestroError::AgentNameRequired);
    }
    let cmd_parts: Vec<String> = model
        .agent_command_input()
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    if cmd_parts.is_empty() {
        return Err(MaestroError::CommandRequired);
    }
    let env = parse_env_input(model.agent_env_input())
        .map_err(MaestroError::EnvParse)?;
    let note = if model.agent_note_input().trim().is_empty() {
        None
    } else {
        Some(model.agent_note_input().trim().to_string())
    };
    Ok(Agent {
        name,
        command: cmd_parts,
        env,
        note,
    })
}

fn apply_agent_create(model: &mut Model, agent: Agent) -> MaestroResult<PathBuf> {
    if model.agents().iter().any(|a| a.name == agent.name) {
        return Err(MaestroError::DuplicateAgentName(agent.name.clone()));
    }
    model.agents_mut().push(agent.clone());
    *model.selected_agent_mut() = model.agents().len().saturating_sub(1);
    persist_agents(model)
}

fn apply_agent_edit(model: &mut Model, agent: Agent) -> MaestroResult<PathBuf> {
    if let Some(idx) = model.form_target_agent() {
        if idx < model.agents().len() {
            if model
                .agents()
                .iter()
                .enumerate()
                .any(|(i, a)| i != idx && a.name == agent.name)
            {
                return Err(MaestroError::DuplicateAgentName(agent.name.clone()));
            }
            model.agents_mut()[idx] = agent;
            *model.selected_agent_mut() = idx;
            return persist_agents(model);
        }
    }
    Err(MaestroError::NoAgentSelected)
}

fn persist_agents(model: &mut Model) -> MaestroResult<PathBuf> {
    let path = default_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| MaestroError::Config(anyhow::anyhow!("create config directory: {e}")))?;
    }
    let user_agents: Vec<_> = model
        .agents()
        .iter()
        .filter(|a| !is_default_agent(&a.name))
        .cloned()
        .collect();
    save_agents(&path, &user_agents)?;
    match crate::agent::load_agents_default() {
        Ok(list) => {
            *model.agents_mut() = list;
            model.clamp_selections();
            model.error_message_mut().clear();
            Ok(path)
        }
        Err(err) => Err(MaestroError::Config(err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::model::Model;
    use zellij_tile::prelude::{BareKey, KeyWithModifier};

    fn create_test_model() -> Model {
        Model::default()
    }

    fn create_test_agent(name: &str) -> Agent {
        Agent {
            name: name.to_string(),
            command: vec!["echo".to_string(), name.to_string()],
            env: None,
            note: None,
        }
    }

    fn char_key(c: char) -> KeyWithModifier {
        KeyWithModifier {
            bare_key: BareKey::Char(c),
            key_modifiers: std::collections::BTreeSet::new(),
        }
    }

    fn backspace_key() -> KeyWithModifier {
        KeyWithModifier {
            bare_key: BareKey::Backspace,
            key_modifiers: std::collections::BTreeSet::new(),
        }
    }

    fn delete_key() -> KeyWithModifier {
        KeyWithModifier {
            bare_key: BareKey::Delete,
            key_modifiers: std::collections::BTreeSet::new(),
        }
    }

    #[test]
    fn test_selected_tab_choice_existing() {
        let mut model = create_test_model();
        model.tab_names_mut().push("tab1".to_string());
        model.tab_names_mut().push("tab2".to_string());
        *model.wizard_tab_idx_mut() = 1;
        let choice = selected_tab_choice(&model);
        assert_eq!(choice, TabChoice::Existing("tab2".to_string()));
    }

    #[test]
    fn test_selected_tab_choice_new() {
        let mut model = create_test_model();
        model.tab_names_mut().push("tab1".to_string());
        *model.wizard_tab_idx_mut() = 1;
        let choice = selected_tab_choice(&model);
        assert_eq!(choice, TabChoice::New);
    }

    #[test]
    fn test_handle_text_edit_char() {
        let mut target = String::new();
        let key = char_key('a');
        assert!(handle_text_edit(&mut target, &key));
        assert_eq!(target, "a");
    }

    #[test]
    fn test_handle_text_edit_backspace() {
        let mut target = "hello".to_string();
        let key = backspace_key();
        assert!(handle_text_edit(&mut target, &key));
        assert_eq!(target, "hell");
    }

    #[test]
    fn test_handle_text_edit_delete() {
        let mut target = "hello".to_string();
        let key = delete_key();
        assert!(handle_text_edit(&mut target, &key));
        assert_eq!(target, "");
    }

    #[test]
    fn test_handle_text_edit_backspace_empty() {
        let mut target = String::new();
        let key = backspace_key();
        assert!(handle_text_edit(&mut target, &key));
        assert_eq!(target, "");
    }

    #[test]
    fn test_handle_form_text_name_field() {
        let mut model = create_test_model();
        *model.agent_form_field_mut() = AgentFormField::Name;
        let key = char_key('t');
        assert!(handle_form_text(&mut model, &key));
        assert_eq!(model.agent_name_input(), "t");
    }

    #[test]
    fn test_handle_form_text_command_field() {
        let mut model = create_test_model();
        *model.agent_form_field_mut() = AgentFormField::Command;
        let key = char_key('e');
        assert!(handle_form_text(&mut model, &key));
        assert_eq!(model.agent_command_input(), "e");
    }

    #[test]
    fn test_build_agent_from_inputs_valid() {
        let mut model = create_test_model();
        *model.agent_name_input_mut() = "test-agent".to_string();
        *model.agent_command_input_mut() = "echo hello".to_string();
        *model.agent_env_input_mut() = "VAR=value".to_string();
        *model.agent_note_input_mut() = "test note".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_ok());
        let agent = result.unwrap();
        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.command, vec!["echo", "hello"]);
        assert!(agent.env.is_some());
        assert_eq!(agent.note, Some("test note".to_string()));
    }

    #[test]
    fn test_build_agent_from_inputs_empty_name() {
        let mut model = create_test_model();
        *model.agent_name_input_mut() = "   ".to_string();
        *model.agent_command_input_mut() = "echo hello".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MaestroError::AgentNameRequired));
    }

    #[test]
    fn test_build_agent_from_inputs_empty_command() {
        let mut model = create_test_model();
        *model.agent_name_input_mut() = "test-agent".to_string();
        *model.agent_command_input_mut() = "   ".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MaestroError::CommandRequired));
    }

    #[test]
    fn test_build_agent_from_inputs_empty_note() {
        let mut model = create_test_model();
        *model.agent_name_input_mut() = "test-agent".to_string();
        *model.agent_command_input_mut() = "echo hello".to_string();
        *model.agent_note_input_mut() = "   ".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_ok());
        let agent = result.unwrap();
        assert_eq!(agent.note, None);
    }

    #[test]
    fn test_build_agent_from_inputs_multiple_command_args() {
        let mut model = create_test_model();
        *model.agent_name_input_mut() = "test-agent".to_string();
        *model.agent_command_input_mut() = "echo hello world".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_ok());
        let agent = result.unwrap();
        assert_eq!(agent.command, vec!["echo", "hello", "world"]);
    }

    #[test]
    fn test_apply_agent_create_duplicate() {
        let mut model = create_test_model();
        model.agents_mut().push(create_test_agent("existing"));
        let agent = create_test_agent("existing");

        let result = apply_agent_create(&mut model, agent);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MaestroError::DuplicateAgentName(_)
        ));
    }

    #[test]
    fn test_apply_agent_edit_no_selection() {
        let mut model = create_test_model();
        let agent = create_test_agent("new-agent");

        let result = apply_agent_edit(&mut model, agent);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MaestroError::NoAgentSelected));
    }

    #[test]
    fn test_apply_agent_edit_duplicate() {
        let mut model = create_test_model();
        model.agents_mut().push(create_test_agent("agent1"));
        model.agents_mut().push(create_test_agent("agent2"));
        *model.form_target_agent_mut() = Some(0);
        let agent = create_test_agent("agent2");

        let result = apply_agent_edit(&mut model, agent);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MaestroError::DuplicateAgentName(_)
        ));
    }

    #[test]
    fn test_build_agent_from_inputs_env_parsing() {
        let mut model = create_test_model();
        *model.agent_name_input_mut() = "test-agent".to_string();
        *model.agent_command_input_mut() = "echo hello".to_string();
        *model.agent_env_input_mut() = "VAR1=value1,VAR2=value2".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_ok());
        let agent = result.unwrap();
        assert!(agent.env.is_some());
        let env = agent.env.unwrap();
        assert_eq!(env.get("VAR1"), Some(&"value1".to_string()));
        assert_eq!(env.get("VAR2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_build_agent_from_inputs_invalid_env() {
        let mut model = create_test_model();
        *model.agent_name_input_mut() = "test-agent".to_string();
        *model.agent_command_input_mut() = "echo hello".to_string();
        *model.agent_env_input_mut() = "=value".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MaestroError::EnvParse(_)));
    }
}

pub fn handle_key_event(model: &mut Model, key: KeyWithModifier) {
    match model.mode() {
        Mode::View => handle_key_event_view(model, key),
        Mode::NewPaneWorkspace => handle_key_event_new_pane_workspace(model, key),
        Mode::NewPaneTabSelect => handle_key_event_new_pane_tab_select(model, key),
        Mode::NewPaneAgentSelect => handle_key_event_new_pane_agent_select(model, key),
        Mode::NewPaneAgentCreate => handle_key_event_agent_form(model, key, true),
        Mode::AgentFormCreate | Mode::AgentFormEdit => {
            handle_key_event_agent_form(model, key, false)
        }
        Mode::DeleteConfirm => handle_key_event_delete_confirm(model, key),
    }
}

fn handle_key_event_view(model: &mut Model, key: KeyWithModifier) {
    if !key.key_modifiers.is_empty() && key.bare_key != BareKey::Tab {
        return;
    }

    if model.filter_active() {
        match key.bare_key {
            BareKey::Char(c) => {
                *model.filter_text_mut() = format!("{}{}", model.filter_text(), c);
                *model.selected_pane_mut() = 0;
                model.clamp_selections();
                return;
            }
            BareKey::Up => {
                move_selection(model, model.focused_section(), -1);
                return;
            }
            BareKey::Down => {
                move_selection(model, model.focused_section(), 1);
                return;
            }
            BareKey::Backspace => {
                let mut filter = model.filter_text().to_string();
                filter.pop();
                *model.filter_text_mut() = filter;
                *model.selected_pane_mut() = 0;
                model.clamp_selections();
                return;
            }
            BareKey::Esc => {
                *model.filter_active_mut() = false;
                model.filter_text_mut().clear();
                *model.selected_pane_mut() = 0;
                model.clamp_selections();
                return;
            }
            _ => {}
        }
    }

    match key.bare_key {
        BareKey::Char('j') | BareKey::Char('J') => {
            move_selection(model, model.focused_section(), 1);
        }
        BareKey::Char('k') | BareKey::Char('K') => {
            move_selection(model, model.focused_section(), -1);
        }
        BareKey::Tab => {
            focus_next_section(model);
        }
        BareKey::Enter => match model.focused_section() {
            Section::AgentPanes => {
                let idx = model.selected_pane();
                focus_selected(model, idx);
                close_self();
            }
            Section::Agents => {
                if model.selected_agent() < model.agents().len() {
                    start_agent_edit(model);
                }
            }
        },
        BareKey::Esc => {
            close_self();
        }
        BareKey::Char('f') | BareKey::Char('F') => {
            if model.focused_section() == Section::AgentPanes {
                *model.filter_active_mut() = true;
                model.filter_text_mut().clear();
                *model.selected_pane_mut() = 0;
                model.clamp_selections();
            }
        }
        BareKey::Char('x') | BareKey::Char('X') => {
            if model.focused_section() == Section::AgentPanes {
                let idx = model.selected_pane();
                kill_selected(model, idx);
            }
        }
        BareKey::Char('e') | BareKey::Char('E') => {
            if model.focused_section() == Section::Agents
                && model.selected_agent() < model.agents().len()
            {
                start_agent_edit(model);
            }
        }
        BareKey::Char('d') | BareKey::Char('D') => {
            if model.focused_section() == Section::Agents
                && model.selected_agent() < model.agents().len()
            {
                start_agent_delete_confirm(model);
            }
        }
        BareKey::Char('n') | BareKey::Char('N') => {
            if model.focused_section() == Section::Agents {
                if model.selected_agent() < model.agents().len() {
                    let agent_name = model.agents()[model.selected_agent()].name.clone();
                    *model.quick_launch_agent_name_mut() = Some(agent_name);
                    start_new_pane_workspace(model);
                }
            } else {
                start_new_pane_workspace(model);
            }
        }
        BareKey::Char('a') | BareKey::Char('A') => {
            if model.focused_section() == Section::Agents {
                start_agent_create(model);
            } else {
                *model.focused_section_mut() = Section::Agents;
                model.clamp_selections();
            }
        }
        _ => {}
    }
}

fn handle_key_event_new_pane_workspace(model: &mut Model, key: KeyWithModifier) {
    if handle_text_edit(model.workspace_input_mut(), &key) {
        return;
    }
    match key.bare_key {
        BareKey::Enter => {
            *model.mode_mut() = Mode::NewPaneTabSelect;
            *model.wizard_tab_idx_mut() = 0;
            *model.wizard_agent_idx_mut() = 0;
            reset_status(model);
        }
        BareKey::Esc => cancel_to_view(model),
        BareKey::Tab => {
            *model.mode_mut() = Mode::NewPaneTabSelect;
            *model.wizard_tab_idx_mut() = 0;
            *model.wizard_agent_idx_mut() = 0;
            reset_status(model);
        }
        _ => {}
    }
}

fn handle_key_event_new_pane_tab_select(model: &mut Model, key: KeyWithModifier) {
    let choices = model.tab_names().len().saturating_add(1);
    match key.bare_key {
        BareKey::Char('k') | BareKey::Char('K') | BareKey::Up => {
            if model.wizard_tab_idx() > 0 {
                *model.wizard_tab_idx_mut() = model.wizard_tab_idx() - 1;
            }
        }
        BareKey::Char('j') | BareKey::Char('J') | BareKey::Down => {
            if model.wizard_tab_idx() + 1 < choices {
                *model.wizard_tab_idx_mut() = model.wizard_tab_idx() + 1;
            }
        }
        BareKey::Enter => {
            if let Some(agent_name) = model.quick_launch_agent_name_mut().take() {
                let workspace = model.workspace_input().trim().to_string();
                let tab_choice = selected_tab_choice(model);
                spawn_agent_pane(model, workspace, agent_name, tab_choice);
                if model.error_message().is_empty() {
                    view_preserve_messages(model);
                }
            } else {
                *model.mode_mut() = Mode::NewPaneAgentSelect;
                *model.wizard_agent_idx_mut() = 0;
            }
        }
        BareKey::Esc => cancel_to_view(model),
        BareKey::Tab => cancel_to_view(model),
        _ => {}
    }
}

fn handle_key_event_new_pane_agent_select(model: &mut Model, key: KeyWithModifier) {
    let choices = model.agents().len().saturating_add(1);
    match key.bare_key {
        BareKey::Char('k') | BareKey::Char('K') | BareKey::Up => {
            if model.wizard_agent_idx() > 0 {
                *model.wizard_agent_idx_mut() = model.wizard_agent_idx() - 1;
            }
        }
        BareKey::Char('j') | BareKey::Char('J') | BareKey::Down => {
            if model.wizard_agent_idx() + 1 < choices {
                *model.wizard_agent_idx_mut() = model.wizard_agent_idx() + 1;
            }
        }
        BareKey::Enter => {
            if model.wizard_agent_idx() < model.agents().len() {
                let agent = model.agents()[model.wizard_agent_idx()].name.clone();
                let workspace = model.workspace_input().trim().to_string();
                let tab_choice = selected_tab_choice(model);
                spawn_agent_pane(model, workspace, agent, tab_choice);
                if model.error_message().is_empty() {
                    view_preserve_messages(model);
                }
            } else {
                *model.mode_mut() = Mode::NewPaneAgentCreate;
                *model.agent_form_source_mut() = Some(Mode::NewPaneAgentSelect);
                model.agent_name_input_mut().clear();
                model.agent_command_input_mut().clear();
                model.agent_env_input_mut().clear();
                model.agent_note_input_mut().clear();
                *model.agent_form_field_mut() = AgentFormField::Name;
                reset_status(model);
            }
        }
        BareKey::Esc => cancel_to_view(model),
        BareKey::Tab => cancel_to_view(model),
        _ => {}
    }
}

fn handle_key_event_agent_form(model: &mut Model, key: KeyWithModifier, launch_after: bool) {
    if handle_form_text(model, &key) {
        return;
    }
    let shift_tab =
        key.bare_key == BareKey::Tab && key.key_modifiers.contains(&KeyModifier::Shift);
    match key.bare_key {
        BareKey::Tab if shift_tab => {
            *model.agent_form_field_mut() = prev_field(model.agent_form_field());
        }
        BareKey::Tab => {
            *model.agent_form_field_mut() = next_field(model.agent_form_field());
        }
        BareKey::Enter => match build_agent_from_inputs(model) {
            Ok(agent) => {
                let result = match model.mode() {
                    Mode::AgentFormEdit => apply_agent_edit(model, agent.clone()),
                    Mode::AgentFormCreate | Mode::NewPaneAgentCreate => {
                        apply_agent_create(model, agent.clone())
                    }
                    _ => Err(MaestroError::InvalidMode),
                };
                match result {
                    Ok(saved_path) => {
                        *model.status_message_mut() =
                            format!("Agents saved to {}", saved_path.display());
                        if launch_after {
                            let workspace = model.workspace_input().trim().to_string();
                            let tab_choice = selected_tab_choice(model);
                            spawn_agent_pane(model, workspace, agent.name.clone(), tab_choice);
                        }
                        if model.error_message().is_empty() {
                            view_preserve_messages(model);
                        }
                    }
                    Err(err) => {
                        *model.error_message_mut() = err.to_string();
                    }
                }
            }
            Err(err) => {
                *model.error_message_mut() = err.to_string();
            }
        },
        BareKey::Esc => {
            *model.agent_form_source_mut() = None;
            cancel_to_view(model);
        }
        _ => {}
    }
}

fn handle_key_event_delete_confirm(model: &mut Model, key: KeyWithModifier) {
    match key.bare_key {
        BareKey::Enter | BareKey::Char('y') | BareKey::Char('Y') => {
            if let Some(idx) = model.form_target_agent_mut().take() {
                if idx < model.agents().len() {
                    let agent_name = model.agents()[idx].name.clone();
                    if is_default_agent(&agent_name) {
                        *model.error_message_mut() =
                            format!("Cannot delete default agent: {agent_name}");
                        *model.mode_mut() = Mode::View;
                        return;
                    }
                    model.agents_mut().remove(idx);
                    *model.selected_agent_mut() =
                        model.selected_agent().min(model.agents().len().saturating_sub(1));
                    match persist_agents(model) {
                        Ok(path) => {
                            *model.status_message_mut() =
                                format!("Agent deleted and saved to {}", path.display());
                            model.error_message_mut().clear();
                        }
                        Err(err) => {
                            *model.error_message_mut() = err.to_string();
                        }
                    }
                }
            }

            *model.mode_mut() = Mode::View;
        }
        BareKey::Esc | BareKey::Char('n') | BareKey::Char('N') => {
            *model.mode_mut() = Mode::View;
        }
        _ => {}
    }
}

fn reset_status(model: &mut Model) {
    model.status_message_mut().clear();
    model.error_message_mut().clear();
}

fn cancel_to_view(model: &mut Model) {
    *model.mode_mut() = Mode::View;
    *model.quick_launch_agent_name_mut() = None;
    reset_status(model);
}

fn view_preserve_messages(model: &mut Model) {
    *model.mode_mut() = Mode::View;
}

fn start_new_pane_workspace(model: &mut Model) {
    model.workspace_input_mut().clear();
    *model.mode_mut() = Mode::NewPaneWorkspace;
    *model.wizard_agent_idx_mut() = 0;
    *model.wizard_tab_idx_mut() = 0;
    reset_status(model);
}

fn start_agent_create(model: &mut Model) {
    *model.mode_mut() = Mode::AgentFormCreate;
    *model.agent_form_source_mut() = Some(Mode::View);
    model.agent_name_input_mut().clear();
    model.agent_command_input_mut().clear();
    model.agent_env_input_mut().clear();
    model.agent_note_input_mut().clear();
    *model.agent_form_field_mut() = AgentFormField::Name;
    *model.form_target_agent_mut() = None;
    reset_status(model);
}

fn start_agent_edit(model: &mut Model) {
    if model.agents().is_empty() {
        *model.error_message_mut() = "no agents to edit".to_string();
        return;
    }
    let idx = model.selected_agent().min(model.agents().len().saturating_sub(1));
    if let Some(agent) = model.agents().get(idx) {
        let agent_name = agent.name.clone();
        let agent_command = agent.command.join(" ");
        let agent_env = agent
            .env
            .as_ref()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        let agent_note = agent.note.clone().unwrap_or_default();
        *model.agent_name_input_mut() = agent_name;
        *model.agent_command_input_mut() = agent_command;
        *model.agent_env_input_mut() = agent_env;
        *model.agent_note_input_mut() = agent_note;
        *model.agent_form_field_mut() = AgentFormField::Name;
        *model.form_target_agent_mut() = Some(idx);
        *model.agent_form_source_mut() = Some(Mode::View);
        *model.mode_mut() = Mode::AgentFormEdit;
        reset_status(model);
    }
}

fn start_agent_delete_confirm(model: &mut Model) {
    if model.agents().is_empty() {
        *model.error_message_mut() = "no agents to delete".to_string();
        return;
    }
    let idx = model.selected_agent().min(model.agents().len().saturating_sub(1));
    *model.form_target_agent_mut() = Some(idx);
    *model.mode_mut() = Mode::DeleteConfirm;
    reset_status(model);
}

fn move_selection(model: &mut Model, section: Section, delta: isize) {
    let (len, current) = match section {
        Section::AgentPanes => {
            let filter_lower = model.filter_text().to_lowercase();
            let panes_len = if filter_lower.is_empty() {
                model.agent_panes().len()
            } else {
                model
                    .agent_panes()
                    .iter()
                    .filter(|p| {
                        p.agent_name.to_lowercase().contains(&filter_lower)
                            || p.tab_name.to_lowercase().contains(&filter_lower)
                    })
                    .count()
            };
            (panes_len, model.selected_pane())
        }
        Section::Agents => (model.agents().len(), model.selected_agent()),
    };
    if len == 0 {
        return;
    }
    let mut next = current as isize + delta;
    if next < 0 {
        next = 0;
    }
    if next >= len as isize {
        next = len as isize - 1;
    }
    let next = next as usize;
    match section {
        Section::AgentPanes => *model.selected_pane_mut() = next,
        Section::Agents => *model.selected_agent_mut() = next,
    }
    model.status_message_mut().clear();
    model.error_message_mut().clear();
}

fn focus_next_section(model: &mut Model) {
    *model.focused_section_mut() = model.focused_section().next();
    model.status_message_mut().clear();
    model.error_message_mut().clear();
    model.clamp_selections();
}
