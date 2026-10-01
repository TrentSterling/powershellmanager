//! Inert native-UI tests: no tray, focus worker, config writes or window moves.
use crate::{
    app::PsmApp,
    config::Config,
    gui,
    theme::{self, ThemeSettings},
};
use egui::{Event, PointerButton, Pos2, Vec2};
mod offscreen;

fn fixture() -> PsmApp {
    let mut app = PsmApp::preview(Config::default());
    app.use_custom = true;
    app.col_weights = vec![0.62, 0.38];
    app.row_weights = vec![0.5, 0.5];
    app.managed_windows = [
        ("WindowsTerminal.exe", "Terminal · build workspace"),
        ("Code.exe", "Editor · project workspace"),
        ("firefox.exe", "Browser · documentation"),
        ("Discord.exe", "Chat · team workspace"),
        ("powershell.exe", "Shell · local tools"),
        ("notepad++.exe", "Notes · 日本語 / café / 🎨"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (name, title))| crate::windows::ManagedWindow {
        hwnd: i as isize + 1,
        process_name: name.into(),
        title: title.into(),
        category: crate::windows::categorize_process(name),
        is_minimized: i == 4,
        rect: crate::monitor::Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        },
    })
    .collect();
    app.status = "TEST DATA · isolated UI preview; window titles are illustrative.".into();
    app
}

#[test]
fn layout_parser_aliases_limits_and_display_labels_are_consistent() {
    use crate::layout::LayoutPreset as P;
    for (input, expected, name) in [
        (" 2 X 3 ", P::Grid { cols: 2, rows: 3 }, "2x3 Grid"),
        ("columns 4", P::Columns(4), "4 Columns"),
        ("columns:1", P::Columns(1), "1 Columns"),
        ("rows 3", P::Rows(3), "3 Rows"),
        ("rows:8", P::Rows(8), "8 Rows"),
        ("left-right", P::LeftRight, "Left / Right"),
        ("leftright", P::LeftRight, "Left / Right"),
        ("split", P::LeftRight, "Left / Right"),
        ("top-bottom", P::TopBottom, "Top / Bottom"),
        ("topbottom", P::TopBottom, "Top / Bottom"),
        ("main-side", P::MainSide { side_count: 2 }, "Main + 2 Side"),
        ("mainside:4", P::MainSide { side_count: 4 }, "Main + 4 Side"),
        ("focus", P::Focus { side_count: 3 }, "Focus + 3 Side"),
        ("focus:8", P::Focus { side_count: 8 }, "Focus + 8 Side"),
    ] {
        assert_eq!(P::parse(input), Some(expected.clone()), "parse {input}");
        assert_eq!(expected.display_name(), name);
        assert!(expected.valid());
        assert!(expected.slot_count() > 0);
    }
    for input in [
        "badx3",
        "2xbad",
        "0x2",
        "2x0",
        "9x1",
        "1x9",
        "1x2x3",
        "columns:bad",
        "columns:0",
        "columns:9",
        "rows:bad",
        "rows:0",
        "rows:9",
        "main-side:bad",
        "main-side:0",
        "main-side:9",
        "focus:bad",
        "focusfocus",
        "main-sidemain-side",
        "columns::3",
        "rows::3",
        "focus:0",
        "focus:9",
        "unknown",
        "",
    ] {
        assert!(P::parse(input).is_none(), "reject {input}");
    }
    let area = crate::monitor::Rect {
        x: -400,
        y: 30,
        w: 1920,
        h: 1040,
    };
    for preset in [
        P::Grid { cols: 0, rows: 2 },
        P::Grid { cols: 2, rows: 9 },
        P::Columns(0),
        P::Rows(9),
        P::MainSide { side_count: 0 },
        P::Focus { side_count: 9 },
    ] {
        assert!(!preset.valid());
        assert_eq!(preset.slot_count(), 0);
        assert!(preset.compute_slots(&area, 4).is_empty());
    }
}

#[test]
fn bright_outlines_in_a_light_flat_workspace_keep_text_readable() {
    let settings = ThemeSettings {
        dark: false,
        high_contrast: true,
        gradient_enabled: false,
        frost_light: 0.0,
        ..Default::default()
    };
    let tokens = theme::tokens(settings);
    assert_eq!(tokens.text, egui::Color32::from_rgb(12, 12, 18));
    assert_eq!(tokens.text_muted, tokens.text);
    for color in [
        egui::Color32::RED,
        egui::Color32::GREEN,
        egui::Color32::BLUE,
    ] {
        assert_eq!(theme::composed_panel(settings, color), tokens.bg);
        assert!(theme::contrast_ratio(tokens.text, theme::composed_panel(settings, color)) >= 4.5);
    }
}

#[test]
fn empty_and_partial_pin_rules_require_every_supplied_condition_to_match() {
    let mut rule = crate::config::PinRule {
        title_exact: None,
        bound_hwnd: None,
        process: None,
        title_contains: None,
        slot: 0,
    };
    assert!(!rule.matches("pwsh.exe", "Build"));
    rule.title_contains = Some(String::new());
    assert!(!rule.matches("pwsh.exe", "Build"));
    rule.title_contains = Some("BUILD".into());
    assert!(rule.matches("pwsh.exe", "Project build workspace"));
    assert!(!rule.matches("pwsh.exe", "Other workspace"));
    rule.process = Some("PWSH.EXE".into());
    assert!(rule.matches("pwsh.exe", "Build"));
    assert!(!rule.matches("code.exe", "Build"));
    rule.title_exact = Some("Build".into());
    assert!(rule.matches("pwsh.exe", "Build"));
    assert!(!rule.matches("pwsh.exe", "build"));
    rule.process = None;
    rule.title_contains = None;
    assert!(rule.matches("anything.exe", "Build"));
}

#[test]
fn tiny_work_areas_and_invalid_grid_dimensions_never_emit_nonpositive_slots() {
    use crate::layout::{compute_weighted_grid, LayoutPreset};
    let mut area = crate::monitor::Rect {
        x: -10,
        y: 20,
        w: 1920,
        h: 1080,
    };
    for (cols, rows, width, height) in [
        (2, 0, 1920, 1080),
        (2, 9, 1920, 1080),
        (2, 2, 1, 1080),
        (2, 2, 1920, 1),
    ] {
        area.w = width;
        area.h = height;
        assert!(compute_weighted_grid(cols, rows, &area, 64, &[], &[]).is_empty());
    }
    for (width, height) in [(7, 1080), (1920, 7), (-1, 1080), (1920, 0)] {
        area.w = width;
        area.h = height;
        assert!(LayoutPreset::LeftRight.compute_slots(&area, 64).is_empty());
    }
    area.w = 2;
    area.h = 2;
    let slots = compute_weighted_grid(2, 2, &area, 64, &[], &[]);
    assert_eq!(slots.len(), 4);
    assert!(slots.iter().all(|slot| slot.w == 1 && slot.h == 1));
}

#[test]
fn duplicate_bound_pins_are_reported_and_an_unknown_drag_target_preserves_order() {
    let mut app = fixture();
    let rule = crate::config::PinRule {
        title_exact: None,
        bound_hwnd: Some(1),
        process: Some("WindowsTerminal.exe".into()),
        title_contains: None,
        slot: 0,
    };
    app.config.pin = vec![rule.clone(), rule];
    let plan = app.assignment();
    assert_eq!(plan.rule_windows, [Some(0), None]);
    assert_eq!(
        plan.warnings,
        ["Duplicate pin for Terminal · build workspace"]
    );
    assert_eq!(
        plan.slots
            .iter()
            .filter(|window| **window == Some(0))
            .count(),
        1
    );
    let original = app
        .managed_windows
        .iter()
        .map(|w| w.hwnd)
        .collect::<Vec<_>>();
    assert!(!crate::order::move_relative(
        &mut app.managed_windows,
        1,
        999,
        true
    ));
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|w| w.hwnd)
            .collect::<Vec<_>>(),
        original
    );
    app.detail_tab = 1;
    let ctx = egui::Context::default();
    let output = frame(&ctx, &mut app, egui::vec2(1080.0, 1200.0), vec![]);
    assert!(visible_text_position(
        &ctx,
        &output,
        "Duplicate pin for Terminal · build workspace"
    )
    .is_some());
}

#[test]
fn viewing_an_out_of_range_pin_keeps_its_reservation_and_reports_the_problem() {
    let mut app = fixture();
    app.detail_tab = 1;
    app.config.pin.push(crate::config::PinRule {
        title_exact: Some(app.managed_windows[0].title.clone()),
        bound_hwnd: Some(app.managed_windows[0].hwnd),
        process: Some(app.managed_windows[0].process_name.clone()),
        title_contains: None,
        slot: 99,
    });
    let ctx = egui::Context::default();
    let size = egui::vec2(1200.0, 1400.0);
    let _ = frame(&ctx, &mut app, size, vec![]);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert_eq!(app.config.pin[0].slot, 99);
    assert!(visible_text_position(&ctx, &output, "Slot unavailable in this layout").is_some());
}

#[test]
fn about_opens_from_the_header_and_returns_to_the_workspace_at_every_size() {
    for size in [
        egui::vec2(280.0, 300.0),
        egui::vec2(480.0, 760.0),
        egui::vec2(1080.0, 800.0),
    ] {
        for scale in [0.75, 1.0, 1.5] {
            let ctx = egui::Context::default();
            let mut app = fixture();
            app.config.defaults.ui_scale = scale;
            let original = toml::to_string(&app.config).unwrap();
            let original_weights = (app.col_weights.clone(), app.row_weights.clone());
            for _ in 0..4 {
                let _ = frame(&ctx, &mut app, size, vec![]);
            }
            let clip = ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("test-workspace-clip")))
                .unwrap();
            let _ = frame(
                &ctx,
                &mut app,
                size,
                vec![
                    Event::PointerMoved(clip.center()),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -10000.0),
                        modifiers: Default::default(),
                    },
                ],
            );
            for _ in 0..3 {
                let _ = frame(&ctx, &mut app, size, vec![]);
            }
            let workspace = frame(&ctx, &mut app, size, vec![]);
            assert!(
                visible_text_position(&ctx, &workspace, "by tront.xyz").is_some(),
                "header credit hidden at {size:?}, scale {scale}"
            );
            let output = click_text(&ctx, &mut app, size, "About");
            assert_eq!(app.detail_tab, 3);
            assert!(
                visible_text_position(
                    &ctx,
                    &output,
                    &format!("PowerShellManager {}", env!("CARGO_PKG_VERSION"))
                )
                .is_some(),
                "About heading hidden at {size:?}, scale {scale}"
            );
            assert!(
                visible_text_position(&ctx, &output, "Built by Trent Sterling").is_some(),
                "About author hidden at {size:?}, scale {scale}"
            );
            let _ = click_text(&ctx, &mut app, size, "Back to workspace");
            assert_eq!(app.detail_tab, 0);
            assert_eq!(toml::to_string(&app.config).unwrap(), original);
            assert_eq!(
                (app.col_weights.clone(), app.row_weights.clone()),
                original_weights
            );
            assert!(!app.native_enabled);
            assert!(!app.history.lock().unwrap().can_undo());
        }
    }
}

