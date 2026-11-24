// Derived from herdr src/client/shell/workspace_navigation.rs (Apache-2.0).

use std::collections::{HashMap, HashSet};

use super::sidebar_render::workspace_entries;
use super::snapshot::{ClientEndpointId, ClientEndpointStatus, ClientShellEndpoint};

/// A client-only preview. Snapshot identity prevents Enter from using a reused
/// workspace ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WorkspaceNavigationTarget {
    pub endpoint_id: ClientEndpointId,
    pub workspace_id: String,
    pub(crate) boot_id: String,
    pub(crate) generation: Option<u64>,
}

impl WorkspaceNavigationTarget {
    pub(crate) fn matches(&self, endpoint_id: &ClientEndpointId, workspace_id: &str) -> bool {
        &self.endpoint_id == endpoint_id && self.workspace_id == workspace_id
    }
}

pub(crate) fn navigation_target(
    endpoints: &[ClientShellEndpoint],
    endpoint_id: &ClientEndpointId,
    workspace_id: &str,
) -> Option<WorkspaceNavigationTarget> {
    let endpoint = endpoints
        .iter()
        .find(|entry| &entry.endpoint_id == endpoint_id)?;
    let snapshot = endpoint.snapshot.as_deref()?;
    Some(WorkspaceNavigationTarget {
        endpoint_id: endpoint_id.clone(),
        workspace_id: workspace_id.to_owned(),
        boot_id: snapshot.boot_id.clone(),
        generation: endpoint.snapshot_generation,
    })
}

pub(crate) fn focused_navigation_target(
    endpoints: &[ClientShellEndpoint],
    active_endpoint_id: &ClientEndpointId,
) -> Option<WorkspaceNavigationTarget> {
    let endpoint = endpoints
        .iter()
        .find(|entry| &entry.endpoint_id == active_endpoint_id)?;
    let workspace_id = endpoint
        .snapshot
        .as_deref()?
        .focused_workspace_id
        .as_deref()?;
    navigation_target(endpoints, active_endpoint_id, workspace_id)
}

pub(crate) fn navigation_target_valid(
    endpoints: &[ClientShellEndpoint],
    target: &WorkspaceNavigationTarget,
) -> bool {
    endpoints.iter().any(|endpoint| {
        endpoint.endpoint_id == target.endpoint_id
            && endpoint.status == ClientEndpointStatus::Online
            && endpoint.snapshot_generation == target.generation
            && endpoint.snapshot.as_deref().is_some_and(|snapshot| {
                snapshot.boot_id == target.boot_id
                    && snapshot
                        .workspaces
                        .iter()
                        .any(|workspace| workspace.workspace_id == target.workspace_id)
            })
    })
}

/// Ordered navigation targets, matching `move_navigate_workspace`'s entry
/// enumeration: online endpoints only; the collapsed sidebar uses flat
/// workspace order, the expanded sidebar honors grouped/collapsed worktrees,
/// and mobile ignores collapsed groups.
pub(crate) fn workspace_navigation_targets(
    endpoints: &[ClientShellEndpoint],
    sidebar_collapsed: bool,
    mobile: bool,
    surface_available: bool,
    collapsed_groups: &HashSet<String>,
    remote_collapsed_groups: &HashMap<ClientEndpointId, HashSet<String>>,
) -> Vec<WorkspaceNavigationTarget> {
    let empty_collapsed_groups = HashSet::new();
    let mut targets = Vec::new();
    for endpoint in endpoints {
        if endpoint.status != ClientEndpointStatus::Online {
            continue;
        }
        let Some(snapshot) = endpoint.snapshot.as_deref() else {
            continue;
        };
        let entries = if sidebar_collapsed && !mobile && surface_available {
            snapshot
                .workspaces
                .iter()
                .enumerate()
                .map(|(index, _)| super::workspace::WorkspaceEntry {
                    index,
                    indented: false,
                    last_child: false,
                })
                .collect()
        } else {
            let groups = if mobile && surface_available {
                &empty_collapsed_groups
            } else if endpoint.endpoint_id.is_local() {
                collapsed_groups
            } else {
                remote_collapsed_groups
                    .get(&endpoint.endpoint_id)
                    .unwrap_or(&empty_collapsed_groups)
            };
            workspace_entries(snapshot, groups)
        };
        for entry in entries {
            targets.push(WorkspaceNavigationTarget {
                endpoint_id: endpoint.endpoint_id.clone(),
                workspace_id: snapshot.workspaces[entry.index].workspace_id.clone(),
                boot_id: snapshot.boot_id.clone(),
                generation: endpoint.snapshot_generation,
            });
        }
    }
    targets
}

