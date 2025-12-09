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
use super::panes::{derive_tab_name_from_workspace, focus_selected, kill_selected, spawn_agent_pane, TabChoice};

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
    if !key.key_modifiers.is_empty() {
        return;
    }

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
            close_self();
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
            reset_status(model);
        }
        _ => {}
    }
}

fn handle_key_event_new_pane_workspace(model: &mut Model, key: KeyWithModifier) {
    if handle_text_edit(&mut model.workspace_input, &key) {
        model.browse_selected_idx = 0;
        return;
    }

    let input = model.workspace_input.clone();
    let suggestions = crate::utils::get_path_suggestions(&input);

    match key.bare_key {
        BareKey::Up => {
            if model.browse_selected_idx > 0 {
                model.browse_selected_idx -= 1;
            }
        }
        BareKey::Down => {
            let max_idx = suggestions.len().saturating_sub(1);
            if model.browse_selected_idx < max_idx {
                model.browse_selected_idx += 1;
            }
        }
        BareKey::Tab => {
            if let Some(suggestion) = suggestions.get(model.browse_selected_idx) {
                model.workspace_input = suggestion.clone();
                model.browse_selected_idx = 0;
            }
        }
        BareKey::Enter => {
            if let Some(selected) = suggestions.get(model.browse_selected_idx) {
                model.workspace_input = selected.clone();
            }
            // Always use workspace path as tab name - skip tab selection step
            let tab_name = derive_tab_name_from_workspace(&model.workspace_input)
                .unwrap_or_else(|| crate::utils::default_tab_name(&model.workspace_input));
            model.custom_tab_name = Some(tab_name);
            model.mode = Mode::NewPaneAgentSelect;
            model.wizard_agent_filter = String::new();
            model.wizard_agent_idx = 0;
            reset_status(model);
        }
        BareKey::Esc => cancel_to_view(model),
        _ => {}
    }
}

fn handle_key_event_new_pane_agent_select(model: &mut Model, key: KeyWithModifier) {
    // Handle text input for filtering
    if handle_text_edit(&mut model.wizard_agent_filter, &key) {
        // Reset selection when filter changes
        model.wizard_agent_idx = 0;
        return;
    }

    let filtered_indices =
        crate::utils::filter_agents_fuzzy(&model.agents, &model.wizard_agent_filter);
    let filtered_count = filtered_indices.len();

    match key.bare_key {
        BareKey::Down => {
            if filtered_count > 0 && model.wizard_agent_idx + 1 < filtered_count {
                model.wizard_agent_idx += 1;
            }
        }
        BareKey::Up => {
            if model.wizard_agent_idx > 0 {
                model.wizard_agent_idx -= 1;
            }
        }
        BareKey::Enter => {
            let selection_idx = model.wizard_agent_idx;
            if let Some(&agent_idx) = filtered_indices.get(selection_idx) {
                let agent = model.agents[agent_idx].name.clone();
                let workspace = model.workspace_input.trim().to_string();
                let tab_name = model
                    .custom_tab_name
                    .clone()
                    .unwrap_or_else(|| crate::utils::default_tab_name(&workspace));
                let tab_choice = if model.tab_names.contains(&tab_name) {
                    TabChoice::Existing(tab_name)
                } else {
                    TabChoice::New
                };
                spawn_agent_pane(model, workspace, agent, tab_choice);
                if model.error_message.is_empty() {
                    view_preserve_messages(model);
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
    let shift_tab = key.bare_key == BareKey::Tab && key.key_modifiers.contains(&KeyModifier::Shift);
    match key.bare_key {
        BareKey::Tab if shift_tab => {
            model.agent_form_field = prev_field(model.agent_form_field);
        }
        BareKey::Tab => {
            model.agent_form_field = next_field(model.agent_form_field);
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
                        view_preserve_messages(model);
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
            model.agent_form_source = None;
            cancel_to_view(model);
        }
        _ => {}
    }
}

fn handle_key_event_delete_confirm(model: &mut Model, key: KeyWithModifier) {
    match key.bare_key {
        BareKey::Enter | BareKey::Char('y') | BareKey::Char('Y') => {
            if let Some(idx) = model.form_target_agent.take() {
                if idx < model.agents.len() {
                    let agent_name = model.agents[idx].name.clone();
                    if is_default_agent(&agent_name) {
                        model.error_message =
                            MaestroError::CannotDeleteDefaultAgent(agent_name).to_string();
                        model.mode = Mode::View;
                        return;
                    }
                    model.agents.remove(idx);
                    model.selected_agent = model
                        .selected_agent
                        .min(model.agents.len().saturating_sub(1));
                    match persist_agents(model) {
                        Ok(_) => {
                            model.error_message.clear();
                        }
                        Err(err) => {
                            model.error_message = err.to_string();
                        }
                    }
                }
            }

            model.mode = Mode::View;
        }
        BareKey::Esc | BareKey::Char('n') | BareKey::Char('N') => {
            model.mode = Mode::View;
        }
        _ => {}
    }
}

fn reset_status(model: &mut Model) {
    model.error_message.clear();
}

fn cancel_to_view(model: &mut Model) {
    model.mode = Mode::View;
    model.quick_launch_agent_name = None;
    model.custom_tab_name = None;
    model.wizard_agent_filter = String::new();
    reset_status(model);
}

fn view_preserve_messages(model: &mut Model) {
    model.mode = Mode::View;
}

fn move_pane_selection(model: &mut Model, delta: isize) {
    let len = model.agent_panes.len();
    if len == 0 {
        return;
    }
    let current = model.selected_pane as isize;
    let next = (current + delta).clamp(0, len as isize - 1) as usize;
    model.selected_pane = next;
    model.error_message.clear();
}

fn move_agent_selection(model: &mut Model, delta: isize) {
    let len = model.agents.len();
    if len == 0 {
        return;
    }
    let current = model.selected_agent as isize;
    let next = (current + delta).clamp(0, len as isize - 1) as usize;
    model.selected_agent = next;
    model.error_message.clear();
}