#[test]
fn about_shows_a_verified_update_and_emits_its_download_link_without_opening_a_browser() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.detail_tab = 3;
    let size = Vec2::new(1080.0, 1600.0);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(visible_text_position(&ctx, &output, "Built by Trent Sterling").is_some());
    assert!(visible_text_position(&ctx, &output, "Download 9.0.0").is_none());
    *app.update_info.lock().unwrap() = Some(crate::updates::UpdateInfo {
        latest_version: "9.0.0".into(),
        download_url: "https://github.com/TrentSterling/powershellmanager/releases/latest".into(),
    });
    let output = click_text(&ctx, &mut app, size, "Download 9.0.0");
    assert!(output.platform_output.commands.iter().any(|command| matches!(command,
        egui::OutputCommand::OpenUrl(url) if url.url == "https://github.com/TrentSterling/powershellmanager/releases/latest"
    )));
    let _ = std::panic::catch_unwind(|| {
        let _guard = app.update_info.lock().unwrap();
        panic!("owned update state poison");
    });
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(visible_text_position(&ctx, &output, "Built by Trent Sterling").is_some());
    assert!(visible_text_position(&ctx, &output, "Download 9.0.0").is_none());
}

pub(crate) fn frame(
    ctx: &egui::Context,
    app: &mut PsmApp,
    size: Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| gui::draw(ctx, app),
    )
}

fn pointer(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

pub(crate) fn visible_text_position(
    ctx: &egui::Context,
    output: &egui::FullOutput,
    label: &str,
) -> Option<Pos2> {
    let text_fields: Vec<_> = ["theme-name", "theme-code"]
        .into_iter()
        .filter_map(|id| {
            ctx.read_response(egui::Id::new(id))
                .map(|response| response.rect)
        })
        .collect();
    fn find(
        shape: &egui::Shape,
        clip: egui::Rect,
        label: &str,
        text_fields: &[egui::Rect],
    ) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let center = text.pos + text.galley.size() * 0.5;
                (clip.contains(center) && !text_fields.iter().any(|rect| rect.contains(center)))
                    .then_some(center)
            }
            egui::Shape::Vec(shapes) => shapes
                .iter()
                .rev()
                .find_map(|shape| find(shape, clip, label, text_fields)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| find(&shape.shape, shape.clip_rect, label, &text_fields))
}

fn scroll_studio(ctx: &egui::Context, app: &mut PsmApp, size: Vec2, delta: f32) {
    let rect = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("psm-theme-studio")))
        .unwrap();
    let _ = frame(
        ctx,
        app,
        size,
        vec![
            Event::PointerMoved(rect.center()),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta),
                modifiers: Default::default(),
            },
        ],
    );
    for _ in 0..20 {
        let _ = frame(ctx, app, size, vec![]);
    }
}

pub(crate) fn click_text(
    ctx: &egui::Context,
    app: &mut PsmApp,
    size: Vec2,
    label: &str,
) -> egui::FullOutput {
    let _ = frame(ctx, app, size, vec![]);
    let mut output = frame(ctx, app, size, vec![]);
    if app.show_theme_studio && visible_text_position(ctx, &output, label).is_none() {
        scroll_studio(ctx, app, size, 10000.0);
        for _ in 0..12 {
            output = frame(ctx, app, size, vec![]);
            if visible_text_position(ctx, &output, label).is_some() {
                break;
            }
            scroll_studio(ctx, app, size, -180.0);
        }
    }
    let position = visible_text_position(ctx, &output, label)
        .unwrap_or_else(|| panic!("Visible control text not found: {label}"));
    let _ = frame(ctx, app, size, vec![Event::PointerMoved(position)]);
    let _ = frame(ctx, app, size, vec![pointer(position, true)]);
    let mut output = frame(ctx, app, size, vec![pointer(position, false)]);
    output.append(frame(ctx, app, size, vec![]));
    output
}

#[test]
fn actual_layout_controls_save_reload_delete_and_reset_without_moving_windows() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    let size = egui::vec2(1080.0, 1200.0);
    app.disabled_cells.insert(1);
    let windows = app
        .managed_windows
        .iter()
        .map(|w| w.hwnd)
        .collect::<Vec<_>>();
    let _ = click_text(&ctx, &mut app, size, "Enable all");
    assert!(app.disabled_cells.is_empty());
    let _ = click_text(&ctx, &mut app, size, "Equalize");
    assert!(app.weights_are_uniform());
    let _ = click_text(&ctx, &mut app, size, "Save this grid");
    assert!(app.show_save_dialog);
    let _ = click_text(&ctx, &mut app, size, "Name this layout");
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Text("Saved audit grid".into())],
    );
    let _ = click_text(&ctx, &mut app, size, "Save layout");
    assert!(!app.show_save_dialog);
    assert_eq!(app.config.saved_grid.len(), 1);
    assert_eq!(app.config.saved_grid[0].name, "Saved audit grid");
    app.col_weights = vec![0.8, 0.2];
    let _ = click_text(&ctx, &mut app, size, "Saved audit grid");
    assert!(app.weights_are_uniform());
    let _ = click_text(&ctx, &mut app, size, "x");
    assert!(app.config.saved_grid.is_empty());
    let _ = click_text(&ctx, &mut app, size, "Presets");
    assert!(!app.use_custom);
    let _ = click_text(&ctx, &mut app, size, "focus:3");
    assert_eq!(
        app.active_preset(),
        crate::layout::LayoutPreset::Focus { side_count: 3 }
    );
    let _ = click_text(&ctx, &mut app, size, "Terminals");
    assert_eq!(app.config.defaults.target, "terminals");
    app.config.defaults.manual_order = true;
    let _ = click_text(&ctx, &mut app, size, "Rank by activity");
    assert!(app.config.defaults.smart_sort);
    assert!(!app.config.defaults.manual_order);
    app.config.defaults.manual_order = true;
    let _ = click_text(&ctx, &mut app, size, "Reset order");
    assert!(!app.config.defaults.manual_order);
    let before_apply = app.status.clone();
    let _ = click_text(&ctx, &mut app, size, "Apply layout");
    assert_eq!(app.status, before_apply, "preview Apply must stay disabled");
    app.apply_current_layout();
    assert_eq!(app.status, "Preview only. No windows moved.");
    for label in ["Minimize all", "Restore all", "Refresh"] {
        let _ = click_text(&ctx, &mut app, size, label);
    }
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|w| w.hwnd)
            .collect::<Vec<_>>(),
        windows
    );
    assert!(!app.history.lock().unwrap().can_undo());
}

#[test]
fn saved_grid_names_cannot_replace_builtin_presets() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.custom_cols = 3;
    app.custom_rows = 1;
    app.ensure_weights();
    app.save_current_as_grid("2x2 Grid".into());
    let size = egui::vec2(1080.0, 1200.0);
    let _ = click_text(&ctx, &mut app, size, "Presets");
    let _ = click_text(&ctx, &mut app, size, "2x2");
    assert!(
        !app.use_custom,
        "a saved-grid name must not change a builtin preset's identity"
    );
    assert_eq!(
        app.active_preset(),
        crate::layout::LayoutPreset::Grid { cols: 2, rows: 2 }
    );
}

#[test]
fn minimum_window_keeps_preview_cells_and_primary_controls_reachable() {
    for scale in [0.75, 1.0, 1.5] {
        let ctx = egui::Context::default();
        let mut app = fixture();
        app.config.defaults.ui_scale = scale;
        let size = egui::vec2(280.0, 300.0);
        for _ in 0..3 {
            let _ = frame(&ctx, &mut app, size, vec![]);
        }
        let screen = egui::Rect::from_min_size(Pos2::ZERO, size);
        let undo = ctx
            .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("test-undo-button")))
            .unwrap();
        for index in 0..4 {
            let cell = preview_cell(&ctx, index);
            assert!(
                screen.contains(cell.min) && screen.contains(cell.max),
                "cell {index} must fit the minimum viewport: {cell:?}"
            );
            assert!(
                cell.bottom() < undo.top(),
                "scale {scale}: cell {index} {cell:?} must stay above the action bar {undo:?}; screen {:?}", ctx.screen_rect()
            );
        }
        let position = preview_cell(&ctx, 0).center();
        let _ = frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerMoved(position), pointer(position, true)],
        );
        let _ = frame(&ctx, &mut app, size, vec![pointer(position, false)]);
        assert!(app.disabled_cells.contains(&0));
        let before_apply = app.status.clone();
        let _ = click_text(&ctx, &mut app, size, "Apply");
        assert_eq!(app.status, before_apply, "preview Apply must stay disabled");
        let _ = click_text(&ctx, &mut app, size, "Theme Studio");
        assert!(app.show_theme_studio);
        assert!(!app.native_enabled);
    }
}

fn replace_text_field(ctx: &egui::Context, app: &mut PsmApp, size: Vec2, id: &str, value: &str) {
    if app.show_theme_studio {
        for _ in 0..20 {
            let _ = frame(ctx, app, size, vec![]);
            let response = ctx
                .read_response(egui::Id::new(id))
                .expect("text field exists");
            if response.interact_rect.contains(response.rect.center())
                && ctx.screen_rect().contains(response.rect.center())
            {
                break;
            }
            let studio = ctx
                .memory(|memory| memory.area_rect(egui::Id::new("psm-theme-studio")))
                .unwrap();
            let delta = if response.rect.center().y < studio.center().y {
                80.0
            } else {
                -80.0
            };
            scroll_studio(ctx, app, size, delta);
        }
    }
    let response = ctx
        .read_response(egui::Id::new(id))
        .expect("text field exists");
    let position = response.rect.center();
    assert!(
        response.interact_rect.contains(position) && ctx.screen_rect().contains(position),
        "text field {id} must be reachable by pointer: {:?}",
        response
    );
    let _ = frame(
        ctx,
        app,
        size,
        vec![Event::PointerMoved(position), pointer(position, true)],
    );
    let _ = frame(ctx, app, size, vec![pointer(position, false)]);
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    let _ = frame(
        ctx,
        app,
        size,
        vec![
            Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
            Event::Text(value.into()),
        ],
    );
}

