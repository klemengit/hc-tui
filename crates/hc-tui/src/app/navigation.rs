// Derived from herdr src/client/shell/input.rs and composition.rs (Apache-2.0).
use super::*;

impl App {
    /// Dispatch navigation and M3a layout/tab operations. Deferred actions
    /// keep their reference bindings (documented in docs/parity.md).
    pub(super) fn on_action(&mut self, action: crate::input::KeybindAction, _width: u16) {
        use crate::input::KeybindAction as A;
        match action {
            A::Detach => self.quitting = true,
            A::Help => {
                self.help = true;
                self.help_state = crate::chrome::help::HelpState::default();
            }
            A::ReloadConfig => self.reload_config(),
            A::Update => self.open_update(),
            A::Settings => self.open_settings_overlay(),
            A::ToggleSidebar => {
                self.shell_state.sidebar_collapsed = !self.shell_state.sidebar_collapsed;
                self.sidebar_collapsed_manual = true;
                self.persist_chrome_preferences();
            }
            A::OpenNotificationTarget => self.focus_visible_notification(),
            A::CopyMode => {
                self.enter_copy_mode();
            }
            A::OpenNavigator => self.open_navigator_overlay(),
            A::WorkspacePicker => self.enter_workspace_picker(),
            // Workspace-level actions don't require a focused workspace to
            // exist (new_workspace is valid with an empty shell too).
            A::NewWorkspace => self.new_workspace(),
            A::RenameWorkspace => self.rename_workspace(),
            A::CloseWorkspace => {
                if let Some(id) = self.projection.snapshot.focused_workspace_id.clone() {
                    self.close_workspace(&id);
                }
            }
            _ => self.layout_action(action),
        }
        self.sync_target();
    }

    /// Enter Navigate with the pending highlight on the focused
    /// workspace — `focused_navigation_target`. Shared by the keybind
    /// and the mobile switch button (the reference's one entry path).
    pub(super) fn enter_workspace_picker(&mut self) {
        self.before_navigate = self.focused.clone();
        self.mode = Mode::Navigate;
        self.prefix.reset();
        self.shell_state.mobile_switcher_scroll = 0;
        self.shell_state.reveal_mobile_workspace = false;
        // Refresh first so a focus set without a publish still
        // resolves (finding 12).
        self.refresh_projection();
        self.navigate_agent = None;
        self.navigate_target = crate::chrome::navigate::focused_navigation_target(
            &self.projection.endpoints,
            &self.projection.active_endpoint_id,
        );
    }

    /// Tree navigation is on and the sidebar can show it: the expanded
    /// desktop sidebar. Mobile and the collapsed sidebar keep the flat walk.
    fn tree_navigation(&self) -> bool {
        let mobile = self.area.0 <= self.shell.mobile_width_threshold;
        self.live.keybinds.navigate.tree && !mobile && !self.shell_state.sidebar_collapsed
    }

    /// Every agent of the highlighted space (idle ones included) in the
    /// order the sidebar lists them: the rows the cursor can enter.
    fn navigate_space_agents(&self) -> Vec<String> {
        let Some(target) = self.navigate_target.as_ref() else {
            return Vec::new();
        };
        self.projection
            .endpoints
            .iter()
            .find(|endpoint| endpoint.endpoint_id == target.endpoint_id)
            .and_then(|endpoint| endpoint.snapshot.as_deref())
            .map(|snapshot| {
                crate::chrome::space_agents::space_agent_pane_ids(
                    snapshot,
                    &target.workspace_id,
                    crate::config::SpaceAgentsConfig::All,
                    None,
                )
            })
            .unwrap_or_default()
    }

    /// The workspace-navigation targets across all online endpoints —
    /// the sidebar-visible rows honoring collapse state.
    pub(super) fn navigate_targets(&self) -> Vec<WorkspaceNavigationTarget> {
        let mobile = self.area.0 <= self.shell.mobile_width_threshold;
        crate::chrome::navigate::workspace_navigation_targets(
            &self.projection.endpoints,
            self.shell_state.sidebar_collapsed,
            mobile,
            true,
            &self.shell_state.collapsed_groups,
            &self.shell_state.remote_collapsed_groups,
        )
    }

