use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use zellij_tile::prelude::*;

mod agents;

// Permissions we intend to request for the MVP.
const REQUESTED_PERMISSIONS: &[PermissionType] = &[
    PermissionType::ReadApplicationState,
    PermissionType::ChangeApplicationState,
    PermissionType::RunCommands,
    PermissionType::OpenTerminalsOrPlugins,
];

pub struct Maestro;

impl Default for Maestro {
    fn default() -> Self {
        Maestro
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
        ]);
    }

    fn update(&mut self, _event: Event) -> bool {
        // TODO: wire state machine and actions.
        true
    }

    fn render(&mut self, rows: usize, cols: usize) {
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
    // TODO: add in-memory maps for agents, sessions, workspaces.
}

register_plugin!(Maestro);