pub(crate) fn edit_text_at(
    ctx: &egui::Context,
    app: &mut PsmApp,
    size: Vec2,
    position: Pos2,
    value: &str,
) {
    for _ in 0..2 {
        let _ = frame(
            ctx,
            app,
            size,
            vec![Event::PointerMoved(position), pointer(position, true)],
        );
        let _ = frame(ctx, app, size, vec![pointer(position, false)]);
    }
    let _ = frame(
        ctx,
        app,
        size,
        vec![
            Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            },
            Event::Text(value.into()),
            Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            },
        ],
    );
}

#[test]
fn actual_grid_dimension_controls_resize_weights_clear_disabled_cells_and_save() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    let size = egui::vec2(1080.0, 1200.0);
    app.disabled_cells.insert(3);
    let _ = frame(&ctx, &mut app, size, vec![]);
    let output = frame(&ctx, &mut app, size, vec![]);
    let cols = visible_text_position(&ctx, &output, "Cols").unwrap() + egui::vec2(35.0, 0.0);
    edit_text_at(&ctx, &mut app, size, cols, "3");
    assert_eq!(app.custom_cols, 3);
    assert_eq!(app.col_weights, vec![1.0 / 3.0; 3]);
    assert!(app.disabled_cells.is_empty());
    let output = frame(&ctx, &mut app, size, vec![]);
    let rows = visible_text_position(&ctx, &output, "Rows").unwrap() + egui::vec2(35.0, 0.0);
    edit_text_at(&ctx, &mut app, size, rows, "1");
    assert_eq!(app.custom_rows, 1);
    assert_eq!(app.row_weights, [1.0]);
    assert_eq!(app.active_preset().slot_count(), 3);
    assert_eq!(app.config.defaults.custom_cols, 3);
    assert_eq!(app.config.defaults.custom_rows, 1);
    assert_eq!(app.config.defaults.col_weights, app.col_weights);
    assert_eq!(app.config.defaults.row_weights, app.row_weights);
}

#[test]
fn actual_search_matches_titles_and_processes_without_changing_the_apply_queue() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    let size = egui::vec2(1080.0, 1600.0);
    let windows = app
        .managed_windows
        .iter()
        .map(|w| w.hwnd)
        .collect::<Vec<_>>();
    let assignment = app.assignment().slots;
    let output = frame(&ctx, &mut app, size, vec![]);
    let position = visible_text_position(&ctx, &output, "Search windows or apps").unwrap();
    edit_text_at(&ctx, &mut app, size, position, "FIREFOX.EXE");
    assert_eq!(app.window_query, "FIREFOX.EXE");
    let mut output = frame(&ctx, &mut app, size, vec![]);
    assert!(
        visible_text_position(&ctx, &output, "Browser · documentation")
            .is_some_and(|row| row.y > position.y)
    );
    assert!(
        visible_text_position(&ctx, &output, "Editor · project workspace")
            .is_none_or(|row| row.y < position.y)
    );
    for (query, expected) in [
        ("日本語", "Notes · 日本語 / café / 🎨"),
        ("no matching app", "No windows match your search."),
    ] {
        let position = visible_text_position(&ctx, &output, &app.window_query).unwrap();
        edit_text_at(&ctx, &mut app, size, position, query);
        output = frame(&ctx, &mut app, size, vec![]);
        assert!(
            visible_text_position(&ctx, &output, expected).is_some(),
            "search {query}"
        );
    }
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|w| w.hwnd)
            .collect::<Vec<_>>(),
        windows
    );
    assert_eq!(app.assignment().slots, assignment);
    app.window_query.clear();
    app.theme_settings.zebra_strength = 0.0;
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(visible_text_position(&ctx, &output, "Editor · project workspace").is_some());
}

#[test]
fn actual_theme_peg_numeric_edit_and_drag_preserve_colors_and_layout() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.show_theme_studio = true;
    let size = egui::vec2(1300.0, 2000.0);
    let colors = app.theme_settings.stops.map(|stop| stop.color);
    let weights = app.col_weights.clone();
    let _ = frame(&ctx, &mut app, size, vec![]);
    let output = frame(&ctx, &mut app, size, vec![]);
    let position = visible_text_position(&ctx, &output, "0%").unwrap();
    edit_text_at(&ctx, &mut app, size, position, "20");
    assert!((app.theme_settings.stops[0].position - 0.2).abs() < 0.001);
    let output = frame(&ctx, &mut app, size, vec![]);
    let color = egui::Color32::from_rgb(colors[1][0], colors[1][1], colors[1][2]);
    let peg = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle) if circle.radius == 7.0 && circle.fill == color => {
                Some(circle.center)
            }
            _ => None,
        })
        .unwrap();
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(peg), pointer(peg, true)],
    );
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(peg + egui::vec2(3.0, 0.0))],
    );
    let end = peg + egui::vec2(55.0, 0.0);
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(end)]);
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerGone]);
    assert_eq!(app.theme_settings.stops.map(|stop| stop.color), colors);
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(end)]);
    let _ = frame(&ctx, &mut app, size, vec![pointer(end, false)]);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(visible_text_position(&ctx, &output, "Peg 2").is_some());
    assert!(app.theme_settings.stops[1].position > 0.43);
    assert!(app
        .theme_settings
        .stops
        .windows(2)
        .all(|pair| pair[1].position > pair[0].position));
    assert_eq!(app.theme_settings.stops.map(|stop| stop.color), colors);
    assert_eq!(app.col_weights, weights);
    assert_eq!(
        ThemeSettings::decode(app.config.defaults.theme_code.as_ref().unwrap()),
        Some(app.theme_settings)
    );
    let settings = app.theme_settings;
    app.studio.undo(&mut app.theme_settings);
    assert_eq!(
        app.theme_settings, settings,
        "Undo without a roll must keep later edits"
    );
}

#[test]
fn actual_theme_scale_edit_and_peg_click_preserve_colors_and_layout() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.show_theme_studio = true;
    let size = egui::vec2(1300.0, 2000.0);
    let theme = app.theme_settings;
    let weights = app.col_weights.clone();
    let _ = frame(&ctx, &mut app, size, vec![]);
    let output = frame(&ctx, &mut app, size, vec![]);
    let label = visible_text_position(&ctx, &output, "UI scale").unwrap();
    let value = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text)
                if text.galley.text().trim().parse::<f32>().ok() == Some(1.0)
                    && (text.pos.y - label.y).abs() < 12.0 =>
            {
                Some(text.pos + text.galley.rect.size() * 0.5)
            }
            _ => None,
        })
        .unwrap();
    edit_text_at(&ctx, &mut app, size, value, "1.25");
    assert_eq!(app.config.defaults.ui_scale, 1.25);
    assert_eq!(app.theme_settings, theme);
    assert_eq!(app.col_weights, weights);
    let output = frame(&ctx, &mut app, size, vec![]);
    let [r, g, b] = theme.stops[2].color;
    let peg = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Circle(circle)
                if circle.radius == 7.0 && circle.fill == egui::Color32::from_rgb(r, g, b) =>
            {
                Some(circle.center)
            }
            _ => None,
        })
        .unwrap();
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(peg), pointer(peg, true)],
    );
    let _ = frame(&ctx, &mut app, size, vec![pointer(peg, false)]);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(visible_text_position(&ctx, &output, "Peg 3").is_some());
    assert_eq!(app.theme_settings, theme);
    assert_eq!(app.col_weights, weights);
}

#[test]
fn invalid_saved_theme_reports_the_problem_and_keeps_current_preferences() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.show_theme_studio = true;
    app.config.saved_theme.push(crate::config::NamedTheme {
        name: "Broken stored theme".into(),
        code: "broken saved document".into(),
    });
    let original = app.theme_settings;
    let weights = app.col_weights.clone();
    let size = egui::vec2(1300.0, 2000.0);
    let _ = click_text(&ctx, &mut app, size, "Saved themes & sharing");
    let _ = click_text(&ctx, &mut app, size, "Broken stored theme");
    scroll_studio(&ctx, &mut app, size, -10000.0);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert_eq!(app.theme_settings, original);
    assert_eq!(app.col_weights, weights);
    assert_eq!(app.config.saved_theme.len(), 1);
    assert!(visible_text_position(
        &ctx,
        &output,
        "Invalid saved theme. Your current theme was kept."
    )
    .is_some());
}

#[test]
fn actual_theme_menus_gradient_controls_and_presets_preserve_the_layout() {
    use crate::theme::typography::FontChoice;
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.show_theme_studio = true;
    app.disabled_cells.insert(3);
    let layout = (
        app.col_weights.clone(),
        app.row_weights.clone(),
        app.disabled_cells.clone(),
    );
    let size = egui::vec2(1300.0, 2000.0);
    let _ = click_text(&ctx, &mut app, size, "Surprise me");
    let output = click_text(&ctx, &mut app, size, "Neon");
    assert!(visible_text_position(&ctx, &output, "Neon").is_some());
    let _ = click_text(&ctx, &mut app, size, "ColorMagic");
    scroll_studio(&ctx, &mut app, size, -10000.0);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(visible_text_position(
        &ctx,
        &output,
        "Neon palette. Your frost, font and peg positions stay put."
    )
    .is_some());
    let _ = click_text(&ctx, &mut app, size, "Undo");

    app.theme_settings.stops[1].position = 0.12;
    app.theme_settings.stops[2].position = 0.91;
    let colors = app.theme_settings.stops.map(|stop| stop.color);
    let _ = click_text(&ctx, &mut app, size, "Space evenly");
    assert_eq!(
        app.theme_settings.stops.map(|stop| stop.position),
        [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]
    );
    assert_eq!(app.theme_settings.stops.map(|stop| stop.color), colors);
    let _ = click_text(&ctx, &mut app, size, "Reverse");
    assert_eq!(
        app.theme_settings.stops.map(|stop| stop.color),
        [colors[3], colors[2], colors[1], colors[0]]
    );
    assert!(app
        .theme_settings
        .stops
        .windows(2)
        .all(|pair| pair[0].position <= pair[1].position));

    for font in [
        FontChoice::Rajdhani,
        FontChoice::RajdhaniBold,
        FontChoice::Mono,
        FontChoice::Sans,
    ] {
        let label = app.theme_settings.font.label();
        let _ = click_text(&ctx, &mut app, size, label);
        let _ = click_text(&ctx, &mut app, size, font.label());
        assert_eq!(app.theme_settings.font, font);
        assert_eq!(
            ThemeSettings::decode(app.config.defaults.theme_code.as_ref().unwrap()),
            Some(app.theme_settings)
        );
    }
    let _ = click_text(&ctx, &mut app, size, "Bright outlines");
    assert!(app.theme_settings.high_contrast);
    let _ = click_text(&ctx, &mut app, size, "Color the workspace");
    assert!(!app.theme_settings.gradient_enabled);
    let _ = click_text(&ctx, &mut app, size, "Tront presets");
    for (name, expected) in ThemeSettings::presets() {
        let _ = click_text(&ctx, &mut app, size, name);
        assert_eq!(app.theme_settings, expected, "preset {name}");
        assert_eq!(
            ThemeSettings::decode(app.config.defaults.theme_code.as_ref().unwrap()),
            Some(expected)
        );
    }
    assert_eq!(
        (
            app.col_weights.clone(),
            app.row_weights.clone(),
            app.disabled_cells.clone()
        ),
        layout
    );
    assert!(app.use_custom);
    assert!(!app.native_enabled);
}

