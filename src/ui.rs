use zellij_tile::ui_components::{serialize_table, Table, Text};

use crate::agent::PaneStatus;
use crate::model::Model;
use crate::utils::truncate;
use crate::WASI_HOST_MOUNT;

const COLOR_GREEN: usize = 2;
const COLOR_RED: usize = 1;
const MAX_SUGGESTIONS_DISPLAYED: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    View,
    AgentConfig,
    NewPaneWorkspace,
    NewPaneAgentSelect,
    AgentFormCreate,
    AgentFormEdit,
    DeleteConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentFormField {
    #[default]
    Name,
    Command,
    Args,
    Note,
}

/// Get the next field in the form navigation cycle
pub fn next_field(current: AgentFormField) -> AgentFormField {
    match current {
        AgentFormField::Name => AgentFormField::Command,
        AgentFormField::Command => AgentFormField::Args,
        AgentFormField::Args => AgentFormField::Note,
        AgentFormField::Note => AgentFormField::Name,
    }
}

/// Get the previous field in the form navigation cycle
pub fn prev_field(current: AgentFormField) -> AgentFormField {
    match current {
        AgentFormField::Name => AgentFormField::Note,
        AgentFormField::Command => AgentFormField::Name,
        AgentFormField::Args => AgentFormField::Command,
        AgentFormField::Note => AgentFormField::Args,
    }
}

/// Render the main UI
pub fn render_permissions_denied(rows: usize, cols: usize) -> String {
    format!(
        "Maestro: permissions denied.\nGrant the requested permissions and reload.\nViewport: {cols}x{rows}"
    )
}

pub fn render_permissions_requesting(rows: usize, cols: usize) -> String {
    format!("Maestro requesting permissions...\nViewport: {cols}x{rows}")
}

pub fn render_ui(model: &Model, _rows: usize, cols: usize) -> String {
    let mut out = String::new();

    out.push_str(&render_agent_panes(model, cols));

    if let Some(overlay) = render_overlay(model, cols) {
        out.push('\n');
        out.push_str(&overlay);
    }
    out.push('\n');
    out.push_str(&render_status(model, cols));
    out
}

fn render_agent_panes(model: &Model, cols: usize) -> String {
    let mut table = Table::new().add_row(vec!["Tab", "Agent", "Status"]);

    for (idx, pane) in model.agent_panes().iter().enumerate() {
        let tab = truncate(&pane.tab_name, cols.saturating_sub(20));
        let agent = if pane.agent_name.is_empty() {
            "(agent)"
        } else {
            &pane.agent_name
        };
        let status_text = match pane.status {
            PaneStatus::Running => "RUNNING",
            PaneStatus::Exited(_) => "EXITED",
        };

        let status_color = match pane.status {
            PaneStatus::Running => COLOR_GREEN,
            PaneStatus::Exited(_) => COLOR_RED,
        };

        let mut row = vec![
            Text::new(tab),
            Text::new(agent.to_string()),
            Text::new(status_text.to_string()).color_all(status_color),
        ];

        if idx == model.selected_pane() {
            row = row.into_iter().map(|t| t.selected()).collect();
        }

        table = table.add_styled_row(row);
    }
    if model.agent_panes().is_empty() {
        table = table.add_row(vec![
            "(no agent panes)".to_string(),
            "".to_string(),
            "".to_string(),
        ]);
    }
    serialize_table(&table)
}

fn render_agent_management(model: &Model, cols: usize) -> String {
    let mut table = Table::new().add_row(vec!["Agent", "Command", "Note"]);

    let command_col_width = (cols as f32 * 0.50) as usize;

    for (idx, agent) in model.agents().iter().enumerate() {
        let name = if agent.name.is_empty() {
            "(agent)"
        } else {
            &agent.name
        };
        let command_full = if let Some(args) = &agent.args {
            format!("{} {}", agent.command, args.join(" "))
        } else {
            agent.command.clone()
        };
        let command = truncate(&command_full, command_col_width);

        let note = agent
            .note
            .as_deref()
            .filter(|n| !n.is_empty())
            .unwrap_or("—");

        let row = vec![name.to_string(), command.to_string(), note.to_string()];
        let styled = if idx == model.selected_agent() {
            row.into_iter().map(|c| Text::new(c).selected()).collect()
        } else {
            row.into_iter().map(Text::new).collect()
        };
        table = table.add_styled_row(styled);
    }

    if model.agents().is_empty() {
        table = table.add_row(vec![
            "(no agents)".to_string(),
            "".to_string(),
            "".to_string(),
        ]);
    }

    serialize_table(&table)
}

