//! `[ui] seen_after_ms`: a completion is seen only after the pane stayed
//! focused and visible for the dwell; 0 keeps the immediate behaviour.

use super::m4s2::{app_alerts, publish_word};
use super::*;
use std::time::Duration;

const DWELL_MS: u64 = 500;

/// `t1` finished (`done`, turn 2) and is focused; the dwell is `ms`.
fn app_on_done_pane(ms: u64) -> App {
    let mut app = app_alerts();
    publish_word(&mut app, "t1", "working", 1);
    publish_word(&mut app, "t1", "done", 2);
    app.shell.seen_after_ms = ms;
    app.focused = Some("t1".into());
    app.refresh_projection();
    app
}

fn written(app: &App) -> bool {
    app.seen_inflight.iter().any(|(id, _)| id == "t1")
}

#[tokio::test]
async fn before_the_dwell_the_pane_is_not_seen_and_nothing_is_written() {
    let mut app = app_on_done_pane(DWELL_MS);
    let t0 = Instant::now();
    assert!(!app.tick_dwell(t0));
    assert!(!app.tick_dwell(t0 + Duration::from_millis(DWELL_MS - 1)));
    assert!(!app.seen_dwell_met("t1"));
    app.share_seen();
    assert!(!written(&app));
    // The loop wakes when the dwell ends.
    assert_eq!(
        app.dwell_deadline(),
        Some(t0 + Duration::from_millis(DWELL_MS))
    );
}

#[tokio::test]
async fn at_the_dwell_the_pane_is_seen_and_herdr_seen_is_written() {
    let mut app = app_on_done_pane(DWELL_MS);
    let t0 = Instant::now();
    app.tick_dwell(t0);
    assert!(app.tick_dwell(t0 + Duration::from_millis(DWELL_MS)));
    assert!(app.seen_dwell_met("t1"));
    assert!(!app.seen_dwell_met("t2"));
    app.share_seen();
    assert!(written(&app));
    assert_eq!(app.dwell_deadline(), None);
    // Reached once: no second wake.
    assert!(!app.tick_dwell(t0 + Duration::from_millis(DWELL_MS + 10)));
}

#[tokio::test]
async fn leaving_the_pane_resets_the_dwell() {
    let mut app = app_on_done_pane(DWELL_MS);
    let t0 = Instant::now();
    app.tick_dwell(t0);
    // Away for a moment, before the dwell is over.
    app.focused = Some("t2".into());
    app.refresh_projection();
    app.tick_dwell(t0 + Duration::from_millis(400));
    app.focused = Some("t1".into());
    app.refresh_projection();
    assert!(!app.tick_dwell(t0 + Duration::from_millis(600)));
    assert!(!app.seen_dwell_met("t1"));
    // The full dwell counts from the return.
    assert!(app.tick_dwell(t0 + Duration::from_millis(600 + DWELL_MS)));
    assert!(app.seen_dwell_met("t1"));
}

#[tokio::test]
async fn an_unfocused_outer_terminal_stops_the_clock() {
    let mut app = app_on_done_pane(DWELL_MS);
    let t0 = Instant::now();
    app.tick_dwell(t0);
    app.outer_focused = Some(false);
    assert!(!app.tick_dwell(t0 + Duration::from_millis(DWELL_MS * 2)));
    app.outer_focused = Some(true);
    assert!(!app.tick_dwell(t0 + Duration::from_millis(DWELL_MS * 2 + 1)));
    assert!(!app.seen_dwell_met("t1"));
}

#[tokio::test]
async fn zero_keeps_the_immediate_behaviour() {
    let mut app = app_on_done_pane(0);
    assert!(!app.tick_dwell(Instant::now()));
    assert!(app.seen_dwell_met("t1"));
    assert_eq!(app.dwell_deadline(), None);
    app.share_seen();
    assert!(written(&app));
}