/// `move_navigate_workspace` selection step: wrap on desktop, clamp on mobile.
pub(crate) fn move_index(
    targets: &[WorkspaceNavigationTarget],
    current: Option<&WorkspaceNavigationTarget>,
    delta: isize,
    mobile: bool,
) -> Option<usize> {
    if targets.is_empty() {
        return None;
    }
    let current = current.and_then(|selected| targets.iter().position(|t| t == selected));
    Some(match current {
        Some(current) if mobile => {
            (current as isize + delta).clamp(0, targets.len() as isize - 1) as usize
        }
        Some(current) => (current as isize + delta).rem_euclid(targets.len() as isize) as usize,
        None if delta < 0 => targets.len() - 1,
        None => 0,
    })
}

/// Tree navigation machine jump over the flat target list, whose targets
/// are grouped by machine in sidebar order (offline, empty and snapshot-less
/// machines have no targets, so they are skipped). Forward goes to the first
/// space of the next machine. Backward goes to the first space of the
/// current machine, or, when the cursor is already there, of the previous
/// one. Both wrap around the ends. `None` when there is no current target.
pub(crate) fn machine_jump(
    targets: &[WorkspaceNavigationTarget],
    current: Option<&WorkspaceNavigationTarget>,
    forward: bool,
) -> Option<usize> {
    let current = current?;
    let at = targets
        .iter()
        .position(|t| t.matches(&current.endpoint_id, &current.workspace_id))?;
    let mut starts: Vec<usize> = Vec::new();
    for (index, target) in targets.iter().enumerate() {
        if index == 0 || targets[index - 1].endpoint_id != target.endpoint_id {
            starts.push(index);
        }
    }
    // The group the cursor is in: the last start at or before it.
    let group = starts.iter().rposition(|&start| start <= at)?;
    let wanted = if forward {
        (group + 1) % starts.len()
    } else if starts[group] != at {
        group
    } else {
        (group + starts.len() - 1) % starts.len()
    };
    Some(starts[wanted])
}

#[cfg(test)]
mod machine_jump_tests {
    use super::*;

    fn target(machine: &str, workspace: &str) -> WorkspaceNavigationTarget {
        WorkspaceNavigationTarget {
            endpoint_id: if machine.is_empty() {
                ClientEndpointId::Local
            } else {
                ClientEndpointId::Remote(machine.into())
            },
            workspace_id: workspace.into(),
            boot_id: String::new(),
            generation: None,
        }
    }

    /// local: a b c | host-a: d e | host-b: f
    fn targets() -> Vec<WorkspaceNavigationTarget> {
        vec![
            target("", "a"),
            target("", "b"),
            target("", "c"),
            target("host-a", "d"),
            target("host-a", "e"),
            target("host-b", "f"),
        ]
    }

    fn jump(from: usize, forward: bool) -> usize {
        let targets = targets();
        machine_jump(&targets, Some(&targets[from]), forward).unwrap()
    }

    #[test]
    fn forward_goes_to_the_first_space_of_the_next_machine_and_wraps() {
        assert_eq!(jump(0, true), 3);
        assert_eq!(jump(2, true), 3);
        assert_eq!(jump(4, true), 5);
        assert_eq!(jump(5, true), 0);
    }

    #[test]
    fn backward_goes_to_the_own_machine_start_first_then_the_previous_machine() {
        assert_eq!(jump(4, false), 3, "inside host-a: its own first space");
        assert_eq!(
            jump(3, false),
            0,
            "on host-a's first space: previous machine"
        );
        assert_eq!(jump(2, false), 0);
        assert_eq!(jump(0, false), 5, "wraps to the last machine");
    }

    #[test]
    fn one_machine_jumps_within_itself_and_nothing_jumps_without_a_cursor() {
        let one = vec![target("", "a"), target("", "b")];
        assert_eq!(machine_jump(&one, Some(&one[1]), true), Some(0));
        assert_eq!(machine_jump(&one, Some(&one[1]), false), Some(0));
        assert_eq!(machine_jump(&one, Some(&one[0]), false), Some(0));
        assert_eq!(machine_jump(&one, None, true), None);
        assert_eq!(machine_jump(&[], Some(&one[0]), true), None);
    }
}
