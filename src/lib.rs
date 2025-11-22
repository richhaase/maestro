use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zellij_tile::prelude::{PaneId, PaneManifest, PermissionStatus, TabInfo};
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

impl ZellijPlugin for Maestro {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        // Load agents from disk; keep errors non-fatal for now.
        match agents::load_agents() {
            Ok(list) => self.model.agents = list,
            Err(err) => {
                // Surface in UI later; for now, keep empty list on failure.
                eprintln!("maestro: load agents: {err}");
                self.model.agents = Vec::new();
            }
        }

        // Request permissions once at load. We apply minimal set defined above.
        request_permission(REQUESTED_PERMISSIONS);

        // Subscribe to core events; will be refined as features land.
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
            Event::CommandPaneOpened(pane_id, ctx) => {
                self.model.handle_command_pane_opened(pane_id, ctx);
                true
            }
            Event::PaneClosed(pane_id) => {
                self.model.handle_pane_closed(pane_id);
                true
            }
            Event::BeforeClose => true,
            _ => true, // TODO: handle more events as actions are added.
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        // If permissions were denied or still pending, show that prominently.
        if self.model.permissions_denied {
            let text = format!(
                "Maestro: permissions denied.\nGrant the requested permissions and reload.\nViewport: {}x{}",
                cols, rows
            );
            print!("{}", text);
            return;
        }
        if !self.model.permissions_granted {
            let text = format!(
                "Maestro requesting permissions...\nViewport: {}x{}",
                cols, rows
            );
            print!("{}", text);
            return;
        }

        let ui = render_ui(&self.model, rows, cols);
        print!("{}", ui);
    }
}

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
pub struct Session {
    pub tab_name: String,
    pub pane_id: Option<u32>,
    pub workspace_path: String,
    pub agent_name: String,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionStatus {
    Running,
    Exited(Option<i32>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct Model {
    pub permissions_granted: bool,
    pub permissions_denied: bool,
    pub agents: Vec<Agent>,
    pub sessions: Vec<Session>,
    pub workspaces: Vec<Workspace>,
    pub status_message: String,
    pub error_message: String,
    pub selected_workspace: usize,
    pub selected_session: usize,
    pub selected_agent: usize,
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

    pub fn spawn_session(&mut self, workspace_path: String, agent_name: String) {
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
        let cmd = build_command_with_env(&agent);
        let mut ctx = BTreeMap::new();
        ctx.insert("pane_title".to_string(), title.clone());
        if !workspace_path.is_empty() {
            ctx.insert("cwd".to_string(), workspace_path.clone());
        }
        ctx.insert("agent".to_string(), agent.name.clone());

        // Fire and forget; pane_id arrives via events.
        open_command_pane(CommandToRun::new(cmd.join(" ")), ctx);

        self.sessions.push(Session {
            tab_name: title,
            pane_id: None,
            workspace_path,
            agent_name,
            status: SessionStatus::Running,
        });
        self.rebuild_workspaces();
        self.error_message.clear();
        self.status_message = "Session launched".to_string();
    }

    pub fn focus_selected(&mut self, selected_idx: usize) {
        if !self.permissions_granted {
            self.error_message = "permissions not granted".to_string();
            return;
        }
        if selected_idx >= self.sessions.len() {
            self.error_message = "no sessions".to_string();
            return;
        }
        let sess = &self.sessions[selected_idx];
        let mut focused = false;
        if !sess.tab_name.is_empty() {
            go_to_tab_name(&sess.tab_name);
            focused = true;
        }
        if !focused {
            if let Some(pid) = sess.pane_id {
                focus_terminal_pane(pid, false);
                focused = true;
            }
        }
        if focused {
            self.error_message.clear();
            self.status_message = "Focused session".to_string();
        } else {
            self.error_message = "no valid target to focus".to_string();
        }
    }

    pub fn kill_selected(&mut self, selected_idx: usize) {
        if !self.permissions_granted {
            self.error_message = "permissions not granted".to_string();
            return;
        }
        if selected_idx >= self.sessions.len() {
            self.error_message = "no sessions".to_string();
            return;
        }
        let sess = self.sessions.get(selected_idx).cloned();
        if let Some(sess) = sess {
            let mut killed = false;
            if let Some(pid) = sess.pane_id {
                close_terminal_pane(pid);
                killed = true;
            }
            if killed {
                self.sessions
                    .retain(|s| s.tab_name != sess.tab_name || s.pane_id != sess.pane_id);
                self.rebuild_workspaces();
                self.error_message.clear();
                self.status_message = "Killed session".to_string();
            } else {
                self.error_message = "no valid target to kill".to_string();
            }
        }
    }

    fn apply_tab_update(&mut self, tabs: Vec<TabInfo>) {
        // Drop sessions whose tab no longer exists.
        let tab_names: Vec<String> = tabs.iter().map(|t| t.name.clone()).collect();
        self.sessions
            .retain(|s| s.pane_id.is_some() || tab_names.contains(&s.tab_name));
        self.rebuild_workspaces();
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
                } else {
                    self.sessions.push(Session {
                        tab_name: title,
                        pane_id: Some(pane.id),
                        workspace_path: String::new(),
                        agent_name: String::new(),
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
    }

    fn handle_pane_closed(&mut self, pane_id: PaneId) {
        let pid = match pane_id {
            PaneId::Terminal(id) | PaneId::Plugin(id) => id,
        };
        self.sessions.retain(|s| s.pane_id != Some(pid));
        self.rebuild_workspaces();
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
}

fn is_maestro_tab(title: &str) -> bool {
    title.starts_with("maestro:")
}

#[cfg(test)]
mod tests {
    use super::*;
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
}

register_plugin!(Maestro);

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

fn render_ui(model: &Model, _rows: usize, cols: usize) -> String {
    let mut out = String::new();

    out.push_str(&render_workspaces(model, cols));
    out.push('\n');
    out.push_str(&render_sessions(model, cols));
    out.push('\n');
    out.push_str(&render_agents(model, cols));
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

fn render_status(model: &Model, cols: usize) -> String {
    let msg = if !model.error_message.is_empty() {
        format!("ERROR: {}", model.error_message)
    } else {
        model.status_message.clone()
    };
    truncate(&msg, cols)
}

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