fn render_overlay(model: &Model, cols: usize) -> Option<String> {
    match model.mode() {
        Mode::View => None,
        Mode::AgentConfig => {
            let lines = [
                "Agent Configuration".to_string(),
                "".to_string(),
                render_agent_management(model, cols),
            ];
            Some(lines.join("\n"))
        }
        Mode::NewPaneWorkspace => {
            let mut lines = Vec::new();
            let input = model.workspace_input();
            let host_prefix = format!("{}/", WASI_HOST_MOUNT);
            let display_input = input.strip_prefix(&host_prefix).unwrap_or(input);
            lines.push("New Agent Pane: workspace path".to_string());
            lines.push(format!(
                "> {}_",
                truncate(display_input, cols.saturating_sub(2))
            ));

            if !input.trim().is_empty() {
                let suggestions = crate::utils::get_path_suggestions(input);
                if !suggestions.is_empty() {
                    lines.push("".to_string());
                    let max_display = MAX_SUGGESTIONS_DISPLAYED;
                    let start_idx = if model.browse_selected_idx() < max_display {
                        0
                    } else {
                        model.browse_selected_idx().saturating_sub(max_display - 1)
                    };
                    let end_idx = (start_idx + max_display).min(suggestions.len());

                    for (display_idx, suggestion) in
                        suggestions[start_idx..end_idx].iter().enumerate()
                    {
                        let actual_idx = start_idx + display_idx;
                        let prefix = if actual_idx == model.browse_selected_idx() {
                            ">"
                        } else {
                            " "
                        };
                        let display_path = suggestion.strip_prefix(&host_prefix).unwrap_or(suggestion);
                        lines.push(format!(
                            "{} {}",
                            prefix,
                            truncate(display_path, cols.saturating_sub(2))
                        ));
                    }

                    if suggestions.len() > max_display {
                        let showing = end_idx - start_idx;
                        lines.push(format!("... showing {} of {}", showing, suggestions.len()));
                    }
                }
            }

            Some(lines.join("\n"))
        }
        Mode::NewPaneAgentSelect => {
            let mut lines = Vec::new();

            // Show filter input
            let filter = model.wizard_agent_filter();
            if filter.is_empty() {
                lines.push("Select agent (type to filter):".to_string());
            } else {
                lines.push(format!("Filter: {}_", filter));
            }

            // Get filtered agent indices
            let filtered_indices =
                crate::utils::filter_agents_fuzzy(model.agents(), filter);

            if filtered_indices.is_empty() {
                lines.push("  (no matching agents)".to_string());
            } else {
                for (display_idx, &agent_idx) in filtered_indices.iter().enumerate() {
                    let prefix = if display_idx == model.wizard_agent_idx() {
                        ">"
                    } else {
                        " "
                    };
                    let agent = &model.agents()[agent_idx];
                    lines.push(format!(
                        "{} {}",
                        prefix,
                        truncate(&agent.name, cols.saturating_sub(2))
                    ));
                }
            }

            Some(lines.join("\n"))
        }
        Mode::AgentFormCreate => Some(render_agent_form_overlay(
            model,
            "Add agent (not yet persisted)",
            cols,
        )),
        Mode::AgentFormEdit => Some(render_agent_form_overlay(
            model,
            "Edit agent (not yet persisted)",
            cols,
        )),
        Mode::DeleteConfirm => {
            let name = model
                .form_target_agent()
                .and_then(|idx| model.agents().get(idx))
                .map(|a| a.name.as_str())
                .unwrap_or("(unknown)");
            Some(format!(
                "Delete agent \"{name}\"? Enter/y to delete, Esc/n to cancel"
            ))
        }
    }
}

fn render_agent_form_overlay(model: &Model, title: &str, cols: usize) -> String {
    let mut lines = Vec::new();
    lines.push(title.to_string());
    let mk = |label: &str, val: &str, field: AgentFormField, current: AgentFormField| {
        let marker = if field == current { ">" } else { " " };
        format!(
            "{marker} {label}: {}",
            truncate(val, cols.saturating_sub(label.len() + 4))
        )
    };
    lines.push(mk(
        "Name",
        model.agent_name_input(),
        AgentFormField::Name,
        model.agent_form_field(),
    ));
    lines.push(mk(
        "Command",
        model.agent_command_input(),
        AgentFormField::Command,
        model.agent_form_field(),
    ));
    lines.push(mk(
        "Args",
        model.agent_args_input(),
        AgentFormField::Args,
        model.agent_form_field(),
    ));
    lines.push(mk(
        "Note",
        model.agent_note_input(),
        AgentFormField::Note,
        model.agent_form_field(),
    ));
    lines.join("\n")
}

fn render_status(model: &Model, cols: usize) -> String {
    let hints = match model.mode() {
        Mode::View => "↑/↓ move • Enter focus • d kill • n new • c config • Esc close",
        Mode::AgentConfig => "↑/↓ move • a add • e edit • d delete • Esc back",
        Mode::NewPaneWorkspace => "↑/↓ select • Enter continue • Esc cancel",
        Mode::NewPaneAgentSelect => "type to filter • ↑/↓ move • Enter select • Esc cancel",
        Mode::AgentFormCreate | Mode::AgentFormEdit => "[Tab] next field • Enter save • Esc cancel",
        Mode::DeleteConfirm => "[Enter/y] confirm • [Esc/n] cancel",
    };
    let msg = if !model.error_message().is_empty() {
        format!("ERROR: {}", model.error_message())
    } else {
        hints.to_string()
    };
    truncate(&msg, cols)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_field() {
        assert_eq!(next_field(AgentFormField::Name), AgentFormField::Command);
        assert_eq!(next_field(AgentFormField::Command), AgentFormField::Args);
        assert_eq!(next_field(AgentFormField::Args), AgentFormField::Note);
        assert_eq!(next_field(AgentFormField::Note), AgentFormField::Name);
    }

    #[test]
    fn test_prev_field() {
        assert_eq!(prev_field(AgentFormField::Name), AgentFormField::Note);
        assert_eq!(prev_field(AgentFormField::Command), AgentFormField::Name);
        assert_eq!(prev_field(AgentFormField::Args), AgentFormField::Command);
        assert_eq!(prev_field(AgentFormField::Note), AgentFormField::Args);
    }
}
