use zellij_tile::prelude::{BareKey, KeyWithModifier};

use crate::agent::{default_config_path, save_agents, Agent};
use crate::error::{MaestroError, MaestroResult};
use crate::model::Model;
use crate::ui::{AgentFormField, Mode};

pub(super) fn handle_text_edit(target: &mut String, key: &KeyWithModifier) -> bool {
    match key.bare_key {
        BareKey::Backspace => {
            target.pop();
            true
        }
        BareKey::Delete => {
            target.clear();
            true
        }
        BareKey::Char(c) => {
            target.push(c);
            true
        }
        _ => false,
    }
}

pub(super) fn handle_form_text(model: &mut Model, key: &KeyWithModifier) -> bool {
    handle_text_edit(model.agent_form.current_input_mut(), key)
}

pub(super) fn start_new_pane_workspace(model: &mut Model) {
    model.pane_wizard.clear();
    model.mode = Mode::NewPaneWorkspace;
    model.error_message.clear();
}

pub(super) fn start_agent_create(model: &mut Model) {
    model.agent_form.clear();
    model.agent_form.source = Some(Mode::View);
    model.mode = Mode::AgentFormCreate;
    model.error_message.clear();
}

pub(super) fn start_agent_edit(model: &mut Model) {
    if model.agents.is_empty() {
        model.error_message = MaestroError::NoAgentsToEdit.to_string();
        return;
    }
    let idx = model
        .selected_agent
        .min(model.agents.len().saturating_sub(1));
    if let Some(agent) = model.agents.get(idx) {
        model.agent_form.name = agent.name.clone();
        model.agent_form.command = agent.command.clone();
        model.agent_form.args = agent.args.join(" ");
        model.agent_form.note = agent.note.clone().unwrap_or_default();
        model.agent_form.field = AgentFormField::Name;
        model.agent_form.target = Some(idx);
        model.agent_form.source = Some(Mode::View);
        model.mode = Mode::AgentFormEdit;
        model.error_message.clear();
    }
}

pub(super) fn start_agent_delete_confirm(model: &mut Model) {
    if model.agents.is_empty() {
        model.error_message = MaestroError::NoAgentsToDelete.to_string();
        return;
    }
    let idx = model
        .selected_agent
        .min(model.agents.len().saturating_sub(1));
    model.agent_form.target = Some(idx);
    model.mode = Mode::DeleteConfirm;
    model.error_message.clear();
}

pub(super) fn build_agent_from_inputs(model: &Model) -> MaestroResult<Agent> {
    let name = model.agent_form.name.trim().to_string();
    if name.is_empty() {
        return Err(MaestroError::AgentNameRequired);
    }
    let command = model.agent_form.command.trim().to_string();
    if command.is_empty() {
        return Err(MaestroError::CommandRequired);
    }
    let args: Vec<String> = model
        .agent_form
        .args
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    let note = if model.agent_form.note.trim().is_empty() {
        None
    } else {
        Some(model.agent_form.note.trim().to_string())
    };
    Ok(Agent {
        name,
        command,
        args,
        note,
    })
}

pub(super) fn apply_agent_create(model: &mut Model, agent: Agent) -> MaestroResult<()> {
    if model.agents.iter().any(|a| a.name == agent.name) {
        return Err(MaestroError::DuplicateAgentName(agent.name.clone()));
    }
    model.agents.push(agent.clone());
    model.selected_agent = model.agents.len().saturating_sub(1);
    persist_agents(model)
}

pub(super) fn apply_agent_edit(model: &mut Model, agent: Agent) -> MaestroResult<()> {
    if let Some(idx) = model.agent_form.target {
        if idx < model.agents.len() {
            if model
                .agents
                .iter()
                .enumerate()
                .any(|(i, a)| i != idx && a.name == agent.name)
            {
                return Err(MaestroError::DuplicateAgentName(agent.name.clone()));
            }
            model.agents[idx] = agent;
            model.selected_agent = idx;
            return persist_agents(model);
        }
    }
    Err(MaestroError::NoAgentSelected)
}

