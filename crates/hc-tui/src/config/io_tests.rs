use super::*;
use crate::test_support::TestDir;

fn load_text(text: &str) -> LoadedConfig {
    let root = TestDir::new("hctui-config-test");
    let path = root.join("config.toml");
    std::fs::write(&path, text).unwrap();
    let loaded = load(&path);
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
    loaded
}

#[test]
fn known_deferred_sections_do_not_hide_unknown_root_keys() {
    let loaded = load_text(
        "bogus = true\nplugin = []\n[terminal]\ndefault_shell = 'sh'\n[ui]\nsidebar_width = 30\n",
    );
    assert_eq!(loaded.config.ui.sidebar_width, 30);
    assert_eq!(
        loaded.diagnostics,
        [
            "unknown config key bogus; ignoring key",
            "unknown config key plugin; ignoring key",
        ]
    );
}

#[test]
fn agent_panel_sort_defaults_to_priority_with_explicit_values_intact() {
    use crate::config::{AgentPanelSortConfig, UiConfig};
    assert_eq!(
        UiConfig::default().agent_panel_sort,
        AgentPanelSortConfig::Priority
    );
    // A config without the key gets the same new default.
    let loaded = load_text("[ui]\nsidebar_width = 30\n");
    assert_eq!(
        loaded.config.ui.agent_panel_sort,
        AgentPanelSortConfig::Priority
    );
    for (value, expected) in [
        ("spaces", AgentPanelSortConfig::Spaces),
        ("workspaces", AgentPanelSortConfig::Spaces),
        ("priority", AgentPanelSortConfig::Priority),
    ] {
        let loaded = load_text(&format!("[ui]\nagent_panel_sort = '{value}'\n"));
        assert_eq!(loaded.config.ui.agent_panel_sort, expected, "{value}");
    }
}

#[test]
fn agent_panel_space_headers_loads_without_unknown_key_diagnostics() {
    assert!(!crate::config::UiConfig::default().agent_panel_space_headers);
    let loaded = load_text("[ui]\nagent_panel_space_headers = true\n");
    assert!(loaded.config.ui.agent_panel_space_headers);
    assert!(
        loaded.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        loaded.diagnostics
    );
}

#[test]
fn agent_panel_defaults_on_and_loads_without_unknown_key_diagnostics() {
    assert!(crate::config::UiConfig::default().agent_panel);
    let loaded = load_text("[ui]\nsidebar_width = 30\n");
    assert!(loaded.config.ui.agent_panel);
    let loaded = load_text("[ui]\nagent_panel = false\n");
    assert!(!loaded.config.ui.agent_panel);
    assert!(
        loaded.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        loaded.diagnostics
    );
}

#[test]
fn spaces_agents_defaults_to_none_and_parses_each_value() {
    use crate::config::{SpaceAgentsConfig, SpacesSidebarConfig};
    assert_eq!(
        SpacesSidebarConfig::default().agents,
        SpaceAgentsConfig::None
    );
    let loaded = load_text("[ui]\nsidebar_width = 30\n");
    assert_eq!(
        loaded.config.ui.sidebar.spaces.agents,
        SpaceAgentsConfig::None
    );
    for (value, expected) in [
        ("none", SpaceAgentsConfig::None),
        ("active", SpaceAgentsConfig::Active),
        ("all", SpaceAgentsConfig::All),
    ] {
        let loaded = load_text(&format!("[ui.sidebar.spaces]\nagents = '{value}'\n"));
        assert_eq!(loaded.config.ui.sidebar.spaces.agents, expected, "{value}");
        assert!(
            loaded.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            loaded.diagnostics
        );
    }
}

#[test]
fn spaces_agents_keeps_the_other_spaces_keys() {
    let loaded = load_text(
        "[ui.sidebar.spaces]\nagents = 'active'\nrow_gap = 1\nrows = [['state_icon', 'workspace']]\n",
    );
    let spaces = &loaded.config.ui.sidebar.spaces;
    assert_eq!(spaces.agents, crate::config::SpaceAgentsConfig::Active);
    assert_eq!(spaces.row_gap, 1);
    assert_eq!(spaces.rows.len(), 1);
    assert!(loaded.diagnostics.is_empty());
}

