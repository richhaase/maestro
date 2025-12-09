//! Utility functions for path handling and string manipulation.

use std::fs;
use std::path::{Path, PathBuf};

use crate::agent::Agent;
use crate::error::{MaestroError, MaestroResult};
use crate::WASI_HOST_MOUNT;

/// A directory entry from filesystem enumeration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
}

/// Read directory entries, filtering to directories only.
pub fn read_directory(path: &Path) -> MaestroResult<Vec<DirEntry>> {
    let entries = fs::read_dir(path).map_err(|e| MaestroError::FileRead {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let mut dirs = Vec::new();

    for entry in entries.flatten() {
        if let Ok(metadata) = entry.metadata() {
            if metadata.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path();
                dirs.push(DirEntry { name, path });
            }
        }
    }

    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(dirs)
}

/// Get autocomplete suggestions for a partial path.
pub fn get_path_suggestions(partial_path: &str) -> Vec<String> {
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;

    let trimmed = partial_path.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let root = wasi_root();
    let host_prefix = format!("{}/", WASI_HOST_MOUNT);

    let (base_path, filter_segment) = if trimmed == WASI_HOST_MOUNT || trimmed == host_prefix {
        (root.clone(), String::new())
    } else if trimmed.starts_with(&host_prefix) {
        let relative = trimmed.strip_prefix(&host_prefix).unwrap_or("");
        if relative.is_empty() {
            (root.clone(), String::new())
        } else if relative.ends_with('/') {
            (root.join(relative.trim_end_matches('/')), String::new())
        } else {
            let parts: Vec<&str> = relative.split('/').collect();
            if parts.len() == 1 {
                (root.clone(), parts[0].to_string())
            } else {
                let base = parts[..parts.len() - 1].join("/");
                let filter = parts.last().unwrap_or(&"").to_string();
                (root.join(base), filter)
            }
        }
    } else if trimmed.ends_with('/') {
        (root.join(trimmed.trim_end_matches('/')), String::new())
    } else {
        let parts: Vec<&str> = trimmed.split('/').collect();
        if parts.len() == 1 {
            (root.clone(), parts[0].to_string())
        } else {
            let base = parts[..parts.len() - 1].join("/");
            let filter = parts.last().unwrap_or(&"").to_string();
            (root.join(base), filter)
        }
    };

    let entries = match read_directory(&base_path) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let matcher = SkimMatcherV2::default();

    let mut scored: Vec<(i64, String)> = entries
        .iter()
        .filter_map(|entry| {
            let name = &entry.name;
            // Neutral score when no filter is provided so we still surface all entries.
            let score = if filter_segment.is_empty() {
                0
            } else {
                matcher.fuzzy_match(name, &filter_segment)?
            };

            let relative = if entry.path.starts_with(&root) {
                entry.path.strip_prefix(&root).unwrap_or(&entry.path)
            } else {
                &entry.path
            };
            let display = format!(
                "{}/{}",
                WASI_HOST_MOUNT,
                relative.to_string_lossy().trim_start_matches('/')
            );
            Some((score, display))
        })
        .collect();

    // Sort by best match first, then by path name for stable ordering.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    scored.into_iter().map(|(_, path)| path).collect()
}

/// Truncate a string to a maximum length, adding ellipsis if needed.
/// The returned string will be at most `max` characters (including ellipsis).
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max {
        return s.to_string();
    }
    // Reserve one slot for the ellipsis
    let mut out = String::new();
    for ch in s.chars().take(max.saturating_sub(1)) {
        out.push(ch);
    }
    out.push('…');
    out
}

/// Build command as a list of strings (command followed by args).
pub fn build_command(agent: &Agent) -> Vec<String> {
    let mut parts = vec![agent.command.clone()];
    parts.extend(agent.args.clone());
    parts
}

/// Extract the basename from a workspace path.
pub fn workspace_basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Generate a default tab name from a workspace path.
pub fn default_tab_name(workspace_path: &str) -> String {
    let basename = workspace_basename(workspace_path);
    if basename.is_empty() {
        "workspace".to_string()
    } else {
        basename
    }
}

/// Get the WASI host mount point (the root of accessible filesystem).
/// In WASI, `/host` maps to the cwd the user launched Zellij with.
fn wasi_root() -> PathBuf {
    PathBuf::from(WASI_HOST_MOUNT)
}

/// Resolve a workspace path for Zellij API calls.
/// Returns `None` for empty paths (Zellij will use default cwd).
pub fn resolve_workspace_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let host_prefix = format!("{}/", WASI_HOST_MOUNT);
    if trimmed.starts_with(&host_prefix) {
        let relative = trimmed.strip_prefix(&host_prefix).unwrap_or("");
        if relative.is_empty() {
            None
        } else {
            Some(PathBuf::from(relative))
        }
    } else if trimmed == WASI_HOST_MOUNT {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

/// Filter agents by fuzzy matching against the filter string.
/// Returns indices of matching agents sorted by score (best first).
pub fn filter_agents_fuzzy(agents: &[Agent], filter: &str) -> Vec<usize> {
    use fuzzy_matcher::skim::SkimMatcherV2;
    use fuzzy_matcher::FuzzyMatcher;

    let filter = filter.trim();
    if filter.is_empty() {
        return (0..agents.len()).collect();
    }

    let matcher = SkimMatcherV2::default();
    let mut scored: Vec<(usize, i64)> = agents
        .iter()
        .enumerate()
        .filter_map(|(idx, agent)| {
            matcher
                .fuzzy_match(&agent.name, filter)
                .map(|score| (idx, score))
        })
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));

    scored.into_iter().map(|(idx, _)| idx).collect()
}

