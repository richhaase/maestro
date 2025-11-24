use std::collections::BTreeMap;

use crate::agent::Agent;

/// Truncate a string to a maximum length, adding ellipsis if needed
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= max {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

/// Truncate a path, replacing home directory with ~ and truncating from the end
pub fn truncate_path(path: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if path.is_empty() {
        return "—".to_string();
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let relative_path = if !home.is_empty() && path.starts_with(&home) {
        path.replacen(&home, "~", 1)
    } else {
        path.to_string()
    };

    if relative_path.chars().count() <= max {
        return relative_path;
    }

    let chars: Vec<char> = relative_path.chars().collect();
    if chars.len() <= max {
        return relative_path;
    }

    let ellipsis = "…";
    let ellipsis_len = ellipsis.chars().count();
    if max <= ellipsis_len {
        return truncate(&relative_path, max);
    }

    let take_from_end = max - ellipsis_len;
    let end: String = chars.iter().rev().take(take_from_end).rev().collect();
    format!("{ellipsis}{end}")
}

/// Build command with environment variables as prefix arguments
pub fn build_command_with_env(agent: &Agent) -> Vec<String> {
    let mut parts = Vec::new();
    if let Some(env) = &agent.env {
        for (k, v) in env {
            parts.push(format!("{k}={v}"));
        }
    }
    parts.extend(agent.command.clone());
    parts
}

/// Extract the basename from a workspace path
pub fn workspace_basename(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Generate a default tab name from a workspace path
pub fn default_tab_name(workspace_path: &str) -> String {
    let basename = workspace_basename(workspace_path);
    if basename.is_empty() {
        "maestro:workspace".to_string()
    } else {
        format!("maestro:{basename}")
    }
}

/// Generate a tab name from a workspace path (deprecated, use default_tab_name)
pub fn workspace_tab_name(path: &str) -> String {
    default_tab_name(path)
}

/// Check if a pane title is a Maestro-managed pane
pub fn is_maestro_tab(title: &str) -> bool {
    title.starts_with("maestro:")
}

/// Parse agent name and workspace hint from a Maestro pane title
pub fn parse_title_hint(title: &str) -> Option<(String, String)> {
    if !is_maestro_tab(title) {
        return None;
    }
    let parts: Vec<&str> = title.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let agent = parts.get(1).unwrap_or(&"").to_string();
    let workspace_hint = parts.get(2).unwrap_or(&"").to_string();
    Some((agent, workspace_hint))
}

/// Find an agent by matching the pane title to the agent's command
pub fn find_agent_by_command<'a>(agents: &'a [Agent], pane_title: &str) -> Option<&'a Agent> {
    let title_base = pane_title.split(" - ").next().unwrap_or(pane_title).trim();
    agents.iter().find(|agent| {
        if agent.command.is_empty() {
            return false;
        }
        let first_cmd = &agent.command[0];
        first_cmd.eq_ignore_ascii_case(title_base)
            || title_base.eq_ignore_ascii_case(&agent.name)
    })
}

/// Parse environment variable input string into a map
pub fn parse_env_input(input: &str) -> Result<Option<BTreeMap<String, String>>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let mut map = BTreeMap::new();
    for pair in trimmed.split(',') {
        if pair.trim().is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().map(str::trim).unwrap_or("").to_string();
        let val = parts.next().map(str::trim).unwrap_or("").to_string();
        if key.is_empty() {
            return Err("env entries must be KEY=VAL".to_string());
        }
        map.insert(key, val);
    }
    Ok(Some(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 3), "hel…");
        assert_eq!(truncate("hello", 0), "");
        assert_eq!(truncate("", 5), "");
    }

    #[test]
    fn test_truncate_path() {
        assert_eq!(truncate_path("", 10), "—");
        assert_eq!(truncate_path("/short/path", 20), "/short/path");

        let long_path = "/very/long/path/that/exceeds/max/length";
        let result = truncate_path(long_path, 20);
        assert!(result.chars().count() <= 20);
        assert!(result.starts_with('…'));
    }

    #[test]
    fn test_build_command_with_env() {
        let agent = Agent {
            name: "test".to_string(),
            command: vec!["echo".to_string(), "hello".to_string()],
            env: Some({
                let mut m = BTreeMap::new();
                m.insert("VAR1".to_string(), "value1".to_string());
                m.insert("VAR2".to_string(), "value2".to_string());
                m
            }),
            note: None,
        };

        let cmd = build_command_with_env(&agent);
        assert_eq!(cmd.len(), 4);
        assert!(cmd.contains(&"VAR1=value1".to_string()));
        assert!(cmd.contains(&"VAR2=value2".to_string()));
        assert_eq!(cmd[2], "echo");
        assert_eq!(cmd[3], "hello");
    }

    #[test]
    fn test_build_command_without_env() {
        let agent = Agent {
            name: "test".to_string(),
            command: vec!["echo".to_string(), "hello".to_string()],
            env: None,
            note: None,
        };

        let cmd = build_command_with_env(&agent);
        assert_eq!(cmd, vec!["echo", "hello"]);
    }

    #[test]
    fn test_workspace_basename() {
        assert_eq!(workspace_basename("/path/to/workspace"), "workspace");
        assert_eq!(workspace_basename("workspace"), "workspace");
        assert_eq!(workspace_basename(""), "");
    }

    #[test]
    fn test_workspace_tab_name() {
        let name1 = workspace_tab_name("/path/to/workspace");
        assert_eq!(name1, "maestro:workspace");
        assert_eq!(workspace_tab_name(""), "maestro:workspace");
    }

    #[test]
    fn test_default_tab_name() {
        assert_eq!(default_tab_name("/path/to/myapp"), "maestro:myapp");
        assert_eq!(default_tab_name("/home/user/docs"), "maestro:docs");
        assert_eq!(default_tab_name("/home/user"), "maestro:user");
        assert_eq!(default_tab_name(""), "maestro:workspace");
        assert_eq!(default_tab_name("/"), "maestro:workspace");
    }

    #[test]
    fn test_is_maestro_tab() {
        assert!(is_maestro_tab("maestro:agent:workspace"));
        assert!(is_maestro_tab("maestro:"));
        assert!(!is_maestro_tab("not-maestro"));
        assert!(!is_maestro_tab(""));
    }

    #[test]
    fn test_parse_title_hint() {
        assert_eq!(
            parse_title_hint("maestro:agent:workspace"),
            Some(("agent".to_string(), "workspace".to_string()))
        );
        assert_eq!(
            parse_title_hint("maestro:agent:workspace:extra"),
            Some(("agent".to_string(), "workspace".to_string()))
        );
        assert_eq!(parse_title_hint("maestro:agent"), None);
        assert_eq!(parse_title_hint("not-maestro"), None);
    }

    #[test]
    fn test_find_agent_by_command() {
        let agents = vec![
            Agent {
                name: "cursor".to_string(),
                command: vec!["cursor-agent".to_string()],
                env: None,
                note: None,
            },
            Agent {
                name: "claude".to_string(),
                command: vec!["claude".to_string()],
                env: None,
                note: None,
            },
            Agent {
                name: "custom".to_string(),
                command: vec!["my-cmd".to_string(), "arg1".to_string()],
                env: None,
                note: None,
            },
        ];

        assert_eq!(
            find_agent_by_command(&agents, "cursor-agent"),
            Some(&agents[0])
        );
        assert_eq!(
            find_agent_by_command(&agents, "cursor-agent - some suffix"),
            Some(&agents[0])
        );
        assert_eq!(find_agent_by_command(&agents, "claude"), Some(&agents[1]));
        assert_eq!(find_agent_by_command(&agents, "my-cmd"), Some(&agents[2]));
        assert_eq!(find_agent_by_command(&agents, "unknown"), None);
        assert_eq!(
            find_agent_by_command(&agents, "cursor"),
            Some(&agents[0])
        );
    }

    #[test]
    fn test_parse_env_input() {
        // Valid input
        let result = parse_env_input("KEY1=value1,KEY2=value2").unwrap();
        assert!(result.is_some());
        let map = result.unwrap();
        assert_eq!(map.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(map.get("KEY2"), Some(&"value2".to_string()));

        // Empty input
        let result = parse_env_input("").unwrap();
        assert!(result.is_none());

        // Whitespace handling
        let result = parse_env_input(" KEY1 = value1 , KEY2 = value2 ").unwrap();
        assert!(result.is_some());
        let map = result.unwrap();
        assert_eq!(map.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(map.get("KEY2"), Some(&"value2".to_string()));

        // Invalid input (missing key)
        assert!(parse_env_input("=value").is_err());

        // Invalid input (empty key)
        assert!(parse_env_input("  =value").is_err());
    }
}
