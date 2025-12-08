pub mod agent;
pub mod error;
pub mod handlers;
pub mod model;
pub mod ui;
pub mod utils;

pub const WASI_HOST_MOUNT: &str = "/host";

pub use agent::{Agent, AgentPane, PaneStatus};
pub use error::{MaestroError, MaestroResult};
pub use model::Model;
pub use ui::{AgentFormField, Mode};

#[cfg(test)]
pub mod test_helpers {
    use crate::Agent;

    pub fn create_test_agent(name: &str) -> Agent {
        Agent {
            name: name.to_string(),
            command: "echo".to_string(),
            args: Some(vec![name.to_string()]),
            note: None,
        }
    }
}
