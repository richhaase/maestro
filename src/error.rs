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
}

pub type MaestroResult<T> = Result<T, MaestroError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MaestroError::AgentNameRequired;
        assert_eq!(err.to_string(), "Agent name required");

        let err = MaestroError::DuplicateAgentName("test".to_string());
        assert_eq!(err.to_string(), "Duplicate agent name: test");

        let err = MaestroError::CommandRequired;
        assert_eq!(err.to_string(), "Command required");

        let err = MaestroError::NoAgentSelected;
        assert_eq!(err.to_string(), "No agent selected");

        let err = MaestroError::InvalidMode;
        assert_eq!(err.to_string(), "Invalid mode");
    }

    #[test]
    fn test_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("test error");
        let err = MaestroError::Config(anyhow_err);
        assert!(err.to_string().contains("test error"));
    }
}