/// Find an agent by matching the pane title to the agent's command.
pub fn find_agent_by_command<'a>(agents: &'a [Agent], pane_title: &str) -> Option<&'a Agent> {
    let title_base = pane_title.split(" - ").next().unwrap_or(pane_title).trim();

    agents.iter().find(|agent| {
        if agent.command.trim().is_empty() {
            return false;
        }
        let full_cmd = build_command(agent).join(" ");
        full_cmd.eq_ignore_ascii_case(title_base)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 6), "hello");
        assert_eq!(truncate("hello", 4), "hel…");
        assert_eq!(truncate("hello", 3), "he…");
        assert_eq!(truncate("hello", 1), "…");
        assert_eq!(truncate("hello", 0), "");
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn test_build_command_with_args() {
        let agent = Agent {
            name: "test".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string(), "world".to_string()],
            note: None,
        };

        let cmd = build_command(&agent);
        assert_eq!(cmd, vec!["echo", "hello", "world"]);
    }

    #[test]
    fn test_build_command_without_args() {
        let agent = Agent {
            name: "test".to_string(),
            command: "echo".to_string(),
            args: Vec::new(),
            note: None,
        };

        let cmd = build_command(&agent);
        assert_eq!(cmd, vec!["echo"]);
    }

    #[test]
    fn test_workspace_basename() {
        assert_eq!(workspace_basename("/path/to/workspace"), "workspace");
        assert_eq!(workspace_basename("workspace"), "workspace");
        assert_eq!(workspace_basename(""), "");
    }

    #[test]
    fn test_default_tab_name() {
        assert_eq!(default_tab_name("/path/to/myapp"), "myapp");
        assert_eq!(default_tab_name("/home/user/docs"), "docs");
        assert_eq!(default_tab_name("/home/user"), "user");
        assert_eq!(default_tab_name(""), "workspace");
        assert_eq!(default_tab_name("/"), "workspace");
    }

    #[test]
    fn test_find_agent_by_command() {
        let agents = vec![
            Agent {
                name: "cursor".to_string(),
                command: "cursor-agent".to_string(),
                args: Vec::new(),
                note: None,
            },
            Agent {
                name: "claude".to_string(),
                command: "claude".to_string(),
                args: Vec::new(),
                note: None,
            },
            Agent {
                name: "custom".to_string(),
                command: "my-cmd".to_string(),
                args: vec!["arg1".to_string()],
                note: None,
            },
        ];

        assert_eq!(
            find_agent_by_command(&agents, "cursor-agent"),
            Some(&agents[0])
        );
        assert_eq!(find_agent_by_command(&agents, "claude"), Some(&agents[1]));
        assert_eq!(
            find_agent_by_command(&agents, "my-cmd arg1"),
            Some(&agents[2])
        );

        assert_eq!(
            find_agent_by_command(&agents, "cursor-agent - some suffix"),
            Some(&agents[0])
        );
        assert_eq!(
            find_agent_by_command(&agents, "my-cmd arg1 - workspace"),
            Some(&agents[2])
        );

        assert_eq!(find_agent_by_command(&agents, "unknown"), None);
        assert_eq!(find_agent_by_command(&agents, "my-cmd"), None);
        assert_eq!(find_agent_by_command(&agents, "my-cmd arg1 arg2"), None);

        let agents_with_overlap = vec![
            Agent {
                name: "codex".to_string(),
                command: "codex".to_string(),
                args: Vec::new(),
                note: None,
            },
            Agent {
                name: "codex-reviewer".to_string(),
                command: "codex".to_string(),
                args: vec!["/review".to_string()],
                note: None,
            },
        ];
        assert_eq!(
            find_agent_by_command(&agents_with_overlap, "codex /review"),
            Some(&agents_with_overlap[1])
        );
        assert_eq!(
            find_agent_by_command(&agents_with_overlap, "codex"),
            Some(&agents_with_overlap[0])
        );
    }

    #[test]
    fn test_resolve_workspace_path() {
        assert_eq!(resolve_workspace_path(""), None);
        assert_eq!(resolve_workspace_path("   "), None);

        assert_eq!(
            resolve_workspace_path("src/maestro"),
            Some(PathBuf::from("src/maestro"))
        );
        assert_eq!(
            resolve_workspace_path("projects/myapp"),
            Some(PathBuf::from("projects/myapp"))
        );

        assert_eq!(
            resolve_workspace_path("  src/maestro  "),
            Some(PathBuf::from("src/maestro"))
        );

        assert_eq!(
            resolve_workspace_path("Documents"),
            Some(PathBuf::from("Documents"))
        );
    }
}
