//! Event handlers for Zellij plugin events.

mod forms;
mod keys;
mod panes;
mod session;

pub use keys::handle_key_event;
pub use panes::{focus_selected, kill_selected, spawn_agent_pane, TabChoice};
pub use session::{
    apply_pane_update, apply_tab_update, handle_command_pane_exited, handle_command_pane_opened,
    handle_command_pane_rerun, handle_pane_closed, handle_permission_result, handle_session_update,
};
