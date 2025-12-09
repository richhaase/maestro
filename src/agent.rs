//! Agent configuration and persistence.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use kdl::{KdlDocument, KdlNode};
use serde::{Deserialize, Serialize};

use crate::error::{MaestroError, MaestroResult};

/// An AI coding agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    /// Display name for the agent.
    pub name: String,
    /// Command to run (e.g., "claude", "cursor-agent").
    pub command: String,
    /// Command-line arguments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Optional description or notes.
    #[serde(default)]
    pub note: Option<String>,
}

/// Runtime status of an agent pane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaneStatus {
    /// The pane is currently running.
    Running,
    /// The pane has exited with an optional exit code.
    Exited(Option<i32>),
}

/// A running instance of an agent in a Zellij pane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPane {
    /// Unique pane title (includes UUID for disambiguation).
    pub pane_title: String,
    /// Name of the tab containing this pane.
    pub tab_name: String,
    /// Tab index while waiting for tab name resolution.
    pub pending_tab_index: Option<usize>,
    /// Zellij pane ID once assigned.
    pub pane_id: Option<u32>,
    /// Working directory for the agent.
    pub workspace_path: String,
    /// Name of the agent configuration.
    pub agent_name: String,
    /// Current execution status.
    pub status: PaneStatus,
}

/// Load agents from a KDL configuration file.
pub fn load_agents(path: &Path) -> MaestroResult<Vec<Agent>> {
    let data = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(MaestroError::FileRead {
                path: path.to_path_buf(),
                message: e.to_string(),
            })
        }
    };

    if data.trim().is_empty() {
        return Ok(Vec::new());
    }

    let doc: KdlDocument = data
        .parse()
        .map_err(|e: kdl::KdlError| MaestroError::ConfigParse(e.to_string()))?;
    let mut agents = Vec::new();
    for node in doc.nodes() {
        if node.name().value() != "agent" {
            continue;
        }
        agents.push(agent_from_kdl(node)?);
    }
    validate_agents(&agents)?;
    Ok(agents)
}

