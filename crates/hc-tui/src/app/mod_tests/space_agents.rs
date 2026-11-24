//! `[ui.sidebar.spaces] agents`: agents listed under their space.

use super::endpoint_nav::{app_multi, pending_attach, remote};
use super::*;
use crate::config::SpaceAgentsConfig;
use crate::sidebar::fixture::{agent, machine};

fn named(address: &str, word: &str, space: &str, name: &str) -> hc_proto::terminal::Terminal {
    let mut term = agent(address, word);
    term.name = Some(name.into());
    term.labels.insert("herdr.space".into(), space.into());
    term.labels.insert("herdr.tab".into(), "tab".into());
    term
}

/// Space `one` has a working `alpha` and an idle `beta`; space `two` has a
/// blocked `gamma`. The agents panel is off, so a name on screen is a
/// child row of the spaces list.
fn app_with_agents(mode: SpaceAgentsConfig) -> App {
    let mut app = app_with_terms(vec![
        named("t1", "idle", "one", "beta"),
        named("t2", "working", "one", "alpha"),
        named("t3", "blocked", "two", "gamma"),
    ]);
    app.shell.agent_panel = false;
    app.shell.spaces.agents = mode;
    app.focused = Some("t1".into());
    app.refresh_projection();
    pending_attach(&mut app);
    app
}

/// The sidebar's lines (its first 40 columns).
fn sidebar(app: &mut App) -> Vec<String> {
    let backend = draw_to(app, 100, 30);
    screen_lines(&backend)
        .into_iter()
        .map(|line| line.chars().take(40).collect())
        .collect()
}

fn line_of<'a>(lines: &'a [String], needle: &str) -> Option<(usize, &'a String)> {
    lines
        .iter()
        .enumerate()
        .find(|(_, line)| line.contains(needle))
}

#[tokio::test]
async fn by_default_no_agent_is_listed_under_a_space() {
    let mut app = app_with_agents(SpaceAgentsConfig::None);
    let lines = sidebar(&mut app);
    for name in ["alpha", "beta", "gamma"] {
        assert!(line_of(&lines, name).is_none(), "{name}: {lines:#?}");
    }
    assert!(app.hit_map.space_agents.is_empty());
}

#[tokio::test]
async fn active_lists_working_and_blocked_agents_but_not_idle_ones() {
    let mut app = app_with_agents(SpaceAgentsConfig::Active);
    let lines = sidebar(&mut app);
    assert!(line_of(&lines, "beta").is_none(), "{lines:#?}");
    let (space_one, _) = line_of(&lines, "one").expect("space one");
    let (alpha, alpha_line) = line_of(&lines, "alpha").expect("alpha");
    let (space_two, _) = line_of(&lines, "two").expect("space two");
    let (gamma, _) = line_of(&lines, "gamma").expect("gamma");
    // Each agent sits under its own space.
    assert!(space_one < alpha && alpha < space_two && space_two < gamma);
    // Indented past the space rows (their text starts in column 3).
    let first_text = alpha_line.find(|c: char| c.is_alphanumeric()).unwrap();
    assert!(first_text > 3, "{alpha_line:?}");
    assert_eq!(
        app.hit_map
            .space_agents
            .iter()
            .map(|(_, _, pane)| pane.as_str())
            .collect::<Vec<_>>(),
        ["t2", "t3"]
    );
}

#[tokio::test]
async fn all_lists_idle_agents_after_the_active_ones() {
    let mut app = app_with_agents(SpaceAgentsConfig::All);
    let lines = sidebar(&mut app);
    let (alpha, _) = line_of(&lines, "alpha").expect("alpha");
    let (beta, _) = line_of(&lines, "beta").expect("beta");
    assert!(alpha < beta, "working before idle: {lines:#?}");
    assert_eq!(app.hit_map.space_agents.len(), 3);
}

#[tokio::test]
async fn child_rows_follow_the_agent_row_layout() {
    let mut app = app_with_agents(SpaceAgentsConfig::Active);
    app.shell.agents.rows = vec![vec![crate::config::AgentSidebarToken::Agent]];
    let lines = sidebar(&mut app);
    // One configured line per agent: `alpha` is directly above space two.
    let (alpha, _) = line_of(&lines, "alpha").unwrap();
    let (space_two, _) = line_of(&lines, "two").unwrap();
    assert_eq!(space_two, alpha + 1, "{lines:#?}");
}

#[tokio::test]
async fn clicking_a_child_row_focuses_its_agent() {
    let mut app = app_with_agents(SpaceAgentsConfig::Active);
    draw_to(&mut app, 100, 30);
    let (rect, _, _) = app
        .hit_map
        .space_agents
        .iter()
        .find(|(_, _, pane)| pane == "t3")
        .cloned()
        .unwrap();
    app.on_input(
        input::UiEvent::Mouse(input::MouseInput {
            kind: crate::app::input::MouseEventKind::Down(crate::app::input::MouseButton::Left),
            col: rect.x + 4,
            row: rect.y,
            mods: hc_vt::Mods::NONE,
            sgr_position: None,
        }),
        100,
    );
    assert_eq!(app.focused.as_deref(), Some("t3"));
}

#[tokio::test]
async fn machines_list_their_own_agents_and_collapsed_ones_list_none() {
    let mut app = app_multi(
        vec![
            named("t1", "working", "one", "alpha"),
            named("host-a/t2", "blocked", "two", "gamma"),
            named("host-a/t3", "idle", "two", "delta"),
        ],
        vec![machine("host-a", "connected")],
    );
    app.shell.agent_panel = false;
    app.shell.spaces.agents = SpaceAgentsConfig::Active;
    pending_attach(&mut app);
    app.focused = Some("t1".into());
    app.refresh_projection();

    let lines = sidebar(&mut app);
    assert!(line_of(&lines, "alpha").is_some(), "{lines:#?}");
    assert!(line_of(&lines, "gamma").is_some(), "{lines:#?}");
    assert!(line_of(&lines, "delta").is_none(), "{lines:#?}");
    assert!(
        app.hit_map
            .space_agents
            .iter()
            .any(|(_, endpoint, pane)| *endpoint == remote("host-a") && pane == "host-a/t2")
    );

    app.shell_state.collapsed_endpoints.insert(remote("host-a"));
    let lines = sidebar(&mut app);
    assert!(line_of(&lines, "alpha").is_some(), "{lines:#?}");
    assert!(line_of(&lines, "gamma").is_none(), "{lines:#?}");
}
