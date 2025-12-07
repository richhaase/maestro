use crate::agent::{Agent, AgentPane};
use crate::ui::{AgentFormField, Mode};

#[derive(Debug, Default)]
struct FormState {
    agent_name_input: String,
    agent_command_input: String,
    agent_env_input: String,
    agent_note_input: String,
    agent_form_field: AgentFormField,
    form_target_agent: Option<usize>,
    agent_form_source: Option<Mode>,
}

#[derive(Debug, Default)]
struct SelectionState {
    selected_pane: usize,
    selected_agent: usize,
    wizard_agent_idx: usize,
    wizard_agent_filter: String,
    browse_selected_idx: usize,
}

#[derive(Debug, Default)]
pub struct Model {
    permissions_granted: bool,
    permissions_denied: bool,
    agents: Vec<Agent>,
    agent_panes: Vec<AgentPane>,
    tab_names: Vec<String>,
    error_message: String,
    mode: Mode,
    quick_launch_agent_name: Option<String>,
    workspace_input: String,
    custom_tab_name: Option<String>,
    session_name: Option<String>,
    form: FormState,
    selection: SelectionState,
}

impl Model {
    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    pub fn agents_mut(&mut self) -> &mut Vec<Agent> {
        &mut self.agents
    }

    pub fn agent_panes(&self) -> &[AgentPane] {
        &self.agent_panes
    }

    pub fn agent_panes_mut(&mut self) -> &mut Vec<AgentPane> {
        &mut self.agent_panes
    }

    pub fn tab_names(&self) -> &[String] {
        &self.tab_names
    }

    pub fn tab_names_mut(&mut self) -> &mut Vec<String> {
        &mut self.tab_names
    }

    pub fn error_message(&self) -> &str {
        &self.error_message
    }

    pub fn selected_pane(&self) -> usize {
        self.selection.selected_pane
    }

