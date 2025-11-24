use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use kdl::{KdlDocument, KdlNode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    pub command: Vec<String>,
    #[serde(default)]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PaneStatus {
    Running,
    Exited(Option<i32>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPane {
    pub pane_title: String,
    pub tab_name: String,
    pub pane_id: Option<u32>,
    pub workspace_path: String,
    pub agent_name: String,
    pub status: PaneStatus,
}

impl Agent {
    pub fn validate(&self) -> Result<()> {
        let name = self.name.trim();
        if name.is_empty() {
            bail!("agent name is required");
        }
        if self.command.is_empty() {
            bail!("agent command is required");
        }
        Ok(())
    }
}

pub fn load_agents(path: &Path) -> Result<Vec<Agent>> {
    let data = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context("read agents file"),
    };

    if data.trim().is_empty() {
        return Ok(Vec::new());
    }

    let doc: KdlDocument = data.parse().context("parse agents kdl")?;
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

pub fn save_agents(path: &Path, agents: &[Agent]) -> Result<()> {
    validate_agents(agents)?;
    let payload = agents_to_kdl(agents).context("serialize agents")?;
    fs::write(path, payload.as_bytes()).context("write agents file")?;
    Ok(())
}

pub fn default_config_path() -> Result<PathBuf> {
    let base = config_base_dir()?;
    let path = base.join("agents.kdl");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create config directory")?;
    }
    Ok(path)
}

pub fn default_agents() -> Vec<Agent> {
    vec![
        Agent {
            name: "cursor".to_string(),
            command: vec!["cursor-agent".to_string()],
            env: None,
            note: Some("Default agent config".to_string()),
        },
        Agent {
            name: "claude".to_string(),
            command: vec!["claude".to_string()],
            env: None,
            note: Some("Default agent config".to_string()),
        },
        Agent {
            name: "gemini".to_string(),
            command: vec!["gemini".to_string()],
            env: None,
            note: Some("Default agent config".to_string()),
        },
        Agent {
            name: "codex".to_string(),
            command: vec!["codex".to_string()],
            env: None,
            note: Some("Default agent config".to_string()),
        },
    ]
}

pub fn is_default_agent(name: &str) -> bool {
    matches!(name.trim(), "cursor" | "claude" | "gemini" | "codex")
}

pub fn load_agents_default() -> Result<Vec<Agent>> {
    let path = default_config_path()?;
    let user_agents = load_agents(&path)?;
    
    let mut merged = default_agents();
    let default_names: std::collections::BTreeSet<String> = merged
        .iter()
        .map(|a| a.name.clone())
        .collect();
    
    for user_agent in user_agents {
        if default_names.contains(&user_agent.name) {
            if let Some(pos) = merged.iter().position(|a| a.name == user_agent.name) {
                merged[pos] = user_agent;
            }
        } else {
            merged.push(user_agent);
        }
    }
    
    Ok(merged)
}

pub fn save_agents_default(agents: &[Agent]) -> Result<()> {
    let path = default_config_path()?;
    let user_agents: Vec<Agent> = agents
        .iter()
        .filter(|a| !is_default_agent(&a.name))
        .cloned()
        .collect();
    save_agents(&path, &user_agents)
}

fn validate_agents(agents: &[Agent]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (idx, agent) in agents.iter().enumerate() {
        let name = agent.name.trim();
        if name.is_empty() {
            bail!("agent {idx}: name is required");
        }
        if agent.command.is_empty() {
            bail!("agent {idx} ({name}): command is required");
        }
        if !seen.insert(name.to_string()) {
            bail!("duplicate agent name: {name}");
        }
    }
    Ok(())
}

fn agent_from_kdl(node: &KdlNode) -> Result<Agent> {
    let name_val = node
        .get("name")
        .and_then(|e| e.value().as_string())
        .ok_or_else(|| anyhow!("agent: missing name"))?;
    let note = node
        .get("note")
        .and_then(|e| e.value().as_string())
        .map(|s| s.to_string());

    let mut command = Vec::new();
    let mut env: BTreeMap<String, String> = BTreeMap::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "cmd" => {
                    for entry in child.entries() {
                        if let Some(s) = entry.value().as_string() {
                            command.push(s.to_string());
                        } else {
                            command.push(entry.value().to_string());
                        }
                    }
                }
                "env" => {
                    let key = child
                        .get(0)
                        .and_then(|e| e.value().as_string())
                        .ok_or_else(|| anyhow!("env missing key"))?;
                    let val = child
                        .get(1)
                        .and_then(|e| e.value().as_string())
                        .unwrap_or("")
                        .to_string();
                    env.insert(key.to_string(), val);
                }
                _ => {}
            }
        }
    }
    Ok(Agent {
        name: name_val.to_string(),
        command,
        env: if env.is_empty() { None } else { Some(env) },
        note,
    })
}

fn agents_to_kdl(agents: &[Agent]) -> Result<String> {
    let mut doc = KdlDocument::new();
    for agent in agents {
        let mut node = KdlNode::new("agent");
        node.insert("name", agent.name.clone());
        if let Some(note) = &agent.note {
            node.insert("note", note.clone());
        }
        let mut children = KdlDocument::new();
        if !agent.command.is_empty() {
            let mut cmd_node = KdlNode::new("cmd");
            for arg in &agent.command {
                cmd_node.push(arg.clone());
            }
            children.nodes_mut().push(cmd_node);
        }
        if let Some(env) = &agent.env {
            for (k, v) in env {
                let mut env_node = KdlNode::new("env");
                env_node.push(k.clone());
                env_node.push(v.clone());
                children.nodes_mut().push(env_node);
            }
        }
        node.set_children(children);
        doc.nodes_mut().push(node);
    }
    Ok(doc.to_string())
}

