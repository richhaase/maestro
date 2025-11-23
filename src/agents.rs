use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use kdl::{KdlDocument, KdlNode};

use crate::Agent;

pub fn config_path() -> Result<PathBuf> {
    let base = config_base_dir()?;
    Ok(base.join("agents.kdl"))
}

pub fn load_agents() -> Result<Vec<Agent>> {
    let path = config_path()?;
    let data = match fs::read_to_string(&path) {
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

pub fn save_agents(agents: &[Agent]) -> Result<()> {
    validate_agents(agents)?;
    let path = config_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).context("create config dir")?;
    }

    let payload = agents_to_kdl(agents).context("serialize agents")?;
    let tmp_path = path.with_extension("kdl.tmp");
    fs::write(&tmp_path, payload.as_bytes()).context("write temp agents file")?;
    fs::rename(&tmp_path, &path).context("atomically replace agents file")?;
    Ok(())
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
    if Path::new(".").exists() {
        return Ok(Path::new(".").to_path_buf());
    }
    Err(anyhow!("No usable config dir"))
}
