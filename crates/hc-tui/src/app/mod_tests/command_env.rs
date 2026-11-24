//! `[[keys.command]]` commands learn which terminal was focused:
//! `HC_FOCUSED_TERMINAL`, `HC_FOCUSED_MACHINE`, `HC_FOCUSED_SPACE` and
//! `HC_FOCUSED_TAB`.

use super::floats::{core_id, run_reply, settle};
use super::m4s2::{operations, pending};
use super::*;
use crate::app::creation::{export_prefix, focused_env};
use crate::config::{CustomCommandAction, CustomCommandKeybind};
use serde_json::json;
use std::collections::BTreeMap;

fn command(action: CustomCommandAction, line: &str) -> CustomCommandKeybind {
    CustomCommandKeybind {
        layout: Default::default(),
        screen: false,
        bindings: Default::default(),
        label: "alt+h".into(),
        command: line.into(),
        action,
        description: None,
        width: None,
        height: None,
        persistent: false,
    }
}

fn labels(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect()
}

#[test]
fn a_local_terminal_gives_its_id_and_an_empty_machine() {
    let env = focused_env(
        Some("t7"),
        Some(&labels(&[("herdr.space", "work"), ("herdr.tab", "main")])),
    );
    assert_eq!(env["HC_FOCUSED_TERMINAL"], "t7");
    assert_eq!(env["HC_FOCUSED_MACHINE"], "");
    assert_eq!(env["HC_FOCUSED_SPACE"], "work");
    assert_eq!(env["HC_FOCUSED_TAB"], "main");
    // Never the variables of the command's own terminal.
    assert!(!env.contains_key("HC_TERMINAL_ID"));
    assert!(!env.contains_key("HC_TERMINAL_SOCKET"));
}

#[test]
fn a_remote_terminal_is_stripped_to_the_id_valid_on_its_machine() {
    let env = focused_env(Some("host-a/t3"), None);
    assert_eq!(env["HC_FOCUSED_TERMINAL"], "t3");
    assert_eq!(env["HC_FOCUSED_MACHINE"], "host-a");
    assert!(!env.contains_key("HC_FOCUSED_SPACE"));
    assert!(!env.contains_key("HC_FOCUSED_TAB"));
}

#[test]
fn no_focused_terminal_sets_nothing_and_exports_nothing() {
    assert!(focused_env(None, None).is_empty());
    assert_eq!(export_prefix(&BTreeMap::new()), "");
}

#[test]
fn the_export_prefix_quotes_values() {
    let env = labels(&[("HC_FOCUSED_SPACE", "it's mine")]);
    assert_eq!(
        export_prefix(&env),
        "export HC_FOCUSED_SPACE='it'\\''s mine'; "
    );
}

fn remote_app() -> App {
    let mut app = super::endpoint_nav::two_machine_app();
    app.focused = Some("host-a/t3".into());
    app.refresh_projection();
    app
}

async fn popup_params(app: &mut App) -> serde_json::Value {
    pending(app);
    app.reconcile_published_view();
    let mut rx = operations(app);
    core_id(app, "core-fake-1");
    let server = tokio::spawn(async move {
        let mut core = rx.recv().await.unwrap();
        core.read_hello().await;
        core.answer_hello(true, &["terminal.run"], &[]).await;
        let run = core.read_request().await;
        assert_eq!(run["method"], "terminal.run");
        let params = run["params"].clone();
        core.reply(&run["id"], run_reply("t9", 9)).await;
        params
    });
    app.run_custom_command(&command(CustomCommandAction::Popup, "claude-hint"));
    settle(app).await;
    server.await.unwrap()
}

#[tokio::test]
async fn a_popup_on_a_local_terminal_gets_the_local_variables() {
    let mut app = app_with_spaces(&[("t1", "work")]);
    let params = popup_params(&mut app).await;
    let env = &params["env"];
    assert_eq!(env["HERDR_ENV"], "1");
    assert_eq!(env["HC_FOCUSED_TERMINAL"], "t1");
    assert_eq!(env["HC_FOCUSED_MACHINE"], "");
    assert_eq!(env["HC_FOCUSED_SPACE"], "work");
    assert!(params.get("on").is_none());
    assert!(env.get("HC_TERMINAL_ID").is_none());
}

#[tokio::test]
async fn a_popup_on_a_remote_terminal_runs_there_with_its_local_id() {
    let mut app = remote_app();
    let params = popup_params(&mut app).await;
    assert_eq!(params["on"], "host-a");
    assert_eq!(params["env"]["HC_FOCUSED_TERMINAL"], "t3");
    assert_eq!(params["env"]["HC_FOCUSED_MACHINE"], "host-a");
}

