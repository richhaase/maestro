use zellij_tile::prelude::*;

use crate::agent::is_default_agent;
use crate::error::MaestroError;
use crate::model::Model;
use crate::ui::{next_field, prev_field, Mode};

use super::forms::{
    apply_agent_create, apply_agent_edit, build_agent_from_inputs, handle_form_text,
    handle_text_edit, persist_agents, start_agent_create, start_agent_delete_confirm,
    start_agent_edit, start_new_pane_workspace,
};
use super::panes::{
    derive_tab_name_from_workspace, focus_selected, kill_selected, spawn_agent_pane, TabChoice,
};

pub fn handle_key_event(model: &mut Model, key: KeyWithModifier) {
    match model.mode {
        Mode::View => handle_key_event_view(model, key),
        Mode::AgentConfig => handle_key_event_agent_config(model, key),
        Mode::NewPaneWorkspace => handle_key_event_new_pane_workspace(model, key),
        Mode::NewPaneAgentSelect => handle_key_event_new_pane_agent_select(model, key),
        Mode::AgentFormCreate | Mode::AgentFormEdit => handle_key_event_agent_form(model, key),
        Mode::DeleteConfirm => handle_key_event_delete_confirm(model, key),
    }
}

fn handle_key_event_view(model: &mut Model, key: KeyWithModifier) {
    match key.bare_key {
        BareKey::Down => {
            move_pane_selection(model, 1);
        }
        BareKey::Up => {
            move_pane_selection(model, -1);
        }
        BareKey::Enter => {
            let idx = model.selected_pane;
            focus_selected(model, idx);
        }
        BareKey::Esc => {
            close_self();
        }
        BareKey::Char('d') => {
            let idx = model.selected_pane;
            kill_selected(model, idx);
        }
        BareKey::Char('n') => {
            start_new_pane_workspace(model);
        }
        BareKey::Char('c') => {
            model.mode = Mode::AgentConfig;
            model.clear_error();
        }
        _ => {}
    }
}

fn handle_key_event_agent_config(model: &mut Model, key: KeyWithModifier) {
    match key.bare_key {
        BareKey::Down => {
            move_agent_selection(model, 1);
        }
        BareKey::Up => {
            move_agent_selection(model, -1);
        }
        BareKey::Char('a') => {
            start_agent_create(model);
        }
        BareKey::Char('e') => {
            if model.selected_agent < model.agents.len() {
                start_agent_edit(model);
            }
        }
        BareKey::Char('d') => {
            if model.selected_agent < model.agents.len() {
                start_agent_delete_confirm(model);
            }
        }
        BareKey::Esc => {
            model.mode = Mode::View;
            model.clear_error();
        }
        _ => {}
    }
}

fn handle_key_event_new_pane_workspace(model: &mut Model, key: KeyWithModifier) {
    if handle_text_edit(&mut model.pane_wizard.workspace, &key) {
        model.pane_wizard.browse_idx = 0;
        return;
    }

    let input = model.pane_wizard.workspace.clone();
    let suggestions = crate::utils::get_path_suggestions(&input);

    match key.bare_key {
        BareKey::Up => {
            if model.pane_wizard.browse_idx > 0 {
                model.pane_wizard.browse_idx -= 1;
            }
        }
        BareKey::Down => {
            let max_idx = suggestions.len().saturating_sub(1);
            if model.pane_wizard.browse_idx < max_idx {
                model.pane_wizard.browse_idx += 1;
            }
        }
        BareKey::Tab => {
            if let Some(suggestion) = suggestions.get(model.pane_wizard.browse_idx) {
                model.pane_wizard.workspace = suggestion.clone();
                model.pane_wizard.browse_idx = 0;
            }
        }
        BareKey::Enter => {
            if let Some(selected) = suggestions.get(model.pane_wizard.browse_idx) {
                model.pane_wizard.workspace = selected.clone();
            }
            // Always use workspace path as tab name - skip tab selection step
            let tab_name = derive_tab_name_from_workspace(&model.pane_wizard.workspace)
                .unwrap_or_else(|| crate::utils::default_tab_name(&model.pane_wizard.workspace));
            model.pane_wizard.tab_name = Some(tab_name);
            model.mode = Mode::NewPaneAgentSelect;
            model.pane_wizard.agent_filter = String::new();
            model.pane_wizard.agent_idx = 0;
            model.clear_error();
        }
        BareKey::Esc => cancel_to_view(model),
        _ => {}
    }
}

