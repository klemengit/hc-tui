//! hc-tui only: agents listed under their space in the spaces list
//! (`[ui.sidebar.spaces] agents`), and the tree navigation that opens a
//! space to list all of its agents (`[keys] navigate_tree`).
//!
//! The pure row model lives here so the single-machine and multi-machine
//! sidebars list the same rows in the same order.

use super::agents::{AgentRow, agent_row};
use super::snapshot::ClientShellSnapshot;
use super::status::{AgentStatus, status_priority};
use crate::config::SpaceAgentsConfig;

/// An agent that is doing or has done something: anything but idle or
/// unknown. Waiting counts, as it overlays an idle agent whose own
/// background work still runs.
pub(crate) fn is_active(status: AgentStatus) -> bool {
    !matches!(status, AgentStatus::Idle | AgentStatus::Unknown)
}

/// The index previous (`forward = false`) or next agent lands on among `len`
/// agents in list order, when only those with `active(index)` count.
/// `current` is the focused agent's index, or `None` when focus is not on an
/// agent. A focused agent that is itself inactive is not in the cycle: next
/// goes to the first active agent after it, previous to the first before it,
/// both wrapping. With no focused agent next starts at the top of the list
/// and previous at the bottom. `None` when no agent is active.
pub(crate) fn cycle_index(
    len: usize,
    current: Option<usize>,
    forward: bool,
    active: impl Fn(usize) -> bool,
) -> Option<usize> {
    let start = current.unwrap_or(if forward { len.saturating_sub(1) } else { 0 });
    (1..=len)
        .map(|step| {
            if forward {
                (start + step) % len
            } else {
                (start + len - step % len) % len
            }
        })
        .find(|&index| active(index))
}

/// How many agents a space has, idle ones included: the rows the tree lists
/// when the cursor is inside the space.
pub(crate) fn space_agent_count(snapshot: &ClientShellSnapshot, workspace_id: &str) -> usize {
    snapshot
        .agents
        .iter()
        .filter(|agent| agent.workspace_id == workspace_id)
        .count()
}

/// What a space lists: the configured filter, or every agent while the
/// navigate cursor is inside the space.
pub(crate) fn effective_filter(configured: SpaceAgentsConfig, inside: bool) -> SpaceAgentsConfig {
    if inside {
        SpaceAgentsConfig::All
    } else {
        configured
    }
}

/// The panes of `workspace_id`'s agents that `filter` lists, ordered as the
/// agents panel orders its priority view: urgent status first, then the
/// most recent status change.
pub(crate) fn space_agent_pane_ids(
    snapshot: &ClientShellSnapshot,
    workspace_id: &str,
    filter: SpaceAgentsConfig,
) -> Vec<String> {
    if filter == SpaceAgentsConfig::None {
        return Vec::new();
    }
    let mut agents = snapshot
        .agents
        .iter()
        .filter(|agent| agent.workspace_id == workspace_id)
        .filter(|agent| filter == SpaceAgentsConfig::All || is_active(agent.agent_status))
        .collect::<Vec<_>>();
    agents.sort_by_key(|agent| {
        (
            std::cmp::Reverse(status_priority(agent.agent_status)),
            std::cmp::Reverse(agent.state_change_seq),
        )
    });
    agents
        .into_iter()
        .map(|agent| agent.pane_id.clone())
        .collect()
}

/// One agent row under a space; `cursor` marks the navigate-mode cursor.
pub(crate) struct SpaceAgentRow {
    pub row: AgentRow,
    pub cursor: bool,
}

/// The agent rows to draw under `workspace_id`, using the `[ui.sidebar.agents]`
/// row layout. `machine` fills the machine token on a multi-machine sidebar.
pub(crate) fn space_agent_rows(
    snapshot: &ClientShellSnapshot,
    workspace_id: &str,
    filter: SpaceAgentsConfig,
    cursor: Option<&str>,
    config: &super::ClientShellConfig,
    machine: Option<&str>,
) -> Vec<SpaceAgentRow> {
    space_agent_pane_ids(snapshot, workspace_id, filter)
        .into_iter()
        .filter_map(|pane_id| {
            let row = agent_row(snapshot, &pane_id, config, machine)?;
            Some(SpaceAgentRow {
                cursor: cursor == Some(pane_id.as_str()),
                row,
            })
        })
        .collect()
}

/// Columns an agent row is indented by under its space row: past the
/// space's own text, and past the worktree branch glyphs of a nested space.
pub(crate) fn agent_indent(workspace_indented: bool, base: u16) -> u16 {
    if workspace_indented { base + 4 } else { base }
}

#[cfg(test)]
#[path = "space_agents_tests.rs"]
mod tests;
