//! Error types for user-facing operations.

use std::path::PathBuf;

/// User-facing errors from Maestro operations.
#[derive(Debug, thiserror::Error)]
pub enum MaestroError {
    // Agent validation errors
    #[error("Agent name required")]
    AgentNameRequired,

    #[error("Command required")]
    CommandRequired,

    #[error("Duplicate agent name: {0}")]
    DuplicateAgentName(String),

    // Agent selection errors
    #[error("No agent selected")]
    NoAgentSelected,

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("No agents to edit")]
    NoAgentsToEdit,

    #[error("No agents to delete")]
    NoAgentsToDelete,

    #[error("Cannot delete default agent: {0}")]
    CannotDeleteDefaultAgent(String),

    // File I/O errors
    #[error("Failed to read {path}: {message}")]
    FileRead { path: PathBuf, message: String },

    #[error("Failed to write {path}: {message}")]
    FileWrite { path: PathBuf, message: String },

    #[error("Failed to create directory {path}: {message}")]
    DirectoryCreate { path: PathBuf, message: String },

    // Config parsing errors
    #[error("Failed to parse config: {0}")]
    ConfigParse(String),

    #[error("Invalid agent config: {0}")]
    InvalidAgentConfig(String),

    // Runtime errors
    #[error("Invalid mode")]
    InvalidMode,

    #[error("Permissions not granted")]
    PermissionsNotGranted,

    #[error("No agent panes")]
    NoAgentPanes,

    #[error("Pane ID not available yet")]
    PaneIdUnavailable,
}

/// Result type for user-facing Maestro operations.
pub type MaestroResult<T> = Result<T, MaestroError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_validation() {
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
    }

    #[test]
    fn test_error_display_selection() {
        assert_eq!(
            MaestroError::NoAgentSelected.to_string(),
            "No agent selected"
        );
        assert_eq!(
            MaestroError::AgentNotFound("claude".to_string()).to_string(),
            "Agent not found: claude"
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
    fn test_error_display_file_io() {
        assert_eq!(
            MaestroError::FileRead {
                path: PathBuf::from("/test/path"),
                message: "not found".to_string()
            }
            .to_string(),
            "Failed to read /test/path: not found"
        );
        assert_eq!(
            MaestroError::FileWrite {
                path: PathBuf::from("/test/path"),
                message: "permission denied".to_string()
            }
            .to_string(),
            "Failed to write /test/path: permission denied"
        );
        assert_eq!(
            MaestroError::DirectoryCreate {
                path: PathBuf::from("/test/dir"),
                message: "exists".to_string()
            }
            .to_string(),
            "Failed to create directory /test/dir: exists"
        );
    }

    #[test]
    fn test_error_display_config() {
        assert_eq!(
            MaestroError::ConfigParse("invalid syntax".to_string()).to_string(),
            "Failed to parse config: invalid syntax"
        );
        assert_eq!(
            MaestroError::InvalidAgentConfig("missing name".to_string()).to_string(),
            "Invalid agent config: missing name"
        );
    }

    #[test]
    fn test_error_display_runtime() {
        assert_eq!(MaestroError::InvalidMode.to_string(), "Invalid mode");
        assert_eq!(
            MaestroError::PermissionsNotGranted.to_string(),
            "Permissions not granted"
        );
        assert_eq!(MaestroError::NoAgentPanes.to_string(), "No agent panes");
        assert_eq!(
            MaestroError::PaneIdUnavailable.to_string(),
            "Pane ID not available yet"
        );
    }
}
