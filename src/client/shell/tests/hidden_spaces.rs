use super::*;
use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};

fn hidden_config() -> Config {
    let mut config = Config::default();
    config.ui.sidebar.spaces.mode = crate::config::SpacesSidebarMode::Hidden;
    config
}

fn agent(pane_id: &str, workspace_id: &str, tab_id: &str, focused: bool) -> ClientShellAgent {
    ClientShellAgent {
        pane_id: pane_id.into(),
        workspace_id: workspace_id.into(),
        tab_id: tab_id.into(),
        name: Some("claude".into()),
        display_agent: None,
        agent: Some("claude".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Idle,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused,
    }
}

/// Two Spaces, one agent in each.
fn snapshot_with_agents() -> ClientShellSnapshot {
    let mut snapshot = snapshot();
    let mut workspace = snapshot.workspaces[0].clone();
    workspace.workspace_id = "ws_2".into();
    workspace.active_tab_id = "tab_2".into();
    workspace.number = 2;
    workspace.label = "other-space".into();
    workspace.focused = false;
    snapshot.workspaces.push(workspace);
    let mut tab = snapshot.tabs[0].clone();
    tab.tab_id = "tab_2".into();
    tab.workspace_id = "ws_2".into();
    tab.focused = false;
    snapshot.tabs.push(tab);
    let mut pane = snapshot.panes[0].clone();
    pane.pane_id = "pane_2".into();
    pane.workspace_id = "ws_2".into();
    pane.tab_id = "tab_2".into();
    pane.focused = false;
    snapshot.panes.push(pane);
    snapshot.agents = vec![
        agent("pane_1", "ws_1", "tab_1", true),
        agent("pane_2", "ws_2", "tab_2", false),
    ];
    snapshot
}

fn state_with(config: &Config) -> ClientShellState {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(config));
    state.set_snapshot(Box::new(snapshot_with_agents()));
    state.set_pane_surface(surface());
    state
}

fn mouse(state: &mut ClientShellState, kind: MouseEventKind, at: Rect) -> ClientShellInput {
    state.handle_raw_events(vec![RawInputEvent::Mouse(crossterm::event::MouseEvent {
        kind,
        column: at.x,
        row: at.y,
        modifiers: KeyModifiers::empty(),
    })])
}

fn agent_rect(state: &ClientShellState, pane_id: &str) -> Rect {
    state
        .hits
        .agents
        .iter()
        .find(|(_, id)| id == pane_id)
        .map(|(rect, _)| *rect)
        .expect("agent row")
}