/// Save agents to a KDL configuration file.
pub fn save_agents(path: &Path, agents: &[Agent]) -> MaestroResult<()> {
    validate_agents(agents)?;
    let payload = agents_to_kdl(agents);
    fs::write(path, payload.as_bytes()).map_err(|e| MaestroError::FileWrite {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    Ok(())
}

/// Get the default configuration file path (`~/.config/maestro/agents.kdl`).
pub fn default_config_path() -> PathBuf {
    config_base_dir().join("agents.kdl")
}

/// Get the built-in default agents.
pub fn default_agents() -> Vec<Agent> {
    vec![
        Agent {
            name: "cursor".to_string(),
            command: "cursor-agent".to_string(),
            args: Vec::new(),
            note: Some("Default agent config".to_string()),
        },
        Agent {
            name: "claude".to_string(),
            command: "claude".to_string(),
            args: Vec::new(),
            note: Some("Default agent config".to_string()),
        },
        Agent {
            name: "gemini".to_string(),
            command: "gemini".to_string(),
            args: Vec::new(),
            note: Some("Default agent config".to_string()),
        },
        Agent {
            name: "codex".to_string(),
            command: "codex".to_string(),
            args: Vec::new(),
            note: Some("Default agent config".to_string()),
        },
    ]
}

/// Check if an agent name is one of the built-in defaults.
pub fn is_default_agent(name: &str) -> bool {
    matches!(
        name.trim().to_lowercase().as_str(),
        "cursor" | "claude" | "gemini" | "codex"
    )
}

/// Load agents, merging user config with built-in defaults.
pub fn load_agents_default() -> MaestroResult<Vec<Agent>> {
    let path = default_config_path();
    let user_agents = load_agents(&path)?;

    let mut merged = default_agents();
    let default_names: std::collections::BTreeSet<String> =
        merged.iter().map(|a| a.name.to_lowercase()).collect();

    for user_agent in user_agents {
        let user_name_normalized = user_agent.name.to_lowercase();
        if default_names.contains(&user_name_normalized) {
            if let Some(pos) = merged
                .iter()
                .position(|a| a.name.to_lowercase() == user_name_normalized)
            {
                merged[pos] = user_agent;
            }
        } else {
            merged.push(user_agent);
        }
    }

    merged.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(merged)
}

const MAX_AGENT_NAME_LENGTH: usize = 64;

fn validate_agent_name(name: &str) -> MaestroResult<()> {
    if name.chars().any(|c| c.is_control()) {
        return Err(MaestroError::InvalidAgentName(
            "cannot contain control characters".to_string(),
        ));
    }
    if name.len() > MAX_AGENT_NAME_LENGTH {
        return Err(MaestroError::InvalidAgentName(format!(
            "cannot exceed {} characters",
            MAX_AGENT_NAME_LENGTH
        )));
    }
    Ok(())
}

fn validate_agents(agents: &[Agent]) -> MaestroResult<()> {
    let mut seen = BTreeSet::new();
    for agent in agents {
        let name = agent.name.trim();
        if name.is_empty() {
            return Err(MaestroError::AgentNameRequired);
        }
        validate_agent_name(name)?;
        if agent.command.trim().is_empty() {
            return Err(MaestroError::CommandRequired);
        }
        let normalized = name.to_lowercase();
        if !seen.insert(normalized) {
            return Err(MaestroError::DuplicateAgentName(name.to_string()));
        }
    }
    Ok(())
}

fn agent_from_kdl(node: &KdlNode) -> MaestroResult<Agent> {
    let name_val = node
        .get("name")
        .and_then(|e| e.value().as_string())
        .ok_or_else(|| MaestroError::InvalidAgentConfig("missing name".to_string()))?;
    let note = node
        .get("note")
        .and_then(|e| e.value().as_string())
        .map(|s| s.to_string());

    let mut command = String::new();
    let mut args: Vec<String> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "cmd" => {
                    let mut entries = child.entries().iter();
                    if let Some(first) = entries.next() {
                        command = first
                            .value()
                            .as_string()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| first.value().to_string());
                    }
                    for entry in entries {
                        if let Some(s) = entry.value().as_string() {
                            args.push(s.to_string());
                        } else {
                            args.push(entry.value().to_string());
                        }
                    }
                }
                "args" => {
                    for entry in child.entries() {
                        if let Some(s) = entry.value().as_string() {
                            args.push(s.to_string());
                        } else {
                            args.push(entry.value().to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(Agent {
        name: name_val.to_string(),
        command,
        args,
        note,
    })
}

fn agents_to_kdl(agents: &[Agent]) -> String {
    let mut doc = KdlDocument::new();
    for agent in agents {
        let mut node = KdlNode::new("agent");
        node.insert("name", agent.name.clone());
        if let Some(note) = &agent.note {
            node.insert("note", note.clone());
        }
        let mut children = KdlDocument::new();
        if !agent.command.trim().is_empty() {
            let mut cmd_node = KdlNode::new("cmd");
            cmd_node.push(agent.command.clone());
            children.nodes_mut().push(cmd_node);
        }
        if !agent.args.is_empty() {
            let mut args_node = KdlNode::new("args");
            for arg in &agent.args {
                args_node.push(arg.clone());
            }
            children.nodes_mut().push(args_node);
        }
        node.set_children(children);
        doc.nodes_mut().push(node);
    }
    doc.to_string()
}

fn config_base_dir() -> PathBuf {
    PathBuf::from(format!("{}/.config/maestro", crate::WASI_HOST_MOUNT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_save_and_load_agents() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let agents = vec![
            Agent {
                name: "agent1".to_string(),
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                note: Some("Test agent".to_string()),
            },
            Agent {
                name: "agent2".to_string(),
                command: "ls".to_string(),
                args: Vec::new(),
                note: None,
            },
        ];

        save_agents(path, &agents).unwrap();
        let loaded = load_agents(path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "agent1");
        assert_eq!(loaded[0].command, "echo");
        assert_eq!(loaded[0].args, vec!["hello".to_string()]);
        assert_eq!(loaded[1].name, "agent2");
    }

    #[test]
    fn test_load_empty_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        std::fs::write(path, "").unwrap();

        let agents = load_agents(path).unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn test_validate_agents_duplicate_names() {
        let agents = vec![
            Agent {
                name: "duplicate".to_string(),
                command: "cmd1".to_string(),
                args: Vec::new(),
                note: None,
            },
            Agent {
                name: "duplicate".to_string(),
                command: "cmd2".to_string(),
                args: Vec::new(),
                note: None,
            },
        ];

        assert!(validate_agents(&agents).is_err());
    }

    #[test]
    fn test_validate_agents_duplicate_names_case_insensitive() {
        let agents = vec![
            Agent {
                name: "Duplicate".to_string(),
                command: "cmd1".to_string(),
                args: Vec::new(),
                note: None,
            },
            Agent {
                name: "duplicate".to_string(),
                command: "cmd2".to_string(),
                args: Vec::new(),
                note: None,
            },
        ];

        let result = validate_agents(&agents);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MaestroError::DuplicateAgentName(_)
        ));
    }

    #[test]
    fn test_validate_agent_name_control_chars() {
        let agents = vec![Agent {
            name: "test\nagent".to_string(),
            command: "cmd".to_string(),
            args: Vec::new(),
            note: None,
        }];
        let result = validate_agents(&agents);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MaestroError::InvalidAgentName(_)
        ));
    }

    #[test]
    fn test_validate_agent_name_too_long() {
        let agents = vec![Agent {
            name: "a".repeat(65),
            command: "cmd".to_string(),
            args: Vec::new(),
            note: None,
        }];
        let result = validate_agents(&agents);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MaestroError::InvalidAgentName(_)
        ));
    }

    #[test]
    fn test_validate_agent_name_max_length_ok() {
        let agents = vec![Agent {
            name: "a".repeat(64),
            command: "cmd".to_string(),
            args: Vec::new(),
            note: None,
        }];
        assert!(validate_agents(&agents).is_ok());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        std::fs::remove_file(path).unwrap();

        let agents = load_agents(path).unwrap();
        assert!(agents.is_empty());
    }

    #[test]
    fn test_save_and_load_with_all_fields() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let agents = vec![Agent {
            name: "full_agent".to_string(),
            command: "cmd".to_string(),
            args: vec!["arg1".to_string(), "arg2".to_string()],
            note: Some("A test agent with all fields".to_string()),
        }];

        save_agents(path, &agents).unwrap();
        let loaded = load_agents(path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "full_agent");
        assert_eq!(loaded[0].command, "cmd");
        assert_eq!(loaded[0].args, vec!["arg1".to_string(), "arg2".to_string()]);
        assert_eq!(
            loaded[0].note,
            Some("A test agent with all fields".to_string())
        );
    }

    #[test]
    fn test_save_and_load_empty_agents() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let agents: Vec<Agent> = vec![];

        save_agents(path, &agents).unwrap();
        let loaded = load_agents(path).unwrap();

        assert!(loaded.is_empty());
    }

    #[test]
    fn test_save_agents_validates_before_saving() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("agents.kdl");

        let invalid_agents = vec![Agent {
            name: "".to_string(),
            command: "cmd".to_string(),
            args: Vec::new(),
            note: None,
        }];

        assert!(save_agents(&path, &invalid_agents).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn test_load_agents_validates_on_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let invalid_kdl = r#"
agent name="duplicate" {
    cmd "cmd1"
}
agent name="duplicate" {
    cmd "cmd2"
}
"#;
        std::fs::write(path, invalid_kdl).unwrap();

        assert!(load_agents(path).is_err());
    }

    #[test]
    fn test_default_config_path() {
        let path = default_config_path();
        assert!(path.to_string_lossy().ends_with("agents.kdl"));
        assert!(path
            .to_string_lossy()
            .contains(&format!("{}/.config/maestro", crate::WASI_HOST_MOUNT)));
    }

    #[test]
    fn test_default_agents() {
        let defaults = default_agents();
        assert_eq!(defaults.len(), 4);
        let names: Vec<&str> = defaults.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"cursor"));
        assert!(names.contains(&"claude"));
        assert!(names.contains(&"gemini"));
        assert!(names.contains(&"codex"));
    }

    #[test]
    fn test_is_default_agent() {
        assert!(is_default_agent("cursor"));
        assert!(is_default_agent("claude"));
        assert!(is_default_agent("gemini"));
        assert!(is_default_agent("codex"));
        assert!(!is_default_agent("custom"));
        assert!(!is_default_agent(""));
        assert!(is_default_agent("  cursor  "));
        assert!(is_default_agent("CODEx"));
    }

    #[test]
    fn test_load_agents_default_merges_with_user_agents() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let user_agents = vec![
            Agent {
                name: "custom".to_string(),
                command: "custom-cmd".to_string(),
                args: Vec::new(),
                note: None,
            },
            Agent {
                name: "Cursor".to_string(),
                command: "custom-cursor".to_string(),
                args: Vec::new(),
                note: None,
            },
        ];

        save_agents(path, &user_agents).unwrap();

        let loaded = load_agents(path).unwrap();
        assert_eq!(loaded.len(), 2);

        let defaults = default_agents();
        let mut merged = defaults.clone();
        let default_names: std::collections::BTreeSet<String> =
            merged.iter().map(|a| a.name.to_lowercase()).collect();

        for user_agent in loaded {
            let normalized = user_agent.name.to_lowercase();
            if default_names.contains(&normalized) {
                if let Some(pos) = merged
                    .iter()
                    .position(|a| a.name.to_lowercase() == normalized)
                {
                    merged[pos] = user_agent;
                }
            } else {
                merged.push(user_agent);
            }
        }

        assert_eq!(merged.len(), 5);
        let cursor_agent = merged
            .iter()
            .find(|a| a.name.to_lowercase() == "cursor")
            .unwrap();
        assert_eq!(cursor_agent.command, "custom-cursor");
        assert!(merged.iter().any(|a| a.name == "custom"));
        assert!(merged.iter().any(|a| a.name == "claude"));
        assert!(merged.iter().any(|a| a.name == "gemini"));
        assert!(merged.iter().any(|a| a.name == "codex"));
    }
}