    /// The terminal a workspace target focuses — its active tab's first
    /// pane, else its first pane (`pane_id` is the route address).
    pub(super) fn workspace_terminal(&self, target: &WorkspaceNavigationTarget) -> Option<String> {
        let endpoint = self
            .projection
            .endpoints
            .iter()
            .find(|e| e.endpoint_id == target.endpoint_id)?;
        let snap = endpoint.snapshot.as_deref()?;
        let ws = snap
            .workspaces
            .iter()
            .find(|w| w.workspace_id == target.workspace_id)?;
        let machine = match &target.endpoint_id {
            ClientEndpointId::Local => None,
            ClientEndpointId::Remote(machine) => Some(machine.as_str()),
        };
        let preferred = self
            .view
            .documents
            .get(&shared::key())
            .and_then(|d| crate::structure::Document::decode(d.value.clone()).ok())
            .and_then(|d| {
                d.tabs
                    .get(&crate::project::layout_key(machine, &ws.active_tab_id))
                    .and_then(|t| t.focused.clone())
            });
        if let Some(id) = preferred.filter(|id| {
            snap.panes.iter().any(|p| {
                &p.pane_id == id
                    && p.tab_id == ws.active_tab_id
                    && p.workspace_id == ws.workspace_id
            })
        }) {
            return Some(id);
        }
        snap.panes
            .iter()
            .find(|p| p.workspace_id == ws.workspace_id && p.tab_id == ws.active_tab_id)
            .or_else(|| {
                snap.panes
                    .iter()
                    .find(|p| p.workspace_id == ws.workspace_id)
            })
            .map(|p| p.pane_id.clone())
    }

    /// `state.rs::reveal_workspace` — expand the highlighted
    /// workspace's collapsed group and position its sidebar entry
    /// on-screen (the reference's `workspace_scroll = pos.min(max)`).
    pub(super) fn reveal_workspace(&mut self, target: &WorkspaceNavigationTarget) {
        self.shell_state
            .collapsed_endpoints
            .remove(&target.endpoint_id);
        if target.endpoint_id.is_local() {
            self.shell_state
                .collapsed_groups
                .remove(&target.workspace_id);
        } else if let Some(groups) = self
            .shell_state
            .remote_collapsed_groups
            .get_mut(&target.endpoint_id)
        {
            groups.remove(&target.workspace_id);
        }
        // The reference scrolls directly only on the single-endpoint
        // desktop path; multi-endpoint/mobile reveal through the flag.
        let mobile = self.area.0 <= self.shell.mobile_width_threshold;
        if self.projection.endpoints.len() != 1 || mobile {
            return;
        }
        // Already on-screen: the hit map covers it, no scroll change.
        if self
            .hit_map
            .workspaces
            .iter()
            .any(|hit| hit.workspace_id == target.workspace_id)
        {
            return;
        }
        let position = self
            .projection
            .endpoints
            .iter()
            .find(|e| e.endpoint_id == target.endpoint_id)
            .and_then(|e| e.snapshot.as_deref())
            .and_then(|snapshot| {
                let groups = if target.endpoint_id.is_local() {
                    &self.shell_state.collapsed_groups
                } else {
                    self.shell_state
                        .remote_collapsed_groups
                        .get(&target.endpoint_id)
                        .unwrap_or(&self.shell_state.collapsed_groups)
                };
                crate::chrome::sidebar_render::workspace_entries(snapshot, groups)
                    .iter()
                    .position(|entry| {
                        snapshot.workspaces[entry.index].workspace_id == target.workspace_id
                    })
            });
        if let Some(position) = position {
            self.shell_state.workspace_scroll = position.min(self.hit_map.workspace_max_scroll);
        }
    }

