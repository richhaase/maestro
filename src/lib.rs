use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use zellij_tile::prelude::{PaneId, PaneManifest, PermissionStatus, TabInfo};
use zellij_tile::prelude::*;

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

        // Placeholder UI: claim the space to avoid blank pane warnings.
        let text = format!(
            "Maestro plugin initializing...\nViewport: {}x{}\n(To be implemented)\nSessions tracked: {}",
            cols,
            rows,
            self.model.sessions.len()
        );
        print!("{}", text);
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

    #[test]
    fn pane_update_adds_session() {
        let mut model = Model::default();
        let pane = Pane {
            id: 1,
            is_plugin: false,
            is_focused: false,
            is_tiled: true,
            is_stacked: false,
            title: Some("maestro:test:ws:uuid".to_string()),
            ..Default::default()
        };
        model.apply_pane_update(PaneUpdate {
            panes: vec![pane],
            ..Default::default()
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
        });
        model.handle_pane_closed(5);
        assert!(model.sessions.is_empty());
    }
}

register_plugin!(Maestro);
