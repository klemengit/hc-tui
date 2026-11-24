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

// ---- tree navigation (`keys.navigate_tree`) ----

fn press_key(app: &mut App, c: char) {
    app.on_input(
        input::UiEvent::Key(term_key(
            hc_vt::Key::Char(c),
            hc_vt::Mods::NONE,
            hc_vt::Action::Press,
        )),
        100,
    );
}

fn press_named(app: &mut App, key: hc_vt::Key) {
    app.on_input(
        input::UiEvent::Key(term_key(key, hc_vt::Mods::NONE, hc_vt::Action::Press)),
        100,
    );
}

/// Navigate mode on the app above with tree navigation on and the
/// cursor on space `one` (whose agents are t1 idle `beta`, t2 `alpha`).
fn tree_app(mode: SpaceAgentsConfig) -> App {
    let mut app = app_with_agents(mode);
    app.live.keybinds.navigate.tree = true;
    app.enter_workspace_picker();
    app.sync_target();
    app
}

#[tokio::test]
async fn tree_mode_lists_every_agent_of_the_open_space_even_with_agents_none() {
    let mut app = tree_app(SpaceAgentsConfig::None);
    let lines = sidebar(&mut app);
    assert!(line_of(&lines, "beta").is_none(), "{lines:#?}");
    press_key(&mut app, 'l');
    assert_eq!(app.navigate_agent.as_deref(), Some("t2"));
    let lines = sidebar(&mut app);
    let (alpha, _) = line_of(&lines, "alpha").expect("alpha listed");
    let (beta, _) = line_of(&lines, "beta").expect("idle beta listed");
    assert!(alpha < beta, "{lines:#?}");
    assert!(line_of(&lines, "gamma").is_none(), "{lines:#?}");
    // h closes the space again and the idle agent hides.
    press_key(&mut app, 'h');
    assert!(app.navigate_agent.is_none());
    let lines = sidebar(&mut app);
    assert!(line_of(&lines, "beta").is_none(), "{lines:#?}");
}

#[tokio::test]
async fn j_and_k_skip_child_rows_on_spaces_and_walk_agents_inside() {
    let mut app = tree_app(SpaceAgentsConfig::All);
    let one = app.navigate_target.clone().unwrap();
    press_key(&mut app, 'j');
    let two = app.navigate_target.clone().unwrap();
    assert_ne!(one.workspace_id, two.workspace_id);
    assert!(app.navigate_agent.is_none());
    press_key(&mut app, 'k');
    assert_eq!(
        app.navigate_target.as_ref().unwrap().workspace_id,
        one.workspace_id
    );
    press_key(&mut app, 'l');
    assert_eq!(app.navigate_agent.as_deref(), Some("t2"));
    press_key(&mut app, 'j');
    assert_eq!(app.navigate_agent.as_deref(), Some("t1"));
    press_key(&mut app, 'j');
    assert_eq!(app.navigate_agent.as_deref(), Some("t2"), "wraps");
    press_key(&mut app, 'k');
    assert_eq!(app.navigate_agent.as_deref(), Some("t1"));
    // The target space did not change while inside.
    assert_eq!(
        app.navigate_target.as_ref().unwrap().workspace_id,
        one.workspace_id
    );
}

#[tokio::test]
async fn enter_on_an_agent_focuses_it_and_leaves_navigate() {
    let mut app = tree_app(SpaceAgentsConfig::None);
    press_key(&mut app, 'l');
    press_key(&mut app, 'j');
    press_named(&mut app, hc_vt::Key::Enter);
    assert_eq!(app.focused.as_deref(), Some("t1"));
    assert_eq!(app.mode, Mode::Normal);
    assert!(app.navigate_agent.is_none());
    assert!(app.navigate_target.is_none());
}

#[tokio::test]
async fn enter_on_a_space_still_jumps_to_the_space() {
    let mut app = tree_app(SpaceAgentsConfig::All);
    press_key(&mut app, 'j');
    press_named(&mut app, hc_vt::Key::Enter);
    assert_eq!(app.mode, Mode::Normal);
    assert_eq!(app.focused.as_deref(), Some("t3"));
}

#[tokio::test]
async fn escape_cancels_from_inside_a_space() {
    let mut app = tree_app(SpaceAgentsConfig::None);
    let before = app.focused.clone();
    press_key(&mut app, 'l');
    press_named(&mut app, hc_vt::Key::Escape);
    assert_eq!(app.mode, Mode::Normal);
    assert!(app.navigate_agent.is_none());
    assert_eq!(app.focused, before);
}

#[tokio::test]
async fn without_the_option_h_j_k_l_do_nothing_in_navigate_mode() {
    let mut app = app_with_agents(SpaceAgentsConfig::None);
    app.enter_workspace_picker();
    let target = app.navigate_target.clone();
    for c in ['l', 'j', 'k', 'h'] {
        press_key(&mut app, c);
    }
    assert!(app.navigate_agent.is_none());
    assert_eq!(app.navigate_target, target);
    assert_eq!(app.mode, Mode::Navigate);
}

#[tokio::test]
async fn tree_mode_is_off_on_the_collapsed_sidebar() {
    let mut app = tree_app(SpaceAgentsConfig::None);
    app.shell_state.sidebar_collapsed = true;
    press_key(&mut app, 'l');
    assert!(app.navigate_agent.is_none());
}

// ---- machine jumps (`navigate_next_machine` / `navigate_prev_machine`) ----

fn upper(app: &mut App, c: char) {
    app.on_input(
        input::UiEvent::Key(term_key(
            hc_vt::Key::Char(c.to_ascii_lowercase()),
            hc_vt::Mods::SHIFT,
            hc_vt::Action::Press,
        )),
        100,
    );
}

fn tree_two_machines() -> App {
    let mut app = super::endpoint_nav::two_machine_app();
    pending_attach(&mut app);
    app.focused = Some("t1".into());
    app.refresh_projection();
    app.live.keybinds.navigate.tree = true;
    app.enter_workspace_picker();
    app
}

#[tokio::test]
async fn shift_j_and_k_jump_between_machines_and_leave_an_open_space() {
    let mut app = tree_two_machines();
    let local = app.navigate_target.clone().unwrap();
    assert_eq!(local.endpoint_id, ClientEndpointId::Local);
    upper(&mut app, 'j');
    assert_eq!(
        app.navigate_target.as_ref().unwrap().endpoint_id,
        remote("host-a")
    );
    let host_first = app.navigate_target.clone().unwrap();
    // Inside a space, J also leaves it and jumps.
    press_key(&mut app, 'l');
    assert!(app.navigate_agent.is_some());
    upper(&mut app, 'j');
    assert!(app.navigate_agent.is_none());
    assert_eq!(
        app.navigate_target.as_ref().unwrap().endpoint_id,
        ClientEndpointId::Local
    );
    // K from the second machine's second space: own first, then previous.
    upper(&mut app, 'j');
    assert_eq!(app.navigate_target, Some(host_first.clone()));
    press_key(&mut app, 'j');
    upper(&mut app, 'k');
    assert_eq!(app.navigate_target, Some(host_first));
    upper(&mut app, 'k');
    assert_eq!(
        app.navigate_target.as_ref().unwrap().endpoint_id,
        ClientEndpointId::Local
    );
}

#[tokio::test]
async fn machine_jumps_do_nothing_without_tree_navigation() {
    let mut app = tree_two_machines();
    app.live.keybinds.navigate.tree = false;
    let before = app.navigate_target.clone();
    upper(&mut app, 'j');
    upper(&mut app, 'k');
    assert_eq!(app.navigate_target, before);
}