pub(super) fn persist_agents(model: &mut Model) -> MaestroResult<()> {
    let path = default_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MaestroError::DirectoryCreate {
            path: parent.to_path_buf(),
            message: e.to_string(),
        })?;
    }
    // Persist full agent list so user customizations to built-in defaults are retained.
    save_agents(&path, &model.agents)?;
    model.agents = crate::agent::load_agents_default()?;
    model.clamp_selections();
    model.error_message.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Model;
    use crate::test_helpers::create_test_agent;
    use zellij_tile::prelude::{BareKey, KeyWithModifier};

    fn create_test_model() -> Model {
        Model::default()
    }

    fn char_key(c: char) -> KeyWithModifier {
        KeyWithModifier {
            bare_key: BareKey::Char(c),
            key_modifiers: std::collections::BTreeSet::new(),
        }
    }

    fn backspace_key() -> KeyWithModifier {
        KeyWithModifier {
            bare_key: BareKey::Backspace,
            key_modifiers: std::collections::BTreeSet::new(),
        }
    }

    fn delete_key() -> KeyWithModifier {
        KeyWithModifier {
            bare_key: BareKey::Delete,
            key_modifiers: std::collections::BTreeSet::new(),
        }
    }

    #[test]
    fn test_handle_text_edit_char() {
        let mut target = String::new();
        let key = char_key('a');
        assert!(handle_text_edit(&mut target, &key));
        assert_eq!(target, "a");
    }

    #[test]
    fn test_handle_text_edit_backspace() {
        let mut target = "hello".to_string();
        let key = backspace_key();
        assert!(handle_text_edit(&mut target, &key));
        assert_eq!(target, "hell");
    }

    #[test]
    fn test_handle_text_edit_delete() {
        let mut target = "hello".to_string();
        let key = delete_key();
        assert!(handle_text_edit(&mut target, &key));
        assert_eq!(target, "");
    }

    #[test]
    fn test_handle_text_edit_backspace_empty() {
        let mut target = String::new();
        let key = backspace_key();
        assert!(handle_text_edit(&mut target, &key));
        assert_eq!(target, "");
    }

    #[test]
    fn test_handle_form_text_name_field() {
        let mut model = create_test_model();
        model.agent_form.field = AgentFormField::Name;
        let key = char_key('t');
        assert!(handle_form_text(&mut model, &key));
        assert_eq!(model.agent_form.name, "t");
    }

    #[test]
    fn test_handle_form_text_command_field() {
        let mut model = create_test_model();
        model.agent_form.field = AgentFormField::Command;
        let key = char_key('e');
        assert!(handle_form_text(&mut model, &key));
        assert_eq!(model.agent_form.command, "e");
    }

    #[test]
    fn test_build_agent_from_inputs_valid() {
        let mut model = create_test_model();
        model.agent_form.name = "test-agent".to_string();
        model.agent_form.command = "echo".to_string();
        model.agent_form.args = "hello world".to_string();
        model.agent_form.note = "test note".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_ok());
        let agent = result.unwrap();
        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.command, "echo");
        assert_eq!(agent.args, vec!["hello".to_string(), "world".to_string()]);
        assert_eq!(agent.note, Some("test note".to_string()));
    }

    #[test]
    fn test_build_agent_from_inputs_empty_name() {
        let mut model = create_test_model();
        model.agent_form.name = "   ".to_string();
        model.agent_form.command = "echo".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MaestroError::AgentNameRequired
        ));
    }

    #[test]
    fn test_build_agent_from_inputs_empty_command() {
        let mut model = create_test_model();
        model.agent_form.name = "test-agent".to_string();
        model.agent_form.command = "   ".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MaestroError::CommandRequired));
    }

    #[test]
    fn test_build_agent_from_inputs_empty_note() {
        let mut model = create_test_model();
        model.agent_form.name = "test-agent".to_string();
        model.agent_form.command = "echo".to_string();
        model.agent_form.note = "   ".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_ok());
        let agent = result.unwrap();
        assert_eq!(agent.note, None);
    }

    #[test]
    fn test_build_agent_from_inputs_with_args() {
        let mut model = create_test_model();
        model.agent_form.name = "test-agent".to_string();
        model.agent_form.command = "codex".to_string();
        model.agent_form.args = "/review --verbose".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_ok());
        let agent = result.unwrap();
        assert_eq!(agent.command, "codex");
        assert_eq!(
            agent.args,
            vec!["/review".to_string(), "--verbose".to_string()]
        );
    }

    #[test]
    fn test_build_agent_from_inputs_empty_args() {
        let mut model = create_test_model();
        model.agent_form.name = "test-agent".to_string();
        model.agent_form.command = "echo".to_string();
        model.agent_form.args = "   ".to_string();

        let result = build_agent_from_inputs(&model);
        assert!(result.is_ok());
        let agent = result.unwrap();
        assert_eq!(agent.command, "echo");
        assert!(agent.args.is_empty());
    }

    #[test]
    fn test_apply_agent_create_duplicate() {
        let mut model = create_test_model();
        model.agents.push(create_test_agent("existing"));
        let agent = create_test_agent("existing");

        let result = apply_agent_create(&mut model, agent);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MaestroError::DuplicateAgentName(_)
        ));
    }

    #[test]
    fn test_apply_agent_edit_no_selection() {
        let mut model = create_test_model();
        let agent = create_test_agent("new-agent");

        let result = apply_agent_edit(&mut model, agent);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MaestroError::NoAgentSelected));
    }

    #[test]
    fn test_apply_agent_edit_duplicate() {
        let mut model = create_test_model();
        model.agents.push(create_test_agent("agent1"));
        model.agents.push(create_test_agent("agent2"));
        model.agent_form.target = Some(0);
        let agent = create_test_agent("agent2");

        let result = apply_agent_edit(&mut model, agent);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MaestroError::DuplicateAgentName(_)
        ));
    }
}
