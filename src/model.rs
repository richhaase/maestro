//! Plugin state model.

use crate::agent::{Agent, AgentPane};
use crate::ui::{AgentFormField, Mode};

/// The complete state of the Maestro plugin.
#[derive(Debug, Default)]
pub struct Model {
    pub permissions_granted: bool,
    pub permissions_denied: bool,
    pub agents: Vec<Agent>,
    pub agent_panes: Vec<AgentPane>,
    pub tab_names: Vec<String>,
    pub error_message: String,
    pub mode: Mode,
    pub quick_launch_agent_name: Option<String>,
    pub workspace_input: String,
    pub custom_tab_name: Option<String>,
    pub session_name: Option<String>,
    pub agent_name_input: String,
    pub agent_command_input: String,
    pub agent_args_input: String,
    pub agent_note_input: String,
    pub agent_form_field: AgentFormField,
    pub form_target_agent: Option<usize>,
    pub agent_form_source: Option<Mode>,
    pub selected_pane: usize,
    pub selected_agent: usize,
    pub wizard_agent_idx: usize,
    pub wizard_agent_filter: String,
    pub browse_selected_idx: usize,
}

impl Model {
    /// Ensure selection indices stay within valid bounds after list changes.
    pub fn clamp_selections(&mut self) {
        let pane_len = self.agent_panes.len();
        if pane_len == 0 {
            self.selected_pane = 0;
        } else if self.selected_pane >= pane_len {
            self.selected_pane = pane_len.saturating_sub(1);
        }

        let agent_len = self.agents.len();
        if agent_len == 0 {
            self.selected_agent = 0;
        } else if self.selected_agent >= agent_len {
            self.selected_agent = agent_len.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{AgentPane, PaneStatus};
    use crate::test_helpers::create_test_agent;

    fn create_test_pane(agent_name: &str, tab_name: &str) -> AgentPane {
        AgentPane {
            pane_title: agent_name.to_string(),
            tab_name: tab_name.to_string(),
            pending_tab_index: None,
            pane_id: Some(1),
            workspace_path: String::new(),
            agent_name: agent_name.to_string(),
            status: PaneStatus::Running,
        }
    }

    #[test]
    fn test_clamp_selections_empty() {
        let mut model = Model::default();
        model.selected_pane = 5;
        model.selected_agent = 3;
        model.clamp_selections();
        assert_eq!(model.selected_pane, 0);
        assert_eq!(model.selected_agent, 0);
    }

    #[test]
    fn test_clamp_selections_out_of_bounds() {
        let mut model = Model::default();
        model.agents.push(create_test_agent("agent1"));
        model.agents.push(create_test_agent("agent2"));
        model.agent_panes.push(create_test_pane("agent1", "tab1"));
        model.selected_pane = 10;
        model.selected_agent = 10;
        model.clamp_selections();
        assert_eq!(model.selected_pane, 0);
        assert_eq!(model.selected_agent, 1);
    }

    #[test]
    fn test_clamp_selections_valid_selection_preserved() {
        let mut model = Model::default();
        model.agents.push(create_test_agent("agent1"));
        model.agents.push(create_test_agent("agent2"));
        model.agent_panes.push(create_test_pane("agent1", "tab1"));
        model.selected_pane = 0;
        model.selected_agent = 1;
        model.clamp_selections();
        assert_eq!(model.selected_pane, 0);
        assert_eq!(model.selected_agent, 1);
    }
}