fn config_base_dir() -> Result<PathBuf> {
    Ok(PathBuf::from("/host/.config/maestro"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_agent_validation() {
        let valid_agent = Agent {
            name: "test".to_string(),
            command: vec!["echo".to_string(), "hello".to_string()],
            env: None,
            note: None,
        };
        assert!(valid_agent.validate().is_ok());

        let invalid_name = Agent {
            name: "   ".to_string(),
            command: vec!["echo".to_string()],
            env: None,
            note: None,
        };
        assert!(invalid_name.validate().is_err());

        let invalid_command = Agent {
            name: "test".to_string(),
            command: vec![],
            env: None,
            note: None,
        };
        assert!(invalid_command.validate().is_err());
    }

    #[test]
    fn test_save_and_load_agents() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let agents = vec![
            Agent {
                name: "agent1".to_string(),
                command: vec!["echo".to_string(), "hello".to_string()],
                env: Some({
                    let mut m = BTreeMap::new();
                    m.insert("VAR".to_string(), "value".to_string());
                    m
                }),
                note: Some("Test agent".to_string()),
            },
            Agent {
                name: "agent2".to_string(),
                command: vec!["ls".to_string()],
                env: None,
                note: None,
            },
        ];

        save_agents(path, &agents).unwrap();
        let loaded = load_agents(path).unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "agent1");
        assert_eq!(loaded[0].command, vec!["echo", "hello"]);
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
                command: vec!["cmd1".to_string()],
                env: None,
                note: None,
            },
            Agent {
                name: "duplicate".to_string(),
                command: vec!["cmd2".to_string()],
                env: None,
                note: None,
            },
        ];

        assert!(validate_agents(&agents).is_err());
    }

    #[test]
    fn test_load_nonexistent_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        // Delete the file so it doesn't exist
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
            command: vec!["cmd".to_string(), "arg1".to_string(), "arg2".to_string()],
            env: Some({
                let mut m = BTreeMap::new();
                m.insert("KEY1".to_string(), "value1".to_string());
                m.insert("KEY2".to_string(), "value2".to_string());
                m
            }),
            note: Some("A test agent with all fields".to_string()),
        }];

        save_agents(path, &agents).unwrap();
        let loaded = load_agents(path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "full_agent");
        assert_eq!(loaded[0].command, vec!["cmd", "arg1", "arg2"]);
        assert_eq!(loaded[0].env.as_ref().unwrap().len(), 2);
        assert_eq!(loaded[0].note, Some("A test agent with all fields".to_string()));
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
            command: vec!["cmd".to_string()],
            env: None,
            note: None,
        }];

        assert!(save_agents(&path, &invalid_agents).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn test_load_agents_validates_on_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Write invalid KDL with duplicate agent names
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
        let path = default_config_path().unwrap();
        assert!(path.to_string_lossy().ends_with("agents.kdl"));
        assert!(path.to_string_lossy().contains("/host/.config/maestro"));
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
    }

    #[test]
    fn test_load_agents_default_merges_with_user_agents() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let user_agents = vec![
            Agent {
                name: "custom".to_string(),
                command: vec!["custom-cmd".to_string()],
                env: None,
                note: None,
            },
            Agent {
                name: "cursor".to_string(),
                command: vec!["custom-cursor".to_string()],
                env: None,
                note: None,
            },
        ];

        save_agents(path, &user_agents).unwrap();
        
        let loaded = load_agents(path).unwrap();
        assert_eq!(loaded.len(), 2);
        
        let defaults = default_agents();
        let mut merged = defaults.clone();
        let default_names: std::collections::BTreeSet<String> = merged
            .iter()
            .map(|a| a.name.clone())
            .collect();
        
        for user_agent in loaded {
            if default_names.contains(&user_agent.name) {
                if let Some(pos) = merged.iter().position(|a| a.name == user_agent.name) {
                    merged[pos] = user_agent;
                }
            } else {
                merged.push(user_agent);
            }
        }
        
        assert_eq!(merged.len(), 5);
        let cursor_agent = merged.iter().find(|a| a.name == "cursor").unwrap();
        assert_eq!(cursor_agent.command, vec!["custom-cursor"]);
        assert!(merged.iter().any(|a| a.name == "custom"));
        assert!(merged.iter().any(|a| a.name == "claude"));
        assert!(merged.iter().any(|a| a.name == "gemini"));
        assert!(merged.iter().any(|a| a.name == "codex"));
    }

    #[test]
    fn test_save_agents_default_filters_out_defaults() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let all_agents = vec![
            Agent {
                name: "cursor".to_string(),
                command: vec!["cursor-agent".to_string()],
                env: None,
                note: None,
            },
            Agent {
                name: "custom".to_string(),
                command: vec!["custom-cmd".to_string()],
                env: None,
                note: None,
            },
        ];

        let user_agents: Vec<Agent> = all_agents
            .iter()
            .filter(|a| !is_default_agent(&a.name))
            .cloned()
            .collect();
        
        save_agents(path, &user_agents).unwrap();
        
        let saved = load_agents(path).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "custom");
    }
}
