use std::collections::{hash_map::DefaultHasher, BTreeMap};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zellij_tile::prelude::{
    BareKey, KeyModifier, KeyWithModifier, PaneId, PaneManifest, PermissionStatus, TabInfo,
};
use zellij_tile::prelude::*;
use zellij_tile::ui_components::{serialize_ribbon_line, Table, Text};

mod agents;

// Permissions we intend to request for the MVP.
const REQUESTED_PERMISSIONS: &[PermissionType] = &[
    PermissionType::ReadApplicationState,
    PermissionType::ChangeApplicationState,
    PermissionType::RunCommands,
    PermissionType::OpenTerminalsOrPlugins,
];

// ---------- Data types ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    Running,
    Exited(Option<i32>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub tab_name: String,
    pub pane_id: Option<u32>,
    pub workspace_path: String,
    pub agent_name: String,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Workspaces,
    Sessions,
    Agents,
}

impl Default for Section {
    fn default() -> Self {
        Section::Workspaces
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    View,
    NewSessionWorkspace,
    NewSessionTabSelect,
    NewSessionAgentSelect,
    NewSessionAgentCreate,
    AgentFormCreate,
    AgentFormEdit,
    DeleteConfirm,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::View
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentFormField {
    Name,
    Command,
    Env,
    Note,
}

impl Default for AgentFormField {
    fn default() -> Self {
        AgentFormField::Name
    }
}

#[derive(Debug, Default)]
pub struct Model {
    pub permissions_granted: bool,
    pub permissions_denied: bool,
    pub agents: Vec<Agent>,
    pub sessions: Vec<Session>,
    pub workspaces: Vec<Workspace>,
    pub tab_names: Vec<String>,
    pub status_message: String,
    pub error_message: String,
    pub selected_workspace: usize,
    pub selected_session: usize,
    pub selected_agent: usize,
    pub focused_section: Section,
    pub mode: Mode,
    pub workspace_input: String,
    pub wizard_tab_idx: usize,
    pub agent_name_input: String,
    pub agent_command_input: String,
    pub agent_env_input: String,
    pub agent_note_input: String,
    pub agent_form_field: AgentFormField,
    pub wizard_agent_idx: usize,
    pub form_target_agent: Option<usize>,
}

pub struct Maestro {
    model: Model,
}

impl Default for Maestro {
    fn default() -> Self {
        Maestro {
            model: Model::default(),
        }
    }
}

// ---------- Model methods ----------

impl Model {
    fn handle_permission_result(&mut self, status: PermissionStatus) {
        match status {
            PermissionStatus::Granted => {
                self.permissions_granted = true;
                self.permissions_denied = false;
            }
            PermissionStatus::Denied => {
                self.permissions_granted = false;
                self.permissions_denied = true;
            }
        }
    }

    fn apply_tab_update(&mut self, tabs: Vec<TabInfo>) {
        let tab_names: Vec<String> = tabs.iter().map(|t| t.name.clone()).collect();
        self.tab_names = tab_names.clone();
        self.sessions
            .retain(|s| s.pane_id.is_some() || tab_names.contains(&s.tab_name));
        self.rebuild_workspaces();
        self.clamp_selections();
    }

    fn apply_pane_update(&mut self, update: PaneManifest) {
        for (_tab_idx, pane_list) in update.panes {
            for pane in pane_list {
                let title = pane.title.clone();
                if !is_maestro_tab(&title) {
                    continue;
                }
                let entry = self
                    .sessions
                    .iter_mut()
                    .find(|s| s.tab_name == title || s.pane_id == Some(pane.id));
                if let Some(existing) = entry {
                    existing.pane_id = Some(pane.id);
                    existing.tab_name = title;
                    existing.status = if pane.exited {
                        SessionStatus::Exited(pane.exit_status)
                    } else {
                        SessionStatus::Running
                    };
                    if existing.agent_name.is_empty() || existing.workspace_path.is_empty() {
                        if let Some((agent, workspace_hint)) = parse_title_hint(&existing.tab_name) {
                            if existing.agent_name.is_empty() {
                                existing.agent_name = agent;
                            }
                            if existing.workspace_path.is_empty() {
                                existing.workspace_path = workspace_hint;
                            }
                        }
                    }
                } else {
                    let (agent_name, workspace_path) = parse_title_hint(&title)
                        .map(|(a, w)| (a, w))
                        .unwrap_or_default();
                    self.sessions.push(Session {
                        tab_name: title,
                        pane_id: Some(pane.id),
                        workspace_path,
                        agent_name,
                        status: if pane.exited {
                            SessionStatus::Exited(pane.exit_status)
                        } else {
                            SessionStatus::Running
                        },
                    });
                }
            }
        }
        self.rebuild_workspaces();
        self.clamp_selections();
    }

    fn handle_command_pane_opened(&mut self, pane_id: u32, ctx: BTreeMap<String, String>) {
        let title = ctx
            .get("pane_title")
            .cloned()
            .unwrap_or_else(|| format!("maestro:{}", pane_id));
        let workspace_path = ctx.get("cwd").cloned().unwrap_or_default();
        let agent_name = ctx.get("agent").cloned().unwrap_or_default();
        let entry = self
            .sessions
            .iter_mut()
            .find(|s| s.tab_name == title || s.pane_id == Some(pane_id));
        if let Some(existing) = entry {
            existing.pane_id = Some(pane_id);
            existing.tab_name = title;
            if !workspace_path.is_empty() {
                existing.workspace_path = workspace_path.clone();
            }
            if !agent_name.is_empty() {
                existing.agent_name = agent_name.clone();
            }
            existing.status = SessionStatus::Running;
        } else {
            self.sessions.push(Session {
                tab_name: title,
                pane_id: Some(pane_id),
                workspace_path,
                agent_name,
                status: SessionStatus::Running,
            });
        }
        self.rebuild_workspaces();
        self.clamp_selections();
    }

    fn rebuild_from_session_infos(&mut self, session_infos: &[SessionInfo]) {
        self.sessions.clear();
        for session in session_infos {
            self.rebuild_from_panes_iter(session.panes.clone().panes.into_iter());
        }
        self.rebuild_workspaces();
        self.clamp_selections();
    }

    fn rebuild_from_panes_iter(
        &mut self,
        panes_iter: impl Iterator<Item = (usize, Vec<PaneInfo>)>,
    ) {
        for (_tab_idx, pane_list) in panes_iter {
            for pane in pane_list {
                let title = pane.title.clone();
                if !is_maestro_tab(&title) {
                    continue;
                }
                let (agent_name, workspace_path) = parse_title_hint(&title)
                    .map(|(a, w)| (a, w))
                    .unwrap_or_default();
                self.sessions.push(Session {
                    tab_name: title,
                    pane_id: Some(pane.id),
                    workspace_path,
                    agent_name,
                    status: if pane.exited {
                        SessionStatus::Exited(pane.exit_status)
                    } else {
                        SessionStatus::Running
                    },
                });
            }
        }
        self.rebuild_workspaces();
        self.clamp_selections();
    }

    fn handle_command_pane_exited(
        &mut self,
        pane_id: u32,
        exit_status: Option<i32>,
        ctx: BTreeMap<String, String>,
    ) {
        let title = ctx
            .get("pane_title")
            .cloned()
            .unwrap_or_else(|| format!("maestro:{}", pane_id));
        if let Some(sess) = self
            .sessions
            .iter_mut()
            .find(|s| s.pane_id == Some(pane_id) || s.tab_name == title)
        {
            sess.status = SessionStatus::Exited(exit_status);
        }
        self.rebuild_workspaces();
        self.clamp_selections();
    }

    fn handle_command_pane_rerun(&mut self, pane_id: u32, ctx: BTreeMap<String, String>) {
        let title = ctx
            .get("pane_title")
            .cloned()
            .unwrap_or_else(|| format!("maestro:{}", pane_id));
        if let Some(sess) = self
            .sessions
            .iter_mut()
            .find(|s| s.pane_id == Some(pane_id) || s.tab_name == title)
        {
            sess.status = SessionStatus::Running;
        }
        self.rebuild_workspaces();
        self.clamp_selections();
    }

    fn handle_session_update(&mut self, sessions: Vec<SessionInfo>) {
        self.rebuild_from_session_infos(&sessions);
    }

    fn handle_pane_closed(&mut self, pane_id: PaneId) {
        let pid = match pane_id {
            PaneId::Terminal(id) | PaneId::Plugin(id) => id,
        };
        self.sessions.retain(|s| s.pane_id != Some(pid));
        self.rebuild_workspaces();
        self.clamp_selections();
    }

    fn rebuild_workspaces(&mut self) {
        let mut seen = BTreeMap::new();
        for sess in &self.sessions {
            let path = sess.workspace_path.clone();
            if path.is_empty() {
                continue;
            }
            let name = path
                .rsplit('/')
                .next()
                .map(|s| s.to_string())
                .unwrap_or_else(|| path.clone());
            seen.entry(path.clone()).or_insert(name);
        }
        self.workspaces = seen
            .into_iter()
            .map(|(path, name)| Workspace { path, name })
            .collect();
    }

    fn reset_status(&mut self) {
        self.status_message.clear();
        self.error_message.clear();
    }

    fn cancel_to_view(&mut self) {
        self.mode = Mode::View;
        self.reset_status();
    }

    fn view_preserve_messages(&mut self) {
        self.mode = Mode::View;
    }

    fn start_new_session_workspace(&mut self) {
        let default_path = self
            .workspaces
            .get(self.selected_workspace)
            .map(|w| w.path.clone())
            .unwrap_or_else(String::new);
        self.workspace_input = default_path;
        self.mode = Mode::NewSessionWorkspace;
        self.wizard_agent_idx = 0;
        self.wizard_tab_idx = 0;
        self.reset_status();
    }

    fn start_agent_create(&mut self) {
        self.mode = Mode::AgentFormCreate;
        self.agent_name_input.clear();
        self.agent_command_input.clear();
        self.agent_env_input.clear();
        self.agent_note_input.clear();
        self.agent_form_field = AgentFormField::Name;
        self.form_target_agent = None;
        self.reset_status();
    }

    fn start_agent_edit(&mut self) {
        if self.agents.is_empty() {
            self.error_message = "no agents to edit".to_string();
            return;
        }
        let idx = self.selected_agent.min(self.agents.len().saturating_sub(1));
        if let Some(agent) = self.agents.get(idx) {
            self.agent_name_input = agent.name.clone();
            self.agent_command_input = agent.command.join(" ");
            self.agent_env_input = agent
                .env
                .as_ref()
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            self.agent_note_input = agent.note.clone().unwrap_or_default();
            self.agent_form_field = AgentFormField::Name;
            self.form_target_agent = Some(idx);
            self.mode = Mode::AgentFormEdit;
            self.reset_status();
        }
    }

    fn start_agent_delete_confirm(&mut self) {
        if self.agents.is_empty() {
            self.error_message = "no agents to delete".to_string();
            return;
        }
        let idx = self.selected_agent.min(self.agents.len().saturating_sub(1));
        self.form_target_agent = Some(idx);
        self.mode = Mode::DeleteConfirm;
        self.reset_status();
    }

    pub fn spawn_session(&mut self, workspace_path: String, agent_name: String, tab_choice: TabChoice) {
        if !self.permissions_granted {
            self.error_message = "permissions not granted".to_string();
            return;
        }
        let agent = match self.agents.iter().find(|a| a.name == agent_name) {
            Some(a) => a.clone(),
            None => {
                self.error_message = "agent not found".to_string();
                return;
            }
        };
        let title = format!(
            "maestro:{}:{}:{}",
            agent.name,
            workspace_basename(&workspace_path),
            Uuid::new_v4()
        );
        let tab_target = match tab_choice {
            TabChoice::Existing(name) => name,
            TabChoice::New => {
                let name = workspace_tab_name(&workspace_path);
                new_tab(Some(name.clone()), Some(workspace_path.clone()));
                if !self.tab_names.contains(&name) {
                    self.tab_names.push(name.clone());
                }
                name
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

        self.sessions.push(Session {
            tab_name: title,
            pane_id: None,
            workspace_path,
            agent_name,
            status: SessionStatus::Running,
        });
        self.rebuild_workspaces();
        self.error_message.clear();
        if self.status_message.is_empty() {
            self.status_message = "Session launched".to_string();
        } else {
            self.status_message = format!("{}; Session launched", self.status_message);
        }
    }

    pub fn focus_selected(&mut self, selected_idx: usize) {
        if !self.permissions_granted {
            self.error_message = "permissions not granted".to_string();
            return;
        }
        let sessions = filtered_sessions(self);
        if selected_idx >= sessions.len() {
            self.error_message = "no sessions".to_string();
            return;
        }
        let sess = &sessions[selected_idx];
        let tab_target = workspace_tab_name(&sess.workspace_path);
        go_to_tab_name(&tab_target);
        if let Some(pid) = sess.pane_id {
            focus_terminal_pane(pid, false);
        }
        self.error_message.clear();
        self.status_message = "Focused session".to_string();
    }

    pub fn kill_selected(&mut self, selected_idx: usize) {
        if !self.permissions_granted {
            self.error_message = "permissions not granted".to_string();
            return;
        }
        let sessions = filtered_sessions(self);
        if selected_idx >= sessions.len() {
            self.error_message = "no sessions".to_string();
            return;
        }
        let sess = &sessions[selected_idx];
        if let Some(pid) = sess.pane_id {
            close_terminal_pane(pid);
            self.sessions
                .retain(|s| s.tab_name != sess.tab_name || s.pane_id != sess.pane_id);
            self.rebuild_workspaces();
            self.error_message.clear();
            self.status_message = "Killed session".to_string();
            self.clamp_selections();
        } else {
            self.error_message = "no valid target to kill".to_string();
        }
    }

    fn clamp_selections(&mut self) {
        let workspace_len = self.workspaces.len();
        if workspace_len == 0 {
            self.selected_workspace = 0;
        } else if self.selected_workspace >= workspace_len {
            self.selected_workspace = workspace_len.saturating_sub(1);
        }

        let session_len = filtered_sessions(self).len();
        if session_len == 0 {
            self.selected_session = 0;
        } else if self.selected_session >= session_len {
            self.selected_session = session_len.saturating_sub(1);
        }

        let agent_len = self.agents.len();
        if agent_len == 0 {
            self.selected_agent = 0;
        } else if self.selected_agent >= agent_len {
            self.selected_agent = agent_len.saturating_sub(1);
        }
    }

    fn move_selection(&mut self, section: Section, delta: isize) {
        let (len, current) = match section {
            Section::Workspaces => (self.workspaces.len(), self.selected_workspace),
            Section::Sessions => (filtered_sessions(self).len(), self.selected_session),
            Section::Agents => (self.agents.len(), self.selected_agent),
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
            Section::Workspaces => self.selected_workspace = next,
            Section::Sessions => self.selected_session = next,
            Section::Agents => self.selected_agent = next,
        }
        if section == Section::Workspaces {
            self.clamp_selections();
        }
        self.status_message.clear();
        self.error_message.clear();
    }

    fn focus_next_section(&mut self) {
        self.focused_section = match self.focused_section {
            Section::Workspaces => Section::Sessions,
            Section::Sessions => Section::Agents,
            Section::Agents => Section::Workspaces,
        };
        self.status_message.clear();
        self.error_message.clear();
        self.clamp_selections();
    }

    fn focus_prev_section(&mut self) {
        self.focused_section = match self.focused_section {
            Section::Workspaces => Section::Agents,
            Section::Sessions => Section::Workspaces,
            Section::Agents => Section::Sessions,
        };
        self.status_message.clear();
        self.error_message.clear();
        self.clamp_selections();
    }

    fn handle_key_event(&mut self, key: KeyWithModifier) {
        match self.mode {
            Mode::View => self.handle_key_event_view(key),
            Mode::NewSessionWorkspace => self.handle_key_event_new_session_workspace(key),
            Mode::NewSessionTabSelect => self.handle_key_event_new_session_tab_select(key),
            Mode::NewSessionAgentSelect => self.handle_key_event_new_session_agent_select(key),
            Mode::NewSessionAgentCreate => self.handle_key_event_agent_form(key, true),
            Mode::AgentFormCreate | Mode::AgentFormEdit => self.handle_key_event_agent_form(key, false),
            Mode::DeleteConfirm => self.handle_key_event_delete_confirm(key),
        }
    }

    fn handle_key_event_view(&mut self, key: KeyWithModifier) {
        let shift_tab = key.bare_key == BareKey::Tab && key.key_modifiers.contains(&KeyModifier::Shift);
        match key.bare_key {
            BareKey::Up => self.move_selection(self.focused_section, -1),
            BareKey::Down => self.move_selection(self.focused_section, 1),
            BareKey::Left => self.focus_prev_section(),
            BareKey::Right => self.focus_next_section(),
            BareKey::Tab if shift_tab => self.focus_prev_section(),
            BareKey::Tab => self.focus_next_section(),
            BareKey::Enter => {
                if self.focused_section == Section::Sessions {
                    let idx = self.selected_session;
                    self.focus_selected(idx);
                }
            }
            BareKey::Esc => {
                close_self();
            }
            BareKey::Char('x') | BareKey::Char('X') => {
                if self.focused_section == Section::Sessions {
                    let idx = self.selected_session;
                    self.kill_selected(idx);
                }
            }
            BareKey::Char('n') | BareKey::Char('N') => {
                self.start_new_session_workspace();
            }
            BareKey::Char('a') | BareKey::Char('A') => {
                self.start_agent_create();
            }
            BareKey::Char('e') | BareKey::Char('E') => {
                self.start_agent_edit();
            }
            BareKey::Char('d') | BareKey::Char('D') => {
                self.start_agent_delete_confirm();
            }
            _ => {}
        }
    }

    fn handle_key_event_new_session_workspace(&mut self, key: KeyWithModifier) {
        if handle_text_edit(&mut self.workspace_input, &key) {
            return;
        }
        match key.bare_key {
            BareKey::Enter => {
                if self.workspace_input.trim().is_empty() {
                    self.error_message = "workspace path required".to_string();
                } else {
                    self.mode = Mode::NewSessionTabSelect;
                    self.wizard_tab_idx = 0;
                    self.wizard_agent_idx = 0;
                    self.reset_status();
                }
            }
            BareKey::Esc => self.cancel_to_view(),
            BareKey::Tab => {
                self.mode = Mode::NewSessionTabSelect;
                self.wizard_tab_idx = 0;
                self.wizard_agent_idx = 0;
                self.reset_status();
            }
            _ => {}
        }
    }

    fn handle_key_event_new_session_tab_select(&mut self, key: KeyWithModifier) {
        let choices = self.tab_names.len().saturating_add(1);
        match key.bare_key {
            BareKey::Up => {
                if self.wizard_tab_idx > 0 {
                    self.wizard_tab_idx -= 1;
                }
            }
            BareKey::Down => {
                if self.wizard_tab_idx + 1 < choices {
                    self.wizard_tab_idx += 1;
                }
            }
            BareKey::Enter => {
                self.mode = Mode::NewSessionAgentSelect;
                self.wizard_agent_idx = 0;
            }
            BareKey::Esc => self.cancel_to_view(),
            BareKey::Tab => self.cancel_to_view(),
            _ => {}
        }
    }

    fn handle_key_event_new_session_agent_select(&mut self, key: KeyWithModifier) {
        let choices = self.agents.len().saturating_add(1);
        match key.bare_key {
            BareKey::Up => {
                if self.wizard_agent_idx > 0 {
                    self.wizard_agent_idx -= 1;
                }
            }
            BareKey::Down => {
                if self.wizard_agent_idx + 1 < choices {
                    self.wizard_agent_idx += 1;
                }
            }
            BareKey::Enter => {
                if self.wizard_agent_idx < self.agents.len() {
                    let agent = self.agents[self.wizard_agent_idx].name.clone();
                    let workspace = self.workspace_input.trim().to_string();
                    let tab_choice = selected_tab_choice(self);
                    self.spawn_session(workspace, agent, tab_choice);
                    if self.error_message.is_empty() {
                        self.view_preserve_messages();
                    }
                } else {
                    self.mode = Mode::NewSessionAgentCreate;
                    self.agent_name_input.clear();
                    self.agent_command_input.clear();
                    self.agent_env_input.clear();
                    self.agent_note_input.clear();
                    self.agent_form_field = AgentFormField::Name;
                    self.reset_status();
                }
            }
            BareKey::Esc => self.cancel_to_view(),
            BareKey::Tab => self.cancel_to_view(),
            _ => {}
        }
    }

    fn handle_key_event_agent_form(&mut self, key: KeyWithModifier, launch_after: bool) {
        if handle_form_text(self, &key) {
            return;
        }
        let shift_tab = key.bare_key == BareKey::Tab && key.key_modifiers.contains(&KeyModifier::Shift);
        match key.bare_key {
            BareKey::Tab if shift_tab => {
                self.agent_form_field = prev_field(self.agent_form_field);
            }
            BareKey::Tab => {
                self.agent_form_field = next_field(self.agent_form_field);
            }
            BareKey::Enter => {
                match self.build_agent_from_inputs() {
                    Ok(agent) => {
                        let result = match self.mode {
                            Mode::AgentFormEdit => self.apply_agent_edit(agent.clone()),
                            Mode::AgentFormCreate | Mode::NewSessionAgentCreate => {
                                self.apply_agent_create(agent.clone())
                            }
                            _ => Err("invalid mode".to_string()),
                        };
                        match result {
                            Ok(saved_path) => {
                                self.status_message = format!("Agents saved to {}", saved_path.display());
                                if launch_after {
                                    let workspace = self.workspace_input.trim().to_string();
                                    let tab_choice = selected_tab_choice(self);
                                    self.spawn_session(workspace, agent.name.clone(), tab_choice);
                                }
                                if self.error_message.is_empty() {
                                    self.view_preserve_messages();
                                }
                            }
                            Err(err) => {
                                self.error_message = err;
                            }
                        }
                    }
                    Err(err) => {
                        self.error_message = err;
                    }
                }
            }
            BareKey::Esc => self.cancel_to_view(),
            _ => {}
        }
    }

    fn handle_key_event_delete_confirm(&mut self, key: KeyWithModifier) {
        match key.bare_key {
            BareKey::Enter | BareKey::Char('y') | BareKey::Char('Y') => {
                if let Some(idx) = self.form_target_agent.take() {
                    if idx < self.agents.len() {
                        self.agents.remove(idx);
                        self.selected_agent = self.selected_agent.min(self.agents.len().saturating_sub(1));
                        match self.persist_agents() {
                            Ok(path) => {
                                self.status_message =
                                    format!("Agent deleted and saved to {}", path.display());
                                self.error_message.clear();
                            }
                            Err(err) => {
                                self.error_message = err;
                            }
                        }
                    }
                }
                self.cancel_to_view();
            }
            BareKey::Esc | BareKey::Char('n') | BareKey::Char('N') => self.cancel_to_view(),
            _ => {}
        }
    }

    fn apply_agent_create(&mut self, agent: Agent) -> Result<PathBuf, String> {
        if self.agents.iter().any(|a| a.name == agent.name) {
            return Err("duplicate agent name".to_string());
        }
        self.agents.push(agent);
        self.selected_agent = self.agents.len().saturating_sub(1);
        self.persist_agents()
    }

    fn apply_agent_edit(&mut self, agent: Agent) -> Result<PathBuf, String> {
        if let Some(idx) = self.form_target_agent {
            if idx < self.agents.len() {
                if self
                    .agents
                    .iter()
                    .enumerate()
                    .any(|(i, a)| i != idx && a.name == agent.name)
                {
                    return Err("duplicate agent name".to_string());
                }
                self.agents[idx] = agent;
                self.selected_agent = idx;
                return self.persist_agents();
            }
        }
        Err("no agent selected".to_string())
    }

    fn build_agent_from_inputs(&self) -> Result<Agent, String> {
        let name = self.agent_name_input.trim().to_string();
        if name.is_empty() {
            return Err("agent name required".to_string());
        }
        let cmd_parts: Vec<String> = self
            .agent_command_input
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        if cmd_parts.is_empty() {
            return Err("command required".to_string());
        }
        let env = parse_env_input(&self.agent_env_input)?;
        let note = if self.agent_note_input.trim().is_empty() {
            None
        } else {
            Some(self.agent_note_input.trim().to_string())
        };
        Ok(Agent {
            name,
            command: cmd_parts,
            env,
            note,
        })
    }

    fn persist_agents(&mut self) -> Result<PathBuf, String> {
        let path = agents::config_path().map_err(|e| format!("config path: {e}"))?;
        agents::save_agents(&self.agents).map_err(|e| format!("save agents: {e}"))?;
        match agents::load_agents() {
            Ok(list) => {
                self.agents = list;
                self.clamp_selections();
                self.error_message.clear();
                Ok(path)
            }
            Err(err) => Err(format!("reload agents: {err}")),
        }
    }
}

// ---------- Zellij plugin entry ----------

impl ZellijPlugin for Maestro {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        match agents::load_agents() {
            Ok(list) => self.model.agents = list,
            Err(err) => {
                eprintln!("maestro: load agents: {err}");
                self.model.agents = Vec::new();
            }
        }

        request_permission(REQUESTED_PERMISSIONS);

        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::SessionUpdate,
            EventType::CommandPaneOpened,
            EventType::CommandPaneExited,
            EventType::CommandPaneReRun,
            EventType::PaneClosed,
            EventType::BeforeClose,
            EventType::PermissionRequestResult,
            EventType::Key,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(status) => {
                self.model.handle_permission_result(status);
                true
            }
            Event::TabUpdate(tabs) => {
                self.model.apply_tab_update(tabs);
                true
            }
            Event::PaneUpdate(manifest) => {
                self.model.apply_pane_update(manifest);
                true
            }
            Event::SessionUpdate(session_info, _resurrectable) => {
                self.model.handle_session_update(session_info);
                true
            }
            Event::CommandPaneOpened(pane_id, ctx) => {
                self.model.handle_command_pane_opened(pane_id, ctx);
                true
            }
            Event::CommandPaneExited(pane_id, exit_status, ctx) => {
                self.model.handle_command_pane_exited(pane_id, exit_status, ctx);
                true
            }
            Event::CommandPaneReRun(pane_id, ctx) => {
                self.model.handle_command_pane_rerun(pane_id, ctx);
                true
            }
            Event::PaneClosed(pane_id) => {
                self.model.handle_pane_closed(pane_id);
                true
            }
            Event::Key(key) => {
                self.model.handle_key_event(key);
                true
            }
            Event::BeforeClose => true,
            _ => true,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if self.model.permissions_denied {
            let text = format!(
                "Maestro: permissions denied.\nGrant the requested permissions and reload.\nViewport: {}x{}",
                cols, rows
            );
            print!("{text}");
            return;
        }
        if !self.model.permissions_granted {
            let text = format!(
                "Maestro requesting permissions...\nViewport: {}x{}",
                cols, rows
            );
            print!("{text}");
            return;
        }

        print!("{}", render_ui(&self.model, rows, cols));
    }
}

// ---------- Rendering ----------

fn render_ui(model: &Model, _rows: usize, cols: usize) -> String {
    let mut out = String::new();
    out.push_str(&render_workspaces(model, cols));
    out.push('\n');
    out.push_str(&render_sessions(model, cols));
    out.push('\n');
    out.push_str(&render_agents(model, cols));
    if let Some(overlay) = render_overlay(model, cols) {
        out.push('\n');
        out.push_str(&overlay);
    }
    out.push('\n');
    out.push_str(&render_status(model, cols));
    out
}

fn render_workspaces(model: &Model, cols: usize) -> String {
    let mut table = Table::new().add_row(vec!["Workspace", "Sessions"]);
    for (idx, ws) in model.workspaces.iter().enumerate() {
        let name = truncate(&ws.name, cols.saturating_sub(10));
        let count = model
            .sessions
            .iter()
            .filter(|s| s.workspace_path == ws.path)
            .count()
            .to_string();
        let row = vec![name, count];
        let styled = if idx == model.selected_workspace {
            row.into_iter().map(|c| Text::new(c).selected()).collect()
        } else {
            row.into_iter().map(Text::new).collect()
        };
        table = table.add_styled_row(styled);
    }
    if model.workspaces.is_empty() {
        table = table.add_row(vec!["(no workspaces)", ""]);
    }
    serialize_table(&table)
}

fn render_sessions(model: &Model, cols: usize) -> String {
    let mut table = Table::new().add_row(vec!["Agent", "Status", "Tab"]);
    let sessions = filtered_sessions(model);
    for (idx, sess) in sessions.iter().enumerate() {
        let agent = if sess.agent_name.is_empty() {
            "(agent)"
        } else {
            &sess.agent_name
        };
        let status = match sess.status {
            SessionStatus::Running => "RUNNING",
            SessionStatus::Exited(_) => "EXITED",
        };
        let tab = truncate(&sess.tab_name, cols.saturating_sub(20));
        let row = vec![agent.to_string(), status.to_string(), tab];
        let styled = if idx == model.selected_session {
            row.into_iter().map(|c| Text::new(c).selected()).collect()
        } else {
            row.into_iter().map(Text::new).collect()
        };
        table = table.add_styled_row(styled);
    }
    if sessions.is_empty() {
        table = table.add_row(vec!["(no sessions)", "", ""]);
    }
    serialize_table(&table)
}

fn render_agents(model: &Model, _cols: usize) -> String {
    let ribbons: Vec<Text> = model
        .agents
        .iter()
        .enumerate()
        .map(|(idx, a)| {
            let t = Text::new(&a.name);
            if idx == model.selected_agent {
                t.selected()
            } else {
                t
            }
        })
        .collect();
    if ribbons.is_empty() {
        return "(no agents)".to_string();
    }
    serialize_ribbon_line(ribbons)
}

fn render_overlay(model: &Model, cols: usize) -> Option<String> {
    match model.mode {
        Mode::View => None,
        Mode::NewSessionWorkspace => Some(format!(
            "New Session: workspace path\n> {}_",
            truncate(&model.workspace_input, cols.saturating_sub(2))
        )),
        Mode::NewSessionTabSelect => {
            let mut lines = Vec::new();
            lines.push("New Session: select tab".to_string());
            for (idx, tab) in model.tab_names.iter().enumerate() {
                let prefix = if idx == model.wizard_tab_idx { ">" } else { " " };
                lines.push(format!("{} {}", prefix, truncate(tab, cols.saturating_sub(2))));
            }
            let create_idx = model.tab_names.len();
            let prefix = if model.wizard_tab_idx == create_idx { ">" } else { " " };
            let suggested = workspace_tab_name(&model.workspace_input);
            lines.push(format!("{prefix} (new tab: {suggested})"));
            Some(lines.join("\n"))
        }
        Mode::NewSessionAgentSelect => {
            let mut lines = Vec::new();
            lines.push("New Session: select agent or create new".to_string());
            for (idx, agent) in model.agents.iter().enumerate() {
                let prefix = if idx == model.wizard_agent_idx { ">" } else { " " };
                lines.push(format!(
                    "{} {}",
                    prefix,
                    truncate(&agent.name, cols.saturating_sub(2))
                ));
            }
            let create_idx = model.agents.len();
            let prefix = if model.wizard_agent_idx == create_idx {
                ">"
            } else {
                " "
            };
            lines.push(format!("{prefix} (create new agent)"));
            Some(lines.join("\n"))
        }
        Mode::NewSessionAgentCreate => Some(render_agent_form_overlay(
            model,
            "New Session: create agent then launch",
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
                .form_target_agent
                .and_then(|idx| model.agents.get(idx))
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
        format!("{marker} {label}: {}", truncate(val, cols.saturating_sub(label.len() + 4)))
    };
    lines.push(mk(
        "Name",
        &model.agent_name_input,
        AgentFormField::Name,
        model.agent_form_field,
    ));
    lines.push(mk(
        "Command",
        &model.agent_command_input,
        AgentFormField::Command,
        model.agent_form_field,
    ));
    lines.push(mk(
        "Env",
        &model.agent_env_input,
        AgentFormField::Env,
        model.agent_form_field,
    ));
    lines.push(mk(
        "Note",
        &model.agent_note_input,
        AgentFormField::Note,
        model.agent_form_field,
    ));
    lines.join("\n")
}

fn render_status(model: &Model, cols: usize) -> String {
    let hints = match model.mode {
        Mode::View => "[Tab] switch • ↑/↓ move • Enter focus • x kill • n new • a add • e edit • d delete",
        Mode::NewSessionWorkspace => "[Enter] tab step • Tab tab step • Esc cancel • type to edit path",
        Mode::NewSessionTabSelect => "[↑/↓] choose tab • Enter confirm • Esc cancel",
        Mode::NewSessionAgentSelect => "[↑/↓] choose • Enter select/create • Esc cancel",
        Mode::NewSessionAgentCreate => "[Tab] next field • Enter save+launch • Esc cancel",
        Mode::AgentFormCreate | Mode::AgentFormEdit => "[Tab] next field • Enter save • Esc cancel",
        Mode::DeleteConfirm => "[Enter/y] confirm • [Esc/n] cancel",
    };
    let msg = if !model.error_message.is_empty() {
        format!("ERROR: {}", model.error_message)
    } else if model.status_message.is_empty() {
        hints.to_string()
    } else {
        format!("{} — {}", model.status_message, hints)
    };
    truncate(&msg, cols)
}

// ---------- Helpers ----------

fn filtered_sessions(model: &Model) -> Vec<Session> {
    if model.workspaces.is_empty() {
        return model.sessions.clone();
    }
    let ws_idx = model.selected_workspace.min(model.workspaces.len().saturating_sub(1));
    let ws_path = model.workspaces.get(ws_idx).map(|w| w.path.clone());
    match ws_path {
        Some(path) => model
            .sessions
            .iter()
            .filter(|s| s.workspace_path == path)
            .cloned()
            .collect(),
        None => Vec::new(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn build_command_with_env(agent: &Agent) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(env) = &agent.env {
        for (k, v) in env {
            parts.push(format!("{}={}", k, v));
        }
    }
    parts.extend(agent.command.clone());
    parts
}

fn workspace_basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn workspace_tab_name(path: &str) -> String {
    let base = if path.is_empty() {
        "workspace".to_string()
    } else {
        workspace_basename(path)
    };
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let hash = hasher.finish();
    let short = format!("{hash:016x}");
    let suffix = &short[..6.min(short.len())];
    format!("maestro:{base}:{suffix}")
}

#[derive(Debug, Clone)]
pub enum TabChoice {
    Existing(String),
    New,
}

fn selected_tab_choice(model: &Model) -> TabChoice {
    if model.wizard_tab_idx < model.tab_names.len() {
        TabChoice::Existing(model.tab_names[model.wizard_tab_idx].clone())
    } else {
        TabChoice::New
    }
}

fn is_maestro_tab(title: &str) -> bool {
    title.starts_with("maestro:")
}

fn parse_title_hint(title: &str) -> Option<(String, String)> {
    if !is_maestro_tab(title) {
        return None;
    }
    let parts: Vec<&str> = title.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let agent = parts.get(1).unwrap_or(&"").to_string();
    let workspace_hint = parts.get(2).unwrap_or(&"").to_string();
    Some((agent, workspace_hint))
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
    match model.agent_form_field {
        AgentFormField::Name => handle_text_edit(&mut model.agent_name_input, key),
        AgentFormField::Command => handle_text_edit(&mut model.agent_command_input, key),
        AgentFormField::Env => handle_text_edit(&mut model.agent_env_input, key),
        AgentFormField::Note => handle_text_edit(&mut model.agent_note_input, key),
    }
}

fn next_field(current: AgentFormField) -> AgentFormField {
    match current {
        AgentFormField::Name => AgentFormField::Command,
        AgentFormField::Command => AgentFormField::Env,
        AgentFormField::Env => AgentFormField::Note,
        AgentFormField::Note => AgentFormField::Name,
    }
}

fn prev_field(current: AgentFormField) -> AgentFormField {
    match current {
        AgentFormField::Name => AgentFormField::Note,
        AgentFormField::Command => AgentFormField::Name,
        AgentFormField::Env => AgentFormField::Command,
        AgentFormField::Note => AgentFormField::Env,
    }
}

fn parse_env_input(input: &str) -> Result<Option<BTreeMap<String, String>>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let mut map = BTreeMap::new();
    for pair in trimmed.split(',') {
        if pair.trim().is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts
            .next()
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        let val = parts.next().map(str::trim).unwrap_or("").to_string();
        if key.is_empty() {
            return Err("env entries must be KEY=VAL".to_string());
        }
        map.insert(key, val);
    }
    Ok(Some(map))
}

// ---------- Plugin registration ----------

register_plugin!(Maestro);

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use zellij_tile::prelude::{PaneId, PaneInfo, PaneManifest};

    #[test]
    fn pane_update_adds_session() {
        let mut model = Model::default();
        let pane = PaneInfo {
            id: 1,
            is_plugin: false,
            is_focused: false,
            is_fullscreen: false,
            is_floating: false,
            is_suppressed: false,
            title: "maestro:test:ws:uuid".to_string(),
            exited: false,
            exit_status: None,
            is_held: false,
            pane_x: 0,
            pane_content_x: 0,
            pane_y: 0,
            pane_content_y: 0,
            pane_rows: 1,
            pane_content_rows: 1,
            pane_columns: 1,
            pane_content_columns: 1,
            cursor_coordinates_in_pane: None,
            terminal_command: None,
            plugin_url: None,
            is_selectable: true,
            index_in_pane_group: Default::default(),
        };
        model.apply_pane_update(PaneManifest {
            panes: [(0_usize, vec![pane])].into_iter().collect(),
        });
        assert_eq!(model.sessions.len(), 1);
        assert_eq!(model.sessions[0].pane_id, Some(1));
    }

    #[test]
    fn pane_closed_removes_session() {
        let mut model = Model::default();
        model.sessions.push(Session {
            tab_name: "maestro:test".to_string(),
            pane_id: Some(5),
            workspace_path: "/tmp/ws".to_string(),
            agent_name: "a".to_string(),
            status: SessionStatus::Running,
        });
        model.handle_pane_closed(PaneId::Terminal(5));
        assert!(model.sessions.is_empty());
    }

    #[test]
    fn spawn_session_builds_title_and_context() {
        let mut model = Model {
            permissions_granted: true,
            ..Default::default()
        };
        model.agents.push(Agent {
            name: "codex".to_string(),
            command: vec!["echo".to_string(), "hi".to_string()],
            env: None,
            note: None,
        });
        model.spawn_session("/tmp/ws".to_string(), "codex".to_string());
        assert_eq!(model.sessions.len(), 1);
        assert!(model.sessions[0].tab_name.starts_with("maestro:codex:ws"));
        assert_eq!(model.sessions[0].workspace_path, "/tmp/ws");
    }

    #[test]
    fn truncate_shortens_strings() {
        assert_eq!(truncate("hello", 3), "hel…");
        assert_eq!(truncate("hi", 10), "hi");
    }

    #[test]
    fn selections_clamp_and_move() {
        let mut model = Model::default();
        model.workspaces = vec![
            Workspace {
                path: "/a".to_string(),
                name: "a".to_string(),
            },
            Workspace {
                path: "/b".to_string(),
                name: "b".to_string(),
            },
        ];
        model.sessions = vec![
            Session {
                tab_name: "maestro:a:1".to_string(),
                pane_id: Some(1),
                workspace_path: "/a".to_string(),
                agent_name: "alpha".to_string(),
                status: SessionStatus::Running,
            },
            Session {
                tab_name: "maestro:a:2".to_string(),
                pane_id: Some(2),
                workspace_path: "/a".to_string(),
                agent_name: "beta".to_string(),
                status: SessionStatus::Running,
            },
            Session {
                tab_name: "maestro:b:1".to_string(),
                pane_id: Some(3),
                workspace_path: "/b".to_string(),
                agent_name: "gamma".to_string(),
                status: SessionStatus::Running,
            },
        ];
        model.agents = vec![
            Agent {
                name: "one".to_string(),
                command: vec!["cmd".to_string()],
                env: None,
                note: None,
            },
            Agent {
                name: "two".to_string(),
                command: vec!["cmd".to_string()],
                env: None,
                note: None,
            },
        ];

        model.selected_workspace = 5;
        model.selected_session = 5;
        model.selected_agent = 5;
        model.clamp_selections();

        assert_eq!(model.selected_workspace, 1);
        assert_eq!(model.selected_session, 0);
        assert_eq!(model.selected_agent, 1);

        model.move_selection(Section::Workspaces, -1);
        assert_eq!(model.selected_workspace, 0);
        model.move_selection(Section::Sessions, 5);
        assert_eq!(model.selected_session, 1);
        model.move_selection(Section::Agents, 2);
        assert_eq!(model.selected_agent, 1);
    }

    #[test]
    fn key_events_cycle_sections_and_move() {
        let mut model = Model::default();
        model.permissions_granted = true;
        model.workspaces = vec![Workspace {
            path: "/a".to_string(),
            name: "a".to_string(),
        }];
        model.sessions = vec![
            Session {
                tab_name: "maestro:a:1".to_string(),
                pane_id: Some(1),
                workspace_path: "/a".to_string(),
                agent_name: "alpha".to_string(),
                status: SessionStatus::Running,
            },
            Session {
                tab_name: "maestro:a:2".to_string(),
                pane_id: Some(2),
                workspace_path: "/a".to_string(),
                agent_name: "beta".to_string(),
                status: SessionStatus::Running,
            },
        ];
        model.agents = vec![Agent {
            name: "one".to_string(),
            command: vec!["cmd".to_string()],
            env: None,
            note: None,
        }];

        model.handle_key_event(KeyWithModifier {
            bare_key: BareKey::Tab,
            key_modifiers: BTreeSet::new(),
        });
        assert_eq!(model.focused_section, Section::Sessions);
        model.handle_key_event(KeyWithModifier {
            bare_key: BareKey::Down,
            key_modifiers: BTreeSet::new(),
        });
        assert_eq!(model.selected_session, 1);
        model.handle_key_event(KeyWithModifier {
            bare_key: BareKey::Esc,
            key_modifiers: BTreeSet::new(),
        });
        assert_eq!(model.focused_section, Section::Workspaces);
    }
}