#[test]
fn about_license_and_credits_expand_to_the_bundled_notices() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.detail_tab = 3;
    let size = egui::vec2(1080.0, 2000.0);
    let _ = click_text(&ctx, &mut app, size, "Theme engine credits");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == include_str!("../assets/theme-engine-NOTICE"))));
    let _ = click_text(&ctx, &mut app, size, "Font license");
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
        egui::Shape::Text(text) if text.galley.text() == theme::typography::LICENSE)));
}

#[test]
fn monitor_resolution_uses_the_primary_or_first_display_for_invalid_selection() {
    use crate::monitor::{resolve_monitor, MonitorInfo, Rect};
    let mut monitors = vec![
        MonitorInfo {
            index: 0,
            is_primary: false,
            work_area: Rect {
                x: -1920,
                y: 0,
                w: 1920,
                h: 1040,
            },
        },
        MonitorInfo {
            index: 1,
            is_primary: true,
            work_area: Rect {
                x: 0,
                y: 0,
                w: 1080,
                h: 1880,
            },
        },
    ];
    for (spec, expected) in [
        ("primary", 1),
        ("", 1),
        ("0", 0),
        ("1", 1),
        ("99", 0),
        ("invalid", 1),
    ] {
        assert!(
            std::ptr::eq(resolve_monitor(&monitors, spec), &monitors[expected]),
            "selection {spec:?}"
        );
    }
    monitors[1].is_primary = false;
    for spec in ["primary", "", "invalid", "99"] {
        assert!(std::ptr::eq(resolve_monitor(&monitors, spec), &monitors[0]));
    }
}

#[test]
fn actual_theme_controls_roll_undo_save_replace_load_copy_and_validate_imports() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.show_theme_studio = true;
    let size = egui::vec2(1300.0, 2000.0);
    let original = app.theme_settings;
    let _ = click_text(&ctx, &mut app, size, "ColorMagic");
    assert_ne!(app.theme_settings, original);
    let _ = click_text(&ctx, &mut app, size, "Undo");
    assert_eq!(app.theme_settings, original);
    let _ = click_text(&ctx, &mut app, size, "Light");
    assert!(!app.theme_settings.dark);
    let _ = click_text(&ctx, &mut app, size, "Dark");
    assert!(app.theme_settings.dark);
    let _ = click_text(&ctx, &mut app, size, "Saved themes & sharing");
    for _ in 0..20 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    replace_text_field(&ctx, &mut app, size, "theme-name", "Audit theme");
    let _ = click_text(&ctx, &mut app, size, "Save");
    assert_eq!(app.config.saved_theme.len(), 1);
    assert_eq!(app.config.saved_theme[0].name, "Audit theme");
    let before = app.theme_settings;
    let _ = click_text(&ctx, &mut app, size, "Reverse");
    assert_ne!(app.theme_settings.stops, before.stops);
    let _ = click_text(&ctx, &mut app, size, "Audit theme");
    assert_eq!(app.theme_settings, before);
    let _ = click_text(&ctx, &mut app, size, "Reverse");
    let changed = app.theme_settings;
    let _ = click_text(&ctx, &mut app, size, "Save");
    assert_eq!(app.config.saved_theme.len(), 1);
    assert_eq!(
        ThemeSettings::decode(&app.config.saved_theme[0].code),
        Some(changed)
    );
    let copied = click_text(&ctx, &mut app, size, "Copy theme");
    assert!(copied
        .platform_output
        .commands
        .iter()
        .any(|command| matches!(command,
        egui::OutputCommand::CopyText(code) if ThemeSettings::decode(code) == Some(changed))));
    let _ = frame(&ctx, &mut app, size, vec![]);
    let response = ctx.read_response(egui::Id::new("theme-code")).unwrap();
    assert!(response.rect.is_positive());
    let _ = click_text(&ctx, &mut app, size, "Import");
    assert_eq!(app.theme_settings, changed);
    replace_text_field(&ctx, &mut app, size, "theme-code", "bad theme");
    let _ = click_text(&ctx, &mut app, size, "Import");
    assert_eq!(app.theme_settings, changed);
    let _ = click_text(&ctx, &mut app, size, "Delete");
    assert!(app.config.saved_theme.is_empty());
    assert!(app.config.defaults.theme_code.is_some());
    assert!(app.use_custom);
    assert!(!app.native_enabled);
}

fn preview_cell(ctx: &egui::Context, index: usize) -> egui::Rect {
    ctx.data(|d| d.get_temp(egui::Id::new(("test-preview-cell", index))))
        .unwrap()
}

#[test]
fn undo_control_uses_shared_history_and_preview_cannot_restore_windows() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    let size = egui::vec2(1080.0, 800.0);
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let button = ctx
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("test-undo-button")))
        .unwrap()
        .center();
    let original_status = app.status.clone();
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(button), pointer(button, true)],
    );
    let _ = frame(&ctx, &mut app, size, vec![pointer(button, false)]);
    assert_eq!(
        app.status, original_status,
        "empty history must disable Undo"
    );
    app.history
        .lock()
        .unwrap()
        .record(vec![crate::windows::WindowSnapshot {
            hwnd: 123,
            process_id: 99,
            visible: false,
            placement: Default::default(),
        }]);
    let _ = frame(&ctx, &mut app, size, vec![]);
    let _ = frame(&ctx, &mut app, size, vec![pointer(button, true)]);
    let _ = frame(&ctx, &mut app, size, vec![pointer(button, false)]);
    assert_eq!(
        app.status, original_status,
        "preview Undo must stay disabled even with history"
    );
    app.undo_layout();
    assert_eq!(app.status, "Preview only. No windows restored.");
    assert!(app.history.lock().unwrap().can_undo());
}
#[test]
fn preview_matches_selected_work_area_and_preserves_aspect_at_every_window_size() {
    for (width, height) in [(1920, 1040), (1080, 1880), (3440, 1400)] {
        for size in [
            egui::vec2(1080.0, 800.0),
            egui::vec2(480.0, 760.0),
            egui::vec2(280.0, 300.0),
        ] {
            let ctx = egui::Context::default();
            let mut app = fixture();
            app.monitors = vec![
                crate::monitor::MonitorInfo {
                    index: 0,
                    is_primary: true,
                    work_area: crate::monitor::Rect {
                        x: 0,
                        y: 0,
                        w: 1600,
                        h: 900,
                    },
                },
                crate::monitor::MonitorInfo {
                    index: 1,
                    is_primary: false,
                    work_area: crate::monitor::Rect {
                        x: -width,
                        y: 40,
                        w: width,
                        h: height,
                    },
                },
            ];
            app.config.defaults.monitor = "1".into();
            app.config.defaults.gap = 32;
            for _ in 0..3 {
                let _ = frame(&ctx, &mut app, size, vec![]);
            }
            let rect = ctx
                .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("test-preview-rect")))
                .unwrap();
            let ratio = (rect.width() - 12.0) / (rect.height() - 12.0);
            assert!(
                (ratio - width as f32 / height as f32).abs() < 0.001,
                "selected {width} x {height} display was stretched to {ratio} at {size:?}"
            );
            let first = preview_cell(&ctx, 0);
            let second = preview_cell(&ctx, 1);
            let last = preview_cell(&ctx, 3);
            let scale = (rect.width() - 12.0) / width as f32;
            assert!((second.left() - first.right() - 32.0 * scale).abs() < 0.001);
            assert!((first.left() - rect.left() - 6.0).abs() < 0.001);
            assert!((last.right() - rect.right() + 6.0).abs() < 0.001);
            assert!((last.bottom() - rect.bottom() + 6.0).abs() < 0.001);
        }
    }
}

#[test]
fn divider_drag_tracks_pointer_with_large_gaps_and_keeps_minimum_cell_weights() {
    for (row_axis, index, grab_offset) in [
        (false, 0, 0.0),
        (false, 2, 3.0),
        (true, 0, 0.0),
        (true, 2, 3.0),
    ] {
        let ctx = egui::Context::default();
        let mut app = fixture();
        app.custom_cols = 4;
        app.custom_rows = 4;
        app.col_weights = vec![0.10, 0.10, 0.40, 0.40];
        app.row_weights = app.col_weights.clone();
        app.config.defaults.gap = 32;
        app.disabled_cells.insert(15);
        let size = egui::vec2(1080.0, 800.0);
        for _ in 0..3 {
            let _ = frame(&ctx, &mut app, size, vec![]);
        }
        let first_index = if row_axis { index * 4 } else { index };
        let adjacent_index = first_index + if row_axis { 4 } else { 1 };
        let first = preview_cell(&ctx, first_index);
        let adjacent = preview_cell(&ctx, adjacent_index);
        let start = if row_axis {
            egui::pos2(
                first.left() + 20.0,
                (first.bottom() + adjacent.top()) * 0.5 + grab_offset,
            )
        } else {
            egui::pos2(
                (first.right() + adjacent.left()) * 0.5 + grab_offset,
                first.top() + 20.0,
            )
        };
        let _ = frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerMoved(start), pointer(start, true)],
        );
        let delta = if row_axis {
            egui::vec2(0.0, 8.0)
        } else {
            egui::vec2(20.0, 0.0)
        };
        let end = start + delta;
        let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(end)]);
        let _ = frame(&ctx, &mut app, size, vec![]);
        let first = preview_cell(&ctx, first_index);
        let adjacent = preview_cell(&ctx, adjacent_index);
        let boundary = if row_axis {
            (first.bottom() + adjacent.top()) * 0.5
        } else {
            (first.right() + adjacent.left()) * 0.5
        };
        let pointer_coordinate = (if row_axis { end.y } else { end.x }) - grab_offset;
        assert!(
            (boundary - pointer_coordinate).abs() < 0.6,
            "divider at {boundary} did not track pointer at {pointer_coordinate}"
        );
        let far = start
            + if row_axis {
                egui::vec2(0.0, 400.0)
            } else {
                egui::vec2(900.0, 0.0)
            };
        let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(far)]);
        let _ = frame(&ctx, &mut app, size, vec![pointer(far, false)]);
        let weights = if row_axis {
            &app.row_weights
        } else {
            &app.col_weights
        };
        let original = [0.10, 0.10, 0.40, 0.40];
        assert!(
            (weights[index] - original[index] - original[index + 1] + 0.05).abs() < 0.0001,
            "{weights:?}"
        );
        assert!((weights[index + 1] - 0.05).abs() < 0.0001, "{weights:?}");
        for untouched in (0..4).filter(|i| *i != index && *i != index + 1) {
            assert_eq!(weights[untouched], original[untouched]);
        }
        assert!((weights.iter().sum::<f32>() - 1.0).abs() < 0.0001);
        assert_eq!(app.disabled_cells, [15].into_iter().collect());
        assert_eq!(app.config.defaults.col_weights, app.col_weights);
        assert_eq!(app.config.defaults.row_weights, app.row_weights);
        assert!(app.dragging_divider.is_none());
    }
}

