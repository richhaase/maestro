use std::collections::BTreeMap;

use zellij_tile::prelude::*;

use crate::agent::{AgentPane, PaneStatus};
use crate::model::Model;
use crate::utils::find_agent_by_command;

pub fn handle_permission_result(model: &mut Model, status: PermissionStatus) {
    match status {
        PermissionStatus::Granted => {
            model.permissions_granted = true;
            model.permissions_denied = false;
        }
        PermissionStatus::Denied => {
            model.permissions_granted = false;
            model.permissions_denied = true;
        }
    }
}

pub fn apply_tab_update(model: &mut Model, mut tabs: Vec<TabInfo>) {
    tabs.sort_by_key(|t| t.position);
    let tab_names: Vec<String> = tabs.iter().map(|t| t.name.clone()).collect();

    // Resolve pending_tab_index to actual tab names
    // Only update if current tab_name is empty or no longer exists in the tab list
    for pane in &mut model.agent_panes {
        if let Some(idx) = pane.pending_tab_index {
            if let Some(name) = tab_names.get(idx) {
                // Only update if we don't have a valid tab name yet
                if pane.tab_name.is_empty() || !tab_names.contains(&pane.tab_name) {
                    pane.tab_name = name.clone();
                }
            }
        }
    }

    // Only retain panes that either have a pane_id or have a valid tab_name or have a pending index
    model.agent_panes.retain(|p| {
        p.pane_id.is_some() || tab_names.contains(&p.tab_name) || p.pending_tab_index.is_some()
    });

    model.tab_names = tab_names;
    model.clamp_selections();
}

pub fn apply_pane_update(model: &mut Model, update: PaneManifest) {
    for (tab_idx, pane_list) in update.panes {
        let tab_name_from_idx = model.tab_names.get(tab_idx).cloned().unwrap_or_default();

        for pane in pane_list {
            if let Some(existing) = model
                .agent_panes
                .iter_mut()
                .find(|p| p.pane_id == Some(pane.id))
            {
                // Update status only - never touch tab_name for existing matched panes
                // The tab_name was set when the pane was spawned and should be preserved
                existing.status = if pane.exited {
                    PaneStatus::Exited(pane.exit_status)
                } else {
                    PaneStatus::Running
                };
                continue;
            }

            // This is a new pane we haven't seen before (discovered via PaneUpdate)
            let title = pane.title.clone();
            let command_hint = pane.terminal_command.as_deref().unwrap_or(&title);

            if !pane.is_plugin {
                if let Some(agent) = find_agent_by_command(&model.agents, command_hint) {
                    let agent_name = agent.name.clone();
                    model.agent_panes.push(AgentPane {
                        pane_title: title,
                        tab_name: tab_name_from_idx.clone(),
                        pending_tab_index: if tab_name_from_idx.is_empty() {
                            Some(tab_idx)
                        } else {
                            None
                        },
                        pane_id: Some(pane.id),
                        workspace_path: String::new(),
                        agent_name,
                        status: if pane.exited {
                            PaneStatus::Exited(pane.exit_status)
                        } else {
                            PaneStatus::Running
                        },
                    });
                }
            }
        }
    }
    model.clamp_selections();
}

pub fn handle_command_pane_opened(model: &mut Model, pane_id: u32, ctx: BTreeMap<String, String>) {
    let title = ctx
        .get("pane_title")
        .cloned()
        .unwrap_or_else(|| format!("pane:{pane_id}"));
    let workspace_path = ctx.get("cwd").cloned().unwrap_or_default();
    let agent_name = ctx.get("agent").cloned().unwrap_or_default();

    let tab_names_snapshot = model.tab_names.clone();
    let first_tab = tab_names_snapshot.first().cloned();
    let ctx_tab_name = ctx.get("tab_name").cloned();
    let entry = model
        .agent_panes
        .iter_mut()
        .find(|p| p.pane_id == Some(pane_id) || (p.pane_id.is_none() && p.pane_title == title));

    if let Some(existing) = entry {
        existing.pane_id = Some(pane_id);
        existing.pane_title = title.clone();

        if existing.tab_name.is_empty() {
            if let Some(tab_name) = ctx_tab_name.clone() {
                existing.tab_name = tab_name;
                existing.pending_tab_index = None;
            } else if let Some(first_tab) = first_tab {
                existing.tab_name = first_tab;
                existing.pending_tab_index = None;
            }
        }
        if !workspace_path.is_empty() {
            existing.workspace_path = workspace_path.clone();
        }
        if !agent_name.is_empty() {
            existing.agent_name = agent_name.clone();
        }
        existing.status = PaneStatus::Running;
    } else {
        let tab_name = ctx_tab_name
            .clone()
            .or_else(|| tab_names_snapshot.first().cloned())
            .unwrap_or_default();
        model.agent_panes.push(AgentPane {
            pane_title: title,
            tab_name,
            pending_tab_index: None,
            pane_id: Some(pane_id),
            workspace_path,
            agent_name,
            status: PaneStatus::Running,
        });
    }
    model.clamp_selections();
}

