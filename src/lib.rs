//! Maestro - A Zellij plugin for managing AI coding agent panes.
//!
//! This plugin provides a UI for spawning, managing, and navigating between
//! terminal panes running AI coding agents like Claude, Cursor, Gemini, etc.

pub mod agent;
pub mod error;
pub mod handlers;
pub mod model;
pub mod ui;
pub mod utils;

/// The WASI host filesystem mount point used by Zellij plugins.
pub const WASI_HOST_MOUNT: &str = "/host";

pub use agent::{Agent, AgentPane, PaneStatus};
pub use error::{MaestroError, MaestroResult};
pub use model::Model;
pub use ui::{AgentFormField, Mode};

#[cfg(test)]
pub mod test_helpers {
    use crate::Agent;

    /// Create a test agent with the given name.
    pub fn create_test_agent(name: &str) -> Agent {
        Agent {
            name: name.to_string(),
            command: "echo".to_string(),
            args: vec![name.to_string()],
            note: None,
        }
    }
}