#[test]
fn divider_drag_preserves_tiny_imported_pairs_without_invalid_weights() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.custom_cols = 4;
    app.col_weights = vec![0.02, 0.02, 0.46, 0.50];
    let size = egui::vec2(1080.0, 800.0);
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let first = preview_cell(&ctx, 0);
    let second = preview_cell(&ctx, 1);
    let start = egui::pos2((first.right() + second.left()) * 0.5, first.top() + 20.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(start), pointer(start, true)],
    );
    let end = start + egui::vec2(120.0, 0.0);
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(end)]);
    let _ = frame(&ctx, &mut app, size, vec![pointer(end, false)]);
    assert_eq!(app.col_weights, vec![0.02, 0.02, 0.46, 0.50]);
    assert_eq!(app.config.defaults.col_weights, app.col_weights);
    assert!(app.dragging_divider.is_none());
}

#[test]
fn actual_grid_click_and_divider_drag_preserve_layout_controls() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    let size = egui::vec2(1080.0, 800.0);
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let rect = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("test-preview-rect")))
        .unwrap();
    let cell = rect.min + egui::vec2(40.0, 40.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(cell), pointer(cell, true)],
    );
    let _ = frame(&ctx, &mut app, size, vec![pointer(cell, false)]);
    assert!(
        app.disabled_cells.contains(&0),
        "actual preview click must disable slot 1"
    );
    let _ = frame(&ctx, &mut app, size, vec![pointer(cell, true)]);
    let _ = frame(&ctx, &mut app, size, vec![pointer(cell, false)]);
    assert!(app.disabled_cells.is_empty());

    let divider = egui::pos2(
        rect.left() + 6.0 + (rect.width() - 12.0) * 0.62,
        rect.top() + 40.0,
    );
    let before = app.col_weights.clone();
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(divider), pointer(divider, true)],
    );
    // Small initial motion acquires the divider before moving beyond its hit area.
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(divider + egui::vec2(2.0, 0.0))],
    );
    let end = divider + egui::vec2(55.0, 0.0);
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(end)]);
    let _ = frame(&ctx, &mut app, size, vec![pointer(end, false)]);
    assert!(
        app.col_weights[0] > before[0] + 0.04,
        "divider drag must change column widths: {:?}",
        app.col_weights
    );
    assert!((app.col_weights.iter().sum::<f32>() - 1.0).abs() < 0.0001);
    assert_eq!(app.config.defaults.col_weights, app.col_weights);
    assert!(app.disabled_cells.is_empty(), "drag must not toggle a cell");
}

#[test]
fn saved_layout_roundtrip_keeps_weights_disabled_cells_and_pins() {
    let mut app = fixture();
    app.toggle_cell(2);
    app.config.pin.push(crate::config::PinRule {
        process: Some("Code.exe".into()),
        title_contains: None,
        title_exact: None,
        bound_hwnd: None,
        slot: 1,
    });
    app.save_current_as_grid("Work".into());
    let encoded = toml::to_string(&app.config).unwrap();
    let config: Config = toml::from_str(&encoded).unwrap();
    let grid = config.saved_grid[0].clone();
    let mut restored = PsmApp::preview(config);
    restored.load_saved_grid(&grid);
    assert_eq!(restored.col_weights, vec![0.62, 0.38]);
    assert!(restored.disabled_cells.contains(&2));
    assert!(restored.config.pin[0].matches("code.EXE", "anything"));
    assert_eq!(restored.config.pin[0].slot, 1);
    restored.apply_current_layout();
    assert_eq!(restored.status, "Preview only. No windows moved.");
}

#[test]
fn hiding_the_preview_during_a_divider_drag_commits_on_return() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    let size = egui::vec2(1080.0, 800.0);
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let first = preview_cell(&ctx, 0);
    let second = preview_cell(&ctx, 1);
    let start = egui::pos2((first.right() + second.left()) * 0.5, first.top() + 20.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(start), pointer(start, true)],
    );
    let end = start + egui::vec2(55.0, 0.0);
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(end)]);
    assert!(app.dragging_divider.is_some());
    let changed = app.col_weights.clone();
    // The root viewport can hide while pressed. Its next input frame receives
    // the release while the preview widget is absent.
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
            events: vec![pointer(end, false)],
            ..Default::default()
        },
        |_| {},
    );
    let _ = frame(&ctx, &mut app, size, vec![]);
    assert!(
        app.dragging_divider.is_none(),
        "lost release must not leave the divider active"
    );
    assert_eq!(app.col_weights, changed);
    assert_eq!(app.config.defaults.col_weights, changed);
    assert!(app.disabled_cells.is_empty());
}

#[test]
fn old_config_retains_layout_and_new_theme_survives_restart() {
    let old = "[defaults]\ntheme=2\nuse_custom=true\ncustom_cols=3\ncustom_rows=1\ncol_weights=[0.5,0.3,0.2]\nrow_weights=[1.0]\n";
    let config: Config = toml::from_str(old).unwrap();
    let mut app = PsmApp::preview(config);
    assert_eq!(app.custom_cols, 3);
    assert_eq!(app.col_weights, vec![0.5, 0.3, 0.2]);
    assert_eq!(app.theme_settings.accent, theme::from_legacy(2).accent);
    app.studio.roll(&mut app.theme_settings, 778);
    app.theme_settings.frost = 0.12;
    app.theme_settings.move_stop(1, 0.23);
    app.config.defaults.theme_code = Some(app.theme_settings.encode());
    let persisted: Config = toml::from_str(&toml::to_string(&app.config).unwrap()).unwrap();
    let restarted = PsmApp::preview(persisted);
    assert_eq!(restarted.theme_settings, app.theme_settings.normalized());
    assert_eq!(restarted.col_weights, vec![0.5, 0.3, 0.2]);
}

#[test]
fn studio_undo_keeps_later_frost_font_and_peg_edits() {
    let mut app = fixture();
    let original = app.theme_settings;
    app.studio.roll(&mut app.theme_settings, 42);
    assert_ne!(original.accent, app.theme_settings.accent);
    app.theme_settings.frost = 0.31;
    app.theme_settings.move_stop(1, 0.22);
    app.theme_settings.font = theme::typography::FontChoice::Mono;
    app.studio.undo(&mut app.theme_settings);
    assert_eq!(app.theme_settings.accent, original.accent);
    assert_eq!(app.theme_settings.frost, 0.31);
    assert_eq!(app.theme_settings.stops[1].position, 0.22);
    assert_eq!(app.theme_settings.font, theme::typography::FontChoice::Mono);
}

#[test]
fn theme_studio_can_move_and_close_without_changing_layout() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    app.show_theme_studio = true;
    let size = egui::vec2(1280.0, 1000.0);
    for _ in 0..20 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let id = egui::Id::new("psm-theme-studio");
    let before = ctx.memory(|m| m.area_rect(id)).unwrap();
    let start = before.center_top() + egui::vec2(0.0, 16.0);
    let end = start + egui::vec2(-160.0, 25.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(start), pointer(start, true)],
    );
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(end)]);
    let _ = frame(&ctx, &mut app, size, vec![pointer(end, false)]);
    let output = frame(&ctx, &mut app, size, vec![]);
    let moved = ctx.memory(|m| m.area_rect(id)).unwrap();
    assert!(
        moved.left() < before.left() - 100.0,
        "Theme Studio must be draggable"
    );
    let roll = visible_text_position(&ctx, &output, "ColorMagic").unwrap();
    let secondary = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![
            Event::PointerMoved(roll),
            secondary(roll, true),
            pointer(roll, true),
        ],
    );
    let _ = frame(&ctx, &mut app, size, vec![pointer(roll, false)]);
    assert!(app.studio.save_pending);
    let close = moved.right_top() + egui::vec2(-17.0, 17.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(close), pointer(close, true)],
    );
    let _ = frame(&ctx, &mut app, size, vec![pointer(close, false)]);
    assert!(
        !app.show_theme_studio,
        "Close button must work after moving the window"
    );
    assert_eq!(app.col_weights, vec![0.62, 0.38]);
    assert!(!app.studio.save_pending);
    let _ = frame(&ctx, &mut app, size, vec![secondary(close, false)]);
}

#[test]
fn theme_studio_fits_the_minimum_viewport_and_keeps_its_close_button_reachable() {
    for size in [egui::vec2(280.0, 300.0), egui::vec2(360.0, 480.0)] {
        for scale in [0.75, 1.0, 1.5] {
            let ctx = egui::Context::default();
            let mut app = fixture();
            app.config.defaults.ui_scale = scale;
            app.show_theme_studio = true;
            for _ in 0..20 {
                let _ = frame(&ctx, &mut app, size, vec![]);
            }
            let rect = ctx
                .memory(|memory| memory.area_rect(egui::Id::new("psm-theme-studio")))
                .unwrap();
            assert!(
                ctx.screen_rect().expand(1.0).contains_rect(rect),
                "studio {rect:?} outside {:?}",
                ctx.screen_rect()
            );
            let close = rect.right_top() + egui::vec2(-17.0, 17.0);
            assert!(ctx.screen_rect().contains(close));
            let _ = frame(
                &ctx,
                &mut app,
                size,
                vec![Event::PointerMoved(close), pointer(close, true)],
            );
            let _ = frame(&ctx, &mut app, size, vec![pointer(close, false)]);
            assert!(
                !app.show_theme_studio,
                "close must remain reachable at {size:?}"
            );
        }
    }
}