#[tokio::test]
async fn a_shell_command_exports_them_before_its_line() {
    let mut app = remote_app();
    pending(&mut app);
    app.reconcile_published_view();
    let mut rx = operations(&mut app);
    core_id(&mut app, "core-fake-1");
    let server = tokio::spawn(async move {
        let mut core = rx.recv().await.unwrap();
        core.read_hello().await;
        core.answer_hello(true, &["machine.exec"], &[]).await;
        let exec = core.read_request().await;
        assert_eq!(exec["params"]["on"], "host-a");
        let line = exec["params"]["line"].as_str().unwrap().to_owned();
        core.reply(&exec["id"], json!({"output": ""})).await;
        line
    });
    app.run_custom_command(&command(CustomCommandAction::Shell, "echo hi"));
    settle(&mut app).await;
    let line = server.await.unwrap();
    assert!(line.starts_with("export "), "{line}");
    assert!(line.contains("HC_FOCUSED_TERMINAL='t3'"), "{line}");
    assert!(line.contains("HC_FOCUSED_MACHINE='host-a'"), "{line}");
    assert!(line.ends_with("; echo hi"), "{line}");
}

/// A one-pane app whose pane shows a small coloured screen.
fn screen_app() -> App {
    let mut vt = hc_vt::Vt::new(8, 3, 0);
    vt.feed(b"\x1b[1;31mred\x1b[0m \x1b[38;2;1;2;3mrgb\x1b[0m\r\n\x1b[7mrev\x1b[0m\xe4\xb8\xadx");
    let mut app = app_with(&["t1"]);
    pending(&mut app);
    app.reconcile_published_view();
    app.focus_pane("t1".into());
    app.pane.install_vt(vt);
    draw_to(&mut app, 80, 24);
    app
}

fn with_screen(action: CustomCommandAction, line: &str) -> CustomCommandKeybind {
    CustomCommandKeybind {
        screen: true,
        ..command(action, line)
    }
}

#[tokio::test]
async fn the_screen_file_holds_one_styled_line_per_row() {
    use std::os::unix::fs::PermissionsExt;
    let app = screen_app();
    let file = app
        .stage_screen(&with_screen(CustomCommandAction::Popup, "x"))
        .expect("a local pane with a screen");
    let path = file.path().to_path_buf();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    assert_eq!(lines.len(), 3, "{text:?}");
    assert!(lines.iter().all(|line| line.ends_with("\x1b[0m\n")));
    assert!(lines[0].contains("\x1b[1m") && lines[0].contains("red"));
    assert!(lines[0].contains("\x1b[38;2;1;2;3mrgb"));
    assert!(lines[1].contains("\x1b[7mrev"));
    assert!(lines[1].contains("\u{4e2d}x"));
    assert_eq!(lines[2], "\x1b[0m\n");
    drop(file);
    assert!(!path.exists());
}

#[tokio::test]
async fn without_screen_true_there_is_no_file_and_no_variable() {
    let app = screen_app();
    assert!(
        app.stage_screen(&command(CustomCommandAction::Popup, "x"))
            .is_none()
    );
    assert!(!app.command_env(None).contains_key("HC_FOCUSED_SCREEN"));
}

#[tokio::test]
async fn a_remote_pane_gets_no_screen_file() {
    let mut app = remote_app();
    app.focused = Some("host-a/t3".into());
    assert!(
        app.stage_screen(&with_screen(CustomCommandAction::Popup, "x"))
            .is_none()
    );
}

#[tokio::test]
async fn a_popup_gets_the_path_and_the_file_goes_with_the_popup() {
    let mut app = screen_app();
    let mut rx = operations(&mut app);
    core_id(&mut app, "core-fake-1");
    let server = tokio::spawn(async move {
        let mut core = rx.recv().await.unwrap();
        core.read_hello().await;
        core.answer_hello(true, &["terminal.run"], &[]).await;
        let run = core.read_request().await;
        let params = run["params"].clone();
        core.reply(&run["id"], run_reply("t9", 9)).await;
        params
    });
    app.run_custom_command(&with_screen(CustomCommandAction::Popup, "viewer"));
    settle(&mut app).await;
    let params = server.await.unwrap();
    let path = std::path::PathBuf::from(params["env"]["HC_FOCUSED_SCREEN"].as_str().unwrap());
    assert!(path.exists());
    assert!(app.popup.is_some());
    app.dismiss_popup();
    assert!(!path.exists());
}

#[tokio::test]
async fn a_shell_command_file_is_removed_after_the_exec() {
    let mut app = screen_app();
    let mut rx = operations(&mut app);
    core_id(&mut app, "core-fake-1");
    let server = tokio::spawn(async move {
        let mut core = rx.recv().await.unwrap();
        core.read_hello().await;
        core.answer_hello(true, &["machine.exec"], &[]).await;
        let exec = core.read_request().await;
        let line = exec["params"]["line"].as_str().unwrap().to_owned();
        let path = line
            .split("HC_FOCUSED_SCREEN='")
            .nth(1)
            .and_then(|rest| rest.split('\'').next())
            .map(std::path::PathBuf::from)
            .expect("the line exports the path");
        assert!(path.exists(), "the file exists while the command runs");
        core.reply(&exec["id"], json!({"output": ""})).await;
        path
    });
    app.run_custom_command(&with_screen(
        CustomCommandAction::Shell,
        "cat \"$HC_FOCUSED_SCREEN\"",
    ));
    settle(&mut app).await;
    let path = server.await.unwrap();
    // The worker removes it right after the call returns.
    for _ in 0..50 {
        if !path.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(!path.exists());
}