#[test]
fn an_unknown_spaces_agents_value_is_a_config_error() {
    let loaded = load_text("[ui.sidebar.spaces]\nagents = 'some'\n");
    assert_eq!(
        loaded.config.ui.sidebar.spaces.agents,
        crate::config::SpaceAgentsConfig::None
    );
    assert!(
        loaded
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.starts_with("config parse error")),
        "{:?}",
        loaded.diagnostics
    );
}

#[test]
fn agent_cycle_defaults_to_all_and_parses_each_value() {
    use crate::config::AgentCycleConfig;
    assert_eq!(
        crate::config::UiConfig::default().agent_cycle,
        AgentCycleConfig::All
    );
    assert_eq!(
        load_text("[ui]\nsidebar_width = 30\n")
            .config
            .ui
            .agent_cycle,
        AgentCycleConfig::All
    );
    for (value, expected) in [
        ("all", AgentCycleConfig::All),
        ("active", AgentCycleConfig::Active),
    ] {
        let loaded = load_text(&format!("[ui]\nagent_cycle = '{value}'\n"));
        assert_eq!(loaded.config.ui.agent_cycle, expected, "{value}");
        assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    }
    let loaded = load_text("[ui]\nagent_cycle = 'some'\n");
    assert_eq!(loaded.config.ui.agent_cycle, AgentCycleConfig::All);
    assert!(
        loaded
            .diagnostics
            .iter()
            .any(|d| d.starts_with("config parse error")),
        "{:?}",
        loaded.diagnostics
    );
}

#[test]
fn seen_after_ms_defaults_to_zero_and_parses() {
    assert_eq!(crate::config::UiConfig::default().seen_after_ms, 0);
    assert_eq!(
        load_text("[ui]\nsidebar_width = 30\n")
            .config
            .ui
            .seen_after_ms,
        0
    );
    let loaded = load_text("[ui]\nseen_after_ms = 1500\n");
    assert_eq!(loaded.config.ui.seen_after_ms, 1500);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let loaded = load_text("[ui]\nseen_after_ms = -1\n");
    assert_eq!(loaded.config.ui.seen_after_ms, 0);
    assert!(!loaded.diagnostics.is_empty());
}

#[test]
fn the_agent_count_space_token_parses_plain_styled_and_with_rules() {
    use crate::config::SpaceSidebarToken as T;
    let loaded = load_text(
        "[ui.sidebar.spaces]\nrows = [['workspace', 'agent_count'], [{ token = 'agent_count', dim = true }], [{ token = 'agent_count', rules = [{ gt = 1, bold = true }] }]]\n",
    );
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let rows = &loaded.config.ui.sidebar.spaces.rows;
    assert_eq!(rows[0][1], T::AgentCount);
    assert!(matches!(&rows[1][0], T::Styled { token, .. } if **token == T::AgentCount));
    assert!(matches!(&rows[2][0], T::Styled { rules, .. } if rules.len() == 1));
    // The default rows do not use it.
    let defaults = crate::config::SpacesSidebarConfig::default();
    assert!(defaults.rows.iter().flatten().all(|t| *t != T::AgentCount));
}

#[test]
fn sidebar_offsets_default_to_the_herdr_layout_and_parse() {
    let defaults = load_text("[ui]\nsidebar_width = 30\n")
        .config
        .ui
        .sidebar
        .indents();
    assert_eq!(
        (
            defaults.machine_padding,
            defaults.machine_indent,
            defaults.space_padding,
            defaults.agent_indent
        ),
        (1, 2, 1, 2)
    );
    let loaded = load_text(
        "[ui.sidebar]\nmachine_padding = 0\nmachine_indent = 1\nspace_padding = 0\nagent_indent = 99\n",
    );
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let indents = loaded.config.ui.sidebar.indents();
    assert_eq!(indents.machine_padding, 0);
    assert_eq!(indents.machine_indent, 1);
    assert_eq!(indents.agent_indent, 8, "capped");
}

#[test]
fn navigate_tree_defaults_off_with_h_and_l_for_leave_and_enter() {
    let loaded = load_text("[ui]\nsidebar_width = 30\n");
    let nav = &loaded.config.live_keybinds().keybinds.navigate;
    assert!(!nav.tree);
    assert_eq!(nav.enter.labels(), ["l"]);
    assert_eq!(nav.leave.labels(), ["h"]);
    assert_eq!(nav.next_machine.labels(), ["shift+j"]);
    assert_eq!(nav.prev_machine.labels(), ["shift+k"]);
    // The pane keys keep their own defaults and share h/l without a conflict.
    assert_eq!(nav.pane_left.labels(), ["h"]);
    assert_eq!(nav.pane_right.labels(), ["l"]);
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
}

