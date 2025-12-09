//! Error types for user-facing operations.

/// User-facing errors from Maestro operations.
#[derive(Debug, thiserror::Error)]
pub enum MaestroError {
    #[error("Agent name required")]
    AgentNameRequired,

    #[error("Command required")]
    CommandRequired,

    #[error("Duplicate agent name: {0}")]
    DuplicateAgentName(String),

    #[error("No agent selected")]
    NoAgentSelected,

    #[error("Invalid mode")]
    InvalidMode,

    #[error("Config error: {0}")]
    Config(#[from] anyhow::Error),

    #[error("Permissions not granted")]
    PermissionsNotGranted,

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("No agent panes")]
    NoAgentPanes,

    #[error("Pane ID not available yet")]
    PaneIdUnavailable,

    #[error("No agents to edit")]
    NoAgentsToEdit,

    #[error("No agents to delete")]
    NoAgentsToDelete,

    #[error("Cannot delete default agent: {0}")]
    CannotDeleteDefaultAgent(String),
}

/// Result type for user-facing Maestro operations.
pub type MaestroResult<T> = Result<T, MaestroError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(
            MaestroError::AgentNameRequired.to_string(),
            "Agent name required"
        );
        assert_eq!(
            MaestroError::CommandRequired.to_string(),
            "Command required"
        );
        assert_eq!(
            MaestroError::DuplicateAgentName("test".to_string()).to_string(),
            "Duplicate agent name: test"
        );
        assert_eq!(
            MaestroError::NoAgentSelected.to_string(),
            "No agent selected"
        );
        assert_eq!(MaestroError::InvalidMode.to_string(), "Invalid mode");
        assert_eq!(
            MaestroError::PermissionsNotGranted.to_string(),
            "Permissions not granted"
        );
        assert_eq!(
            MaestroError::AgentNotFound("claude".to_string()).to_string(),
            "Agent not found: claude"
        );
        assert_eq!(MaestroError::NoAgentPanes.to_string(), "No agent panes");
        assert_eq!(
            MaestroError::PaneIdUnavailable.to_string(),
            "Pane ID not available yet"
        );
        assert_eq!(
            MaestroError::NoAgentsToEdit.to_string(),
            "No agents to edit"
        );
        assert_eq!(
            MaestroError::NoAgentsToDelete.to_string(),
            "No agents to delete"
        );
        assert_eq!(
            MaestroError::CannotDeleteDefaultAgent("claude".to_string()).to_string(),
            "Cannot delete default agent: claude"
        );
    }

    #[test]
    fn test_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("test error");
        let err = MaestroError::Config(anyhow_err);
        assert!(err.to_string().contains("test error"));
    }
}