fn sidebar_text(state: &ClientShellState, rows: &[String]) -> String {
    let width = usize::from(state.hits.sidebar_divider.x);
    rows.iter()
        .map(|row| row.chars().take(width).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn spaces_mode_defaults_to_shown_and_keeps_the_spaces_section() {
    let mut state = state_with(&Config::default());
    let frame = state.compose(106, 30).expect("shown frame");
    let text = sidebar_text(&state, &frame_rows(&frame));

    assert!(text.contains(" spaces"), "{text}");
    assert!(text.contains("other-space"), "{text}");
    assert_eq!(state.hits.workspaces.len(), 2);
    assert_ne!(state.hits.sidebar_section_divider, Rect::default());
    assert!(state.hits.new_workspace.y < state.hits.sidebar_toggle.y);
}

#[test]
fn hidden_spaces_give_the_sidebar_to_agents_and_keep_new_and_menu() {
    let mut state = state_with(&hidden_config());
    let frame = state.compose(106, 30).expect("hidden frame");
    let rows = frame_rows(&frame);
    let text = sidebar_text(&state, &rows);
    let top = usize::from(state.hits.sidebar_divider.y);
    let bottom = state.hits.sidebar_toggle.y;

    assert!(!text.contains(" spaces"), "{text}");
    assert!(state.hits.workspaces.is_empty());
    assert_eq!(state.hits.workspace_body, Rect::default());
    assert_eq!(state.hits.sidebar_section_divider, Rect::default());
    assert!(rows[top].starts_with(" agents"), "{text}");
    assert!(
        !rows[top].starts_with('─'),
        "no rule above the agents title"
    );

    assert_eq!(state.hits.agents.len(), 2);
    assert!(state
        .hits
        .agents
        .iter()
        .all(|(rect, _)| rect.bottom() <= bottom));

    let footer = &rows[usize::from(bottom)];
    assert_eq!(state.hits.new_workspace.y, bottom);
    assert_eq!(state.hits.global_launcher.y, bottom);
    assert!(state.hits.global_launcher.right() < state.hits.sidebar_toggle.x);
    assert!(footer.starts_with(" new"), "{footer}");
    assert!(footer.contains("menu"), "{footer}");
    assert!(footer.contains('«'), "{footer}");
}

#[test]
fn hidden_spaces_footer_mirrors_on_a_right_sidebar() {
    let mut config = hidden_config();
    config.ui.sidebar_position = crate::config::SidebarPositionConfig::Right;
    let mut state = state_with(&config);
    let frame = state.compose(106, 30).expect("right hidden frame");
    let rows = frame_rows(&frame);
    let divider = state.hits.sidebar_divider;
    let toggle = state.hits.sidebar_toggle;
    let row_from_divider = |y: u16| -> String {
        rows[usize::from(y)]
            .chars()
            .skip(usize::from(divider.x))
            .collect()
    };

    assert_eq!(divider.x, 106 - 26, "the sidebar sits on the right edge");
    assert!(state.hits.workspaces.is_empty());
    assert_eq!(state.hits.sidebar_section_divider, Rect::default());
    assert!(row_from_divider(divider.y).starts_with("│ agents"));

    // `»` takes the outer column; `new` follows it and `menu` ends at the
    // window's right edge.
    assert_eq!(toggle.x, divider.x + 1);
    assert_eq!(state.hits.new_workspace.x, toggle.x + 1);
    assert_eq!(state.hits.new_workspace.y, toggle.y);
    assert_eq!(state.hits.global_launcher.y, toggle.y);
    assert_eq!(state.hits.global_launcher.right(), 106);
    let footer = row_from_divider(toggle.y);
    assert!(footer.starts_with("│» new"), "{footer}");
    assert!(footer.trim_end().ends_with("menu"), "{footer}");
    assert!(state
        .hits
        .agents
        .iter()
        .all(|(rect, _)| rect.x > divider.x && rect.bottom() <= toggle.y));
}

#[test]
fn hidden_footer_still_creates_a_space_and_opens_the_menu() {
    let mut state = state_with(&hidden_config());
    state.compose(106, 30).expect("hidden frame");

    let new_workspace = state.hits.new_workspace;
    let create = mouse(
        &mut state,
        MouseEventKind::Down(MouseButton::Left),
        new_workspace,
    );
    assert!(
        create.actions.iter().any(|action| matches!(
            action,
            ClientShellAction::Endpoint { request, .. }
                if matches!(request.method, crate::api::schema::Method::WorkspaceCreate(_))
        )),
        "{:?}",
        create.actions
    );

    state.compose(106, 30).expect("hidden frame");
    let launcher = state.hits.global_launcher;
    let open = mouse(
        &mut state,
        MouseEventKind::Down(MouseButton::Left),
        launcher,
    );
    assert!(open.repaint);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::GlobalMenu(_))
    ));
}

#[test]
fn global_menu_attention_dot_stays_in_the_hidden_footer() {
    let mut state = state_with(&hidden_config());
    let mut attention = snapshot_with_agents();
    attention.update_available = Some("9.9.9".into());
    state.set_snapshot(Box::new(attention));
    let frame = state.compose(106, 30).expect("hidden frame");
    let rows = frame_rows(&frame);
    let footer = &rows[usize::from(state.hits.sidebar_toggle.y)];

    assert!(footer.contains("● menu"), "{footer}");
    assert!(state.hits.global_launcher.right() < state.hits.sidebar_toggle.x);
}