#[test]
fn navigate_tree_and_its_keys_parse_without_diagnostics() {
    let loaded = load_text(
        "[keys]\nnavigate_tree = true\nnavigate_enter = ['p', 'o']\nnavigate_leave = 'u'\n",
    );
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let nav = &loaded.config.live_keybinds().keybinds.navigate;
    assert!(nav.tree);
    assert_eq!(nav.enter.labels().len(), 2);
    assert_eq!(nav.leave.labels(), ["u"]);
}

#[test]
fn machine_jump_keys_are_bindable_and_conflict_checked() {
    let loaded = load_text(
        "[keys]\nnavigate_tree = true\nnavigate_next_machine = ['n', 'shift+n']\nnavigate_prev_machine = 'p'\n",
    );
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    let nav = &loaded.config.live_keybinds().keybinds.navigate;
    assert_eq!(nav.next_machine.labels().len(), 2);
    assert_eq!(nav.prev_machine.labels(), ["p"]);
    // Same-key clash with navigate_enter: the first (enter) is kept.
    let loaded = load_text("[keys]\nnavigate_next_machine = 'l'\nnavigate_enter = 'l'\n");
    assert!(
        loaded.diagnostics.iter().any(|d| d.contains("navigate_")),
        "{:?}",
        loaded.diagnostics
    );
    // Reserved keys are refused.
    let loaded = load_text("[keys]\nnavigate_prev_machine = 'tab'\n");
    assert!(
        loaded
            .config
            .live_keybinds()
            .keybinds
            .navigate
            .prev_machine
            .labels()
            .is_empty()
    );
}

#[test]
fn navigate_enter_cannot_take_a_workspace_navigation_key() {
    let loaded = load_text("[keys]\nnavigate_enter = 'down'\n");
    assert!(
        loaded
            .diagnostics
            .iter()
            .any(|d| d.contains("navigate_enter")),
        "{:?}",
        loaded.diagnostics
    );
    let nav = &loaded.config.live_keybinds().keybinds.navigate;
    assert!(nav.enter.labels().is_empty());
}

#[test]
fn herdr_only_experimental_keys_are_accepted_and_ignored() {
    let loaded = load_text("[experimental]\npane_history = true\n");
    assert!(loaded.diagnostics.is_empty());
}

/// `[advanced]` carries Herdr's own scrollback key and hc-tui's attach
/// history. Both default without a config, parse with one, and the legacy
/// `scrollback_lines` alias keeps a Herdr-era file working.
#[test]
fn advanced_keys_default_parse_and_alias() {
    use crate::config::{
        DEFAULT_ATTACH_HISTORY_LINES, DEFAULT_SCROLLBACK_LIMIT_BYTES, MAX_ATTACH_HISTORY_LINES,
    };
    let loaded = load_text("[ui]\nsidebar_width = 30\n");
    assert_eq!(
        loaded.config.advanced.scrollback_limit_bytes,
        DEFAULT_SCROLLBACK_LIMIT_BYTES
    );
    assert_eq!(DEFAULT_SCROLLBACK_LIMIT_BYTES, 10_000_000);
    assert_eq!(
        loaded.config.advanced.attach_history_lines,
        DEFAULT_ATTACH_HISTORY_LINES
    );
    assert_eq!(DEFAULT_ATTACH_HISTORY_LINES, 5000);
    assert_eq!(loaded.config.advanced.attach_history(), 5000);
    assert!(loaded.diagnostics.is_empty());

    let loaded =
        load_text("[advanced]\nscrollback_limit_bytes = 2000000\nattach_history_lines = 1200\n");
    assert_eq!(loaded.config.advanced.scrollback_limit_bytes, 2_000_000);
    assert_eq!(loaded.config.advanced.attach_history(), 1200);
    assert!(loaded.diagnostics.is_empty());

    let loaded = load_text("[advanced]\nscrollback_lines = 42\n");
    assert_eq!(loaded.config.advanced.scrollback_limit_bytes, 42);

    // `0` is legal and means "no history"; anything past the ceiling is
    // clamped on use rather than rejected.
    let loaded = load_text("[advanced]\nattach_history_lines = 0\n");
    assert_eq!(loaded.config.advanced.attach_history(), 0);
    let loaded = load_text(&format!(
        "[advanced]\nattach_history_lines = {}\n",
        MAX_ATTACH_HISTORY_LINES * 4
    ));
    assert_eq!(
        loaded.config.advanced.attach_history(),
        MAX_ATTACH_HISTORY_LINES
    );
}

