use std::collections::BTreeMap;
use std::path::PathBuf;

use uuid::Uuid;
use zellij_tile::prelude::*;
use zellij_tile::prelude::{
    BareKey, KeyModifier, KeyWithModifier, PaneId, PaneManifest, PermissionStatus, TabInfo,
};

mod agent;
mod model;
mod ui;
mod utils;

use agent::{load_agents_default, Agent, AgentPane, PaneStatus};
use model::Model;
use ui::{render_ui, AgentFormField, Mode, Section, next_field, prev_field};
use utils::{build_command_with_env, is_maestro_tab, parse_env_input, parse_title_hint, workspace_basename, workspace_tab_name};

const REQUESTED_PERMISSIONS: &[PermissionType] = &[
    PermissionType::ReadApplicationState,
    PermissionType::ChangeApplicationState,
    PermissionType::OpenFiles,
    PermissionType::FullHdAccess,
    PermissionType::RunCommands,
    PermissionType::OpenTerminalsOrPlugins,
];


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

        for pane in &mut self.agent_panes {
            if pane.tab_name.is_empty() && pane.pane_id.is_some() {}
        }

        self.agent_panes
            .retain(|p| p.pane_id.is_some() || tab_names.contains(&p.tab_name));
        self.clamp_selections();
    }

    fn apply_pane_update(&mut self, update: PaneManifest) {
        for (tab_idx, pane_list) in update.panes {
            let tab_name = self.tab_names.get(tab_idx).cloned().unwrap_or_default();

            for pane in pane_list {
                if let Some(existing) = self
                    .agent_panes
                    .iter_mut()
                    .find(|p| p.pane_id == Some(pane.id))
                {
                    if existing.tab_name.is_empty()
                        || (!tab_name.is_empty() && !self.tab_names.contains(&existing.tab_name))
                    {
                        if !tab_name.is_empty() {
                            existing.tab_name = tab_name.clone();
                        }
                    }
                    existing.status = if pane.exited {
                        PaneStatus::Exited(pane.exit_status)
                    } else {
                        PaneStatus::Running
                    };
                    continue;
                }

                let title = pane.title.clone();

                if is_maestro_tab(&title) {
                    let (agent_name, workspace_path) = parse_title_hint(&title)
                        .map(|(a, w)| (a, w))
                        .unwrap_or_default();
                    self.agent_panes.push(AgentPane {
                        pane_title: title,
                        tab_name: tab_name.clone(),
                        pane_id: Some(pane.id),
                        workspace_path,
                        agent_name,
                        status: if pane.exited {
                            PaneStatus::Exited(pane.exit_status)
                        } else {
                            PaneStatus::Running
                        },
                    });
                    continue;
                }

                if !pane.is_plugin {
                    let title_base = title.split(" - ").next().unwrap_or(&title).trim();
                    if self
                        .agents
                        .iter()
                        .any(|a| a.name.eq_ignore_ascii_case(title_base))
                    {
                        let agent_name = self
                            .agents
                            .iter()
                            .find(|a| a.name.eq_ignore_ascii_case(title_base))
                            .map(|a| a.name.clone())
                            .unwrap_or_else(|| title_base.to_string());
                        let reconstructed_title = format!("maestro:{}::recovered", agent_name);
                        self.agent_panes.push(AgentPane {
                            pane_title: reconstructed_title,
                            tab_name: tab_name.clone(),
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
            .agent_panes
            .iter_mut()
            .find(|p| p.pane_id == Some(pane_id) || (p.pane_id.is_none() && p.pane_title == title));

        if let Some(existing) = entry {
            existing.pane_id = Some(pane_id);
            existing.pane_title = title.clone();

            if existing.tab_name.is_empty() {
                if let Some(tab_name) = ctx.get("tab_name").cloned() {
                    existing.tab_name = tab_name;
                } else if let Some(first_tab) = self.tab_names.first().cloned() {
                    existing.tab_name = first_tab;
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
            let tab_name = ctx
                .get("tab_name")
                .cloned()
                .or_else(|| self.tab_names.first().cloned())
                .unwrap_or_default();
            self.agent_panes.push(AgentPane {
                pane_title: title,
                tab_name,
                pane_id: Some(pane_id),
                workspace_path,
                agent_name,
                status: PaneStatus::Running,
            });
        }
        self.clamp_selections();
    }

    fn rebuild_from_session_infos(&mut self, session_infos: &[SessionInfo]) {
        let has_tab_names = !self.tab_names.is_empty();

        for session in session_infos {
            let session_name = session.name.clone();
            if let Some(current) = &self.session_name {
                    if &session_name != current {
                    continue;
                }
            } else {
                self.session_name = Some(session_name.clone());
            }
            for (tab_idx, pane_list) in session.panes.clone().panes {
                let tab_name = if has_tab_names {
                    self.tab_names.get(tab_idx).cloned().unwrap_or_default()
                } else {
                    String::new()
                };

                let mut unmatched_in_tab: Vec<usize> = if has_tab_names {
                    self.agent_panes
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| p.pane_id.is_none() && p.tab_name == tab_name)
                        .map(|(idx, _)| idx)
                        .collect()
                } else {
                    Vec::new()
                };

                for pane in pane_list {
                    if let Some(existing) = self
                        .agent_panes
                        .iter_mut()
                        .find(|p| p.pane_id == Some(pane.id))
                    {
                        existing.status = if pane.exited {
                            PaneStatus::Exited(pane.exit_status)
                        } else {
                            PaneStatus::Running
                        };

                        if existing.tab_name.is_empty()
                            || (!tab_name.is_empty()
                                && !self.tab_names.contains(&existing.tab_name))
                        {
                            if !tab_name.is_empty() {
                                existing.tab_name = tab_name.clone();
                            }
                        }
                        continue;
                    }

                    if is_maestro_tab(&pane.title) {
                        let (agent_name, workspace_path) = parse_title_hint(&pane.title)
                            .map(|(a, w)| (a, w))
                            .unwrap_or_default();
                        self.agent_panes.push(AgentPane {
                            pane_title: pane.title.clone(),
                            tab_name: tab_name.clone(),
                            pane_id: Some(pane.id),
                            workspace_path,
                            agent_name,
                            status: if pane.exited {
                                PaneStatus::Exited(pane.exit_status)
                            } else {
                                PaneStatus::Running
                            },
                        });
                        continue;
                    }

                    if let Some(unmatched_idx) = unmatched_in_tab.pop() {
                        let existing = &mut self.agent_panes[unmatched_idx];
                        existing.pane_id = Some(pane.id);
                        existing.status = if pane.exited {
                            PaneStatus::Exited(pane.exit_status)
                        } else {
                            PaneStatus::Running
                        };
                        if !tab_name.is_empty() {
                            existing.tab_name = tab_name.clone();
                        }
                        continue;
                    }

                    if !pane.is_plugin {
                        let title_base =
                            pane.title.split(" - ").next().unwrap_or(&pane.title).trim();
                        if self
                            .agents
                            .iter()
                            .any(|a| a.name.eq_ignore_ascii_case(title_base))
                        {
                            let agent_name = self
                                .agents
                                .iter()
                                .find(|a| a.name.eq_ignore_ascii_case(title_base))
                                .map(|a| a.name.clone())
                                .unwrap_or_else(|| title_base.to_string());
                            let reconstructed_title = format!("maestro:{}::recovered", agent_name);
                            self.agent_panes.push(AgentPane {
                                pane_title: reconstructed_title,
                                tab_name: tab_name.clone(),
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
        self.clamp_selections();
    }

    fn rebuild_from_panes_iter(
        &mut self,
        panes_iter: impl Iterator<Item = (usize, Vec<PaneInfo>)>,
    ) {
        for (tab_idx, pane_list) in panes_iter {
            let tab_name = self.tab_names.get(tab_idx).cloned().unwrap_or_default();
            for pane in pane_list {
                let title = pane.title.clone();

                if self.agent_panes.iter().any(|p| p.pane_id == Some(pane.id)) {
                    continue;
                }

                if is_maestro_tab(&title) {
                    let (agent_name, workspace_path) = parse_title_hint(&title)
                        .map(|(a, w)| (a, w))
                        .unwrap_or_default();
                    self.agent_panes.push(AgentPane {
                        pane_title: title,
                        tab_name: tab_name.clone(),
                        pane_id: Some(pane.id),
                        workspace_path,
                        agent_name,
                        status: if pane.exited {
                            PaneStatus::Exited(pane.exit_status)
                        } else {
                            PaneStatus::Running
                        },
                    });
                } else {
                    if let Some(existing) = self
                        .agent_panes
                        .iter_mut()
                        .find(|p| p.pane_id.is_none() && p.tab_name == tab_name)
                    {
                        existing.pane_id = Some(pane.id);
                        existing.status = if pane.exited {
                            PaneStatus::Exited(pane.exit_status)
                        } else {
                            PaneStatus::Running
                        };
                    }
                }
            }
        }
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
        if let Some(pane) = self
            .agent_panes
            .iter_mut()
            .find(|p| p.pane_id == Some(pane_id) || p.pane_title == title)
        {
            pane.status = PaneStatus::Exited(exit_status);
        }
        self.clamp_selections();
    }

    fn handle_command_pane_rerun(&mut self, pane_id: u32, ctx: BTreeMap<String, String>) {
        let title = ctx
            .get("pane_title")
            .cloned()
            .unwrap_or_else(|| format!("maestro:{}", pane_id));
        if let Some(pane) = self
            .agent_panes
            .iter_mut()
            .find(|p| p.pane_id == Some(pane_id) || p.pane_title == title)
        {
            pane.status = PaneStatus::Running;
        }
        self.clamp_selections();
    }

    fn handle_session_update(&mut self, sessions: Vec<SessionInfo>) {
        self.rebuild_from_session_infos(&sessions);
    }

    fn handle_pane_closed(&mut self, pane_id: PaneId) {
        let pid = match pane_id {
            PaneId::Terminal(id) | PaneId::Plugin(id) => id,
        };
        self.agent_panes.retain(|p| p.pane_id != Some(pid));
        self.clamp_selections();
    }

    fn reset_status(&mut self) {
        self.status_message.clear();
        self.error_message.clear();
    }

    fn cancel_to_view(&mut self) {
        self.mode = Mode::View;
        self.quick_launch_agent_name = None;
        self.reset_status();
    }

    fn view_preserve_messages(&mut self) {
        self.mode = Mode::View;
    }

    fn start_new_pane_workspace(&mut self) {
        self.workspace_input.clear();
        self.mode = Mode::NewPaneWorkspace;
        self.wizard_agent_idx = 0;
        self.wizard_tab_idx = 0;
        self.reset_status();
    }

    fn start_agent_create(&mut self) {
        self.mode = Mode::AgentFormCreate;
        self.agent_form_source = Some(Mode::View);
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
            self.agent_form_source = Some(Mode::View);
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

    pub fn spawn_agent_pane(
        &mut self,
        workspace_path: String,
        agent_name: String,
        tab_choice: TabChoice,
    ) {
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
        let (tab_target, _is_new_tab) = match tab_choice {
            TabChoice::Existing(name) => (name, false),
            TabChoice::New => {
                let name = workspace_tab_name(&workspace_path);
                new_tab(Some(name.clone()), Some(workspace_path.clone()));
                if !self.tab_names.contains(&name) {
                    self.tab_names.push(name.clone());
                }
                (name, true)
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

        self.agent_panes.push(AgentPane {
            pane_title: title.clone(),
            tab_name: tab_target.clone(),
            pane_id: None,
            workspace_path,
            agent_name,
            status: PaneStatus::Running,
        });
        self.error_message.clear();
        if self.status_message.is_empty() {
            self.status_message = "Agent pane launched".to_string();
        } else {
            self.status_message = format!("{}; Agent pane launched", self.status_message);
        }
    }

    pub fn focus_selected(&mut self, selected_idx: usize) {
        if !self.permissions_granted {
            self.error_message = "permissions not granted".to_string();
            return;
        }

        let filter_lower = self.filter_text.to_lowercase();
        let panes: Vec<&AgentPane> = if filter_lower.is_empty() {
            self.agent_panes.iter().collect()
        } else {
            self.agent_panes
                .iter()
                .filter(|p| {
                    p.agent_name.to_lowercase().contains(&filter_lower)
                        || p.tab_name.to_lowercase().contains(&filter_lower)
                })
                .collect()
        };

        if selected_idx >= panes.len() {
            self.error_message = "no agent panes".to_string();
            return;
        }
        let pane = panes[selected_idx];
        go_to_tab_name(&pane.tab_name);
        if let Some(pid) = pane.pane_id {
            focus_terminal_pane(pid, false);
            self.error_message.clear();
            self.status_message = "Focused agent pane".to_string();
        } else {
            self.error_message = "Pane ID not available yet".to_string();
        }
    }

    pub fn kill_selected(&mut self, selected_idx: usize) {
        if !self.permissions_granted {
            self.error_message = "permissions not granted".to_string();
            return;
        }

        let filter_lower = self.filter_text.to_lowercase();
        let panes: Vec<&AgentPane> = if filter_lower.is_empty() {
            self.agent_panes.iter().collect()
        } else {
            self.agent_panes
                .iter()
                .filter(|p| {
                    p.agent_name.to_lowercase().contains(&filter_lower)
                        || p.tab_name.to_lowercase().contains(&filter_lower)
                })
                .collect()
        };

        if selected_idx >= panes.len() {
            self.error_message = "no agent panes".to_string();
            return;
        }
        let pane = panes[selected_idx];
        if let Some(pid) = pane.pane_id {
            close_terminal_pane(pid);
            self.agent_panes.retain(|p| p.pane_id != Some(pid));
            self.error_message.clear();
            self.status_message = "Killed agent pane".to_string();
            self.clamp_selections();
        } else {
            self.error_message = "no valid target to kill".to_string();
        }
    }

    fn clamp_selections(&mut self) {
        let filter_lower = self.filter_text.to_lowercase();
        let pane_len = if filter_lower.is_empty() {
            self.agent_panes.len()
        } else {
            self.agent_panes
                .iter()
                .filter(|p| {
                    p.agent_name.to_lowercase().contains(&filter_lower)
                        || p.tab_name.to_lowercase().contains(&filter_lower)
                })
                .count()
        };
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

    fn move_selection(&mut self, section: Section, delta: isize) {
        let (len, current) = match section {
            Section::AgentPanes => {
                let filter_lower = self.filter_text.to_lowercase();
                let panes_len = if filter_lower.is_empty() {
                    self.agent_panes.len()
                } else {
                    self.agent_panes
                        .iter()
                        .filter(|p| {
                            p.agent_name.to_lowercase().contains(&filter_lower)
                                || p.tab_name.to_lowercase().contains(&filter_lower)
                        })
                        .count()
                };
                (panes_len, self.selected_pane)
            }
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
            Section::AgentPanes => self.selected_pane = next,
            Section::Agents => self.selected_agent = next,
        }
        self.status_message.clear();
        self.error_message.clear();
    }

    fn focus_next_section(&mut self) {
        self.focused_section = self.focused_section.next();
        self.status_message.clear();
        self.error_message.clear();
        self.clamp_selections();
    }

    fn handle_key_event(&mut self, key: KeyWithModifier) {
        match self.mode {
            Mode::View => self.handle_key_event_view(key),
            Mode::NewPaneWorkspace => self.handle_key_event_new_pane_workspace(key),
            Mode::NewPaneTabSelect => self.handle_key_event_new_pane_tab_select(key),
            Mode::NewPaneAgentSelect => self.handle_key_event_new_pane_agent_select(key),
            Mode::NewPaneAgentCreate => self.handle_key_event_agent_form(key, true),
            Mode::AgentFormCreate | Mode::AgentFormEdit => {
                self.handle_key_event_agent_form(key, false)
            }
            Mode::DeleteConfirm => self.handle_key_event_delete_confirm(key),
        }
    }

    fn handle_key_event_view(&mut self, key: KeyWithModifier) {
        if !key.key_modifiers.is_empty() && key.bare_key != BareKey::Tab {
            return;
        }

        if self.filter_active {
            match key.bare_key {
                BareKey::Char(c) => {
                    self.filter_text.push(c);
                    self.selected_pane = 0;
                    self.clamp_selections();
                    return;
                }
                BareKey::Up => {
                    self.move_selection(self.focused_section, -1);
                    return;
                }
                BareKey::Down => {
                    self.move_selection(self.focused_section, 1);
                    return;
                }
                BareKey::Backspace => {
                    self.filter_text.pop();
                    self.selected_pane = 0;
                    self.clamp_selections();
                    return;
                }
                BareKey::Esc => {
                    self.filter_active = false;
                    self.filter_text.clear();
                    self.selected_pane = 0;
                    self.clamp_selections();
                    return;
                }
                _ => {}
            }
        }

        match key.bare_key {
            BareKey::Char('j') | BareKey::Char('J') => {
                self.move_selection(self.focused_section, 1);
            }
            BareKey::Char('k') | BareKey::Char('K') => {
                self.move_selection(self.focused_section, -1);
            }
            BareKey::Tab => {
                self.focus_next_section();
            }
            BareKey::Enter => match self.focused_section {
                Section::AgentPanes => {
                    let idx = self.selected_pane;
                    self.focus_selected(idx);

                    close_self();
                }
                Section::Agents => {
                    if self.selected_agent < self.agents.len() {
                        self.start_agent_edit();
                    }
                }
            },
            BareKey::Esc => {
                close_self();
            }
            BareKey::Char('f') | BareKey::Char('F') => {
                if self.focused_section == Section::AgentPanes {
                    self.filter_active = true;
                    self.filter_text.clear();
                    self.selected_pane = 0;
                    self.clamp_selections();
                }
            }
            BareKey::Char('x') | BareKey::Char('X') => {
                if self.focused_section == Section::AgentPanes {
                    let idx = self.selected_pane;
                    self.kill_selected(idx);
                }
            }
            BareKey::Char('e') | BareKey::Char('E') => {
                if self.focused_section == Section::Agents {
                    if self.selected_agent < self.agents.len() {
                        self.start_agent_edit();
                    }
                }
            }
            BareKey::Char('d') | BareKey::Char('D') => {
                if self.focused_section == Section::Agents {
                    if self.selected_agent < self.agents.len() {
                        self.start_agent_delete_confirm();
                    }
                }
            }
            BareKey::Char('n') | BareKey::Char('N') => {
                if self.focused_section == Section::Agents {
                    if self.selected_agent < self.agents.len() {
                        let agent_name = self.agents[self.selected_agent].name.clone();
                        self.quick_launch_agent_name = Some(agent_name);
                        self.start_new_pane_workspace();
                    }
                } else {
                    self.start_new_pane_workspace();
                }
            }
            BareKey::Char('a') | BareKey::Char('A') => {
                if self.focused_section == Section::Agents {
                    self.start_agent_create();
                } else {
                    self.focused_section = Section::Agents;
                    self.clamp_selections();
                }
            }
            _ => {}
        }
    }

    fn handle_key_event_new_pane_workspace(&mut self, key: KeyWithModifier) {
        if handle_text_edit(&mut self.workspace_input, &key) {
            return;
        }
        match key.bare_key {
            BareKey::Enter => {
                self.mode = Mode::NewPaneTabSelect;
                self.wizard_tab_idx = 0;
                self.wizard_agent_idx = 0;
                self.reset_status();
            }
            BareKey::Esc => self.cancel_to_view(),
            BareKey::Tab => {
                self.mode = Mode::NewPaneTabSelect;
                self.wizard_tab_idx = 0;
                self.wizard_agent_idx = 0;
                self.reset_status();
            }
            _ => {}
        }
    }

    fn handle_key_event_new_pane_tab_select(&mut self, key: KeyWithModifier) {
        let choices = self.tab_names.len().saturating_add(1);
        match key.bare_key {
            BareKey::Char('k') | BareKey::Char('K') | BareKey::Up => {
                if self.wizard_tab_idx > 0 {
                    self.wizard_tab_idx -= 1;
                }
            }
            BareKey::Char('j') | BareKey::Char('J') | BareKey::Down => {
                if self.wizard_tab_idx + 1 < choices {
                    self.wizard_tab_idx += 1;
                }
            }
            BareKey::Enter => {
                if let Some(agent_name) = self.quick_launch_agent_name.take() {
                    let workspace = self.workspace_input.trim().to_string();
                    let tab_choice = selected_tab_choice(self);
                    self.spawn_agent_pane(workspace, agent_name, tab_choice);
                    if self.error_message.is_empty() {
                        self.view_preserve_messages();
                    }
                } else {
                    self.mode = Mode::NewPaneAgentSelect;
                    self.wizard_agent_idx = 0;
                }
            }
            BareKey::Esc => self.cancel_to_view(),
            BareKey::Tab => self.cancel_to_view(),
            _ => {}
        }
    }

    fn handle_key_event_new_pane_agent_select(&mut self, key: KeyWithModifier) {
        let choices = self.agents.len().saturating_add(1);
        match key.bare_key {
            BareKey::Char('k') | BareKey::Char('K') | BareKey::Up => {
                if self.wizard_agent_idx > 0 {
                    self.wizard_agent_idx -= 1;
                }
            }
            BareKey::Char('j') | BareKey::Char('J') | BareKey::Down => {
                if self.wizard_agent_idx + 1 < choices {
                    self.wizard_agent_idx += 1;
                }
            }
            BareKey::Enter => {
                if self.wizard_agent_idx < self.agents.len() {
                    let agent = self.agents[self.wizard_agent_idx].name.clone();
                    let workspace = self.workspace_input.trim().to_string();
                    let tab_choice = selected_tab_choice(self);
                    self.spawn_agent_pane(workspace, agent, tab_choice);
                    if self.error_message.is_empty() {
                        self.view_preserve_messages();
                    }
                } else {
                    self.mode = Mode::NewPaneAgentCreate;
                    self.agent_form_source = Some(Mode::NewPaneAgentSelect);
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
        let shift_tab =
            key.bare_key == BareKey::Tab && key.key_modifiers.contains(&KeyModifier::Shift);
        match key.bare_key {
            BareKey::Tab if shift_tab => {
                self.agent_form_field = prev_field(self.agent_form_field);
            }
            BareKey::Tab => {
                self.agent_form_field = next_field(self.agent_form_field);
            }
            BareKey::Enter => match self.build_agent_from_inputs() {
                Ok(agent) => {
                    let result = match self.mode {
                        Mode::AgentFormEdit => self.apply_agent_edit(agent.clone()),
                        Mode::AgentFormCreate | Mode::NewPaneAgentCreate => {
                            self.apply_agent_create(agent.clone())
                        }
                        _ => Err("invalid mode".to_string()),
                    };
                    match result {
                        Ok(saved_path) => {
                            self.status_message =
                                format!("Agents saved to {}", saved_path.display());
                            if launch_after {
                                let workspace = self.workspace_input.trim().to_string();
                                let tab_choice = selected_tab_choice(self);
                                self.spawn_agent_pane(workspace, agent.name.clone(), tab_choice);
                            }
                            if self.error_message.is_empty() {
                                if launch_after {
                                    self.view_preserve_messages();
                                } else {
                                    self.view_preserve_messages();
                                }
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
            },
            BareKey::Esc => {
                self.agent_form_source = None;
                self.cancel_to_view();
            }
            _ => {}
        }
    }

    fn handle_key_event_delete_confirm(&mut self, key: KeyWithModifier) {
        match key.bare_key {
            BareKey::Enter | BareKey::Char('y') | BareKey::Char('Y') => {
                if let Some(idx) = self.form_target_agent.take() {
                    if idx < self.agents.len() {
                        self.agents.remove(idx);
                        self.selected_agent =
                            self.selected_agent.min(self.agents.len().saturating_sub(1));
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

                self.mode = Mode::View;
            }
            BareKey::Esc | BareKey::Char('n') | BareKey::Char('N') => {
                self.mode = Mode::View;
            }
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
        use agent::{default_config_path, load_agents, save_agents};
        let path = default_config_path().map_err(|e| format!("config path: {e}"))?;
        save_agents(&path, &self.agents).map_err(|e| format!("save agents: {e}"))?;
        match load_agents(&path) {
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

impl ZellijPlugin for Maestro {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        match load_agents_default() {
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
                self.model
                    .handle_command_pane_exited(pane_id, exit_status, ctx);
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

#[derive(Debug, Clone, PartialEq)]
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


register_plugin!(Maestro);