#[test]
fn theme_studio_saved_controls_remain_usable_at_the_minimum_viewport() {
    for scale in [0.75, 1.0, 1.5] {
        let ctx = egui::Context::default();
        let mut app = fixture();
        app.config.defaults.ui_scale = scale;
        app.show_theme_studio = true;
        let size = egui::vec2(280.0, 300.0);
        let original = app.theme_settings;
        let _ = click_text(&ctx, &mut app, size, "Saved themes & sharing");
        scroll_studio(&ctx, &mut app, size, -10000.0);
        replace_text_field(&ctx, &mut app, size, "theme-name", "Compact theme");
        let _ = click_text(&ctx, &mut app, size, "Save");
        assert_eq!(app.config.saved_theme.len(), 1, "scale {scale}");
        assert_eq!(app.config.saved_theme[0].name, "Compact theme");
        let _ = click_text(&ctx, &mut app, size, "Copy theme");
        let _ = click_text(&ctx, &mut app, size, "Compact theme");
        assert_eq!(app.theme_settings, original);
        let _ = click_text(&ctx, &mut app, size, "Delete");
        assert!(app.config.saved_theme.is_empty(), "scale {scale}");
        let rect = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("psm-theme-studio")))
            .unwrap();
        assert!(
            ctx.screen_rect().expand(1.0).contains_rect(rect),
            "scale {scale}: {rect:?}"
        );
        assert_eq!(app.col_weights, vec![0.62, 0.38]);
    }
}

#[test]
fn dragging_a_cell_and_clicking_a_divider_leave_slots_and_weights_unchanged() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    let size = egui::vec2(1080.0, 800.0);
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let original = app.col_weights.clone();
    let first = preview_cell(&ctx, 0);
    let second = preview_cell(&ctx, 1);
    let cell = first.center();
    let end = cell + egui::vec2(15.0, 0.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(cell), pointer(cell, true)],
    );
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(end)]);
    assert!(app.dragging_divider.is_none());
    let _ = frame(&ctx, &mut app, size, vec![pointer(end, false)]);
    let divider = egui::pos2((first.right() + second.left()) * 0.5, first.top() + 20.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(divider), pointer(divider, true)],
    );
    let _ = frame(&ctx, &mut app, size, vec![pointer(divider, false)]);
    assert!(app.disabled_cells.is_empty());
    assert_eq!(app.col_weights, original);
}

#[test]
fn keyboard_activation_during_a_divider_drag_cannot_toggle_a_slot() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    let size = egui::vec2(1080.0, 800.0);
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let first = preview_cell(&ctx, 0);
    let second = preview_cell(&ctx, 1);
    let divider = egui::pos2((first.right() + second.left()) * 0.5, first.top() + 20.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(divider), pointer(divider, true)],
    );
    let end = divider + egui::vec2(-55.0, 0.0);
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(end)]);
    assert!(app.dragging_divider.is_some());
    let id = ctx
        .data(|data| data.get_temp::<egui::Id>(egui::Id::new("test-preview-id")))
        .unwrap();
    ctx.memory_mut(|memory| memory.request_focus(id));
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Key {
            key: egui::Key::Space,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    assert!(
        ctx.read_response(id).unwrap().clicked(),
        "Space must activate the actual preview response"
    );
    assert!(app.dragging_divider.is_some());
    assert!(app.disabled_cells.is_empty());
    app.use_custom = false;
    let _ = frame(&ctx, &mut app, size, vec![]);
    assert!(
        app.dragging_divider.is_none(),
        "switching to a preset cancels the active drag"
    );
    assert_eq!(app.config.defaults.col_weights, app.col_weights);
    let _ = frame(&ctx, &mut app, size, vec![pointer(end, false)]);
}

#[test]
fn a_work_area_too_small_for_the_grid_renders_a_message_without_dividers() {
    for cols in [0, 2] {
        let ctx = egui::Context::default();
        let mut app = fixture();
        app.custom_cols = cols;
        app.monitors = vec![crate::monitor::MonitorInfo {
            index: 0,
            is_primary: true,
            work_area: crate::monitor::Rect {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
            },
        }];
        let size = egui::vec2(1080.0, 800.0);
        let output = frame(&ctx, &mut app, size, vec![]);
        assert!(visible_text_position(&ctx, &output, "No space for this grid").is_some());
        assert!(app.dragging_divider.is_none());
        assert!(app.disabled_cells.is_empty());
    }
}

#[test]
fn actual_window_drag_scrolls_at_both_edges_and_an_outside_release_keeps_order() {
    let mut app = fixture();
    let original: Vec<_> = app
        .managed_windows
        .iter()
        .map(|window| window.hwnd)
        .collect();
    let ctx = egui::Context::default();
    let size = egui::vec2(1200.0, 1000.0);
    for _ in 0..4 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let scroll = ctx
        .data(|data| data.get_temp::<egui::Id>(egui::Id::new("test-workspace-scroll")))
        .unwrap();
    let clip = ctx
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("test-workspace-clip")))
        .unwrap();
    let handle = ctx
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(("test-window-handle", 1isize))))
        .unwrap()
        .center();
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(handle), pointer(handle, true)],
    );
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(handle + egui::vec2(0.0, 8.0))],
    );
    assert_eq!(*egui::DragAndDrop::payload::<isize>(&ctx).unwrap(), 1);
    let bottom = egui::pos2(handle.x + 60.0, clip.bottom() - 4.0);
    for _ in 0..8 {
        let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(bottom)]);
    }
    let down = egui::scroll_area::State::load(&ctx, scroll)
        .unwrap()
        .offset
        .y;
    assert!(down > 10.0, "edge drag must scroll down; offset {down}");
    let top = egui::pos2(handle.x + 60.0, clip.top() + 4.0);
    for _ in 0..8 {
        let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(top)]);
    }
    let up = egui::scroll_area::State::load(&ctx, scroll)
        .unwrap()
        .offset
        .y;
    assert!(
        up < down,
        "edge drag must scroll up; offsets {down} -> {up}"
    );
    let _ = frame(&ctx, &mut app, size, vec![pointer(top, false)]);
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|window| window.hwnd)
            .collect::<Vec<_>>(),
        original
    );
    assert!(!app.config.defaults.manual_order);
    assert!(egui::DragAndDrop::payload::<isize>(&ctx).is_none());
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerGone]);
    egui::DragAndDrop::set_payload(&ctx, 1isize);
    let _ = frame(&ctx, &mut app, size, vec![]);
    assert_eq!(
        egui::scroll_area::State::load(&ctx, scroll)
            .unwrap()
            .offset
            .y,
        up
    );
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|window| window.hwnd)
            .collect::<Vec<_>>(),
        original
    );
    egui::DragAndDrop::clear_payload(&ctx);
}

#[test]
fn actual_window_drop_after_a_row_draws_the_marker_and_commits_that_order() {
    let mut app = fixture();
    app.managed_windows.truncate(3);
    let ctx = egui::Context::default();
    let size = egui::vec2(1200.0, 1400.0);
    for _ in 0..4 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let handle = ctx
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(("test-window-handle", 1isize))))
        .unwrap()
        .center();
    let target = ctx
        .data(|data| data.get_temp::<egui::Rect>(egui::Id::new(("test-window-row", 2isize))))
        .unwrap();
    let drop = egui::pos2(target.left() + 90.0, target.bottom() - 4.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(handle), pointer(handle, true)],
    );
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(handle + egui::vec2(0.0, 8.0))],
    );
    let output = frame(&ctx, &mut app, size, vec![Event::PointerMoved(drop)]);
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::LineSegment { points, stroke } if stroke.width == 3.0 && points[0].y == target.bottom())));
    let _ = frame(&ctx, &mut app, size, vec![pointer(drop, false)]);
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|window| window.hwnd)
            .collect::<Vec<_>>(),
        [2, 1, 3]
    );
    assert!(app.config.defaults.manual_order);
}

#[test]
fn unavailable_quick_preset_keeps_the_current_selection() {
    let mut app = fixture();
    app.use_custom = false;
    app.presets
        .retain(|(_, preset)| *preset != crate::layout::LayoutPreset::Columns(3));
    let selected = app.selected_preset;
    let ctx = egui::Context::default();
    let size = egui::vec2(1200.0, 1400.0);
    let _ = frame(&ctx, &mut app, size, vec![]);
    let _ = frame(&ctx, &mut app, size, vec![]);
    click_text(&ctx, &mut app, size, "columns:3");
    assert_eq!(app.selected_preset, selected);
    assert!(!app.use_custom);
}

#[test]
fn malformed_custom_grid_recovers_and_disabled_slots_survive_restart() {
    let mut config = Config::default();
    config.defaults.use_custom = true;
    config.defaults.custom_cols = 0;
    config.defaults.custom_rows = 100;
    config.defaults.col_weights = vec![f32::NAN];
    config.defaults.ui_scale = f32::NAN;
    let mut app = PsmApp::preview(config);
    assert_eq!((app.custom_cols, app.custom_rows), (1, 8));
    assert_eq!(app.col_weights, vec![1.0]);
    assert!(app.row_weights.iter().all(|w| w.is_finite() && *w > 0.0));
    assert_eq!(app.config.defaults.ui_scale, 1.0);
    app.toggle_cell(2);
    let restored = PsmApp::preview(toml::from_str(&toml::to_string(&app.config).unwrap()).unwrap());
    assert!(restored.disabled_cells.contains(&2));
    assert_eq!(restored.active_preset(), app.active_preset());
}

#[test]
#[ignore = "GPU offscreen review; writes only target/ui-review"]
fn render_ui_review() {
    let folder = std::path::Path::new("target/ui-review");
    std::fs::create_dir_all(folder).unwrap();
    let mut renderer = offscreen::Renderer::new();
    for (name, size, light, studio, preset) in [
        ("workspace", egui::vec2(1200.0, 900.0), false, false, false),
        ("presets", egui::vec2(1080.0, 800.0), false, false, true),
        ("studio", egui::vec2(1280.0, 1000.0), false, true, false),
        ("light", egui::vec2(1080.0, 800.0), true, false, false),
        ("compact", egui::vec2(480.0, 760.0), false, false, false),
        ("about", egui::vec2(1080.0, 800.0), false, false, false),
        (
            "minimum-about",
            egui::vec2(280.0, 300.0),
            false,
            false,
            false,
        ),
        (
            "inventory-bottom",
            egui::vec2(480.0, 760.0),
            false,
            false,
            false,
        ),
        (
            "inventory-wide",
            egui::vec2(1200.0, 900.0),
            false,
            false,
            false,
        ),
        ("portrait", egui::vec2(1080.0, 800.0), false, false, false),
        ("ultrawide", egui::vec2(1080.0, 800.0), false, false, false),
        ("minimum", egui::vec2(280.0, 300.0), false, false, false),
        (
            "minimum-studio",
            egui::vec2(280.0, 300.0),
            false,
            true,
            false,
        ),
        (
            "minimum-studio-saved",
            egui::vec2(280.0, 300.0),
            false,
            true,
            false,
        ),
    ] {
        let ctx = egui::Context::default();
        let mut app = fixture();
        app.theme_settings.dark = !light;
        app.use_custom = !preset;
        app.selected_preset = 2;
        app.show_theme_studio = studio;
        if name.ends_with("about") {
            app.detail_tab = 3;
        }
        if name == "portrait" || name == "ultrawide" {
            let (w, h) = if name == "portrait" {
                (1080, 1880)
            } else {
                (3440, 1400)
            };
            app.monitors = vec![crate::monitor::MonitorInfo {
                index: 0,
                is_primary: true,
                work_area: crate::monitor::Rect { x: 0, y: 40, w, h },
            }];
        }
        let mut output = egui::FullOutput::default();
        for _ in 0..20 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        if name == "minimum-studio-saved" {
            let _ = click_text(&ctx, &mut app, size, "Saved themes & sharing");
            scroll_studio(&ctx, &mut app, size, -10000.0);
            replace_text_field(&ctx, &mut app, size, "theme-name", "Compact theme");
            let _ = click_text(&ctx, &mut app, size, "Save");
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        if name.starts_with("inventory-") {
            let clip = ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("test-workspace-clip")))
                .unwrap();
            output.append(frame(
                &ctx,
                &mut app,
                size,
                vec![
                    Event::PointerMoved(clip.center()),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: egui::vec2(0.0, -10000.0),
                        modifiers: Default::default(),
                    },
                ],
            ));
            for _ in 0..20 {
                output.append(frame(&ctx, &mut app, size, vec![]));
            }
            let clip = ctx
                .data(|data| data.get_temp::<egui::Rect>(egui::Id::new("test-workspace-clip")))
                .unwrap();
            let last_row = ctx
                .data(|data| {
                    data.get_temp::<egui::Rect>(egui::Id::new(("test-window-row", 6isize)))
                })
                .unwrap();
            assert!(
                clip.expand(1.0).contains_rect(last_row),
                "last row {last_row:?} clipped by {clip:?}"
            );
        }
        renderer.save(&ctx, output, size, &folder.join(format!("{name}.png")));
        println!("Rendered {name} with production UI and inert app state");
    }
}