fn handle_key_event_new_pane_agent_select(model: &mut Model, key: KeyWithModifier) {
    // Handle text input for filtering
    if handle_text_edit(&mut model.pane_wizard.agent_filter, &key) {
        // Reset selection when filter changes
        model.pane_wizard.agent_idx = 0;
        return;
    }

    let filtered_indices =
        crate::utils::filter_agents_fuzzy(&model.agents, &model.pane_wizard.agent_filter);
    let filtered_count = filtered_indices.len();

    match key.bare_key {
        BareKey::Down => {
            if filtered_count > 0 && model.pane_wizard.agent_idx + 1 < filtered_count {
                model.pane_wizard.agent_idx += 1;
            }
        }
        BareKey::Up => {
            if model.pane_wizard.agent_idx > 0 {
                model.pane_wizard.agent_idx -= 1;
            }
        }
        BareKey::Enter => {
            let selection_idx = model.pane_wizard.agent_idx;
            if let Some(&agent_idx) = filtered_indices.get(selection_idx) {
                let agent = model.agents[agent_idx].name.clone();
                let workspace = model.pane_wizard.workspace.trim().to_string();
                let tab_name = model
                    .pane_wizard
                    .tab_name
                    .clone()
                    .unwrap_or_else(|| crate::utils::default_tab_name(&workspace));
                let tab_choice = if model.tab_names.contains(&tab_name) {
                    TabChoice::Existing(tab_name)
                } else {
                    TabChoice::New
                };
                spawn_agent_pane(model, workspace, agent, tab_choice);
                if model.error_message.is_empty() {
                    model.mode = Mode::View;
                }
            }
        }
        BareKey::Esc => cancel_to_view(model),
        _ => {}
    }
}

fn handle_key_event_agent_form(model: &mut Model, key: KeyWithModifier) {
    if handle_form_text(model, &key) {
        return;
    }
    match key.bare_key {
        BareKey::Down => {
            model.agent_form.field = next_field(model.agent_form.field);
        }
        BareKey::Up => {
            model.agent_form.field = prev_field(model.agent_form.field);
        }
        BareKey::Enter => match build_agent_from_inputs(model) {
            Ok(agent) => {
                let result = match model.mode {
                    Mode::AgentFormEdit => apply_agent_edit(model, agent),
                    Mode::AgentFormCreate => apply_agent_create(model, agent),
                    _ => Err(MaestroError::InvalidMode),
                };
                match result {
                    Ok(_) => {
                        model.mode = Mode::AgentConfig;
                    }
                    Err(err) => {
                        model.error_message = err.to_string();
                    }
                }
            }
            Err(err) => {
                model.error_message = err.to_string();
            }
        },
        BareKey::Esc => {
            model.agent_form.clear();
            model.mode = Mode::AgentConfig;
            model.clear_error();
        }
        _ => {}
    }
}

fn handle_key_event_delete_confirm(model: &mut Model, key: KeyWithModifier) {
    match key.bare_key {
        BareKey::Enter | BareKey::Char('y') | BareKey::Char('Y') => {
            if let Some(idx) = model.agent_form.target.take() {
                if idx < model.agents.len() {
                    let agent_name = model.agents[idx].name.clone();
                    if is_default_agent(&agent_name) {
                        model.error_message =
                            MaestroError::CannotDeleteDefaultAgent(agent_name).to_string();
                        model.mode = Mode::AgentConfig;
                        return;
                    }
                    model.agents.remove(idx);
                    model.selected_agent = model
                        .selected_agent
                        .min(model.agents.len().saturating_sub(1));
                    match persist_agents(model, None) {
                        Ok(_) => {
                            model.clear_error();
                        }
                        Err(err) => {
                            model.error_message = err.to_string();
                        }
                    }
                }
            }

            model.mode = Mode::AgentConfig;
        }
        BareKey::Esc | BareKey::Char('n') | BareKey::Char('N') => {
            model.mode = Mode::AgentConfig;
        }
        _ => {}
    }
}

fn cancel_to_view(model: &mut Model) {
    model.mode = Mode::View;
    model.pane_wizard.clear();
    model.clear_error();
}

fn move_pane_selection(model: &mut Model, delta: isize) {
    let len = model.agent_panes.len();
    if len == 0 {
        return;
    }
    let current = model.selected_pane as isize;
    let next = (current + delta).clamp(0, len as isize - 1) as usize;
    model.selected_pane = next;
}

fn move_agent_selection(model: &mut Model, delta: isize) {
    let len = model.agents.len();
    if len == 0 {
        return;
    }
    let current = model.selected_agent as isize;
    let next = (current + delta).clamp(0, len as isize - 1) as usize;
    model.selected_agent = next;
}
