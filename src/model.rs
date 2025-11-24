use crate::agent::{Agent, AgentPane};
use crate::ui::{AgentFormField, Mode, Section};

#[derive(Debug, Default)]
pub struct Model {
    pub permissions_granted: bool,
    pub permissions_denied: bool,
    pub agents: Vec<Agent>,
    pub agent_panes: Vec<AgentPane>,
    pub tab_names: Vec<String>,
    pub status_message: String,
    pub error_message: String,
    pub selected_tab: usize,
    pub selected_pane: usize,
    pub selected_agent: usize,
    pub focused_section: Section,
    pub filter_text: String,
    pub filter_active: bool,
    pub mode: Mode,
    pub agent_form_source: Option<Mode>,
    pub quick_launch_agent_name: Option<String>,
    pub workspace_input: String,
    pub wizard_tab_idx: usize,
    pub agent_name_input: String,
    pub agent_command_input: String,
    pub agent_env_input: String,
    pub agent_note_input: String,
    pub agent_form_field: AgentFormField,
    pub wizard_agent_idx: usize,
    pub form_target_agent: Option<usize>,
    pub session_name: Option<String>,
}
