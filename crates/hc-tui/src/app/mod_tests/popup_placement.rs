//! `placement = "pane"` / `border = false`: the popup terminal covers the
//! focused pane's terminal grid cell for cell.

use super::*;
use crate::config::PaneBordersConfig;
use crate::popup_size::{PopupLayout, PopupPlacement, resolve_popup_geometry};
use ratatui::layout::Rect;

const PANE: PopupLayout = PopupLayout {
    placement: PopupPlacement::Pane,
    border: false,
};

fn split_app() -> App {
    let mut app = app_with(&["t1", "t2"]);
    app.focused = Some("t1".into());
    app.refresh_projection();
    let tab = app.projection.snapshot.focused_tab_id.clone().unwrap();
    let key = crate::project::layout_key(None, &tab);
    let split = serde_json::json!({"type":"split","vertical":true,"ratio":0.5,
        "first":{"type":"pane","terminal":"t1"},
        "second":{"type":"pane","terminal":"t2"}});
    app.view.documents_mut().insert(
        hc_proto::watch::DocKey {
            machine: None,
            name: "hc-tui".into(),
        },
        Arc::new(hc_proto::watch::Document {
            value: serde_json::json!({"schema":1,"focus":"t1","tabs":{key:{"root":split}}}),
            version: 1,
            ..Default::default()
        }),
    );
    app.reconcile_published_view();
    app
}

fn pane_rect(app: &App) -> Rect {
    pane_geometry::focused_pane_inner(app, app.area.0, app.area.1)
}

fn placed(app: &App) -> crate::popup_size::PopupResolvedGeometry {
    app.popup_geometry(None, None, PANE).unwrap()
}

#[tokio::test]
async fn a_lone_pane_is_covered_exactly() {
    let mut app = app_with(&["t1"]);
    app.area = (100, 30);
    app.shell.pane_scrollbars = false;
    for borders in [PaneBordersConfig::Auto, PaneBordersConfig::Off] {
        app.shell.pane_borders = borders;
        let geometry = placed(&app);
        let rect = pane_rect(&app);
        assert_eq!(geometry.outer, rect);
        assert_eq!(geometry.inner, rect);
        assert_eq!(
            (geometry.terminal_cols, geometry.terminal_rows),
            (rect.width, rect.height)
        );
        // Without borders the lone pane is the whole pane surface.
        assert_eq!(rect, app.frame_area());
    }
}

#[tokio::test]
async fn a_framed_lone_pane_is_covered_inside_its_border() {
    let mut app = app_with(&["t1"]);
    app.area = (100, 30);
    app.shell.pane_scrollbars = false;
    app.shell.pane_borders = PaneBordersConfig::Always;
    app.shell.pane_outer_borders = true;
    let surface = app.frame_area();
    let geometry = placed(&app);
    assert_eq!(geometry.outer, pane_rect(&app));
    assert_eq!(
        (geometry.terminal_cols, geometry.terminal_rows),
        (surface.width - 2, surface.height - 2)
    );
    assert_eq!(
        (geometry.outer.x, geometry.outer.y),
        (surface.x + 1, surface.y + 1)
    );
}

#[tokio::test]
async fn a_split_pane_gets_its_own_half() {
    let mut app = split_app();
    app.area = (120, 40);
    app.refresh_projection();
    for (borders, gaps) in [
        (PaneBordersConfig::Auto, false),
        (PaneBordersConfig::Off, false),
        (PaneBordersConfig::Always, false),
        (PaneBordersConfig::Always, true),
    ] {
        app.shell.pane_borders = borders;
        app.shell.pane_gaps = gaps;
        let surface = app.frame_area();
        let geometry = placed(&app);
        let rect = pane_rect(&app);
        assert_eq!(geometry.outer, rect, "{borders:?} {gaps}");
        assert_eq!(
            (geometry.terminal_cols, geometry.terminal_rows),
            (rect.width, rect.height)
        );
        assert!(rect.width < surface.width && rect.width > 0);
        assert!(rect.x >= surface.x);
    }
}

#[tokio::test]
async fn defaults_keep_the_centered_framed_geometry() {
    let app = app_with(&["t1"]);
    let size = Some(crate::popup_size::PopupSize::Percent(60));
    assert_eq!(
        app.popup_geometry(size, size, PopupLayout::default()),
        resolve_popup_geometry(size, size, app.frame_area())
    );
    // A centered popup without a frame keeps its outer rect and fills it.
    let bare = app
        .popup_geometry(
            size,
            size,
            PopupLayout {
                placement: PopupPlacement::Center,
                border: false,
            },
        )
        .unwrap();
    let framed = resolve_popup_geometry(size, size, app.frame_area()).unwrap();
    assert_eq!(bare.outer, framed.outer);
    assert_eq!(bare.inner, framed.outer);
    assert_eq!(
        (bare.terminal_cols, bare.terminal_rows),
        (framed.outer.width, framed.outer.height)
    );
}

#[tokio::test]
async fn the_popup_draws_on_the_pane_and_keys_go_to_it() {
    use ratatui::{Terminal, backend::TestBackend};
    let mut app = split_app();
    app.area = (120, 40);
    app.open_attach = Arc::new(|_| Box::pin(std::future::pending()));
    app.pane = pane::Pane::new(app.pane_tx.clone(), app.open_attach.clone());
    app.refresh_projection();
    let rect = pane_rect(&app);
    let geometry = placed(&app);
    let mut pane = app.new_pane();
    popup::popup_fit(&mut pane, geometry);
    app.popup = Some(popup::PopupState {
        pane,
        terminal: "t9".into(),
        width: None,
        height: None,
        layout: PANE,
        owner_tab: app.projection.snapshot.focused_tab_id.clone(),
        persistent: false,
        key: None,
        created_ms: 1,
        seen: true,
    });
    let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
    app.sched.mark_dirty(Instant::now());
    app.maybe_draw(&mut term, Instant::now()).unwrap();
    let hit = app.hit_map.popup.clone().unwrap();
    assert_eq!(hit.rect, rect);
    assert_eq!(hit.inner_rect, rect);
    assert_eq!(
        app.popup.as_ref().unwrap().pane.allocated_size(),
        (rect.width, rect.height)
    );
    // The popup is modal: a key is swallowed by it, not the pane under it.
    assert!(app.popup.is_some());
    assert_eq!(app.focused.as_deref(), Some("t1"));
    app.dismiss_popup();
    assert!(app.popup.is_none());
    assert_eq!(app.focused.as_deref(), Some("t1"));
}
