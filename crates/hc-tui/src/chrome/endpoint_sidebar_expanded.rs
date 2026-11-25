// Derived from herdr src/client/shell/endpoint_sidebar.rs (Apache-2.0).

use std::collections::HashSet;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
};

use super::endpoint_sidebar_rows::{Row, build_rows, reveal_row, row_gaps, row_heights};
use super::endpoints::collapsed_groups_for_endpoint;
use super::render::{
    MachineHit, ShellHitMap, ShellRenderState, WORKSPACE_HEADER_ROWS, WorkspaceHit, display_width,
    put_right_text, put_text, put_version_badge,
};
use super::sidebar_render::{
    displayed_workspace_status, render_parent_group_toggle, render_workspace_rows, workspace_rows,
};
use super::snapshot::{ClientEndpointStatus, ClientShellSnapshot};
use super::spans::{expanded_sidebar_sections, sidebar_section_divider_rect};

pub(crate) fn render_expanded(
    buffer: &mut Buffer,
    area: Rect,
    active_snapshot: Option<&ClientShellSnapshot>,
    config: &super::ClientShellConfig,
    state: &mut ShellRenderState<'_>,
    hits: &mut ShellHitMap,
) {
    let palette = &config.palette;
    let looks = config.row_looks();
    super::render::render_sidebar_background(buffer, area, palette);
    hits.sidebar_divider = if area.is_empty() {
        Rect::default()
    } else {
        Rect::new(area.right().saturating_sub(1), area.y, 1, area.height)
    };
    let (workspace_area, detail_area) =
        expanded_sidebar_sections(area, state.sidebar_section_split, config.agent_panel);
    hits.sidebar_section_divider =
        sidebar_section_divider_rect(area, state.sidebar_section_split, config.agent_panel);
    put_text(
        buffer,
        workspace_area.x,
        workspace_area.y,
        workspace_area.width,
        " machines",
        Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::BOLD),
    );
    // The version badge shares the header row, right-aligned a cell
    // short of the divider column and never touching `machines`.
    let badge_row = Rect::new(
        workspace_area.x,
        workspace_area.y,
        workspace_area.width.saturating_sub(1),
        1,
    );
    hits.update_badge = put_version_badge(
        buffer,
        badge_row,
        workspace_area.x.saturating_add(10),
        state.update_badge,
        palette,
    );

    let empty_collapsed_groups = HashSet::new();

    let rows = build_rows(state, config);
    let body = Rect::new(
        workspace_area.x,
        workspace_area.y.saturating_add(WORKSPACE_HEADER_ROWS),
        workspace_area.width,
        workspace_area
            .height
            .saturating_sub(WORKSPACE_HEADER_ROWS + 1),
    );
    hits.workspace_body = body;
    let row_heights = row_heights(&rows, state, config);
    let gaps = row_gaps(&rows, config);
    let reveal_navigation = !body.is_empty() && std::mem::take(state.reveal_navigation_workspace);
    let reveal_focus = !body.is_empty() && std::mem::take(state.reveal_focused_workspace);
    if (reveal_navigation || reveal_focus)
        && let Some(selected_row) = reveal_row(&rows, state, active_snapshot, reveal_navigation)
    {
        *state.workspace_scroll = super::scroll::list_scroll_start_to_reveal(
            &row_heights,
            &gaps,
            body.height,
            *state.workspace_scroll,
            selected_row,
        );
    }
    let metrics = super::scroll::list_scroll_metrics(
        &row_heights,
        &gaps,
        body.height,
        *state.workspace_scroll,
    );
    hits.workspace_max_scroll = metrics.max_offset_from_bottom;
    hits.workspace_scroll_metrics = Some(metrics);
    *state.workspace_scroll = metrics
        .max_offset_from_bottom
        .saturating_sub(metrics.offset_from_bottom);
    let show_scrollbar = metrics.max_offset_from_bottom > 0 && body.width > 1;
    let content_width = body.width.saturating_sub(u16::from(show_scrollbar));
    let mut y = body.y;
    for (row_index, row) in rows.iter().enumerate().skip(*state.workspace_scroll) {
        match row {
            Row::Endpoint(index) => {
                if y >= body.bottom() {
                    break;
                }
                let endpoint = &state.endpoints[*index];
                let rect = Rect::new(body.x, y, content_width, 1);
                let collapsed = state.collapsed_endpoints.contains(&endpoint.endpoint_id);
                let marker = if collapsed { "▸" } else { "▾" };
                let status_badge = super::endpoint_sidebar_row::render_endpoint_row(
                    buffer,
                    rect,
                    marker,
                    endpoint,
                    collapsed && &endpoint.endpoint_id == state.active_endpoint_id,
                    state.machine_diagnostics,
                    config.indents.machine_padding,
                    palette,
                );
                hits.machines.push(MachineHit {
                    rect,
                    status_badge,
                    collapse_toggle: Rect::new(
                        rect.x.saturating_add(config.indents.machine_padding),
                        rect.y,
                        u16::from(rect.width > config.indents.machine_padding),
                        1,
                    ),
                    endpoint_id: endpoint.endpoint_id.clone(),
                });
                y = y
                    .saturating_add(1)
                    .saturating_add(gaps.get(row_index).copied().unwrap_or(0));
            }
            Row::Workspace { endpoint, entry } => {
                let endpoint = &state.endpoints[*endpoint];
                let Some(snapshot) = endpoint.snapshot.as_deref() else {
                    continue;
                };
                let Some(workspace) = snapshot.workspaces.get(entry.index) else {
                    continue;
                };
                let collapsed_groups = collapsed_groups_for_endpoint(state, &endpoint.endpoint_id)
                    .unwrap_or(&empty_collapsed_groups);
                let status = displayed_workspace_status(snapshot, workspace, collapsed_groups);
                let tokens =
                    workspace_rows(snapshot, workspace, status, entry.indented, &config.spaces);
                let height = (tokens.len().max(1).min(u16::MAX as usize) as u16).min(body.height);
                if y.saturating_add(height) > body.bottom() {
                    break;
                }
                let rect = Rect::new(body.x, y, content_width, height);
                let nested = Rect::new(
                    rect.x.saturating_add(config.indents.machine_indent),
                    rect.y,
                    rect.width.saturating_sub(config.indents.machine_indent),
                    rect.height,
                );
                let endpoint_active = &endpoint.endpoint_id == state.active_endpoint_id;
                let selected = state.navigate_agent.is_none()
                    && state.selected_workspace_id.is_some_and(|target| {
                        target.matches(&endpoint.endpoint_id, &workspace.workspace_id)
                    });
                render_workspace_rows(
                    buffer,
                    nested,
                    status,
                    config.status_indicators,
                    entry,
                    tokens,
                    endpoint_active && workspace.focused,
                    selected,
                    state.selected_workspace_id.is_some(),
                    false,
                    state.navigate_agent.is_some()
                        && state.selected_workspace_id.is_some_and(|target| {
                            target.matches(&endpoint.endpoint_id, &workspace.workspace_id)
                        }),
                    config.indents.space_padding,
                    &looks,
                    palette,
                );
                if endpoint.status != ClientEndpointStatus::Online {
                    buffer.set_style(
                        rect,
                        Style::default()
                            .fg(palette.overlay0)
                            .add_modifier(Modifier::DIM),
                    );
                }
                let group_toggle = render_parent_group_toggle(
                    buffer,
                    rect,
                    snapshot,
                    entry.index,
                    collapsed_groups,
                    palette,
                );
                hits.workspaces.push(WorkspaceHit {
                    rect,
                    endpoint_id: endpoint.endpoint_id.clone(),
                    workspace_id: workspace.workspace_id.clone(),

                    group_toggle,
                });
                y = y
                    .saturating_add(height)
                    .saturating_add(gaps.get(row_index).copied().unwrap_or(0));
            }
            Row::Agent {
                endpoint,
                workspace_indented,
                row,
            } => {
                let endpoint = &state.endpoints[*endpoint];
                let height = row_heights[row_index].min(body.height);
                if y.saturating_add(height) > body.bottom() {
                    break;
                }
                let rect = Rect::new(body.x, y, content_width, height);
                super::agent_rows::render_agent_row_indented(
                    buffer,
                    rect,
                    &row.row,
                    config,
                    // Past the machine row's own nesting.
                    config.indents.machine_indent
                        + super::space_agents::agent_indent(
                            *workspace_indented,
                            config.indents.agent_indent,
                        ),
                    row.cursor,
                    Some(&looks),
                );
                if endpoint.status != ClientEndpointStatus::Online {
                    buffer.set_style(
                        rect,
                        Style::default()
                            .fg(palette.overlay0)
                            .add_modifier(Modifier::DIM),
                    );
                }
                hits.space_agents.push((
                    rect,
                    endpoint.endpoint_id.clone(),
                    row.row.pane_id.clone(),
                ));
                y = y
                    .saturating_add(height)
                    .saturating_add(gaps.get(row_index).copied().unwrap_or(0));
            }
        }
    }
    if show_scrollbar {
        let track = Rect::new(body.right().saturating_sub(1), body.y, 1, body.height);
        hits.workspace_scrollbar = track;
        super::scroll::render_list_scrollbar(buffer, track, metrics, palette);
    }

    let footer_y = workspace_area.bottom().saturating_sub(1);
    if config.mouse_capture {
        let label = format!(
            " new · {}",
            super::endpoint_sidebar_row::active_endpoint_label(state)
        );
        hits.new_workspace = Rect::new(
            workspace_area.x,
            footer_y,
            display_width(&label).min(workspace_area.width),
            u16::from(workspace_area.height > 0),
        );
        put_text(
            buffer,
            workspace_area.x,
            footer_y,
            workspace_area.width,
            &label,
            Style::default().fg(palette.overlay0),
        );
        let dot = state.update_badge.accent_style(palette);
        let width = if dot.is_some() { 8 } else { 6 };
        let width = width.min(workspace_area.width);
        hits.global_launcher = Rect::new(
            workspace_area.right().saturating_sub(width),
            footer_y,
            width,
            1,
        );
        put_right_text(
            buffer,
            workspace_area,
            footer_y,
            "menu",
            Style::default().fg(palette.overlay0),
        );
        if let Some(style) = dot.filter(|_| workspace_area.width >= 6) {
            put_text(
                buffer,
                workspace_area.right().saturating_sub(6),
                footer_y,
                2,
                "● ",
                style,
            );
        }
    }
    super::endpoint_agents::render_expanded(
        buffer,
        detail_area,
        active_snapshot.and_then(|snapshot| snapshot.agent_view_label.as_deref()),
        state.endpoints,
        state.active_endpoint_id,
        config,
        state.agent_scroll,
        state.reveal_focused_agent,
        hits,
    );
    hits.sidebar_toggle = Rect::new(
        area.right().saturating_sub(2),
        area.bottom().saturating_sub(1),
        u16::from(area.width > 1),
        u16::from(area.height > 0),
    );
    put_text(
        buffer,
        hits.sidebar_toggle.x,
        hits.sidebar_toggle.y,
        hits.sidebar_toggle.width,
        "«",
        Style::default().fg(palette.overlay0),
    );
}