    /// One Navigate outcome against the highlight.
    pub(super) fn on_navigate(&mut self, nav: input::Navigate) {
        use input::Navigate::*;
        let mobile = self.area.0 <= self.shell.mobile_width_threshold;
        match nav {
            // Inside a space the open key does what Enter does: focus the agent.
            Enter if self.tree_navigation() && self.navigate_agent.is_some() => {
                self.on_navigate(Accept);
                return;
            }
            Enter if self.tree_navigation() => {
                if self.navigate_agent.is_none() {
                    let agents = self.navigate_space_agents();
                    let jump = agents.len() == 1
                        && self.live.keybinds.navigate.enter_single
                            == crate::config::NavigateEnterSingle::Jump;
                    // With `show_last_agent` the cursor starts on the agent
                    // focused last in this space.
                    let last = self
                        .navigate_target
                        .as_ref()
                        .filter(|_| self.shell.spaces.show_last_agent)
                        .and_then(|target| {
                            self.last_agents
                                .get(&(target.endpoint_id.clone(), target.workspace_id.clone()))
                        })
                        .filter(|last| agents.contains(last))
                        .cloned();
                    if let Some(first) = last.or_else(|| agents.into_iter().next()) {
                        self.navigate_agent = Some(first);
                        self.shell_state.reveal_navigation_workspace = true;
                        if jump {
                            self.on_navigate(Accept);
                            return;
                        }
                    }
                }
            }
            Leave if self.tree_navigation() => {
                if self.navigate_agent.take().is_some() {
                    self.shell_state.reveal_navigation_workspace = true;
                }
            }
            MachineJump(direction) if self.tree_navigation() => {
                // From inside a space this also leaves it.
                self.navigate_agent = None;
                let mut targets = self.navigate_targets();
                if let Some(index) = crate::chrome::navigate::machine_jump(
                    &targets,
                    self.navigate_target.as_ref(),
                    direction > 0,
                ) {
                    let target = targets.swap_remove(index);
                    self.reveal_workspace(&target);
                    self.navigate_target = Some(target);
                    self.shell_state.reveal_navigation_workspace = true;
                }
            }
            // Without tree navigation the keys do not exist.
            Enter | Leave | MachineJump(_) => {}
            Move(delta) if self.navigate_agent.is_some() => {
                let agents = self.navigate_space_agents();
                let current = agents
                    .iter()
                    .position(|pane| Some(pane) == self.navigate_agent.as_ref());
                if !agents.is_empty() {
                    let len = agents.len() as isize;
                    let next = match current {
                        Some(index) => (index as isize + delta as isize).rem_euclid(len),
                        None => 0,
                    };
                    self.navigate_agent = Some(agents[next as usize].clone());
                    self.shell_state.reveal_navigation_workspace = true;
                }
            }
            Accept if self.navigate_agent.is_some() => {
                let pane = self.navigate_agent.take();
                let valid = self
                    .navigate_space_agents()
                    .into_iter()
                    .any(|candidate| Some(&candidate) == pane.as_ref());
                if let (true, Some(pane)) = (valid, pane) {
                    self.navigate_target = None;
                    self.mode = Mode::Normal;
                    self.focus_pane(pane);
                } else {
                    // The agent went away while the cursor was on it.
                    self.show_notice(
                        "Agent unavailable",
                        "That agent is gone; the cursor is back on its space",
                    );
                }
            }
            Move(delta) => {
                let mut targets = self.navigate_targets();
                if let Some(index) = crate::chrome::navigate::move_index(
                    &targets,
                    self.navigate_target.as_ref(),
                    delta as isize,
                    mobile,
                ) {
                    let target = targets.swap_remove(index);
                    self.reveal_workspace(&target);
                    self.navigate_target = Some(target);
                    self.shell_state.reveal_navigation_workspace = true;
                    // The mobile switcher scrolls to the highlight too.
                    self.shell_state.reveal_mobile_workspace = mobile;
                }
            }
            // `1..9` selects the nth *active-endpoint* target and
            // commits like Enter — the reference's digit binding is
            // `SwitchWorkspace(index)` against the active snapshot's
            // navigation entries, exiting Navigate in one press.
            Select(index) => {
                let targets: Vec<_> = self
                    .navigate_targets()
                    .into_iter()
                    .filter(|t| t.endpoint_id == self.projection.active_endpoint_id)
                    .collect();
                if let Some(target) = targets.get(index).cloned() {
                    self.reveal_workspace(&target);
                    if crate::chrome::navigate::navigation_target_valid(
                        &self.projection.endpoints,
                        &target,
                    ) && let Some(terminal) = self.workspace_terminal(&target)
                    {
                        self.focused = Some(terminal);
                        self.refresh_projection();
                        self.mark_active_seen();
                    }
                    self.navigate_target = None;
                    self.navigate_agent = None;
                    self.mode = Mode::Normal;
                }
            }
            Accept => {
                if self.navigate_target.as_ref().is_none_or(|target| {
                    !crate::chrome::navigate::navigation_target_valid(
                        &self.projection.endpoints,
                        target,
                    )
                }) {
                    self.show_notice(
                        "Endpoint unavailable",
                        "Workspace is no longer available; select a connected workspace",
                    );
                    return;
                }
                if let Some(target) = self.navigate_target.clone()
                    && crate::chrome::navigate::navigation_target_valid(
                        &self.projection.endpoints,
                        &target,
                    )
                    && let Some(terminal) = self.workspace_terminal(&target)
                {
                    self.focused = Some(terminal);
                    self.refresh_projection();
                    self.mark_active_seen();
                }
                self.navigate_target = None;
                self.navigate_agent = None;
                self.mode = Mode::Normal;
            }
            Cancel => {
                self.navigate_target = None;
                self.navigate_agent = None;
                self.focused = self.before_navigate.clone();
                self.mode = Mode::Normal;
            }
        }
        self.sync_target();
    }

