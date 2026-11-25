//! `[keys] recent_agent`: the most recently focused other agent, with quick
//! repeats stepping further back (`[ui] recent_agent_window_ms`).

use super::endpoint_nav::{app_multi, pending_attach};
use super::*;
use crate::input::KeybindAction as A;
use crate::sidebar::fixture::{agent, machine};
use std::sync::Mutex;
use std::time::Duration;

fn spaced(address: &str, word: &str, space: &str) -> hc_proto::terminal::Terminal {
    let mut term = agent(address, word);
    term.labels.insert("herdr.space".into(), space.into());
    term.labels.insert("herdr.tab".into(), "tab".into());
    term
}

/// Three agents on one machine and a clock the test moves by hand.
fn app_three() -> (App, Arc<Mutex<Instant>>) {
    let mut app = app_with_terms(vec![
        spaced("t1", "idle", "one"),
        spaced("t2", "working", "one"),
        spaced("t3", "blocked", "two"),
    ]);
    pending_attach(&mut app);
    let now = Arc::new(Mutex::new(Instant::now()));
    let clock = now.clone();
    app.recent_agents.clock = Arc::new(move || *clock.lock().unwrap());
    (app, now)
}

fn visit(app: &mut App, id: &str) {
    app.focus_pane(id.into());
}

fn later(now: &Arc<Mutex<Instant>>, ms: u64) {
    *now.lock().unwrap() += Duration::from_millis(ms);
}

fn press(app: &mut App) -> Option<String> {
    app.on_action(A::RecentAgent, 100);
    app.focused.clone()
}

fn order(app: &App) -> Vec<&str> {
    app.recent_agents
        .order()
        .iter()
        .map(String::as_str)
        .collect()
}

#[test]
fn recent_agent_is_unbound_by_default() {
    let keybinds = crate::config::Keybinds::default();
    assert!(keybinds.recent_agent.labels().is_empty());
    assert_eq!(
        crate::config::UiConfig::default().recent_agent_window_ms,
        1000
    );
}

#[tokio::test]
async fn one_press_returns_to_the_previous_agent_and_the_next_toggles_back() {
    let (mut app, now) = app_three();
    visit(&mut app, "t1");
    visit(&mut app, "t2");
    visit(&mut app, "t3");
    assert_eq!(order(&app), ["t3", "t2", "t1"]);
    assert_eq!(press(&mut app).as_deref(), Some("t2"));
    later(&now, 1500);
    assert_eq!(press(&mut app).as_deref(), Some("t3"));
    later(&now, 1500);
    assert_eq!(press(&mut app).as_deref(), Some("t2"));
}

#[tokio::test]
async fn quick_presses_step_further_back_and_wrap() {
    let (mut app, now) = app_three();
    visit(&mut app, "t1");
    visit(&mut app, "t2");
    visit(&mut app, "t3");
    assert_eq!(press(&mut app).as_deref(), Some("t2"));
    later(&now, 300);
    assert_eq!(press(&mut app).as_deref(), Some("t1"));
    later(&now, 300);
    // A full lap returns to where the run started.
    assert_eq!(press(&mut app).as_deref(), Some("t3"));
    later(&now, 300);
    assert_eq!(press(&mut app).as_deref(), Some("t2"));
}

#[tokio::test]
async fn the_list_is_not_reordered_while_stepping_and_commits_after_the_window() {
    let (mut app, now) = app_three();
    visit(&mut app, "t1");
    visit(&mut app, "t2");
    visit(&mut app, "t3");
    press(&mut app);
    later(&now, 300);
    assert_eq!(press(&mut app).as_deref(), Some("t1"));
    assert_eq!(order(&app), ["t3", "t2", "t1"]);
    // The window passes: the next press first commits t1 as the newest.
    later(&now, 1001);
    assert_eq!(press(&mut app).as_deref(), Some("t3"));
    assert_eq!(order(&app), ["t1", "t3", "t2"]);
}

#[tokio::test]
async fn focusing_another_agent_ends_the_run() {
    let (mut app, now) = app_three();
    visit(&mut app, "t1");
    visit(&mut app, "t2");
    visit(&mut app, "t3");
    press(&mut app);
    later(&now, 300);
    assert_eq!(press(&mut app).as_deref(), Some("t1"));
    visit(&mut app, "t2");
    assert_eq!(order(&app), ["t2", "t1", "t3"]);
    later(&now, 100);
    assert_eq!(press(&mut app).as_deref(), Some("t1"));
}

#[tokio::test]
async fn another_key_ends_the_run() {
    let (mut app, now) = app_three();
    visit(&mut app, "t1");
    visit(&mut app, "t2");
    visit(&mut app, "t3");
    press(&mut app);
    later(&now, 300);
    assert_eq!(press(&mut app).as_deref(), Some("t1"));
    app.on_key(
        term_key(
            hc_vt::Key::Char('x'),
            hc_vt::Mods::NONE,
            hc_vt::Action::Press,
        ),
        100,
    );
    assert_eq!(order(&app), ["t1", "t3", "t2"]);
    later(&now, 100);
    assert_eq!(press(&mut app).as_deref(), Some("t3"));
}

#[tokio::test]
async fn a_vanished_agent_is_skipped() {
    let (mut app, now) = app_three();
    visit(&mut app, "t1");
    visit(&mut app, "t2");
    visit(&mut app, "t3");
    let mut view = app.view.clone();
    view.inventory_mut().replace(
        2,
        vec![spaced("t1", "idle", "one"), spaced("t3", "blocked", "two")],
    );
    app.view = view;
    app.refresh_projection();
    assert_eq!(press(&mut app).as_deref(), Some("t1"));
    later(&now, 1500);
    assert_eq!(press(&mut app).as_deref(), Some("t3"));
    assert!(!order(&app).contains(&"t2"));
}

#[tokio::test]
async fn with_no_other_agent_a_press_does_nothing() {
    let mut app = app_with_terms(vec![spaced("t1", "working", "one")]);
    pending_attach(&mut app);
    visit(&mut app, "t1");
    assert_eq!(press(&mut app).as_deref(), Some("t1"));
}

#[tokio::test]
async fn recent_agent_crosses_machines() {
    let mut app = app_multi(
        vec![
            spaced("t1", "working", "one"),
            spaced("host-a/t3", "working", "three"),
        ],
        vec![machine("host-a", "connected")],
    );
    pending_attach(&mut app);
    visit(&mut app, "host-a/t3");
    visit(&mut app, "t1");
    assert_eq!(press(&mut app).as_deref(), Some("host-a/t3"));
    assert_eq!(press(&mut app).as_deref(), Some("t1"));
}
