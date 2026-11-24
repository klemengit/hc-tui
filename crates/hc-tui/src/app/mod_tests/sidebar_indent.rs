//! `[ui.sidebar]` left offsets: machine_padding, machine_indent,
//! space_padding and agent_indent. The defaults are the herdr layout.

use super::endpoint_nav::{pending_attach, spaced_agent, two_machine_app};
use super::*;
use crate::config::{SidebarIndents, SpaceAgentsConfig};

fn sidebar(app: &mut App) -> Vec<String> {
    let backend = draw_to(app, 100, 30);
    screen_lines(&backend)
        .into_iter()
        .map(|line| {
            line.chars()
                .take(30)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

fn multi(indents: Option<SidebarIndents>) -> App {
    let mut app = two_machine_app();
    pending_attach(&mut app);
    app.shell.agent_panel = false;
    app.shell.spaces.agents = SpaceAgentsConfig::All;
    if let Some(indents) = indents {
        app.shell.indents = indents;
    }
    app.focused = Some("t1".into());
    app.refresh_projection();
    app
}

fn single(indents: Option<SidebarIndents>) -> App {
    let mut app = app_with_terms(vec![spaced_agent("t1", "one"), spaced_agent("t2", "two")]);
    app.shell.agent_panel = false;
    app.shell.spaces.agents = SpaceAgentsConfig::All;
    if let Some(indents) = indents {
        app.shell.indents = indents;
    }
    app.focused = Some("t1".into());
    app.refresh_projection();
    app
}

#[tokio::test]
async fn defaults_render_the_herdr_layout() {
    let multi_lines = sidebar(&mut multi(None));
    let single_lines = sidebar(&mut single(None));
    assert_eq!(multi_lines[..12], EXPECTED_MULTI);
    assert_eq!(single_lines[..6], EXPECTED_SINGLE);
}

const EXPECTED_MULTI: [&str; 12] = [
    " machines        v0.1.19 │  ta",
    "                         │",
    " ▾ Local                 │",
    "   ● one                 │",
    "     ● Local · one · tab │",
    "   ● two                 │",
    "     ● Local · two · tab │",
    " ▾ host-a               ●│",
    "   ● three               │",
    "     ● host… · thr… · tab│",
    "   ● four                │",
    "     ● host… · four · tab│",
];

const EXPECTED_SINGLE: [&str; 6] = [
    " spaces          v0.1.19 │  ta",
    "                         │",
    " ● one                   │",
    "   ● one · tab           │",
    " ● two                   │",
    "   ● two · tab           │",
];

const COMPACT: SidebarIndents = SidebarIndents {
    machine_padding: 0,
    machine_indent: 0,
    space_padding: 0,
    agent_indent: 0,
};

#[tokio::test]
async fn zero_offsets_pull_every_level_to_the_left_edge() {
    let lines = sidebar(&mut multi(Some(COMPACT)));
    assert_eq!(lines[2], "▾ Local                  │");
    assert_eq!(lines[3], "● one                    │");
    assert_eq!(lines[4], " ● Local · one · tab     │");
    assert_eq!(lines[7], "▾ host-a                ●│");
    let lines = sidebar(&mut single(Some(COMPACT)));
    assert_eq!(lines[2], "● one                    │");
    assert_eq!(lines[3], " ● one · tab             │");
}

#[tokio::test]
async fn the_machine_toggle_and_row_clicks_follow_the_offsets() {
    let mut app = multi(Some(COMPACT));
    draw_to(&mut app, 100, 30);
    let machine = app.hit_map.machines[0].clone();
    assert_eq!(machine.collapse_toggle.x, 0);
    assert_eq!(machine.collapse_toggle.width, 1);
    // Agent rows keep their full-width hit rect.
    let (rect, _, pane) = app.hit_map.space_agents[0].clone();
    assert_eq!(rect.x, 0);
    assert_eq!(pane, "t1");
    // The default keeps the toggle on the marker column.
    let mut app = multi(None);
    draw_to(&mut app, 100, 30);
    assert_eq!(app.hit_map.machines[0].collapse_toggle.x, 1);
}

#[tokio::test]
async fn navigate_highlight_and_big_offsets_stay_inside_the_sidebar() {
    for indents in [
        COMPACT,
        SidebarIndents {
            machine_padding: 8,
            machine_indent: 8,
            space_padding: 8,
            agent_indent: 8,
        },
    ] {
        let mut app = multi(Some(indents));
        app.live.keybinds.navigate.tree = true;
        app.enter_workspace_picker();
        let lines = sidebar(&mut app);
        assert!(lines.iter().all(|l| l.chars().count() <= 30));
        draw_to(&mut app, 24, 10);
    }
}
