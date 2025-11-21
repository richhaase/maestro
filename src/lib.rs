use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use zellij_tile::prelude::*;
use zellij_tile::prelude::PermissionStatus;

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
            _ => true, // TODO: wire state machine and actions.
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
            "Maestro plugin initializing...\nViewport: {}x{}\n(To be implemented)",
            cols, rows
        );
        print!("{}", text);
    }
}

// Placeholder structs for upcoming persistence/state work.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Default)]
pub struct Model {
    pub permissions_granted: bool,
    pub permissions_denied: bool,
    // TODO: add in-memory maps for agents, sessions, workspaces.
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
}

register_plugin!(Maestro);