#[test]
fn right_click_on_an_agent_opens_its_space_menu_in_both_modes() {
    for config in [Config::default(), hidden_config()] {
        let mut state = state_with(&config);
        state.compose(106, 30).expect("frame");
        let row = agent_rect(&state, "pane_2");

        let open = mouse(&mut state, MouseEventKind::Down(MouseButton::Right), row);
        assert!(open.repaint);
        assert!(
            matches!(
                &state.overlay,
                Some(ClientShellOverlay::ContextMenu(ClientContextMenuOverlay {
                    target: ClientContextMenuTarget::Workspace { workspace_id, .. },
                    ..
                })) if workspace_id == "ws_2"
            ),
            "{:?}",
            config.ui.sidebar.spaces.mode
        );
    }
}

#[test]
fn collapsed_rail_with_hidden_spaces_lists_only_agents() {
    let mut state = state_with(&hidden_config());
    state.sidebar_collapsed = true;
    state.compose(106, 30).expect("collapsed frame");

    assert!(state.hits.workspaces.is_empty());
    let first = state
        .hits
        .agents
        .iter()
        .map(|(rect, _)| rect.y)
        .min()
        .expect("agent rows");
    assert_eq!(first, 0, "agents start at the top of the rail");

    // The compact rail keeps its own rule: no Space menus from it.
    let row = agent_rect(&state, "pane_2");
    mouse(&mut state, MouseEventKind::Down(MouseButton::Right), row);
    assert!(!matches!(
        state.overlay,
        Some(ClientShellOverlay::ContextMenu(_))
    ));

    let mut shown = state_with(&Config::default());
    shown.sidebar_collapsed = true;
    shown.compose(106, 30).expect("collapsed frame");
    assert_eq!(shown.hits.workspaces.len(), 2);
}

#[test]
fn workspace_picker_reveals_hidden_spaces_until_it_ends() {
    let mut state = state_with(&hidden_config());
    state.compose(106, 30).expect("hidden frame");
    assert!(state.hits.workspaces.is_empty());

    state.handle_input_bytes(&[0x02]);
    state.handle_input_bytes(b"w");
    assert_eq!(state.mode, ClientShellMode::Navigate);
    let picking = state.compose(106, 30).expect("picker frame");
    let text = sidebar_text(&state, &frame_rows(&picking));
    assert!(text.contains(" spaces"), "{text}");
    assert_eq!(state.hits.workspaces.len(), 2);

    state.handle_input_bytes(b"\x1b");
    assert_ne!(state.mode, ClientShellMode::Navigate);
    let done = state.compose(106, 30).expect("hidden again");
    let text = sidebar_text(&state, &frame_rows(&done));
    assert!(!text.contains(" spaces"), "{text}");
    assert!(state.hits.workspaces.is_empty());
}

#[test]
fn hiding_spaces_keeps_the_dragged_split_for_when_they_return() {
    let mut state = state_with(&Config::default());
    state.sidebar_section_split = 0.7;
    state.compose(106, 30).expect("shown frame");
    let divider = state.hits.sidebar_section_divider;
    assert_ne!(divider, Rect::default());

    state.config.spaces.mode = crate::config::SpacesSidebarMode::Hidden;
    state.compose(106, 30).expect("hidden frame");
    assert_eq!(state.hits.sidebar_section_divider, Rect::default());
    assert_eq!(state.sidebar_section_split, 0.7);

    state.config.spaces.mode = crate::config::SpacesSidebarMode::Shown;
    state.compose(106, 30).expect("shown again");
    assert_eq!(state.hits.sidebar_section_divider, divider);
}

#[test]
fn mobile_layout_ignores_the_spaces_mode() {
    let mut shown = state_with(&Config::default());
    let mut hidden = state_with(&hidden_config());
    let shown_frame = shown.compose(60, 30).expect("mobile frame");
    let hidden_frame = hidden.compose(60, 30).expect("mobile frame");

    assert_eq!(frame_rows(&shown_frame), frame_rows(&hidden_frame));
}

