// Derived from herdr src/client/shell/endpoint_sidebar.rs (Apache-2.0).

//! The row list of the multi-machine spaces list: machine rows, their
//! spaces, and (with `[ui.sidebar.spaces] agents`) the agents listed under
//! each space. Row heights, gaps and the navigate cursor's row come from
//! here so the renderer only draws.

use std::collections::HashSet;

use super::endpoints::collapsed_groups_for_endpoint;
use super::render::ShellRenderState;
use super::sidebar_render::{displayed_workspace_status, workspace_entries, workspace_rows};
use super::space_agents::{SpaceAgentRow, effective_filter, space_agent_rows};
use super::workspace::WorkspaceEntry;

pub(super) enum Row {
    Endpoint(usize),
    Workspace {
        endpoint: usize,
        entry: WorkspaceEntry,
    },
    Agent {
        endpoint: usize,
        workspace_indented: bool,
        row: SpaceAgentRow,
    },
}

pub(super) fn build_rows(
    state: &ShellRenderState<'_>,
    config: &super::ClientShellConfig,
) -> Vec<Row> {
    let empty_collapsed_groups = HashSet::new();
    let mut rows = Vec::new();
    for (endpoint_index, endpoint) in state.endpoints.iter().enumerate() {
        rows.push(Row::Endpoint(endpoint_index));
        if state.collapsed_endpoints.contains(&endpoint.endpoint_id) {
            continue;
        }
        let Some(snapshot) = endpoint.snapshot.as_deref() else {
            continue;
        };
        let collapsed_groups = collapsed_groups_for_endpoint(state, &endpoint.endpoint_id)
            .unwrap_or(&empty_collapsed_groups);
        for entry in workspace_entries(snapshot, collapsed_groups) {
            rows.push(Row::Workspace {
                endpoint: endpoint_index,
                entry,
            });
            let Some(workspace) = snapshot.workspaces.get(entry.index) else {
                continue;
            };
            let inside = state.navigate_agent.is_some()
                && state.selected_workspace_id.is_some_and(|target| {
                    target.matches(&endpoint.endpoint_id, &workspace.workspace_id)
                });
            let active_endpoint = &endpoint.endpoint_id == state.active_endpoint_id;
            rows.extend(
                space_agent_rows(
                    snapshot,
                    &workspace.workspace_id,
                    effective_filter(config.spaces.agents, inside),
                    state.navigate_agent,
                    config,
                    Some(&endpoint.label),
                )
                .into_iter()
                .map(|mut row| {
                    row.row.focused &= active_endpoint;
                    Row::Agent {
                        endpoint: endpoint_index,
                        workspace_indented: entry.indented,
                        row,
                    }
                }),
            );
        }
    }
    rows
}

pub(super) fn row_heights(
    rows: &[Row],
    state: &ShellRenderState<'_>,
    config: &super::ClientShellConfig,
) -> Vec<u16> {
    let empty_collapsed_groups = HashSet::new();
    rows.iter()
        .map(|row| match row {
            Row::Endpoint(_) => 1,
            Row::Agent { row, .. } => row.row.rows.len().max(1).min(u16::MAX as usize) as u16,
            Row::Workspace { endpoint, entry } => {
                let endpoint = &state.endpoints[*endpoint];
                let collapsed_groups = collapsed_groups_for_endpoint(state, &endpoint.endpoint_id)
                    .unwrap_or(&empty_collapsed_groups);
                endpoint
                    .snapshot
                    .as_deref()
                    .and_then(|snapshot| {
                        let workspace = snapshot.workspaces.get(entry.index)?;
                        Some(
                            workspace_rows(
                                snapshot,
                                workspace,
                                displayed_workspace_status(snapshot, workspace, collapsed_groups),
                                entry.indented,
                                &config.spaces,
                            )
                            .len()
                            .max(1)
                            .min(u16::MAX as usize) as u16,
                        )
                    })
                    .unwrap_or(1)
            }
        })
        .collect()
}

/// The blank rows after each row: `row_gap` between a space (with its
/// agents) and the next top-level space of the same machine.
pub(super) fn row_gaps(rows: &[Row], config: &super::ClientShellConfig) -> Vec<u16> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            let endpoint = match row {
                Row::Workspace { endpoint, .. } | Row::Agent { endpoint, .. } => endpoint,
                Row::Endpoint(_) => return 0,
            };
            match rows.get(index + 1) {
                Some(Row::Workspace {
                    endpoint: next_endpoint,
                    entry,
                }) if endpoint == next_endpoint => {
                    u16::from(!entry.indented) * config.spaces.row_gap
                }
                _ => 0,
            }
        })
        .collect()
}

/// The row the sidebar reveals: the navigate cursor's agent inside an open
/// space, else the navigate target's space, else the focused space.
pub(super) fn reveal_row(
    rows: &[Row],
    state: &ShellRenderState<'_>,
    active_snapshot: Option<&super::snapshot::ClientShellSnapshot>,
    reveal_navigation: bool,
) -> Option<usize> {
    rows.iter().position(|row| match row {
        Row::Agent { row, .. } => reveal_navigation && row.cursor,
        Row::Workspace { endpoint, entry } => {
            let endpoint = &state.endpoints[*endpoint];
            endpoint
                .snapshot
                .as_deref()
                .and_then(|snapshot| snapshot.workspaces.get(entry.index))
                .is_some_and(|workspace| {
                    if reveal_navigation {
                        state.navigate_agent.is_none()
                            && state.selected_workspace_id.is_some_and(|target| {
                                target.matches(&endpoint.endpoint_id, &workspace.workspace_id)
                            })
                    } else {
                        &endpoint.endpoint_id == state.active_endpoint_id
                            && active_snapshot.is_some_and(|snapshot| {
                                snapshot.focused_workspace_id.as_deref()
                                    == Some(workspace.workspace_id.as_str())
                            })
                    }
                })
        }
        Row::Endpoint(_) => false,
    })
}
