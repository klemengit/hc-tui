//! `[ui] agent_panel = false`: the spaces list takes the whole sidebar and
//! the agent actions keep working.

use super::*;
use crate::input::KeybindAction as A;
use crate::sidebar::fixture::agent;

fn two_agents_app() -> App {
    let terms = ["t1", "t2"]
        .iter()
        .map(|address| {
            let mut term = agent(address, "working");
            term.labels
                .insert("herdr.space".into(), format!("space-{address}"));
            term
        })
        .collect();
    let mut app = app_with_terms(terms);
    app.focused = Some("t1".into());
    app.refresh_projection();
    app.open_attach = Arc::new(|_| Box::pin(std::future::pending()));
    app.pane = pane::Pane::new(app.pane_tx.clone(), app.open_attach.clone());
    app
}

#[tokio::test]
async fn the_panel_is_drawn_by_default() {
    let mut app = two_agents_app();
    let backend = draw_to(&mut app, 100, 30);
    assert!(
        screen_lines(&backend)
            .iter()
            .any(|line| line.contains(" agents"))
    );
    assert!(!app.hit_map.agent_body.is_empty());
    assert!(!app.hit_map.sidebar_section_divider.is_empty());
}

#[tokio::test]
async fn without_the_panel_the_spaces_take_the_whole_sidebar() {
    let mut app = two_agents_app();
    draw_to(&mut app, 100, 30);
    let with_panel = app.hit_map.workspace_body.height;

    app.shell.agent_panel = false;
    let backend = draw_to(&mut app, 100, 30);
    assert!(
        !screen_lines(&backend)
            .iter()
            .any(|line| line.contains(" agents")),
        "{:#?}",
        screen_lines(&backend)
    );
    assert!(app.hit_map.agent_body.is_empty());
    assert!(app.hit_map.sidebar_section_divider.is_empty());
    assert!(app.hit_map.agents.is_empty());
    assert!(app.hit_map.workspace_body.height > with_panel);
    // Spaces header, rows and the `new` footer fill the sidebar column.
    assert_eq!(app.hit_map.workspace_body.bottom() + 1, 30);
}

#[tokio::test]
async fn agent_actions_work_without_the_panel() {
    let mut app = two_agents_app();
    app.shell.agent_panel = false;
    draw_to(&mut app, 100, 30);
    app.on_action(A::NextAgent, 100);
    assert_eq!(app.focused.as_deref(), Some("t2"));
    app.on_action(A::PreviousAgent, 100);
    assert_eq!(app.focused.as_deref(), Some("t1"));
    draw_to(&mut app, 100, 30);
}

#[tokio::test]
async fn the_collapsed_sidebar_drops_its_agent_icons_too() {
    let mut app = two_agents_app();
    app.shell_state.sidebar_collapsed = true;
    draw_to(&mut app, 100, 30);
    assert!(!app.hit_map.agents.is_empty());

    app.shell.agent_panel = false;
    draw_to(&mut app, 100, 30);
    assert!(app.hit_map.agents.is_empty());
}
