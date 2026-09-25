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
        &TrayAction::ApplyLayout(LayoutChoice::Saved("Work".into())),
    )
    .unwrap();
    assert!(req.disabled.contains(&2));
    assert_eq!(req.weights.unwrap().0, vec![0.2, 0.3, 0.5]);
    cfg.saved_grid.clear();
    assert!(layout_request(
        &cfg,
        &TrayAction::ApplyLayout(LayoutChoice::Saved("Work".into()))
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
    crate::config::atomic_write(&path, b"[defaults]\ngap=4").unwrap();
    crate::config::atomic_write(&path, b"[defaults]\ngap=12").unwrap();
    let cfg: Config = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(cfg.defaults.gap, 12);
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
            app.capture_ui = true;
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
    for size in [egui::vec2(1200.0, 1000.0), egui::vec2(480.0, 1800.0)] {
        let ctx = egui::Context::default();
        let mut app = fixture();
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
    }
}