#[test]
fn every_theme_and_small_window_renders_without_invalid_geometry() {
    for (_, theme) in ThemeSettings::presets() {
        for size in [egui::vec2(280.0, 300.0), egui::vec2(1080.0, 800.0)] {
            let ctx = egui::Context::default();
            let mut app = fixture();
            app.theme_settings = theme;
            for tab in 0..4 {
                app.detail_tab = tab;
                let output = frame(&ctx, &mut app, size, vec![]);
                assert!(!output.shapes.is_empty());
                for job in ctx.tessellate(output.shapes, output.pixels_per_point) {
                    if let egui::epaint::Primitive::Mesh(mesh) = job.primitive {
                        assert!(mesh
                            .vertices
                            .iter()
                            .all(|v| v.pos.x.is_finite() && v.pos.y.is_finite()));
                    }
                }
            }
        }
    }
}

#[test]
fn pin_button_is_individual_and_actual_drag_changes_apply_plan() {
    let mut app = fixture();
    app.managed_windows.truncate(3);
    for (i, w) in app.managed_windows.iter_mut().enumerate() {
        w.process_name = "WindowsTerminal.exe".into();
        w.title = format!("Terminal {i}");
    }
    app.config.defaults.smart_sort = true;
    let ctx = egui::Context::default();
    let size = egui::vec2(1200.0, 1000.0);
    for _ in 0..4 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let pin = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-window-pin", 2isize))))
        .unwrap()
        .center();
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(pin), pointer(pin, true)],
    );
    let _ = frame(&ctx, &mut app, size, vec![pointer(pin, false)]);
    assert_eq!(app.config.pin.len(), 1);
    assert_eq!(
        app.assignment().rule_windows,
        vec![Some(1)],
        "one terminal only"
    );
    let handle = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-window-handle", 2isize))))
        .unwrap()
        .center();
    let target = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-window-row", 1isize))))
        .unwrap()
        .left_top()
        + egui::vec2(80.0, 4.0);
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(handle), pointer(handle, true)],
    );
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![Event::PointerMoved(handle + egui::vec2(0.0, -7.0))],
    );
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(target)]);
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerGone]);
    assert!(!app.config.defaults.manual_order);
    let _ = frame(&ctx, &mut app, size, vec![Event::PointerMoved(target)]);
    let _ = frame(&ctx, &mut app, size, vec![pointer(target, false)]);
    assert!(
        app.config.defaults.manual_order,
        "real handle must switch off ranking"
    );
    assert!(!app.config.defaults.smart_sort);
    assert_eq!(app.managed_windows[0].hwnd, 2);
    assert_eq!(
        app.config.pin[0].slot, 0,
        "pins follow intentional dragging"
    );
    let area = crate::monitor::Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };
    let (plan, warnings) = crate::order::placements(
        &app.active_preset(),
        &area,
        4,
        &app.disabled_cells,
        None,
        &app.managed_windows,
        &app.config.pin,
    );
    assert!(warnings.is_empty());
    assert_eq!(plan[0].0, 2);
    assert_eq!((plan[0].1.x, plan[0].1.y), (0, 0));
}

#[test]
fn exact_pins_collisions_disabled_cells_and_identical_titles_keep_every_window() {
    let mut app = fixture();
    app.managed_windows.truncate(3);
    for w in &mut app.managed_windows {
        w.process_name = "WindowsTerminal.exe".into();
        w.title = "same title".into();
    }
    app.toggle_pin(2);
    assert_eq!(app.assignment().rule_windows, vec![Some(1)]);
    app.toggle_pin(3);
    app.config.pin[0].slot = 0;
    app.config.pin[1].slot = 0;
    let plan = app.assignment();
    assert_eq!(plan.warnings.len(), 1);
    let mut placed: Vec<_> = plan.slots.into_iter().flatten().collect();
    placed.sort();
    assert_eq!(placed, vec![0, 1, 2]);
    app.disabled_cells.insert(0);
    let plan = app.assignment();
    assert_eq!(plan.slots[0], None);
    assert_eq!(plan.warnings.len(), 2);
    assert_eq!(plan.slots.into_iter().flatten().count(), 3);
    app.toggle_pin(2);
    assert_eq!(app.config.pin.len(), 1);
    assert_eq!(app.config.pin[0].bound_hwnd, Some(3));
    let encoded = toml::to_string(&app.config).unwrap();
    assert!(!encoded.contains("bound_hwnd"));
    let config: Config = toml::from_str(&encoded).unwrap();
    assert!(config.pin[0].bound_hwnd.is_none());
}

#[test]
fn legacy_pin_conditions_are_anded_and_each_rule_consumes_only_one_window() {
    let rule: crate::config::PinRule =
        toml::from_str("process='WindowsTerminal.exe'\ntitle_contains='Avatar'\nslot=0").unwrap();
    assert!(rule.matches("windowsterminal.EXE", "Avatar hand grips"));
    assert!(!rule.matches("WindowsTerminal.exe", "Other terminal"));
    assert!(!rule.matches("notepad.exe", "Avatar"));
    let mut app = fixture();
    for w in &mut app.managed_windows {
        w.process_name = "WindowsTerminal.exe".into();
        w.title = "Avatar".into();
    }
    let p = crate::order::assign(&app.managed_windows, &[rule], 6, &Default::default());
    assert_eq!(p.rule_windows, vec![Some(0)]);
    assert_eq!(p.slots.into_iter().flatten().count(), 6);
}

#[test]
fn refresh_keeps_live_order_titles_and_adds_new_windows_without_duplicates() {
    let app = fixture();
    let mut previous = app.managed_windows.clone();
    previous.swap(0, 2);
    let keys: Vec<_> = previous
        .iter()
        .map(crate::order::WindowKey::from_window)
        .collect();
    let mut fresh = app.managed_windows.clone();
    fresh[2].title = "Renamed during build".into();
    fresh.retain(|w| w.hwnd != 2);
    let mut new = fresh[0].clone();
    new.hwnd = 99;
    fresh.insert(0, new);
    let result = crate::order::reconcile(fresh, &previous, &keys);
    assert_eq!(result[0].hwnd, 3);
    assert_eq!(result.last().unwrap().hwnd, 99);
    assert_eq!(result.len(), 6);
    let restarted = crate::order::reconcile(app.managed_windows.clone(), &[], &keys);
    assert_eq!(restarted[0].hwnd, 3);
}

#[test]
fn adversarial_plans_never_duplicate_drop_or_place_into_disabled_cells() {
    let app = fixture();
    for mask in 0u32..64 {
        for slot in 0..9 {
            let disabled = (0..6).filter(|i| mask & (1 << i) != 0).collect();
            let pins = vec![
                crate::config::PinRule {
                    process: Some("Code.exe".into()),
                    title_exact: None,
                    title_contains: None,
                    bound_hwnd: None,
                    slot
                };
                3
            ];
            let plan = crate::order::assign(&app.managed_windows, &pins, 6, &disabled);
            let mut seen = std::collections::HashSet::new();
            for (s, w) in plan.slots.iter().enumerate() {
                if let Some(w) = w {
                    assert!(!disabled.contains(&s));
                    assert!(seen.insert(*w));
                }
            }
            assert_eq!(seen.len(), 6 - disabled.len());
        }
    }
}

#[test]
fn malformed_layouts_and_weights_are_bounded_and_pixels_close_exactly() {
    use crate::layout::{compute_weighted_grid, LayoutPreset};
    for bad in [
        "0x2",
        "999999x2",
        "columns:0",
        "rows:4294967295",
        "focus:garbage",
        "focus:0",
        "main-side:99",
    ] {
        assert!(LayoutPreset::parse(bad).is_none(), "{bad}");
    }
    let area = crate::monitor::Rect {
        x: -3840,
        y: 30,
        w: 3839,
        h: 2109,
    };
    for weights in [
        vec![],
        vec![f32::NAN],
        vec![f32::INFINITY; 3],
        vec![-1.0; 3],
        vec![f32::MAX; 3],
        vec![0.01, 0.38, 0.61],
    ] {
        let slots = compute_weighted_grid(3, 2, &area, 4, &weights, &[0.5, 0.5]);
        assert_eq!(slots.len(), 6);
        assert_eq!(slots[2].x + slots[2].w, area.x + area.w);
        assert_eq!(slots[5].y + slots[5].h, area.y + area.h);
        for pair in slots[..3].windows(2) {
            assert_eq!(pair[0].x + pair[0].w + 4, pair[1].x);
        }
        assert!(slots.iter().all(|s| s.w > 0 && s.h > 0));
    }
    assert!(LayoutPreset::Columns(0).compute_slots(&area, 4).is_empty());
    assert!(compute_weighted_grid(u32::MAX, 2, &area, i32::MAX, &[], &[]).is_empty());
}

