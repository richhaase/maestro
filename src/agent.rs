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
    /// Validates that the agent has required fields
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

/// Load agents from a KDL file at the given path
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

/// Save agents to a KDL file at the given path
pub fn save_agents(path: &Path, agents: &[Agent]) -> Result<()> {
    validate_agents(agents)?;
    let payload = agents_to_kdl(agents).context("serialize agents")?;
    fs::write(path, payload.as_bytes()).context("write agents file")?;
    Ok(())
}

/// Get the default config path for agents
pub fn default_config_path() -> Result<PathBuf> {
    let base = config_base_dir()?;
    Ok(base.join("agents.kdl"))
}

/// Load agents from the default config path
pub fn load_agents_default() -> Result<Vec<Agent>> {
    let path = default_config_path()?;
    load_agents(&path)
}

/// Save agents to the default config path
pub fn save_agents_default(agents: &[Agent]) -> Result<()> {
    let path = default_config_path()?;
    save_agents(&path, agents)
}

fn validate_agents(agents: &[Agent]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (idx, agent) in agents.iter().enumerate() {
        let name = agent.name.trim();
        if name.is_empty() {
            bail!("agent {}: name is required", idx);
        }
        if agent.command.is_empty() {
            bail!("agent {} ({}): command is required", idx, name);
        }
        if !seen.insert(name.to_string()) {
            bail!("duplicate agent name: {}", name);
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
    Ok(PathBuf::from("/host"))
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
}