fn rebuild_from_session_infos(model: &mut Model, session_infos: &[SessionInfo]) {
    for session in session_infos {
        let session_name = session.name.clone();
        if let Some(current) = &model.session_name {
            if &session_name != current {
                continue;
            }
        } else {
            model.session_name = Some(session_name.clone());
        }

        // Build tab lookup from session info
        let mut tab_lookup = BTreeMap::new();
        for tab in &session.tabs {
            tab_lookup.insert(tab.position, tab.name.clone());
        }

        for (tab_idx, pane_list) in &session.panes.panes {
            let tab_name_from_idx = tab_lookup.get(tab_idx).cloned().unwrap_or_default();

            let mut unmatched_in_tab: Vec<usize> = if !tab_name_from_idx.is_empty() {
                model
                    .agent_panes
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.pane_id.is_none() && p.tab_name == tab_name_from_idx)
                    .map(|(idx, _)| idx)
                    .collect()
            } else {
                Vec::new()
            };

            for pane in pane_list {
                if let Some(existing) = model
                    .agent_panes
                    .iter_mut()
                    .find(|p| p.pane_id == Some(pane.id))
                {
                    // Update status only - preserve tab_name
                    existing.status = if pane.exited {
                        PaneStatus::Exited(pane.exit_status)
                    } else {
                        PaneStatus::Running
                    };
                    continue;
                }

                if let Some(unmatched_idx) = unmatched_in_tab.pop() {
                    let existing = &mut model.agent_panes[unmatched_idx];
                    existing.pane_id = Some(pane.id);
                    existing.status = if pane.exited {
                        PaneStatus::Exited(pane.exit_status)
                    } else {
                        PaneStatus::Running
                    };
                    // tab_name already set, don't overwrite
                    continue;
                }

                // New pane discovered via session info
                if !pane.is_plugin {
                    let command_hint = pane.terminal_command.as_deref().unwrap_or(&pane.title);
                    if let Some(agent) = find_agent_by_command(&model.agents, command_hint) {
                        let agent_name = agent.name.clone();
                        model.agent_panes.push(AgentPane {
                            pane_title: pane.title.clone(),
                            tab_name: tab_name_from_idx.clone(),
                            pending_tab_index: if tab_name_from_idx.is_empty() {
                                Some(*tab_idx)
                            } else {
                                None
                            },
                            pane_id: Some(pane.id),
                            workspace_path: String::new(),
                            agent_name,
                            status: if pane.exited {
                                PaneStatus::Exited(pane.exit_status)
                            } else {
                                PaneStatus::Running
                            },
                        });
                    }
                }
            }
        }
    }
    model.clamp_selections();
}

pub fn handle_command_pane_exited(
    model: &mut Model,
    pane_id: u32,
    exit_status: Option<i32>,
    ctx: BTreeMap<String, String>,
) {
    let title = ctx
        .get("pane_title")
        .cloned()
        .unwrap_or_else(|| format!("pane:{pane_id}"));
    if let Some(pane) = model
        .agent_panes
        .iter_mut()
        .find(|p| p.pane_id == Some(pane_id) || p.pane_title == title)
    {
        pane.status = PaneStatus::Exited(exit_status);
    }
    model.clamp_selections();
}

pub fn handle_command_pane_rerun(model: &mut Model, pane_id: u32, ctx: BTreeMap<String, String>) {
    let title = ctx
        .get("pane_title")
        .cloned()
        .unwrap_or_else(|| format!("pane:{pane_id}"));
    if let Some(pane) = model
        .agent_panes
        .iter_mut()
        .find(|p| p.pane_id == Some(pane_id) || p.pane_title == title)
    {
        pane.status = PaneStatus::Running;
    }
    model.clamp_selections();
}

pub fn handle_session_update(model: &mut Model, sessions: Vec<SessionInfo>) {
    let current_session_name = sessions
        .iter()
        .find(|s| s.is_current_session)
        .map(|s| s.name.clone());

    if let Some(new_session_name) = current_session_name {
        if let Some(ref old_session_name) = model.session_name {
            if old_session_name != &new_session_name {
                model.agent_panes.clear();
            }
        }
        model.session_name = Some(new_session_name);
    }

    // Keep tab_names in sync with the current session snapshot to avoid stale state
    // between session updates and subsequent TabUpdate events.
    if let Some(ref session_name) = model.session_name {
        if let Some(session) = sessions.iter().find(|s| &s.name == session_name) {
            let mut tabs = session.tabs.clone();
            tabs.sort_by_key(|t| t.position);
            model.tab_names = tabs.into_iter().map(|t| t.name).collect();
        }
    }

    rebuild_from_session_infos(model, &sessions);
}

pub fn handle_pane_closed(model: &mut Model, pane_id: PaneId) {
    let pid = match pane_id {
        PaneId::Terminal(id) | PaneId::Plugin(id) => id,
    };
    model.agent_panes.retain(|p| p.pane_id != Some(pid));
    model.clamp_selections();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn make_tab(name: &str, position: usize) -> TabInfo {
        TabInfo {
            position,
            name: name.to_string(),
            active: position == 0,
            panes_to_hide: 0,
            is_fullscreen_active: false,
            is_sync_panes_active: false,
            are_floating_panes_visible: false,
            other_focused_clients: Vec::new(),
            active_swap_layout_name: None,
            is_swap_layout_dirty: false,
            viewport_rows: 0,
            viewport_columns: 0,
            display_area_rows: 0,
            display_area_columns: 0,
            selectable_tiled_panes_count: 0,
            selectable_floating_panes_count: 0,
        }
    }

    fn make_session(name: &str, tabs: Vec<TabInfo>, is_current: bool) -> SessionInfo {
        SessionInfo {
            name: name.to_string(),
            tabs,
            panes: PaneManifest::default(),
            connected_clients: 0,
            is_current_session: is_current,
            available_layouts: Vec::new(),
            plugins: BTreeMap::new(),
            web_clients_allowed: false,
            web_client_count: 0,
            tab_history: BTreeMap::new(),
        }
    }

    #[test]
    fn tab_names_hydrate_on_session_update() {
        let tabs = vec![make_tab("two", 1), make_tab("one", 0)];
        let session = make_session("s", tabs, true);
        let mut model = Model::default();

        handle_session_update(&mut model, vec![session]);

        assert_eq!(model.session_name.as_deref(), Some("s"));
        assert_eq!(model.tab_names, vec!["one".to_string(), "two".to_string()]);
    }
}
