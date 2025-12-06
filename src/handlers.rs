use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow;
use uuid::Uuid;
use zellij_tile::prelude::*;
use zellij_tile::prelude::{
    BareKey, KeyModifier, KeyWithModifier, PaneId, PaneManifest, PermissionStatus, TabInfo,
};

use crate::agent::{default_config_path, is_default_agent, save_agents};
use crate::agent::{Agent, AgentPane, PaneStatus};
use crate::error::{MaestroError, MaestroResult};
use crate::model::Model;
use crate::ui::{next_field, prev_field, AgentFormField, Mode};
use crate::utils::{
    build_command_with_env, find_agent_by_command, parse_env_input, workspace_basename,
};

#[derive(Debug, Clone, PartialEq)]
pub enum TabChoice {
    Existing(String),
    New,
}

fn derive_tab_name_from_workspace(input: &str) -> Option<String> {
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

pub fn apply_tab_update(model: &mut Model, mut tabs: Vec<TabInfo>) {
    tabs.sort_by_key(|t| t.position);
    let tab_names: Vec<String> = tabs.iter().map(|t| t.name.clone()).collect();
    *model.tab_names_mut() = tab_names.clone();

    // Resolve pending_tab_index to actual tab names
    // Only update if current tab_name is empty or no longer exists in the tab list
    for pane in model.agent_panes_mut() {
        if let Some(idx) = pane.pending_tab_index {
            if let Some(name) = tab_names.get(idx) {
                // Only update if we don't have a valid tab name yet
                if pane.tab_name.is_empty() || !tab_names.contains(&pane.tab_name) {
                    pane.tab_name = name.clone();
                }
            }
        }
    }

    // Only retain panes that either have a pane_id or have a valid tab_name or have a pending index
    model
        .agent_panes_mut()
        .retain(|p| p.pane_id.is_some() || tab_names.contains(&p.tab_name) || p.pending_tab_index.is_some());
    model.clamp_selections();
}

pub fn apply_pane_update(model: &mut Model, update: PaneManifest) {
    for (tab_idx, pane_list) in update.panes {
        let tab_name_from_idx = model.tab_names().get(tab_idx).cloned().unwrap_or_default();

        for pane in pane_list {
            if let Some(existing) = model
                .agent_panes_mut()
                .iter_mut()
                .find(|p| p.pane_id == Some(pane.id))
            {
                // Update status only - never touch tab_name for existing matched panes
                // The tab_name was set when the pane was spawned and should be preserved
                existing.status = if pane.exited {
                    PaneStatus::Exited(pane.exit_status)
                } else {
                    PaneStatus::Running
                };
                continue;
            }

            // This is a new pane we haven't seen before (discovered via PaneUpdate)
            let title = pane.title.clone();
            let command_hint = pane.terminal_command.as_deref().unwrap_or(&title);

            if !pane.is_plugin {
                if let Some(agent) = find_agent_by_command(model.agents(), command_hint) {
                    let agent_name = agent.name.clone();
                    model.agent_panes_mut().push(AgentPane {
                        pane_title: title,
                        tab_name: tab_name_from_idx.clone(),
                        pending_tab_index: if tab_name_from_idx.is_empty() { Some(tab_idx) } else { None },
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
        .unwrap_or_else(|| format!("pane:{pane_id}"));
    let workspace_path = ctx.get("cwd").cloned().unwrap_or_default();
    let agent_name = ctx.get("agent").cloned().unwrap_or_default();

    let tab_names_snapshot = model.tab_names().to_vec();
    let first_tab = tab_names_snapshot.first().cloned();
    let ctx_tab_name = ctx.get("tab_name").cloned();
    let entry = model
        .agent_panes_mut()
        .iter_mut()
        .find(|p| p.pane_id == Some(pane_id) || (p.pane_id.is_none() && p.pane_title == title));

    if let Some(existing) = entry {
        existing.pane_id = Some(pane_id);
        existing.pane_title = title.clone();

        if existing.tab_name.is_empty() {
            if let Some(tab_name) = ctx_tab_name.clone() {
                existing.tab_name = tab_name;
                existing.pending_tab_index = None;
            } else if let Some(first_tab) = first_tab {
                existing.tab_name = first_tab;
                existing.pending_tab_index = None;
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
        let tab_name = ctx_tab_name
            .clone()
            .or_else(|| tab_names_snapshot.first().cloned())
            .unwrap_or_default();
        model.agent_panes_mut().push(AgentPane {
            pane_title: title,
            tab_name,
            pending_tab_index: None,
            pane_id: Some(pane_id),
            workspace_path,
            agent_name,
            status: PaneStatus::Running,
        });
    }
    model.clamp_selections();
}

fn rebuild_from_session_infos(model: &mut Model, session_infos: &[SessionInfo]) {
    for session in session_infos {
        let session_name = session.name.clone();
        if let Some(current) = model.session_name() {
            if &session_name != current {
                continue;
            }
        } else {
            *model.session_name_mut() = Some(session_name.clone());
        }

        // Build tab lookup from session info
        let mut tab_lookup = BTreeMap::new();
        for tab in &session.tabs {
            tab_lookup.insert(tab.position, tab.name.clone());
        }

        for (tab_idx, pane_list) in session.panes.clone().panes {
            let tab_name_from_idx = tab_lookup.get(&tab_idx).cloned().unwrap_or_default();

            let mut unmatched_in_tab: Vec<usize> = if !tab_name_from_idx.is_empty() {
                model
                    .agent_panes()
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.pane_id.is_none() && p.tab_name == tab_name_from_idx)
                    .map(|(idx, _)| idx)
                    .collect()
            } else {
                Vec::new()
            };

            for pane in pane_list {
                if let Some(existing) = model
                    .agent_panes_mut()
                    .iter_mut()
                    .find(|p| p.pane_id == Some(pane.id))
                {
                    // Update status only - preserve tab_name
                    existing.status = if pane.exited {
                        PaneStatus::Exited(pane.exit_status)
                    } else {
                        PaneStatus::Running
                    };
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
                    // tab_name already set, don't overwrite
                    continue;
                }

                // New pane discovered via session info
                if !pane.is_plugin {
                    let command_hint = pane.terminal_command.as_deref().unwrap_or(&pane.title);
                    if let Some(agent) = find_agent_by_command(model.agents(), command_hint) {
                        let agent_name = agent.name.clone();
                        model.agent_panes_mut().push(AgentPane {
                            pane_title: pane.title.clone(),
                            tab_name: tab_name_from_idx.clone(),
                            pending_tab_index: if tab_name_from_idx.is_empty() { Some(tab_idx) } else { None },
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
        .unwrap_or_else(|| format!("pane:{pane_id}"));
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
        .unwrap_or_else(|| format!("pane:{pane_id}"));
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
    let workspace_label = workspace_basename(&workspace_path);
    let title_label = if workspace_label.is_empty() {
        agent.name.clone()
    } else {
        workspace_label
    };
    let title = format!("{}:{}", title_label, Uuid::new_v4());
    let tab_name = match &tab_choice {
        TabChoice::Existing(name) => name.clone(),
        TabChoice::New => {
            model
                .custom_tab_name()
                .filter(|s| !s.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| crate::utils::default_tab_name(&workspace_path))
        }
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
    let cmd = build_command_with_env(&agent);
    let mut ctx = BTreeMap::new();
    ctx.insert("pane_title".to_string(), title.clone());
    if let Some(ref resolved) = resolved_workspace {
        ctx.insert("cwd".to_string(), resolved.to_string_lossy().to_string());
    }
    ctx.insert("agent".to_string(), agent.name.clone());
    ctx.insert("tab_name".to_string(), tab_target.clone());

    let mut command_to_run = CommandToRun::new(cmd.join(" "));
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

    if selected_idx >= model.agent_panes().len() {
        *model.error_message_mut() = "no agent panes".to_string();
        return;
    }
    let pane = &model.agent_panes()[selected_idx];
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

    if selected_idx >= model.agent_panes().len() {
        *model.error_message_mut() = "no agent panes".to_string();
        return;
    }
    let pane = &model.agent_panes()[selected_idx];
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
    let env = parse_env_input(model.agent_env_input()).map_err(MaestroError::EnvParse)?;
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

pub fn handle_key_event(model: &mut Model, key: KeyWithModifier) {
    match model.mode() {
        Mode::View => handle_key_event_view(model, key),
        Mode::AgentConfig => handle_key_event_agent_config(model, key),
        Mode::NewPaneWorkspace => handle_key_event_new_pane_workspace(model, key),
        Mode::NewPaneAgentSelect => handle_key_event_new_pane_agent_select(model, key),
        Mode::NewPaneAgentCreate => handle_key_event_agent_form(model, key, true),
        Mode::AgentFormCreate | Mode::AgentFormEdit => {
            handle_key_event_agent_form(model, key, false)
        }
        Mode::DeleteConfirm => handle_key_event_delete_confirm(model, key),
    }
}

fn handle_key_event_view(model: &mut Model, key: KeyWithModifier) {
    if !key.key_modifiers.is_empty() {
        return;
    }

    match key.bare_key {
        BareKey::Char('j') | BareKey::Down => {
            move_pane_selection(model, 1);
        }
        BareKey::Char('k') | BareKey::Up => {
            move_pane_selection(model, -1);
        }
        BareKey::Enter => {
            let idx = model.selected_pane();
            focus_selected(model, idx);
            close_self();
        }
        BareKey::Esc => {
            close_self();
        }
        BareKey::Char('d') => {
            let idx = model.selected_pane();
            kill_selected(model, idx);
        }
        BareKey::Char('n') => {
            start_new_pane_workspace(model);
        }
        BareKey::Char('c') => {
            *model.mode_mut() = Mode::AgentConfig;
            reset_status(model);
        }
        _ => {}
    }
}

fn handle_key_event_agent_config(model: &mut Model, key: KeyWithModifier) {
    if !key.key_modifiers.is_empty() {
        return;
    }

    match key.bare_key {
        BareKey::Char('j') | BareKey::Down => {
            move_agent_selection(model, 1);
        }
        BareKey::Char('k') | BareKey::Up => {
            move_agent_selection(model, -1);
        }
        BareKey::Char('a') => {
            start_agent_create(model);
        }
        BareKey::Char('e') => {
            if model.selected_agent() < model.agents().len() {
                start_agent_edit(model);
            }
        }
        BareKey::Char('d') => {
            if model.selected_agent() < model.agents().len() {
                start_agent_delete_confirm(model);
            }
        }
        BareKey::Esc => {
            *model.mode_mut() = Mode::View;
            reset_status(model);
        }
        _ => {}
    }
}

fn handle_key_event_new_pane_workspace(model: &mut Model, key: KeyWithModifier) {
    if handle_text_edit(model.workspace_input_mut(), &key) {
        *model.browse_selected_idx_mut() = 0;
        return;
    }

    let input = model.workspace_input().to_string();
    let suggestions = crate::utils::get_path_suggestions(&input);

    match key.bare_key {
        BareKey::Up => {
            if model.browse_selected_idx() > 0 {
                *model.browse_selected_idx_mut() = model.browse_selected_idx() - 1;
            }
        }
        BareKey::Down => {
            let max_idx = suggestions.len().saturating_sub(1);
            if model.browse_selected_idx() < max_idx {
                *model.browse_selected_idx_mut() = model.browse_selected_idx() + 1;
            }
        }
        BareKey::Tab => {
            if let Some(suggestion) = suggestions.get(model.browse_selected_idx()) {
                *model.workspace_input_mut() = suggestion.clone();
                *model.browse_selected_idx_mut() = 0;
            }
        }
        BareKey::Enter => {
            if let Some(selected) = suggestions.get(model.browse_selected_idx()) {
                *model.workspace_input_mut() = selected.clone();
            }
            // Always use workspace path as tab name - skip tab selection step
            let tab_name = derive_tab_name_from_workspace(model.workspace_input())
                .unwrap_or_else(|| crate::utils::default_tab_name(model.workspace_input()));
            *model.custom_tab_name_mut() = Some(tab_name);
            *model.mode_mut() = Mode::NewPaneAgentSelect;
            *model.wizard_agent_filter_mut() = String::new();
            *model.wizard_agent_idx_mut() = 0;
            reset_status(model);
        }
        BareKey::Esc => cancel_to_view(model),
        _ => {}
    }
}

fn handle_key_event_new_pane_agent_select(model: &mut Model, key: KeyWithModifier) {
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;

    if handle_text_edit(model.wizard_agent_filter_mut(), &key) {
        *model.wizard_agent_idx_mut() = 0;
        return;
    }

    let filter_text = model.wizard_agent_filter();
    let filtered_agents: Vec<(usize, &Agent)> = if filter_text.is_empty() {
        model.agents().iter().enumerate().collect()
    } else {
        let matcher = SkimMatcherV2::default();
        model
            .agents()
            .iter()
            .enumerate()
            .filter(|(_, agent)| matcher.fuzzy_match(&agent.name, filter_text).is_some())
            .collect()
    };

    let has_exact_match = filtered_agents
        .iter()
        .any(|(_, agent)| agent.name.eq_ignore_ascii_case(filter_text));
    let show_new_agent = !filter_text.is_empty() && !has_exact_match;
    let choices = filtered_agents.len()
        + if show_new_agent || filter_text.is_empty() {
            1
        } else {
            0
        };

    match key.bare_key {
        BareKey::Up => {
            if model.wizard_agent_idx() > 0 {
                *model.wizard_agent_idx_mut() = model.wizard_agent_idx() - 1;
            }
        }
        BareKey::Down => {
            if model.wizard_agent_idx() + 1 < choices {
                *model.wizard_agent_idx_mut() = model.wizard_agent_idx() + 1;
            }
        }
        BareKey::Enter => {
            let idx = model.wizard_agent_idx();
            let filter_text = model.wizard_agent_filter().to_string();
            if idx < filtered_agents.len() {
                let (original_idx, _) = filtered_agents[idx];
                let agent = model.agents()[original_idx].name.clone();
                let workspace = model.workspace_input().trim().to_string();
                let tab_name = model.custom_tab_name().cloned()
                    .unwrap_or_else(|| crate::utils::default_tab_name(&workspace));
                let tab_choice = if model.tab_names().contains(&tab_name) {
                    TabChoice::Existing(tab_name)
                } else {
                    TabChoice::New
                };
                spawn_agent_pane(model, workspace, agent, tab_choice);
                if model.error_message().is_empty() {
                    view_preserve_messages(model);
                }
            } else {
                if !filter_text.trim().is_empty() {
                    *model.agent_name_input_mut() = filter_text.trim().to_string();
                } else {
                    model.agent_name_input_mut().clear();
                }
                *model.mode_mut() = Mode::NewPaneAgentCreate;
                *model.agent_form_source_mut() = Some(Mode::NewPaneAgentSelect);
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
    let shift_tab = key.bare_key == BareKey::Tab && key.key_modifiers.contains(&KeyModifier::Shift);
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
                            let tab_name = model.custom_tab_name().cloned()
                                .unwrap_or_else(|| crate::utils::default_tab_name(&workspace));
                            let tab_choice = if model.tab_names().contains(&tab_name) {
                                TabChoice::Existing(tab_name)
                            } else {
                                TabChoice::New
                            };
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
                    *model.selected_agent_mut() = model
                        .selected_agent()
                        .min(model.agents().len().saturating_sub(1));
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
    *model.custom_tab_name_mut() = None;
    *model.wizard_agent_filter_mut() = String::new();
    reset_status(model);
}

fn view_preserve_messages(model: &mut Model) {
    *model.mode_mut() = Mode::View;
}

fn start_new_pane_workspace(model: &mut Model) {
    model.workspace_input_mut().clear();
    *model.custom_tab_name_mut() = None;
    *model.wizard_agent_filter_mut() = String::new();
    *model.browse_selected_idx_mut() = 0;
    *model.mode_mut() = Mode::NewPaneWorkspace;
    *model.wizard_agent_idx_mut() = 0;
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
    let idx = model
        .selected_agent()
        .min(model.agents().len().saturating_sub(1));
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
    let idx = model
        .selected_agent()
        .min(model.agents().len().saturating_sub(1));
    *model.form_target_agent_mut() = Some(idx);
    *model.mode_mut() = Mode::DeleteConfirm;
    reset_status(model);
}

fn move_pane_selection(model: &mut Model, delta: isize) {
    let len = model.agent_panes().len();
    if len == 0 {
        return;
    }
    let current = model.selected_pane() as isize;
    let next = (current + delta).clamp(0, len as isize - 1) as usize;
    *model.selected_pane_mut() = next;
    model.status_message_mut().clear();
    model.error_message_mut().clear();
}

fn move_agent_selection(model: &mut Model, delta: isize) {
    let len = model.agents().len();
    if len == 0 {
        return;
    }
    let current = model.selected_agent() as isize;
    let next = (current + delta).clamp(0, len as isize - 1) as usize;
    *model.selected_agent_mut() = next;
    model.status_message_mut().clear();
    model.error_message_mut().clear();
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
    fn test_derive_tab_name_from_workspace_relative() {
        let derived = derive_tab_name_from_workspace("src/maestro");
        assert_eq!(derived, Some("src/maestro".to_string()));
    }

    #[test]
    fn test_derive_tab_name_from_workspace_host_prefix() {
        let derived = derive_tab_name_from_workspace("/host/src/maestro");
        assert_eq!(derived, Some("src/maestro".to_string()));
    }

    #[test]
    fn test_handle_text_edit_char() {
        let mut target = String::new();
        let key = char_key('a');
        assert!(handle_text_edit(&mut target, &key));
        assert_eq!(target, "a");
    }

    #[test]
    fn test_handle_command_pane_opened_uses_ctx_tab_name() {
        let mut model = create_test_model();
        model.tab_names_mut().extend([
            "Tab #1".to_string(),
            "src/plonk".to_string(),
        ]);
        model.agent_panes_mut().push(AgentPane {
            pane_title: "pane:1".to_string(),
            tab_name: String::new(),
            pending_tab_index: None,
            pane_id: None,
            workspace_path: String::new(),
            agent_name: String::new(),
            status: PaneStatus::Running,
        });
        let mut ctx = BTreeMap::new();
        ctx.insert("pane_title".to_string(), "pane:1".to_string());
        ctx.insert("tab_name".to_string(), "src/plonk".to_string());
        handle_command_pane_opened(&mut model, 1, ctx);
        assert_eq!(model.agent_panes()[0].tab_name, "src/plonk");
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
        assert!(matches!(
            result.unwrap_err(),
            MaestroError::AgentNameRequired
        ));
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
