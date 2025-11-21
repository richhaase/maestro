use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::Agent;

const CONFIG_SUBPATH: &str = ".config/maestro/agents.toml";

pub fn config_path() -> Result<PathBuf> {
    let home = env::var("HOME").context("HOME is not set")?;
    Ok(Path::new(&home).join(CONFIG_SUBPATH))
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

    let mut agents: Vec<Agent> = toml::from_str(&data).context("parse agents toml")?;
    validate_agents(&agents)?;

    // Normalize optional fields
    for agent in agents.iter_mut() {
        if agent.env.is_none() {
            agent.env = Some(BTreeMap::new());
        }
    }

    Ok(agents)
}

pub fn save_agents(agents: &[Agent]) -> Result<()> {
    validate_agents(agents)?;
    let path = config_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).context("create config dir")?;
    }

    let payload = toml::to_string_pretty(agents).context("serialize agents")?;
    let tmp_path = path.with_extension("toml.tmp");
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn with_home<F: FnOnce() -> R, R>(dir: &Path, f: F) -> R {
        let prev = env::var("HOME").ok();
        env::set_var("HOME", dir);
        let result = f();
        if let Some(prev) = prev {
            env::set_var("HOME", prev);
        }
        result
    }

    #[test]
    fn load_missing_returns_empty() {
        let tmp = TempDir::new().unwrap();
        with_home(tmp.path(), || {
            let agents = load_agents().unwrap();
            assert!(agents.is_empty());
        });
    }

    #[test]
    fn save_creates_parent_and_roundtrips() {
        let tmp = TempDir::new().unwrap();
        with_home(tmp.path(), || {
            let agents = vec![Agent {
                name: "codex".to_string(),
                command: vec!["codex".to_string(), "chat".to_string()],
                env: Some(BTreeMap::from([("CODEX_PROFILE".to_string(), "default".to_string())])),
                note: Some("test".to_string()),
            }];
            save_agents(&agents).unwrap();

            let loaded = load_agents().unwrap();
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded[0].name, "codex");
            assert_eq!(loaded[0].command, vec!["codex", "chat"]);
            assert_eq!(
                loaded[0]
                    .env
                    .as_ref()
                    .unwrap()
                    .get("CODEX_PROFILE")
                    .unwrap(),
                "default"
            );
        });
    }

    #[test]
    fn validate_rejects_duplicates() {
        let tmp = TempDir::new().unwrap();
        with_home(tmp.path(), || {
            let agents = vec![
                Agent {
                    name: "dup".to_string(),
                    command: vec!["cmd".to_string()],
                    env: None,
                    note: None,
                },
                Agent {
                    name: "dup".to_string(),
                    command: vec!["cmd2".to_string()],
                    env: None,
                    note: None,
                },
            ];
            let err = save_agents(&agents).unwrap_err();
            assert!(err.to_string().contains("duplicate agent name"));
        });
    }

    #[test]
    fn validate_rejects_empty_command() {
        let tmp = TempDir::new().unwrap();
        with_home(tmp.path(), || {
            let agents = vec![Agent {
                name: "empty".to_string(),
                command: vec![],
                env: None,
                note: None,
            }];
            let err = save_agents(&agents).unwrap_err();
            assert!(err.to_string().contains("command is required"));
        });
    }
}
