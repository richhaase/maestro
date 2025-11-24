use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use zellij_tile::ui_components::{serialize_ribbon_line, serialize_table, Table, Text};

use crate::agent::{AgentPane, PaneStatus};
use crate::model::Model;
use crate::utils::truncate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Section {
    #[default]
    AgentPanes,
    Agents,
}

impl Section {
    pub fn next(self) -> Self {
        match self {
            Section::AgentPanes => Section::Agents,
            Section::Agents => Section::AgentPanes,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Section::AgentPanes => "Maestro",
            Section::Agents => "Agents",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    View,
    NewPaneWorkspace,
    NewPaneTabSelect,
    NewPaneAgentSelect,
    NewPaneAgentCreate,
    AgentFormCreate,
    AgentFormEdit,
    DeleteConfirm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentFormField {
    #[default]
    Name,
    Command,
    Env,
    Note,
}

/// Get the next field in the form navigation cycle
pub fn next_field(current: AgentFormField) -> AgentFormField {
    match current {
        AgentFormField::Name => AgentFormField::Command,
        AgentFormField::Command => AgentFormField::Env,
        AgentFormField::Env => AgentFormField::Note,
        AgentFormField::Note => AgentFormField::Name,
    }
}

/// Get the previous field in the form navigation cycle
pub fn prev_field(current: AgentFormField) -> AgentFormField {
    match current {
        AgentFormField::Name => AgentFormField::Note,
        AgentFormField::Command => AgentFormField::Name,
        AgentFormField::Env => AgentFormField::Command,
        AgentFormField::Note => AgentFormField::Env,
    }
}

/// Render the main UI
pub fn render_permissions_denied(rows: usize, cols: usize) -> String {
    format!(
        "Maestro: permissions denied.\nGrant the requested permissions and reload.\nViewport: {cols}x{rows}"
    )
}

pub fn render_permissions_requesting(rows: usize, cols: usize) -> String {
    format!(
        "Maestro requesting permissions...\nViewport: {cols}x{rows}"
    )
}

pub fn render_ui(model: &Model, _rows: usize, cols: usize) -> String {
    let mut out = String::new();

    out.push_str(&render_section_tabs(model, cols));
    out.push('\n');

    if model.filter_active() && model.focused_section() == Section::AgentPanes {
        out.push_str(&render_filter(model, cols));
        out.push('\n');
    }

    match model.focused_section() {
        Section::AgentPanes => {
            out.push_str(&render_agent_panes(model, cols));
        }
        Section::Agents => {
            out.push_str(&render_agent_management(model, cols));
        }
    }

    if let Some(overlay) = render_overlay(model, cols) {
        out.push('\n');
        out.push_str(&overlay);
    }
    out.push('\n');
    out.push_str(&render_status(model, cols));
    out
}

fn render_section_tabs(model: &Model, _cols: usize) -> String {
    let mut ribbon_items = Vec::new();
    for section in [Section::AgentPanes, Section::Agents] {
        let label = section.label();
        let is_active = model.focused_section() == section;
        let text = if is_active {
            Text::new(label).selected()
        } else {
            Text::new(label)
        };
        ribbon_items.push(text);
    }
    serialize_ribbon_line(ribbon_items)
}

fn render_filter(model: &Model, _cols: usize) -> String {
    let filter_prompt = if model.filter_text().is_empty() {
        "Filter: (type to filter by agent name or tab)"
    } else {
        "Filter:"
    };
    format!("{} {}", filter_prompt, model.filter_text())
}

fn render_agent_panes(model: &Model, cols: usize) -> String {
    let panes: Vec<&AgentPane> = if model.filter_text().is_empty() {
        model.agent_panes().iter().collect()
    } else {
        let matcher = SkimMatcherV2::default();
        let filter_text = model.filter_text();

        model
            .agent_panes()
            .iter()
            .filter(|p| {
                let status_text = match p.status {
                    PaneStatus::Running => "RUNNING",
                    PaneStatus::Exited(_) => "EXITED",
                };
                let searchable = format!("{} {} {}", p.agent_name, p.tab_name, status_text);

                matcher.fuzzy_match(&searchable, filter_text).is_some()
            })
            .collect()
    };

    let mut table = Table::new().add_row(vec!["Tab", "Agent", "Status"]);

    for (idx, pane) in panes.iter().enumerate() {
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
            PaneStatus::Running => 2,
            PaneStatus::Exited(_) => 1,
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
    if panes.is_empty() {
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
        let command_full = agent.command.join(" ");
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
        Mode::NewPaneWorkspace => Some(format!(
            "New Agent Pane: workspace path (optional)\n> {}_",
            truncate(model.workspace_input(), cols.saturating_sub(2))
        )),
        Mode::NewPaneTabSelect => {
            let mut lines = Vec::new();
            lines.push("New Agent Pane: select tab".to_string());
            for (idx, tab) in model.tab_names().iter().enumerate() {
                let prefix = if idx == model.wizard_tab_idx() {
                    ">"
                } else {
                    " "
                };
                lines.push(format!(
                    "{} {}",
                    prefix,
                    truncate(tab, cols.saturating_sub(2))
                ));
            }
            let create_idx = model.tab_names().len();
            let prefix = if model.wizard_tab_idx() == create_idx {
                ">"
            } else {
                " "
            };
            let suggested = crate::utils::workspace_tab_name(model.workspace_input());
            lines.push(format!("{prefix} (new tab: {suggested})"));
            Some(lines.join("\n"))
        }
        Mode::NewPaneAgentSelect => {
            let mut lines = Vec::new();
            lines.push("New Agent Pane: select agent or create new".to_string());
            for (idx, agent) in model.agents().iter().enumerate() {
                let prefix = if idx == model.wizard_agent_idx() {
                    ">"
                } else {
                    " "
                };
                lines.push(format!(
                    "{} {}",
                    prefix,
                    truncate(&agent.name, cols.saturating_sub(2))
                ));
            }
            let create_idx = model.agents().len();
            let prefix = if model.wizard_agent_idx() == create_idx {
                ">"
            } else {
                " "
            };
            lines.push(format!("{prefix} (create new agent)"));
            Some(lines.join("\n"))
        }
        Mode::NewPaneAgentCreate => Some(render_agent_form_overlay(
            model,
            "New Agent Pane: create agent then launch",
            cols,
        )),
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
        "Env",
        model.agent_env_input(),
        AgentFormField::Env,
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
        Mode::View => {
            if model.filter_active() {
                "Filter mode: type to filter • ↑/↓ move • Esc exit filter"
            } else {
                match model.focused_section() {
                    Section::AgentPanes => "j/k move • Tab switch section • f filter • Enter focus • Esc close • x kill • n new • a switch to agents",
                    Section::Agents => "j/k move • Tab switch section • Enter/e edit • d delete • n launch • a create • Esc close",
                }
            }
        }
        Mode::NewPaneWorkspace => "[Enter/Tab] continue • Esc cancel • type to edit path",
        Mode::NewPaneTabSelect => "[j/k or ↑/↓] choose tab • Enter confirm • Esc cancel",
        Mode::NewPaneAgentSelect => "[j/k or ↑/↓] choose • Enter select/create • Esc cancel",
        Mode::NewPaneAgentCreate => "[Tab] next field • Enter save+launch • Esc cancel",
        Mode::AgentFormCreate | Mode::AgentFormEdit => "[Tab] next field • Enter save • Esc cancel",
        Mode::DeleteConfirm => "[Enter/y] confirm • [Esc/n] cancel",
    };
    let msg = if !model.error_message().is_empty() {
        format!("ERROR: {}", model.error_message())
    } else if model.status_message().is_empty() {
        hints.to_string()
    } else {
        format!("{} — {}", model.status_message(), hints)
    };
    truncate(&msg, cols)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_next() {
        assert_eq!(Section::AgentPanes.next(), Section::Agents);
        assert_eq!(Section::Agents.next(), Section::AgentPanes);
    }

    #[test]
    fn test_section_label() {
        assert_eq!(Section::AgentPanes.label(), "Maestro");
        assert_eq!(Section::Agents.label(), "Agents");
    }

    #[test]
    fn test_next_field() {
        assert_eq!(next_field(AgentFormField::Name), AgentFormField::Command);
        assert_eq!(next_field(AgentFormField::Command), AgentFormField::Env);
        assert_eq!(next_field(AgentFormField::Env), AgentFormField::Note);
        assert_eq!(next_field(AgentFormField::Note), AgentFormField::Name);
    }

    #[test]
    fn test_prev_field() {
        assert_eq!(prev_field(AgentFormField::Name), AgentFormField::Note);
        assert_eq!(prev_field(AgentFormField::Command), AgentFormField::Name);
        assert_eq!(prev_field(AgentFormField::Env), AgentFormField::Command);
        assert_eq!(prev_field(AgentFormField::Note), AgentFormField::Env);
    }
}
