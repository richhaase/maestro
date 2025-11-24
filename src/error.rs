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

    #[error("Env parse error: {0}")]
    EnvParse(String),
}

pub type MaestroResult<T> = Result<T, MaestroError>;