#[test]
fn every_preset_fills_odd_work_areas_without_remainder_seams_or_overlaps() {
    use crate::layout::LayoutPreset;
    let presets = [
        LayoutPreset::Grid { cols: 3, rows: 2 },
        LayoutPreset::Columns(7),
        LayoutPreset::Rows(7),
        LayoutPreset::LeftRight,
        LayoutPreset::TopBottom,
        LayoutPreset::MainSide { side_count: 7 },
        LayoutPreset::Focus { side_count: 7 },
    ];
    for preset in presets {
        for (width, height) in [(1919, 1039), (1079, 1879), (3439, 1399), (8, 8)] {
            for gap in [0, 4, 32, 64, -1, i32::MAX] {
                let area = crate::monitor::Rect {
                    x: -width,
                    y: 41,
                    w: width,
                    h: height,
                };
                let slots = preset.compute_slots(&area, gap);
                assert_eq!(slots.len(), preset.slot_count(), "{preset:?}");
                assert_eq!(
                    slots.iter().map(|slot| slot.x + slot.w).max(),
                    Some(area.x + area.w)
                );
                assert_eq!(
                    slots.iter().map(|slot| slot.y + slot.h).max(),
                    Some(area.y + area.h)
                );
                for (index, slot) in slots.iter().enumerate() {
                    assert!(slot.w > 0 && slot.h > 0, "{preset:?}: {slot:?}");
                    assert!(slot.x >= area.x && slot.y >= area.y);
                    assert!(
                        slot.x + slot.w <= area.x + area.w && slot.y + slot.h <= area.y + area.h
                    );
                    for other in &slots[index + 1..] {
                        assert!(
                            slot.x + slot.w <= other.x
                                || other.x + other.w <= slot.x
                                || slot.y + slot.h <= other.y
                                || other.y + other.h <= slot.y,
                            "overlapping {preset:?}: {slot:?}, {other:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn tray_and_hotkey_resolve_latest_settings_and_saved_grid_masks() {
    use crate::tray::{layout_request, LayoutChoice, TrayAction};
    let mut cfg = Config::default();
    cfg.defaults.use_custom = true;
    cfg.defaults.custom_cols = 3;
    cfg.defaults.custom_rows = 2;
    cfg.defaults.disabled_cells = vec![1];
    cfg.defaults.col_weights = vec![0.2, 0.3, 0.5];
    cfg.defaults.row_weights = vec![0.5, 0.5];
    let req = layout_request(&cfg, &TrayAction::ApplyCurrent).unwrap();
    assert!(req.disabled.contains(&1));
    assert_eq!(req.preset.slot_count(), 6);
    cfg.defaults.custom_cols = 1;
    let req = layout_request(&cfg, &TrayAction::ApplyCurrent).unwrap();
    assert_eq!(req.preset.slot_count(), 2);
    cfg.saved_grid.push(crate::config::SavedGrid {
        name: "Work".into(),
        cols: 3,
        rows: 2,
        col_weights: vec![0.2, 0.3, 0.5],
        row_weights: vec![0.5, 0.5],
        disabled_cells: vec![2],
    });
    let req = layout_request(
        &cfg,
        &TrayAction::ApplyLayout(LayoutChoice::Saved {
            index: 0,
            name: "Work".into(),
        }),
    )
    .unwrap();
    assert!(req.disabled.contains(&2));
    assert_eq!(req.weights.unwrap().0, vec![0.2, 0.3, 0.5]);
    cfg.saved_grid.clear();
    assert!(layout_request(
        &cfg,
        &TrayAction::ApplyLayout(LayoutChoice::Saved {
            index: 0,
            name: "Work".into()
        })
    )
    .is_none());
}

#[test]
fn branding_has_theme_color_and_both_contrasting_edges_at_small_sizes() {
    for rgb in [[0, 0, 0], [255, 255, 255], [128, 128, 128], [255, 0, 255]] {
        for size in [16, 20, 24, 32, 64] {
            let theme = ThemeSettings {
                accent: rgb,
                secondary: rgb,
                ..Default::default()
            };
            let icon = crate::branding::icon(theme, size);
            assert_eq!(icon.rgba.len(), (size * size * 4) as usize);
            let opaque: Vec<_> = icon.rgba.chunks_exact(4).filter(|p| p[3] > 200).collect();
            assert!(opaque.iter().any(|p| p[0] < 75));
            assert!(opaque.iter().any(|p| p[0] > 180));
            assert!(icon.rgba.chunks_exact(4).any(|p| p[3] == 0));
        }
    }
}

#[test]
fn atomic_settings_write_replaces_complete_file() {
    let folder = std::path::Path::new("target/audit");
    std::fs::create_dir_all(folder).unwrap();
    let path = folder.join("atomic-config.toml");
    crate::persistence::atomic_write(&path, b"[defaults]\ngap=4").unwrap();
    crate::persistence::atomic_write(&path, b"[defaults]\ngap=12").unwrap();
    let cfg: Config = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(cfg.defaults.gap, 12);
}

#[test]
fn failed_atomic_replace_preserves_destination_and_removes_its_temporary_file() {
    let folder = std::path::PathBuf::from("target/storage-replace-failure");
    std::fs::create_dir_all(&folder).unwrap();
    let destination = folder.join("occupied-directory");
    std::fs::create_dir_all(&destination).unwrap();
    let marker = destination.join("keep.txt");
    std::fs::write(&marker, b"original").unwrap();
    assert!(crate::persistence::atomic_write(&destination, b"replacement").is_err());
    assert_eq!(std::fs::read(&marker).unwrap(), b"original");
    let remaining: Vec<_> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(
        remaining,
        vec![std::ffi::OsString::from("occupied-directory")]
    );
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
#[ignore = "Generates production UI marketing captures; enumerates window metadata read-only and anonymizes titles"]
fn render_public_gallery() {
    let folder = std::path::Path::new("target/public-media");
    std::fs::create_dir_all(folder).unwrap();
    let mut renderer = offscreen::Renderer::new();
    let mut windows =
        crate::windows::find_windows(&crate::windows::TargetFilter::Terminals, 0, &[]);
    for (i, w) in windows.iter_mut().enumerate() {
        w.title = format!("Terminal {:02} / workspace", i + 1);
    }
    let media = std::path::Path::new("../tmp/trontop-launch/site/trontop/media");
    for name in [
        "electric", "spectrum", "carbon", "daylight", "ember", "vector",
    ] {
        let code = std::fs::read_to_string(media.join(format!("{name}.json"))).unwrap();
        let settings = ThemeSettings::decode(&code).unwrap();
        for studio in [false, true] {
            if studio && name != "electric" {
                continue;
            }
            let ctx = egui::Context::default();
            let mut app = fixture();
            app.managed_windows = windows.clone();
            app.config.defaults.target = "terminals".into();
            app.config.defaults.manual_order = true;
            app.custom_cols = 3;
            app.custom_rows = 2;
            app.col_weights = vec![0.40, 0.30, 0.30];
            app.row_weights = vec![0.5, 0.5];
            app.theme_settings = settings;
            app.theme_dirty = true;
            app.show_theme_studio = studio;
            app.status = "Ready. Drag a handle to choose slots, then Apply layout.".into();
            let size = egui::vec2(1200.0, 900.0);
            let mut output = egui::FullOutput::default();
            for _ in 0..15 {
                output.append(frame(&ctx, &mut app, size, vec![]));
            }
            renderer.save(
                &ctx,
                output,
                size,
                &folder.join(format!(
                    "{name}-{}.png",
                    if studio { "studio" } else { "workspace" }
                )),
            );
        }
        std::fs::write(folder.join(format!("{name}.json")), settings.encode()).unwrap();
    }
    std::fs::write(folder.join("capture.json"),format!("{{\"version\":\"{}\",\"source\":\"production egui offscreen renderer\",\"window_count\":{},\"titles\":\"anonymized\",\"native_actions\":false}}",env!("CARGO_PKG_VERSION"),windows.len())).unwrap();
    println!("PUBLIC MEDIA: 7 production UI captures; {} actual detected windows; titles anonymized; no native actions or settings writes",windows.len());
}

#[test]
fn inventory_rows_do_not_overlap_at_wide_and_compact_widths() {
    for (size, scale) in [
        (egui::vec2(1200.0, 1000.0), 1.0),
        (egui::vec2(480.0, 1800.0), 0.75),
        (egui::vec2(480.0, 1800.0), 1.0),
        (egui::vec2(480.0, 1800.0), 1.5),
    ] {
        let ctx = egui::Context::default();
        let mut app = fixture();
        app.config.defaults.ui_scale = scale;
        app.config.pin.push(crate::config::PinRule {
            bound_hwnd: Some(1),
            process: Some("WindowsTerminal.exe".into()),
            title_exact: None,
            title_contains: None,
            slot: 0,
        });
        for _ in 0..4 {
            let _ = frame(&ctx, &mut app, size, vec![]);
        }
        let rects: Vec<_> = (1isize..=6)
            .map(|hwnd| {
                ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("test-window-row", hwnd))))
                    .unwrap()
            })
            .collect();
        for rows in rects.windows(2) {
            assert!(
                rows[0].bottom() <= rows[1].top(),
                "overlapping rows: {rows:?}"
            );
            assert_eq!(rows[0].width(), rows[1].width());
        }
        for (hwnd, row) in (1isize..=6).zip(rects) {
            let get = |part| {
                ctx.data(|d| d.get_temp::<egui::Rect>(egui::Id::new((part, hwnd))))
                    .unwrap()
            };
            let focus = get("test-window-focus");
            let pin = get("test-window-pin");
            let state = get("test-window-state");
            assert!(
                (focus.width() - pin.width()).abs() < 1.0
                    && (focus.height() - pin.height()).abs() < 1.0,
                "scale {scale}: {focus:?} / {pin:?}"
            );
            assert!(
                (focus.top() - pin.top()).abs() < 1.0,
                "buttons must align: {focus:?} / {pin:?}"
            );
            assert!(focus.right() < pin.left(), "buttons must not overlap");
            for content in [focus, pin, state] {
                assert!(row.contains_rect(content), "row {row:?} clips {content:?}");
            }
            assert!(
                !state.intersects(focus) && !state.intersects(pin),
                "status overlaps buttons"
            );
        }
    }
}

#[test]
fn auto_mode_toggles_save_and_unlock_their_options() {
    let ctx = egui::Context::default();
    let mut app = fixture();
    let size = egui::vec2(1200.0, 1400.0);
    let _ = click_text(&ctx, &mut app, size, "Slide to fill gaps");
    assert!(!app.config.defaults.slide_to_fill);
    let _ = click_text(&ctx, &mut app, size, "Auto-arrange new windows");
    assert!(app.config.defaults.auto_arrange);
    let _ = click_text(&ctx, &mut app, size, "Slide to fill gaps");
    let _ = click_text(&ctx, &mut app, size, "Overflow to other display");
    assert!(app.config.defaults.slide_to_fill && app.config.defaults.overflow_display);
}
