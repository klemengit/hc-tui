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
    assert!(space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::None, None).is_empty());
}

#[test]
fn active_skips_idle_and_orders_by_priority() {
    let snapshot = snapshot();
    let one = workspace_id(&snapshot, "one");
    // Blocked > done > working; the idle t1 stays out.
    assert_eq!(
        space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::Active, None),
        ["t3", "t4", "t2"]
    );
}

#[test]
fn all_lists_idle_agents_last() {
    let snapshot = snapshot();
    let one = workspace_id(&snapshot, "one");
    assert_eq!(
        space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::All, None),
        ["t3", "t4", "t2", "t1"]
    );
}

#[test]
fn the_kept_agent_is_listed_once_beside_the_active_ones() {
    let snapshot = snapshot();
    let one = workspace_id(&snapshot, "one");
    // The idle t1 joins the active rows, in the usual order, once.
    assert_eq!(
        space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::Active, Some("t1")),
        ["t3", "t4", "t2", "t1"]
    );
    // A kept agent that is active already is not doubled.
    assert_eq!(
        space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::Active, Some("t2")),
        ["t3", "t4", "t2"]
    );
    // It is listed even with `agents = none`.
    assert_eq!(
        space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::None, Some("t1")),
        ["t1"]
    );
}

#[test]
fn the_kept_agent_needs_a_space_of_two_agents() {
    let snapshot = snapshot();
    let two = workspace_id(&snapshot, "two");
    assert!(space_agent_pane_ids(&snapshot, &two, SpaceAgentsConfig::None, Some("t5")).is_empty());
}

#[test]
fn a_space_lists_only_its_own_agents() {
    let snapshot = snapshot();
    let two = workspace_id(&snapshot, "two");
    assert_eq!(
        space_agent_pane_ids(&snapshot, &two, SpaceAgentsConfig::All, None),
        ["t5"]
    );
    assert!(space_agent_pane_ids(&snapshot, "missing", SpaceAgentsConfig::All, None).is_empty());
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
        space_agent_pane_ids(&snapshot, &one, SpaceAgentsConfig::All, None),
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

mod cycle {
    use super::super::cycle_index;

    /// Agents 0..5, active where `mask` has a true.
    fn go(mask: &[bool], current: Option<usize>, forward: bool) -> Option<usize> {
        cycle_index(mask.len(), current, forward, |i| mask[i])
    }

    const MASK: [bool; 5] = [true, false, true, false, false];

    #[test]
    fn steps_over_inactive_agents_and_wraps() {
        assert_eq!(go(&MASK, Some(0), true), Some(2));
        assert_eq!(go(&MASK, Some(2), true), Some(0));
        assert_eq!(go(&MASK, Some(0), false), Some(2));
        assert_eq!(go(&MASK, Some(2), false), Some(0));
    }

    #[test]
    fn an_inactive_focused_agent_goes_to_the_neighbour_in_list_order() {
        assert_eq!(go(&MASK, Some(1), true), Some(2));
        assert_eq!(go(&MASK, Some(1), false), Some(0));
        assert_eq!(go(&MASK, Some(4), true), Some(0), "wraps past the end");
        assert_eq!(go(&MASK, Some(3), false), Some(2));
    }

    #[test]
    fn no_focused_agent_starts_at_the_top_or_the_bottom() {
        assert_eq!(go(&MASK, None, true), Some(0));
        assert_eq!(go(&MASK, None, false), Some(2));
    }

    #[test]
    fn nothing_active_or_empty_does_nothing() {
        assert_eq!(go(&[false, false], Some(0), true), None);
        assert_eq!(go(&[false, false], None, false), None);
        assert_eq!(go(&[], None, true), None);
    }

    #[test]
    fn a_single_active_agent_stays_put() {
        assert_eq!(go(&[true, false], Some(0), true), Some(0));
        assert_eq!(go(&[true, false], Some(0), false), Some(0));
    }
}