#[test]
fn settings_sidebar_section_writes_and_applies_the_spaces_mode() {
    let _guard = crate::config::test_config_env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = std::env::temp_dir().join(format!("herdr-hidden-spaces-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("config.toml");
    std::fs::write(
        &path,
        "[ui.sidebar.agents]\nrows = [[\"state_icon\", \"agent\"]]\n",
    )
    .expect("seed config");
    std::env::set_var(crate::config::CONFIG_PATH_ENV_VAR, &path);

    let mut state = state_with(&Config::default());
    state.open_settings_overlay();
    state.compose(106, 30).expect("settings overlay");
    let tab = state
        .hits
        .settings_tabs
        .iter()
        .find(|(_, section)| *section == ClientSettingsSection::Sidebar)
        .map(|(rect, _)| *rect)
        .expect("sidebar tab");
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), tab);
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Settings(ClientSettingsOverlay {
            section: ClientSettingsSection::Sidebar,
            selected: 0,
            ..
        }))
    ));
    state.handle_input_bytes(b"j");
    let save = state.handle_input_bytes(b"\r");

    let written = std::fs::read_to_string(&path).expect("written config");
    std::env::remove_var(crate::config::CONFIG_PATH_ENV_VAR);
    let _ = std::fs::remove_dir_all(&dir);

    assert!(save.actions.iter().any(|action| matches!(
        action,
        ClientShellAction::Endpoint { request, .. }
            if matches!(request.method, crate::api::schema::Method::ServerReloadConfig(_))
    )));
    let parsed: Config = toml::from_str(&written).expect("written config parses");
    assert_eq!(
        parsed.ui.sidebar.spaces.mode,
        crate::config::SpacesSidebarMode::Hidden
    );
    assert_eq!(
        parsed.ui.sidebar.agents.rows.len(),
        1,
        "existing sidebar rows survive: {written}"
    );
    assert_eq!(
        state.config.spaces.mode,
        crate::config::SpacesSidebarMode::Hidden
    );

    state.overlay = None;
    state.compose(106, 30).expect("hidden after save");
    assert!(state.hits.workspaces.is_empty());
}

#[test]
fn settings_sidebar_section_marks_current_values_and_moves_the_sidebar() {
    let _guard = crate::config::test_config_env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = std::env::temp_dir().join(format!("herdr-sidebar-edge-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("config.toml");
    std::fs::write(&path, "[ui]\naccent = \"#f5a97f\"\n").expect("seed config");
    std::env::set_var(crate::config::CONFIG_PATH_ENV_VAR, &path);

    let mut state = state_with(&Config::default());
    state.open_settings_overlay();
    state.compose(106, 30).expect("settings overlay");
    let tab = state
        .hits
        .settings_tabs
        .iter()
        .find(|(_, section)| *section == ClientSettingsSection::Sidebar)
        .map(|(rect, _)| *rect)
        .expect("sidebar tab");
    mouse(&mut state, MouseEventKind::Down(MouseButton::Left), tab);
    let frame = state.compose(106, 30).expect("sidebar settings");
    let text = frame_rows(&frame).join("\n");
    assert!(text.contains("sidebar edge"), "{text}");
    assert!(text.contains("shown ✓"), "{text}");
    assert!(text.contains("left ✓"), "{text}");
    assert!(!text.contains("right ✓"), "{text}");

    for _ in 0..3 {
        state.handle_input_bytes(b"j");
    }
    state.handle_input_bytes(b"\r");

    let written = std::fs::read_to_string(&path).expect("written config");
    std::env::remove_var(crate::config::CONFIG_PATH_ENV_VAR);
    let _ = std::fs::remove_dir_all(&dir);

    let parsed: Config = toml::from_str(&written).expect("written config parses");
    assert_eq!(
        parsed.ui.sidebar_position,
        crate::config::SidebarPositionConfig::Right,
        "{written}"
    );
    assert_eq!(
        parsed.ui.sidebar.spaces.mode,
        crate::config::SpacesSidebarMode::Shown
    );
    assert_eq!(
        state.config.sidebar_position,
        crate::config::SidebarPositionConfig::Right
    );

    let frame = state.compose(106, 30).expect("settings after save");
    let text = frame_rows(&frame).join("\n");
    assert!(text.contains("right ✓"), "{text}");
    assert!(!text.contains("left ✓"), "{text}");
    state.overlay = None;
    state.compose(106, 30).expect("right sidebar");
    assert_eq!(state.hits.sidebar_divider.x, 106 - 26);
}
