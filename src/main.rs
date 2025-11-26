use std::collections::BTreeMap;

use zellij_tile::prelude::*;

use maestro::agent::load_agents_default;
use maestro::handlers::{
    apply_pane_update, apply_tab_update, handle_command_pane_exited, handle_command_pane_opened,
    handle_command_pane_rerun, handle_key_event, handle_pane_closed, handle_permission_result,
    handle_session_update,
};
use maestro::model::Model;
use maestro::ui::{render_permissions_denied, render_permissions_requesting, render_ui};

const REQUESTED_PERMISSIONS: &[PermissionType] = &[
    PermissionType::ReadApplicationState,
    PermissionType::ChangeApplicationState,
    PermissionType::OpenFiles,
    PermissionType::FullHdAccess,
    PermissionType::RunCommands,
    PermissionType::OpenTerminalsOrPlugins,
];

#[derive(Default)]
pub struct Maestro {
    model: Model,
}

impl ZellijPlugin for Maestro {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        match load_agents_default() {
            Ok(list) => *self.model.agents_mut() = list,
            Err(err) => {
                eprintln!("maestro: load agents: {err}");
                *self.model.agents_mut() = Vec::new();
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
                handle_permission_result(&mut self.model, status);
                true
            }
            Event::TabUpdate(tabs) => {
                apply_tab_update(&mut self.model, tabs);
                true
            }
            Event::PaneUpdate(manifest) => {
                apply_pane_update(&mut self.model, manifest);
                true
            }
            Event::SessionUpdate(session_info, _resurrectable) => {
                handle_session_update(&mut self.model, session_info);
                true
            }
            Event::CommandPaneOpened(pane_id, ctx) => {
                handle_command_pane_opened(&mut self.model, pane_id, ctx);
                true
            }
            Event::CommandPaneExited(pane_id, exit_status, ctx) => {
                handle_command_pane_exited(&mut self.model, pane_id, exit_status, ctx);
                true
            }
            Event::CommandPaneReRun(pane_id, ctx) => {
                handle_command_pane_rerun(&mut self.model, pane_id, ctx);
                true
            }
            Event::PaneClosed(pane_id) => {
                handle_pane_closed(&mut self.model, pane_id);
                true
            }
            Event::Key(key) => {
                handle_key_event(&mut self.model, key);
                true
            }
            Event::BeforeClose => true,
            _ => true,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        if self.model.permissions_denied() {
            print!("{}", render_permissions_denied(rows, cols));
            return;
        }
        if !self.model.permissions_granted() {
            print!("{}", render_permissions_requesting(rows, cols));
            return;
        }

        print!("{}", render_ui(&self.model, rows, cols));
    }
}

register_plugin!(Maestro);