    pub fn selected_agent(&self) -> usize {
        self.selection.selected_agent
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn workspace_input(&self) -> &str {
        &self.workspace_input
    }

    pub fn wizard_agent_idx(&self) -> usize {
        self.selection.wizard_agent_idx
    }

    pub fn wizard_agent_filter(&self) -> &str {
        &self.selection.wizard_agent_filter
    }

    pub fn wizard_agent_filter_mut(&mut self) -> &mut String {
        &mut self.selection.wizard_agent_filter
    }

    pub fn browse_selected_idx(&self) -> usize {
        self.selection.browse_selected_idx
    }

    pub fn browse_selected_idx_mut(&mut self) -> &mut usize {
        &mut self.selection.browse_selected_idx
    }

    pub fn agent_name_input(&self) -> &str {
        &self.form.agent_name_input
    }

    pub fn agent_command_input(&self) -> &str {
        &self.form.agent_command_input
    }

    pub fn agent_env_input(&self) -> &str {
        &self.form.agent_env_input
    }

    pub fn agent_note_input(&self) -> &str {
        &self.form.agent_note_input
    }

    pub fn agent_form_field(&self) -> AgentFormField {
        self.form.agent_form_field
    }

    pub fn form_target_agent(&self) -> Option<usize> {
        self.form.form_target_agent
    }

    pub fn permissions_granted(&self) -> bool {
        self.permissions_granted
    }

    pub fn permissions_denied(&self) -> bool {
        self.permissions_denied
    }

    pub fn permissions_granted_mut(&mut self) -> &mut bool {
        &mut self.permissions_granted
    }

    pub fn permissions_denied_mut(&mut self) -> &mut bool {
        &mut self.permissions_denied
    }

    pub fn error_message_mut(&mut self) -> &mut String {
        &mut self.error_message
    }

    pub fn selected_pane_mut(&mut self) -> &mut usize {
        &mut self.selection.selected_pane
    }

    pub fn selected_agent_mut(&mut self) -> &mut usize {
        &mut self.selection.selected_agent
    }

    pub fn mode_mut(&mut self) -> &mut Mode {
        &mut self.mode
    }

    pub fn workspace_input_mut(&mut self) -> &mut String {
        &mut self.workspace_input
    }

    pub fn wizard_agent_idx_mut(&mut self) -> &mut usize {
        &mut self.selection.wizard_agent_idx
    }

    pub fn agent_name_input_mut(&mut self) -> &mut String {
        &mut self.form.agent_name_input
    }

    pub fn agent_command_input_mut(&mut self) -> &mut String {
        &mut self.form.agent_command_input
    }

    pub fn agent_env_input_mut(&mut self) -> &mut String {
        &mut self.form.agent_env_input
    }

    pub fn agent_note_input_mut(&mut self) -> &mut String {
        &mut self.form.agent_note_input
    }

    pub fn agent_form_field_mut(&mut self) -> &mut AgentFormField {
        &mut self.form.agent_form_field
    }

    pub fn form_target_agent_mut(&mut self) -> &mut Option<usize> {
        &mut self.form.form_target_agent
    }

    pub fn quick_launch_agent_name_mut(&mut self) -> &mut Option<String> {
        &mut self.quick_launch_agent_name
    }

    pub fn agent_form_source_mut(&mut self) -> &mut Option<Mode> {
        &mut self.form.agent_form_source
    }

    pub fn session_name_mut(&mut self) -> &mut Option<String> {
        &mut self.session_name
    }

    pub fn session_name(&self) -> Option<&String> {
        self.session_name.as_ref()
    }

    pub fn quick_launch_agent_name(&self) -> Option<&String> {
        self.quick_launch_agent_name.as_ref()
    }

    pub fn custom_tab_name(&self) -> Option<&String> {
        self.custom_tab_name.as_ref()
    }

    pub fn custom_tab_name_mut(&mut self) -> &mut Option<String> {
        &mut self.custom_tab_name
    }

    pub fn clamp_selections(&mut self) {
        let pane_len = self.agent_panes.len();
        if pane_len == 0 {
            self.selection.selected_pane = 0;
        } else if self.selection.selected_pane >= pane_len {
            self.selection.selected_pane = pane_len.saturating_sub(1);
        }

        let agent_len = self.agents.len();
        if agent_len == 0 {
            self.selection.selected_agent = 0;
        } else if self.selection.selected_agent >= agent_len {
            self.selection.selected_agent = agent_len.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, AgentPane, PaneStatus};

    fn create_test_agent(name: &str) -> Agent {
        Agent {
            name: name.to_string(),
            command: vec!["echo".to_string(), name.to_string()],
            env: None,
            note: None,
        }
    }

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
        model.selection.selected_pane = 5;
        model.selection.selected_agent = 3;
        model.clamp_selections();
        assert_eq!(model.selected_pane(), 0);
        assert_eq!(model.selected_agent(), 0);
    }

    #[test]
    fn test_clamp_selections_out_of_bounds() {
        let mut model = Model::default();
        model.agents_mut().push(create_test_agent("agent1"));
        model.agents_mut().push(create_test_agent("agent2"));
        model
            .agent_panes_mut()
            .push(create_test_pane("agent1", "tab1"));
        model.selection.selected_pane = 10;
        model.selection.selected_agent = 10;
        model.clamp_selections();
        assert_eq!(model.selected_pane(), 0);
        assert_eq!(model.selected_agent(), 1);
    }

    #[test]
    fn test_clamp_selections_valid_selection_preserved() {
        let mut model = Model::default();
        model.agents_mut().push(create_test_agent("agent1"));
        model.agents_mut().push(create_test_agent("agent2"));
        model
            .agent_panes_mut()
            .push(create_test_pane("agent1", "tab1"));
        model.selection.selected_pane = 0;
        model.selection.selected_agent = 1;
        model.clamp_selections();
        assert_eq!(model.selected_pane(), 0);
        assert_eq!(model.selected_agent(), 1);
    }
}
