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
        hwnd: 0,
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

fn frame(
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
    let _ = frame(&ctx, &mut app, size, vec![]);
    let moved = ctx.memory(|m| m.area_rect(id)).unwrap();
    assert!(
        moved.left() < before.left() - 100.0,
        "Theme Studio must be draggable"
    );
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
    ] {
        let ctx = egui::Context::default();
        let mut app = fixture();
        app.theme_settings.dark = !light;
        app.use_custom = !preset;
        app.selected_preset = 2;
        app.show_theme_studio = studio;
        let mut output = egui::FullOutput::default();
        for _ in 0..20 {
            output.append(frame(&ctx, &mut app, size, vec![]));
        }
        renderer.save(&ctx, output, size, &folder.join(format!("{name}.png")));
        println!("Rendered {name} with production UI and inert app state");
    }
}

#[test]
fn every_theme_and_small_window_renders_without_invalid_geometry() {
    for (_, theme) in ThemeSettings::presets() {
        for size in [egui::vec2(440.0, 480.0), egui::vec2(1080.0, 800.0)] {
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
