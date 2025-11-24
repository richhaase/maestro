use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use zellij_tile::ui_components::{serialize_ribbon_line, serialize_table, Table, Text};

use crate::agent::{AgentPane, PaneStatus};
use crate::agent::Agent;
use crate::model::Model;
use crate::utils::{truncate, get_home_directory, read_directory, DirEntry};

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
            Section::Agents => "Agent Config",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    View,
    NewPaneWorkspace,
    NewPaneWorkspaceBrowse,
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
        Mode::NewPaneWorkspace => {
            let mut lines = Vec::new();
            let input = model.workspace_input();
            let display_input = input.strip_prefix("/host/").unwrap_or(input);
            lines.push("New Agent Pane: workspace path (optional)".to_string());
            lines.push(format!("> {}_", truncate(display_input, cols.saturating_sub(2))));
            
            if !input.trim().is_empty() {
                let suggestions = crate::utils::get_path_suggestions(input);
                if !suggestions.is_empty() {
                    lines.push("".to_string());
                    let max_display = 5;
                    let start_idx = if model.browse_selected_idx() < max_display {
                        0
                    } else {
                        model.browse_selected_idx().saturating_sub(max_display - 1)
                    };
                    let end_idx = (start_idx + max_display).min(suggestions.len());
                    
                    for (display_idx, suggestion) in suggestions[start_idx..end_idx].iter().enumerate() {
                        let actual_idx = start_idx + display_idx;
                        let prefix = if actual_idx == model.browse_selected_idx() {
                            ">"
                        } else {
                            " "
                        };
                        let display_path = suggestion.strip_prefix("/host/").unwrap_or(suggestion);
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
            
            lines.push("".to_string());
            lines.push("[↑/↓] select suggestion • Enter continue • Esc cancel".to_string());
            Some(lines.join("\n"))
        },
        Mode::NewPaneWorkspaceBrowse => {
            let mut lines = Vec::new();
            let filter_text = model.browse_filter();
            let current_path = if model.browse_path().is_empty() {
                "/host".to_string()
            } else {
                model.browse_path().to_string()
            };
            
            let dir_path = if current_path == "/host" {
                get_home_directory()
            } else {
                std::path::PathBuf::from(&current_path)
            };
            
            let entries = match read_directory(&dir_path) {
                Ok(entries) => entries,
                Err(e) => {
                    lines.push("New Agent Pane: select workspace directory".to_string());
                    lines.push(format!("> {}_", truncate(filter_text, cols.saturating_sub(2))));
                    lines.push("".to_string());
                    lines.push(format!("Error reading directory: {}", truncate(&e, cols.saturating_sub(20))));
                    lines.push("Press Esc to go back".to_string());
                    return Some(lines.join("\n"));
                }
            };
            
            let filtered_entries: Vec<&DirEntry> = if filter_text.is_empty() {
                entries.iter().collect()
            } else {
                use fuzzy_matcher::FuzzyMatcher;
                use fuzzy_matcher::skim::SkimMatcherV2;
                let matcher = SkimMatcherV2::default();
                entries.iter()
                    .filter(|entry| matcher.fuzzy_match(&entry.name, filter_text).is_some())
                    .collect()
            };
            
            lines.push("New Agent Pane: select workspace directory".to_string());
            lines.push(format!("Path: {}", truncate(&current_path, cols.saturating_sub(6))));
            lines.push(format!("> {}_", truncate(filter_text, cols.saturating_sub(2))));
            lines.push("".to_string());
            
            let max_display = 5;
            let start_idx = if model.browse_selected_idx() < max_display {
                0
            } else {
                model.browse_selected_idx().saturating_sub(max_display - 1)
            };
            let end_idx = (start_idx + max_display).min(filtered_entries.len());
            
            for (display_idx, entry) in filtered_entries[start_idx..end_idx].iter().enumerate() {
                let actual_idx = start_idx + display_idx;
                let prefix = if actual_idx == model.browse_selected_idx() {
                    ">"
                } else {
                    " "
                };
                lines.push(format!(
                    "{} 📁 {}",
                    prefix,
                    truncate(&entry.name, cols.saturating_sub(4))
                ));
            }
            
            if filtered_entries.len() > max_display {
                let showing = end_idx - start_idx;
                lines.push(format!("... showing {} of {} directories", showing, filtered_entries.len()));
            }
            
            lines.push("".to_string());
            lines.push("[↑/↓] navigate • →/Tab enter dir • Enter select • ←/h up • Esc cancel".to_string());
            
            Some(lines.join("\n"))
        },
        Mode::NewPaneTabSelect => {
            let mut lines = Vec::new();
            let filter_text = model.wizard_tab_filter();
            let default_tab_name = crate::utils::default_tab_name(model.workspace_input());
            
            let filtered_tabs: Vec<(usize, &String)> = if filter_text.is_empty() {
                model.tab_names().iter().enumerate().collect()
            } else {
                let matcher = SkimMatcherV2::default();
                model.tab_names()
                    .iter()
                    .enumerate()
                    .filter(|(_, tab)| matcher.fuzzy_match(tab, filter_text).is_some())
                    .collect()
            };
            
            lines.push("New Agent Pane: select tab or type to create".to_string());
            if !filter_text.is_empty() {
                lines.push(format!("Filter: {}_", truncate(filter_text, cols.saturating_sub(8))));
            }
            
            for (display_idx, (_, tab)) in filtered_tabs.iter().enumerate() {
                let prefix = if display_idx == model.wizard_tab_idx() {
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
            
            let has_exact_match = filtered_tabs.iter().any(|(_, tab)| tab.eq_ignore_ascii_case(filter_text));
            let show_new_tab = !filter_text.is_empty() && !has_exact_match;
            let new_tab_idx = filtered_tabs.len();
            
            if show_new_tab || (filter_text.is_empty() && model.wizard_tab_idx() == new_tab_idx) {
                let is_selected = model.wizard_tab_idx() == new_tab_idx;
                let prefix = if is_selected {
                    ">"
                } else {
                    " "
                };
                let tab_name = if filter_text.is_empty() {
                    default_tab_name.clone()
                } else {
                    filter_text.to_string()
                };
                lines.push(format!("{prefix} (new tab: {})", truncate(&tab_name, cols.saturating_sub(15))));
            } else if filter_text.is_empty() {
                let is_selected = model.wizard_tab_idx() == new_tab_idx;
                let prefix = if is_selected {
                    ">"
                } else {
                    " "
                };
                lines.push(format!("{prefix} (new tab: {default_tab_name})"));
            }
            
            Some(lines.join("\n"))
        }
        Mode::NewPaneAgentSelect => {
            let mut lines = Vec::new();
            let filter_text = model.wizard_agent_filter();
            
            let filtered_agents: Vec<(usize, &Agent)> = if filter_text.is_empty() {
                model.agents().iter().enumerate().collect()
            } else {
                let matcher = SkimMatcherV2::default();
                model.agents()
                    .iter()
                    .enumerate()
                    .filter(|(_, agent)| matcher.fuzzy_match(&agent.name, filter_text).is_some())
                    .collect()
            };
            
            lines.push("New Agent Pane: select agent or type to create".to_string());
            if !filter_text.is_empty() {
                lines.push(format!("Filter: {}_", truncate(filter_text, cols.saturating_sub(8))));
            }
            
            for (display_idx, (_, agent)) in filtered_agents.iter().enumerate() {
                let prefix = if display_idx == model.wizard_agent_idx() {
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
            
            let has_exact_match = filtered_agents.iter().any(|(_, agent)| agent.name.eq_ignore_ascii_case(filter_text));
            let show_new_agent = !filter_text.is_empty() && !has_exact_match;
            let new_agent_idx = filtered_agents.len();
            
            if show_new_agent || (filter_text.is_empty() && model.wizard_agent_idx() == new_agent_idx) {
                let is_selected = model.wizard_agent_idx() == new_agent_idx;
                let prefix = if is_selected {
                    ">"
                } else {
                    " "
                };
                let agent_name = if filter_text.is_empty() {
                    "(create new agent)".to_string()
                } else {
                    format!("(create new agent: {})", truncate(filter_text, cols.saturating_sub(25)))
                };
                lines.push(format!("{prefix} {agent_name}"));
            } else if filter_text.is_empty() {
                let is_selected = model.wizard_agent_idx() == new_agent_idx;
                let prefix = if is_selected {
                    ">"
                } else {
                    " "
                };
                lines.push(format!("{prefix} (create new agent)"));
            }
            
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
                    Section::AgentPanes => "↑/↓ move • Tab switch section • f filter • Enter focus • Esc close • x kill • n new • a switch to agents",
                    Section::Agents => "↑/↓ move • Tab switch section • Enter/e edit • d delete • n launch • a create • Esc close",
                }
            }
        }
        Mode::NewPaneWorkspace => "[Enter] continue • Esc cancel • type to edit path",
        Mode::NewPaneWorkspaceBrowse => "[↑/↓] navigate • type to filter • Enter select • Esc cancel",
        Mode::NewPaneTabSelect => "[↑/↓] choose tab • type to edit new tab name • Enter confirm • Esc cancel",
        Mode::NewPaneAgentSelect => "[↑/↓] choose • Enter select/create • Esc cancel",
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
        assert_eq!(Section::Agents.label(), "Agent Config");
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