/// `[advanced]` is consumed now, so a misspelt key is diagnosed like any
/// other modelled section — Herdr's own section has no other keys to
/// tolerate.
#[test]
fn unknown_advanced_key_is_diagnosed() {
    let loaded = load_text("[advanced]\nscrollback_limit_byte = 1\n");
    assert_eq!(
        loaded.diagnostics,
        ["unknown config key advanced.scrollback_limit_byte; ignoring key"]
    );
}

#[test]
fn unknown_section_uses_the_reference_hint_once() {
    let loaded = load_text("[toast]\ndelivery = 'system'\n");
    assert_eq!(
        loaded.diagnostics,
        ["unknown config section [toast]; did you mean [ui.toast]? ignoring section"]
    );
}

#[test]
fn displaced_bom_is_recovered_and_string_bom_is_preserved() {
    let loaded = load_text("onboarding = false\n\u{feff}[ui]\nsidebar_width = 30\n");
    assert_eq!(loaded.config.ui.sidebar_width, 30);
    assert!(loaded.diagnostics.is_empty());
    let text = "[theme]\nname = '''tokyo-\u{feff}night'''\n";
    let loaded = load_text(text);
    assert_eq!(
        loaded.config.theme.name.as_deref(),
        Some("tokyo-\u{feff}night")
    );
}

#[test]
fn popup_placement_and_border_parse_with_defaults_unchanged() {
    use crate::popup_size::{PopupLayout, PopupPlacement};
    let layouts = |text: &str| {
        let loaded = load_text(text);
        let (live, mut diagnostics) = loaded.config.live_keybinds_with_diagnostics().unwrap();
        diagnostics.extend(loaded.diagnostics);
        let layouts: Vec<_> = live
            .keybinds
            .custom_commands
            .iter()
            .map(|command| (command.layout, command.width.is_some()))
            .collect();
        (layouts, diagnostics)
    };
    let entry = |extra: &str| {
        format!("[[keys.command]]\nkey = 'alt+h'\ntype = 'popup'\ncommand = 'htop'\n{extra}\n")
    };
    let (plain, diagnostics) = layouts(&entry(""));
    assert_eq!(plain, vec![(PopupLayout::default(), false)]);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let (pane, diagnostics) = layouts(&entry("placement = 'pane'\nborder = false"));
    assert_eq!(
        pane,
        vec![(
            PopupLayout {
                placement: PopupPlacement::Pane,
                border: false
            },
            false
        )]
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    // width/height are ignored with a diagnostic under placement = "pane".
    let (sized, diagnostics) = layouts(&entry("placement = 'pane'\nwidth = '50%'"));
    assert_eq!(sized[0].0.placement, PopupPlacement::Pane);
    assert!(!sized[0].1);
    assert!(
        diagnostics.iter().any(|d| d.contains("placement")),
        "{diagnostics:?}"
    );

    // A bad value, a persistent float and a non-popup all keep the default.
    for extra in [
        "placement = 'left'",
        "persistent = true\nplacement = 'pane'",
    ] {
        let (kept, diagnostics) = layouts(&entry(extra));
        assert_eq!(kept[0].0, PopupLayout::default(), "{extra}");
        assert!(!diagnostics.is_empty(), "{extra}");
    }
}

#[test]
fn screen_defaults_off_and_parses() {
    let screens = |extra: &str| {
        let loaded = load_text(&format!(
            "[[keys.command]]\nkey = 'alt+h'\ntype = 'popup'\ncommand = 'htop'\n{extra}\n"
        ));
        let (live, diagnostics) = loaded.config.live_keybinds_with_diagnostics().unwrap();
        (live.keybinds.custom_commands[0].screen, diagnostics)
    };
    assert_eq!(screens(""), (false, vec![]));
    assert_eq!(screens("screen = true"), (true, vec![]));
    let (screen, diagnostics) = screens("persistent = true\nscreen = true");
    assert!(!screen);
    assert!(!diagnostics.is_empty());
}