    /// A mobile-switcher row press — the reference's
    /// `ClientMobileTarget` dispatch. Supported dispatch: Machine picks
    /// the endpoint's first pane; Workspace/Tab pick their panes;
    /// Agent picks its pane; NewTab opens its dialog, NewWorkspace is deferred.
    /// A successful focus
    /// exits Navigate back to Terminal.
    pub(super) fn on_mobile_target(&mut self, target: &crate::chrome::render::ClientMobileTarget) {
        use crate::chrome::render::ClientMobileTarget as T;
        if let T::Menu(index) = target {
            use crate::input::KeybindAction as A;
            if let Some(action) = [A::Settings, A::Help, A::ReloadConfig, A::Detach].get(*index) {
                // Source mobile.rs suspends Navigate for every actionable
                // menu row, retaining the report-all keyboard flags while
                // an overlay is open; the switcher resumes once no overlay
                // remains (no overlay rows resume within the same event).
                self.mobile_switcher_suspended = true;
                self.on_action(*action, self.area.0);
            }
            return;
        }
        let pane = match target {
            T::Machine(endpoint_id) => self
                .projection
                .endpoints
                .iter()
                .find(|e| &e.endpoint_id == endpoint_id)
                .and_then(|e| e.snapshot.as_deref())
                .and_then(|s| s.panes.first())
                .map(|p| p.pane_id.clone()),
            T::Workspace {
                endpoint_id,
                workspace_id,
            } => {
                let nav = WorkspaceNavigationTarget {
                    endpoint_id: endpoint_id.clone(),
                    workspace_id: workspace_id.clone(),
                    boot_id: String::new(),
                    generation: None,
                };
                self.workspace_terminal(&nav)
            }
            T::Tab {
                endpoint_id,
                tab_id,
            } => self
                .projection
                .endpoints
                .iter()
                .find(|e| &e.endpoint_id == endpoint_id)
                .and_then(|e| e.snapshot.as_deref())
                .and_then(|s| s.panes.iter().find(|p| &p.tab_id == tab_id))
                .map(|p| p.pane_id.clone()),
            T::Agent { pane_id, .. } => Some(pane_id.clone()),
            T::NewTab => {
                self.mode = Mode::Normal;
                self.navigate_target = None;
                self.navigate_agent = None;
                self.new_tab();
                None
            }
            // Reference mobile.rs suspends the switcher while the action
            // (dialog or direct create) runs, then resumes on close.
            T::NewWorkspace => {
                self.mobile_switcher_suspended = true;
                self.new_workspace();
                None
            }
            T::Menu(_) => None,
        };
        if let Some(pane_id) = pane {
            self.focused = Some(pane_id);
            self.refresh_projection();
            self.mark_active_seen();
            self.navigate_target = None;
            self.navigate_agent = None;
            self.mode = Mode::Normal;
            self.sync_target();
        }
    }

    /// Derived from herdr src/client/shell/state.rs (Apache-2.0).
    pub(super) fn resume_mobile_switcher_if_ready(&mut self) {
        let overlay = self.help
            || self.dialog.is_some()
            || self.menu.is_some()
            || self.navigator.is_some()
            || self.settings.is_some();
        if self.mobile_switcher_suspended && !overlay {
            self.mobile_switcher_suspended = false;
            if self.projection.snapshot.focused_workspace_id.is_some() {
                self.mode = Mode::Normal;
                self.navigate_target = None;
                self.navigate_agent = None;
            } else {
                self.mode = Mode::Navigate;
            }
        }
    }
}
