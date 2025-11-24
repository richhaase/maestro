use std::collections::BTreeMap;

use zellij_tile::prelude::*;

use maestro::agent::load_agents_default;
use maestro::model::Model;
use maestro::ui::render_ui;

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



register_plugin!(Maestro);
