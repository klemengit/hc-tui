//! Which agents a space lists, and in what order.

use super::*;
use crate::project::project;
use crate::sidebar::fixture::{agent, view};

fn spaced(address: &str, word: &str, space: &str) -> hc_proto::terminal::Terminal {
    let mut term = agent(address, word);
    term.labels.insert("herdr.space".into(), space.into());
    term
}

/// `one` holds an idle, a working, a blocked and a done agent; `two` holds
/// one working agent.
fn snapshot() -> ClientShellSnapshot {
    let terms = vec![
        spaced("t1", "idle", "one"),
        spaced("t2", "working", "one"),
        spaced("t3", "blocked", "one"),
        spaced("t4", "done", "one"),
        spaced("t5", "working", "two"),
    ];
    project(&view(terms, vec![]), Some("t1")).snapshot
}

fn workspace_id(snapshot: &ClientShellSnapshot, label: &str) -> String {
    snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.label == label)
        .map(|workspace| workspace.workspace_id.clone())
        .unwrap()
}

#[test]
fn none_lists_no_agents() {
    let snapshot = snapshot();
    let one = workspace_id(&snapshot, "one");
    assert!(space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::None).is_empty());
}

#[test]
fn active_skips_idle_and_orders_by_priority() {
    let snapshot = snapshot();
    let one = workspace_id(&snapshot, "one");
    // Blocked > done > working; the idle t1 stays out.
    assert_eq!(
        space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::Active),
        ["t3", "t4", "t2"]
    );
}

#[test]
fn all_lists_idle_agents_last() {
    let snapshot = snapshot();
    let one = workspace_id(&snapshot, "one");
    assert_eq!(
        space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::All),
        ["t3", "t4", "t2", "t1"]
    );
}

#[test]
fn a_space_lists_only_its_own_agents() {
    let snapshot = snapshot();
    let two = workspace_id(&snapshot, "two");
    assert_eq!(
        space_agent_pane_ids(&snapshot, &two, SpaceAgentsConfig::All),
        ["t5"]
    );
    assert!(space_agent_pane_ids(&snapshot, "missing", SpaceAgentsConfig::All).is_empty());
}

#[test]
fn the_most_recent_change_wins_between_equal_statuses() {
    let mut snapshot = snapshot();
    let one = workspace_id(&snapshot, "one");
    for agent in &mut snapshot.agents {
        agent.agent_status = AgentStatus::Working;
        agent.state_change_seq = match agent.pane_id.as_str() {
            "t1" => 7,
            "t2" => 9,
            _ => 1,
        };
    }
    assert_eq!(
        space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::All),
        ["t2", "t1", "t3", "t4"]
    );
}

#[test]
fn active_means_anything_but_idle_or_unknown() {
    for (status, active) in [
        (AgentStatus::Blocked, true),
        (AgentStatus::Done, true),
        (AgentStatus::Working, true),
        (AgentStatus::Waiting, true),
        (AgentStatus::Idle, false),
        (AgentStatus::Unknown, false),
    ] {
        assert_eq!(is_active(status), active, "{status:?}");
    }
}

#[test]
fn opening_a_space_lists_every_agent_whatever_the_setting() {
    for configured in [
        SpaceAgentsConfig::None,
        SpaceAgentsConfig::Active,
        SpaceAgentsConfig::All,
    ] {
        assert_eq!(effective_filter(configured, true), SpaceAgentsConfig::All);
        assert_eq!(effective_filter(configured, false), configured);
    }
}

#[test]
fn rows_mark_the_cursor_agent() {
    let snapshot = snapshot();
    let one = workspace_id(&snapshot, "one");
    let shell = crate::chrome::ClientShellConfig::from_config(
        &crate::config::Config::default(),
        None,
        None,
        std::path::PathBuf::new(),
    );
    let rows = space_agent_rows(
        &snapshot,
        &one,
        SpaceAgentsConfig::All,
        Some("t2"),
        &shell,
        None,
    );
    let marked = rows
        .iter()
        .map(|row| (row.row.pane_id.as_str(), row.cursor))
        .collect::<Vec<_>>();
    assert_eq!(
        marked,
        [("t3", false), ("t4", false), ("t2", true), ("t1", false)]
    );
}
