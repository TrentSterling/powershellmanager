use super::*;
use crate::{
    desktop::Desktop,
    history::{RestoreStatus, WindowBackend},
    monitor::{MonitorInfo, Rect},
    windows::{AppCategory, WindowSnapshot},
};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
};
use windows::Win32::{Foundation::RECT, UI::WindowsAndMessaging::WINDOWPLACEMENT};

type MoveRecord = (isize, i32, i32, i32, i32);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/app-tests");
        std::fs::create_dir_all(&root).unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = root.join(format!(
            "{}-{nonce}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        assert!(self
            .0
            .starts_with(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/app-tests")));
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[derive(Clone)]
struct TestDesktop(Arc<Mutex<Desk>>);
struct Desk {
    fresh: Vec<ManagedWindow>,
    snapshots: BTreeMap<isize, WindowSnapshot>,
    monitors: Vec<MonitorInfo>,
    moved: Vec<MoveRecord>,
    restored: Vec<isize>,
    captures: Vec<isize>,
    queries: Vec<(TargetFilter, isize, Vec<String>)>,
    capture_error: HashSet<isize>,
    position_error: HashSet<isize>,
    restore_error: HashSet<isize>,
    app_visible: bool,
    shown: Vec<isize>,
    hidden: Vec<isize>,
    woken: Vec<isize>,
    focused: Vec<isize>,
    minimized: Vec<isize>,
    restored_controls: Vec<isize>,
}

fn window(hwnd: isize) -> ManagedWindow {
    ManagedWindow {
        hwnd,
        title: format!("Owned app test {hwnd}"),
        process_name: format!("owned-{hwnd}.exe"),
        category: AppCategory::Other,
        rect: Rect {
            x: 20,
            y: 30,
            w: 300,
            h: 200,
        },
        is_minimized: false,
    }
}
fn snapshot(hwnd: isize) -> WindowSnapshot {
    WindowSnapshot {
        hwnd,
        process_id: 42,
        visible: true,
        placement: WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            showCmd: 1,
            rcNormalPosition: RECT {
                left: 20,
                top: 30,
                right: 320,
                bottom: 230,
            },
            ..Default::default()
        },
    }
}
impl TestDesktop {
    fn new(handles: impl IntoIterator<Item = isize>) -> Self {
        let fresh: Vec<_> = handles.into_iter().map(window).collect();
        Self(Arc::new(Mutex::new(Desk {
            snapshots: fresh
                .iter()
                .map(|window| (window.hwnd, snapshot(window.hwnd)))
                .collect(),
            fresh,
            monitors: vec![MonitorInfo {
                index: 0,
                is_primary: true,
                work_area: Rect {
                    x: -1201,
                    y: -701,
                    w: 1201,
                    h: 901,
                },
            }],
            moved: vec![],
            restored: vec![],
            captures: vec![],
            queries: vec![],
            capture_error: HashSet::new(),
            position_error: HashSet::new(),
            restore_error: HashSet::new(),
            app_visible: true,
            shown: vec![],
            hidden: vec![],
            woken: vec![],
            focused: vec![],
            minimized: vec![],
            restored_controls: vec![],
        })))
    }
}
impl WindowBackend for TestDesktop {
    fn capture(&mut self, hwnd: isize) -> Result<WindowSnapshot, String> {
        let mut desk = self.0.lock().unwrap();
        desk.captures.push(hwnd);
        if desk.capture_error.contains(&hwnd) {
            return Err(format!("snapshot denied: {hwnd}"));
        }
        desk.snapshots
            .get(&hwnd)
            .copied()
            .ok_or_else(|| format!("closed: {hwnd}"))
    }
    fn position(
        &mut self,
        snapshot: &WindowSnapshot,
        slot: &crate::layout::Slot,
    ) -> Result<(), String> {
        let mut desk = self.0.lock().unwrap();
        if desk.position_error.contains(&snapshot.hwnd) {
            return Err(format!("position denied: {}", snapshot.hwnd));
        }
        let current = desk.snapshots.get_mut(&snapshot.hwnd).unwrap();
        current.placement.rcNormalPosition = RECT {
            left: slot.x,
            top: slot.y,
            right: slot.x + slot.w,
            bottom: slot.y + slot.h,
        };
        desk.moved
            .push((snapshot.hwnd, slot.x, slot.y, slot.w, slot.h));
        Ok(())
    }
    fn restore(&mut self, snapshot: &WindowSnapshot) -> Result<RestoreStatus, String> {
        let mut desk = self.0.lock().unwrap();
        if desk.restore_error.contains(&snapshot.hwnd) {
            return Err(format!("restore denied: {}", snapshot.hwnd));
        }
        let Some(current) = desk.snapshots.get_mut(&snapshot.hwnd) else {
            return Ok(RestoreStatus::Closed);
        };
        *current = *snapshot;
        desk.restored.push(snapshot.hwnd);
        Ok(RestoreStatus::Restored)
    }
}
impl Desktop for TestDesktop {
    fn monitors(&mut self) -> Vec<MonitorInfo> {
        self.0.lock().unwrap().monitors.clone()
    }
    fn windows(
        &mut self,
        filter: &TargetFilter,
        app_hwnd: isize,
        excluded: &[String],
    ) -> Vec<ManagedWindow> {
        let mut desk = self.0.lock().unwrap();
        desk.queries
            .push((filter.clone(), app_hwnd, excluded.to_vec()));
        desk.fresh
            .iter()
            .map(|window| {
                let mut window = window.clone();
                if let Some(snapshot) = desk.snapshots.get(&window.hwnd) {
                    let rect = snapshot.placement.rcNormalPosition;
                    window.rect = Rect {
                        x: rect.left,
                        y: rect.top,
                        w: rect.right - rect.left,
                        h: rect.bottom - rect.top,
                    };
                }
                window
            })
            .collect()
    }
    fn visible(&self, _: isize) -> bool {
        self.0.lock().unwrap().app_visible
    }
    fn show_app(&mut self, hwnd: isize) {
        let mut desk = self.0.lock().unwrap();
        desk.shown.push(hwnd);
        desk.app_visible = true;
    }
    fn hide_app(&mut self, hwnd: isize) {
        let mut desk = self.0.lock().unwrap();
        desk.hidden.push(hwnd);
        desk.app_visible = false;
    }
    fn wake_for_close(&mut self, hwnd: isize) {
        let mut desk = self.0.lock().unwrap();
        desk.woken.push(hwnd);
        desk.app_visible = true;
    }
    fn focus_window(&mut self, hwnd: isize) {
        let mut desk = self.0.lock().unwrap();
        desk.focused.push(hwnd);
        if let Some(window) = desk.fresh.iter_mut().find(|window| window.hwnd == hwnd) {
            window.is_minimized = false;
        }
    }
    fn minimize_window(&mut self, hwnd: isize) {
        let mut desk = self.0.lock().unwrap();
        desk.minimized.push(hwnd);
        desk.fresh
            .iter_mut()
            .find(|window| window.hwnd == hwnd)
            .unwrap()
            .is_minimized = true;
    }
    fn restore_window(&mut self, hwnd: isize) {
        let mut desk = self.0.lock().unwrap();
        desk.restored_controls.push(hwnd);
        desk.fresh
            .iter_mut()
            .find(|window| window.hwnd == hwnd)
            .unwrap()
            .is_minimized = false;
    }
}

fn live_app(config: Config, desktop: TestDesktop, scratch: &Scratch) -> PsmApp {
    crate::action_tests::init_logging();
    let mut app = PsmApp::preview(config);
    app.desktop = Box::new(desktop);
    app.settings_path = Some(scratch.0.join("config.toml"));
    app.native_enabled = true;
    app.app_hwnd = 444;
    app
}

pub(crate) fn audit_theme_update_failure_is_visible(tray: tray::TrayIcon) {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app._tray_icon = Some(tray);
    app.theme_dirty = true;
    let ctx = egui::Context::default();
    let _ = tick(&mut app, &ctx);
    assert_eq!(
        app.status,
        "Could not update tray icon: owned icon update failure"
    );
    assert!(!app.theme_dirty);
    assert!(app._tray_icon.is_some());
    assert!(desktop.0.lock().unwrap().moved.is_empty());
}

pub(crate) fn audit_tray_status_failure_is_visible(tray: tray::TrayIcon, recover: &dyn Fn()) {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1]);
    let mut app = live_app(Config::default(), desktop, &scratch);
    app._tray_icon = Some(tray);
    app.refresh_windows();
    app.sync_tray_state();
    assert_eq!(
        app.status,
        "Could not update tray status: owned tooltip update failure"
    );
    app.sync_tray_state();
    assert_eq!(
        app.status,
        "Could not update tray status: owned tooltip update failure"
    );
    recover();
    app.status = "Recovered tray status".into();
    app.sync_tray_state();
    assert_eq!(app.status, "Recovered tray status");
}

#[test]
fn actual_display_and_preset_menus_persist_selection_and_load_saved_weights() {
    use crate::ui_tests::{click_text, frame, visible_text_position};
    let scratch = Scratch::new();
    let mut desktop = TestDesktop::new([1, 2]);
    let secondary = MonitorInfo {
        index: 1,
        is_primary: false,
        work_area: Rect {
            x: -1000,
            y: 40,
            w: 1000,
            h: 600,
        },
    };
    desktop.0.lock().unwrap().monitors.push(secondary);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.refresh_windows();
    app.monitors = desktop.monitors();
    let ctx = egui::Context::default();
    let size = egui::vec2(1080.0, 1200.0);
    let _ = click_text(&ctx, &mut app, size, "Primary display");
    let _ = click_text(&ctx, &mut app, size, "Display 1: 1000 x 600");
    assert_eq!(app.config.defaults.monitor, "1");
    let saved: Config = crate::persistence::load_toml([scratch.0.join("config.toml")]);
    assert_eq!(saved.defaults.monitor, "1");
    assert_eq!(app.live.lock().unwrap().config.defaults.monitor, "1");

    app.disabled_cells.insert(0);
    let selected = app.presets[app.selected_preset].0.clone();
    let _ = click_text(&ctx, &mut app, size, &selected);
    let _ = click_text(&ctx, &mut app, size, "2x3 Grid");
    assert_eq!(app.active_preset(), LayoutPreset::Grid { cols: 2, rows: 3 });
    assert!(app.disabled_cells.is_empty());
    assert_eq!(app.config.defaults.selected_preset, 3);
    assert_eq!(app.live.lock().unwrap().config.defaults.selected_preset, 3);

    app.custom_cols = 2;
    app.custom_rows = 2;
    app.col_weights = vec![0.7, 0.3];
    app.row_weights = vec![0.4, 0.6];
    app.disabled_cells.insert(3);
    app.save_current_as_grid("Menu audit grid".into());
    let _ = click_text(&ctx, &mut app, size, "2x3 Grid");
    let output = frame(&ctx, &mut app, size, vec![]);
    let position = visible_text_position(&ctx, &output, "1x2 Grid").unwrap();
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -10000.0),
                modifiers: Default::default(),
            },
        ],
    );
    for _ in 0..20 {
        let _ = frame(&ctx, &mut app, size, vec![]);
    }
    let _ = click_text(&ctx, &mut app, size, "Menu audit grid");
    assert!(app.use_custom);
    assert_eq!(app.col_weights, [0.7, 0.3]);
    assert_eq!(app.row_weights, [0.4, 0.6]);
    assert_eq!(app.disabled_cells, [3].into_iter().collect());
    let saved: Config = crate::persistence::load_toml([scratch.0.join("config.toml")]);
    assert!(saved.defaults.use_custom);
    assert_eq!(saved.defaults.col_weights, app.col_weights);
    assert_eq!(saved.defaults.row_weights, app.row_weights);
    assert_eq!(saved.defaults.disabled_cells, [3]);
    assert!(desktop.0.lock().unwrap().moved.is_empty());
    assert!(desktop.0.lock().unwrap().focused.is_empty());
}

#[test]
fn actual_pin_controls_edit_slots_show_unavailable_rules_and_persist_removal() {
    use crate::ui_tests::{click_text, frame, visible_text_position};
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1, 2]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.use_custom = true;
    app.detail_tab = 1;
    app.disabled_cells.insert(3);
    app.config.pin = vec![
        config::PinRule {
            title_exact: Some("Exact title".into()),
            bound_hwnd: None,
            process: None,
            title_contains: None,
            slot: 0,
        },
        config::PinRule {
            title_exact: None,
            bound_hwnd: None,
            process: None,
            title_contains: Some("Partial title".into()),
            slot: 1,
        },
        config::PinRule {
            title_exact: None,
            bound_hwnd: None,
            process: Some("owned-2.exe".into()),
            title_contains: None,
            slot: 2,
        },
        config::PinRule {
            title_exact: None,
            bound_hwnd: None,
            process: None,
            title_contains: None,
            slot: 3,
        },
    ];
    let ctx = egui::Context::default();
    let size = egui::vec2(1080.0, 1200.0);
    let output = frame(&ctx, &mut app, size, vec![]);
    for text in [
        "Exact title",
        "Partial title",
        "owned-2.exe",
        "Unnamed rule",
        "Slot unavailable in this layout",
    ] {
        assert!(
            visible_text_position(&ctx, &output, text).is_some(),
            "pin label {text}"
        );
    }
    let _ = click_text(&ctx, &mut app, size, "4");
    let _ = click_text(&ctx, &mut app, size, "4");
    let _ = frame(
        &ctx,
        &mut app,
        size,
        vec![
            egui::Event::Key {
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
            egui::Event::Text("2".into()),
            egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            },
        ],
    );
    assert_eq!(app.config.pin[3].slot, 1);
    let saved: Config = crate::persistence::load_toml([scratch.0.join("config.toml")]);
    assert_eq!(saved.pin[3].slot, 1);
    let _ = click_text(&ctx, &mut app, size, "Remove");
    assert_eq!(app.config.pin.len(), 3);
    let saved: Config = crate::persistence::load_toml([scratch.0.join("config.toml")]);
    assert_eq!(saved.pin.len(), 3);
    assert_eq!(saved.pin[2].process.as_deref(), Some("owned-2.exe"));
    assert!(desktop.0.lock().unwrap().moved.is_empty());
}

#[test]
fn saving_the_same_grid_replaces_its_weights_and_disabled_cells() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new(vec![]);
    let mut app = live_app(Config::default(), desktop, &scratch);
    app.save_current_as_grid("Replace audit".into());
    app.custom_cols = 3;
    app.custom_rows = 1;
    app.col_weights = vec![0.2, 0.3, 0.5];
    app.row_weights = vec![1.0];
    app.disabled_cells.insert(1);
    app.save_current_as_grid("Replace audit".into());
    assert_eq!(app.config.saved_grid.len(), 1);
    let saved: Config = crate::persistence::load_toml([scratch.0.join("config.toml")]);
    assert_eq!(saved.saved_grid.len(), 1);
    assert_eq!(saved.saved_grid[0].cols, 3);
    assert_eq!(saved.saved_grid[0].rows, 1);
    assert_eq!(saved.saved_grid[0].col_weights, [0.2, 0.3, 0.5]);
    assert_eq!(saved.saved_grid[0].disabled_cells, [1]);
    assert_eq!(
        app.presets
            .iter()
            .filter(|(name, _)| name == "Replace audit")
            .count(),
        1
    );
}

#[test]
fn pinning_an_overflow_window_uses_the_first_unreserved_slot_and_preserves_existing_pins() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1, 2, 3]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.refresh_windows();
    app.toggle_pin(1);
    app.toggle_pin(3);
    assert_eq!(app.config.pin.len(), 2);
    assert_eq!(app.config.pin[0].bound_hwnd, Some(1));
    assert_eq!(app.config.pin[0].slot, 0);
    assert_eq!(app.config.pin[1].bound_hwnd, Some(3));
    assert_eq!(app.config.pin[1].slot, 1);
    assert_eq!(app.assignment().slots, [Some(0), Some(2)]);
    app.toggle_pin(2);
    assert_eq!(app.config.pin.len(), 2);
    assert_eq!(app.status, "No available slot to pin. Enable a slot first.");
    let saved: Config = crate::persistence::load_toml([scratch.0.join("config.toml")]);
    assert_eq!(saved.pin.len(), 2);
    assert!(desktop.0.lock().unwrap().moved.is_empty());
}

fn worker_state(app: &PsmApp) -> WorkerState {
    WorkerState {
        live: app.live.clone(),
        activity: app.activity.clone(),
        history: app.history.clone(),
        quitting: app.quitting.clone(),
    }
}
fn poison<T>(mutex: &Mutex<T>) {
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = mutex.lock().unwrap();
        panic!("isolated app state failure");
    }))
    .is_err());
}
fn tick(app: &mut PsmApp, ctx: &egui::Context) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1080.0, 800.0),
            )),
            ..Default::default()
        },
        |ctx| app.update_view(ctx),
    )
}

#[test]
fn gui_and_tray_apply_share_live_order_pins_weights_masks_and_multi_step_undo() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new(1..=4);
    let original = desktop.0.lock().unwrap().snapshots.clone();
    let mut config = Config::default();
    config.defaults.use_custom = true;
    config.defaults.manual_order = true;
    config.defaults.smart_sort = true;
    config.defaults.custom_cols = 3;
    config.defaults.custom_rows = 2;
    config.defaults.col_weights = vec![0.5, 0.3, 0.2];
    config.defaults.row_weights = vec![0.4, 0.6];
    config.defaults.disabled_cells = vec![1];
    config.defaults.gap = 7;
    config.defaults.target = " terminals ".into();
    config.categories.exclude = vec!["Keep.exe".into()];
    config.pin.push(config::PinRule {
        process: Some("owned-4.exe".into()),
        title_exact: Some("Owned app test 4".into()),
        title_contains: None,
        bound_hwnd: None,
        slot: 5,
    });
    let mut app = live_app(config, desktop.clone(), &scratch);
    app.managed_windows = [3, 1, 4, 2].into_iter().map(window).collect();
    app.apply_current_layout();
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|w| w.hwnd)
            .collect::<Vec<_>>(),
        [3, 1, 4, 2]
    );
    assert_eq!(app.config.pin[0].bound_hwnd, Some(4));
    assert_eq!(app.status, "Arranged 4 windows. 0 skipped. 0 errors.");
    let first_moves = desktop.0.lock().unwrap().moved.clone();
    assert_eq!(first_moves.len(), 4);
    assert_eq!(
        desktop.0.lock().unwrap().queries[0],
        (TargetFilter::Terminals, 444, vec!["keep.exe".into()])
    );
    let expected = crate::order::placements(
        &app.active_preset(),
        &app.monitors[0].work_area,
        7,
        &app.disabled_cells,
        Some((&app.col_weights, &app.row_weights)),
        &app.managed_windows,
        &app.config.pin,
    )
    .0
    .into_iter()
    .map(|(hwnd, slot)| (hwnd, slot.x, slot.y, slot.w, slot.h))
    .collect::<Vec<_>>();
    assert_eq!(first_moves, expected);
    app.undo_layout();
    assert_eq!(desktop.0.lock().unwrap().snapshots, original);
    let state = worker_state(&app);
    let ctx = egui::Context::default();
    dispatch_tray_action(
        tray::TrayAction::ApplyCurrent,
        &mut desktop.clone(),
        &ctx,
        444,
        &state,
    );
    assert_eq!(&desktop.0.lock().unwrap().moved[4..], first_moves);
    app.theme_dirty = false;
    let _ = tick(&mut app, &ctx);
    assert_eq!(app.status, "Arranged 4 windows. 0 skipped. 0 errors.");
    assert!(app.history.lock().unwrap().pending_status.is_none());
    dispatch_tray_action(
        tray::TrayAction::UndoLayout,
        &mut desktop.clone(),
        &ctx,
        444,
        &state,
    );
    assert_eq!(desktop.0.lock().unwrap().snapshots, original);
    app.undo_layout();
    assert_eq!(app.status, "No layout changes to undo.");
}

#[test]
fn app_reports_missing_monitors_partial_apply_and_retryable_undo_without_losing_history() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new(1..=3);
    let mut config = Config::default();
    config.defaults.use_custom = true;
    let mut app = live_app(config, desktop.clone(), &scratch);
    desktop.0.lock().unwrap().monitors.clear();
    app.apply_current_layout();
    assert_eq!(app.status, "Arranged 0 windows. 3 skipped. 1 errors.");
    assert!(desktop.0.lock().unwrap().captures.is_empty());
    desktop.0.lock().unwrap().monitors = TestDesktop::new([]).0.lock().unwrap().monitors.clone();
    desktop.0.lock().unwrap().capture_error.insert(1);
    desktop.0.lock().unwrap().position_error.insert(2);
    app.apply_current_layout();
    assert_eq!(app.status, "Arranged 1 windows. 0 skipped. 2 errors.");
    assert_eq!(
        desktop
            .0
            .lock()
            .unwrap()
            .moved
            .iter()
            .map(|record| record.0)
            .collect::<Vec<_>>(),
        [3]
    );
    desktop.0.lock().unwrap().restore_error.insert(3);
    app.undo_layout();
    assert_eq!(
        app.status,
        "Restored 0 windows. 0 closed or replaced. 1 errors."
    );
    assert!(app.history.lock().unwrap().can_undo());
    desktop.0.lock().unwrap().restore_error.clear();
    app.undo_layout();
    assert_eq!(
        app.status,
        "Restored 1 windows. 0 closed or replaced. 0 errors."
    );
    assert!(!app.history.lock().unwrap().can_undo());
}

#[test]
fn poisoned_history_prevents_gui_and_tray_moves_and_poisoned_live_settings_are_reported() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    poison(&app.history);
    app.apply_current_layout();
    assert_eq!(
        app.status,
        "Could not access layout history. No windows moved."
    );
    app.undo_layout();
    assert_eq!(app.status, "Could not access layout history.");
    let state = worker_state(&app);
    dispatch_tray_action(
        tray::TrayAction::ApplyCurrent,
        &mut desktop.clone(),
        &egui::Context::default(),
        444,
        &state,
    );
    dispatch_tray_action(
        tray::TrayAction::UndoLayout,
        &mut desktop.clone(),
        &egui::Context::default(),
        444,
        &state,
    );
    assert!(desktop.0.lock().unwrap().moved.is_empty());
    app.history = Arc::new(Mutex::new(Default::default()));
    poison(&app.live);
    app.sync_live();
    dispatch_tray_action(
        tray::TrayAction::ApplyCurrent,
        &mut desktop.clone(),
        &egui::Context::default(),
        444,
        &worker_state(&app),
    );
    assert_eq!(
        app.history.lock().unwrap().pending_status.as_deref(),
        Some("Could not access current settings. No windows moved.")
    );
    assert!(desktop.0.lock().unwrap().moved.is_empty());
}

#[test]
fn app_save_errors_reach_status_and_successful_saves_use_only_the_supplied_path() {
    let scratch = Scratch::new();
    let mut app = live_app(Config::default(), TestDesktop::new([]), &scratch);
    app.settings_path = None;
    app.save_config();
    assert_eq!(app.status, "Could not locate the settings directory");
    let blocked = scratch.0.join("blocked");
    std::fs::write(&blocked, b"original").unwrap();
    app.settings_path = Some(blocked.join("config.toml"));
    app.save_config();
    assert!(app.status.contains("Could not create"));
    assert_eq!(std::fs::read(&blocked).unwrap(), b"original");
    app.settings_path = Some(scratch.0.join("valid/config.toml"));
    app.config.defaults.gap = 17;
    app.save_config();
    let saved: Config = crate::persistence::load_toml([app.settings_path.clone().unwrap()]);
    assert_eq!(saved.defaults.gap, 17);
    assert_eq!(app.live.lock().unwrap().config.defaults.gap, 17);
}

struct Events {
    steps: VecDeque<(tray::TrayAction, bool)>,
    active: Option<(tray::TrayAction, bool)>,
    quitting: Arc<AtomicBool>,
    stop_during_wait: bool,
    waits: usize,
}
impl ActionEvents for Events {
    fn wait(&mut self) {
        self.waits += 1;
        if self.stop_during_wait {
            self.quitting.store(true, Ordering::Relaxed);
        }
        self.active = self.steps.pop_front();
    }
    fn menu(&mut self) -> tray::TrayAction {
        self.active
            .as_ref()
            .expect("scripted worker step")
            .0
            .clone()
    }
    fn hotkey(&mut self) -> bool {
        self.active.take().unwrap().1
    }
}

#[test]
fn complete_worker_loop_preserves_menu_priority_updates_shared_history_and_closes_owned_viewport() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1, 2]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.config.defaults.use_custom = true;
    app.config.defaults.custom_cols = 2;
    app.config.defaults.custom_rows = 1;
    app.sync_live();
    let state = worker_state(&app);
    let mut events = Events {
        steps: [
            (tray::TrayAction::ShowGui, true),
            (tray::TrayAction::None, false),
            (tray::TrayAction::UndoLayout, true),
            (tray::TrayAction::None, false),
            (tray::TrayAction::UndoLayout, false),
            (tray::TrayAction::Quit, true),
        ]
        .into_iter()
        .collect(),
        active: None,
        quitting: app.quitting.clone(),
        stop_during_wait: false,
        waits: 0,
    };
    let ctx = egui::Context::default();
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        run_action_worker(&mut events, &mut desktop.clone(), ctx, 444, &state)
    });
    assert_eq!(events.waits, 6);
    assert!(state.quitting.load(Ordering::Relaxed));
    let desk = desktop.0.lock().unwrap();
    assert_eq!(desk.moved.len(), 2);
    assert_eq!(desk.restored.len(), 2);
    assert_eq!(desk.shown, [444]);
    assert_eq!(desk.woken, [444]);
    assert_eq!(
        state.history.lock().unwrap().pending_status.as_deref(),
        Some("No layout changes to undo.")
    );
    assert!(output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .any(|cmd| matches!(cmd, egui::ViewportCommand::Close)));
}

#[test]
fn tray_hide_show_and_refresh_commands_preserve_history_and_refresh_the_inventory() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1, 2]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.refresh_windows();
    let state = worker_state(&app);
    let ctx = egui::Context::default();
    app.history.lock().unwrap().record(vec![snapshot(1)]);
    dispatch_tray_action(
        tray::TrayAction::HideGui,
        &mut desktop.clone(),
        &ctx,
        444,
        &state,
    );
    let _ = tick(&mut app, &ctx);
    assert!(!app.gui_visible);
    assert!(app.history.lock().unwrap().can_undo());
    assert!(!app.quitting.load(Ordering::Relaxed));
    desktop.0.lock().unwrap().fresh.push(window(3));
    dispatch_tray_action(
        tray::TrayAction::RefreshWindows,
        &mut desktop.clone(),
        &ctx,
        444,
        &state,
    );
    let _ = tick(&mut app, &ctx);
    assert_eq!(app.managed_windows.len(), 3);
    assert_eq!(app.status, "Window list refreshed.");
    assert!(!app.gui_visible);
    dispatch_tray_action(
        tray::TrayAction::ShowGui,
        &mut desktop.clone(),
        &ctx,
        444,
        &state,
    );
    let _ = tick(&mut app, &ctx);
    assert!(app.gui_visible);
    assert!(app.history.lock().unwrap().can_undo());
    assert!(desktop.0.lock().unwrap().moved.is_empty());
    poison(&app.history);
    dispatch_tray_action(
        tray::TrayAction::RefreshWindows,
        &mut desktop.clone(),
        &ctx,
        444,
        &state,
    );
    assert_eq!(app.status, "Window list refreshed.");
}

#[test]
fn worker_stopping_before_start_or_during_wait_cannot_process_a_pending_apply() {
    let scratch = Scratch::new();
    for before_start in [false, true] {
        let desktop = TestDesktop::new([1]);
        let app = live_app(Config::default(), desktop.clone(), &scratch);
        app.quitting.store(before_start, Ordering::Relaxed);
        let mut events = Events {
            steps: [(tray::TrayAction::ApplyCurrent, true)]
                .into_iter()
                .collect(),
            active: None,
            quitting: app.quitting.clone(),
            stop_during_wait: true,
            waits: 0,
        };
        let state = worker_state(&app);
        run_action_worker(
            &mut events,
            &mut desktop.clone(),
            &egui::Context::default(),
            444,
            &state,
        );
        assert_eq!(events.waits, usize::from(!before_start));
        assert!(desktop.0.lock().unwrap().queries.is_empty());
        assert!(desktop.0.lock().unwrap().moved.is_empty());
    }
    tray_event_loop(
        Arc::new(Mutex::new(None)),
        egui::Context::default(),
        444,
        WorkerState {
            live: Arc::new(Mutex::new(LiveState {
                config: Config::default(),
                windows: vec![],
            })),
            activity: Arc::new(Mutex::new(ActivityTracker::inert())),
            history: Arc::new(Mutex::new(Default::default())),
            quitting: Arc::new(AtomicBool::new(true)),
        },
        Box::new(Keys::default()),
    );
}

#[test]
fn refresh_and_worker_ranking_use_live_scores_manual_order_wins_and_unavailable_activity_falls_back(
) {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new(1..=3);
    let mut config = Config::default();
    config.defaults.smart_sort = true;
    let mut app = live_app(config, desktop.clone(), &scratch);
    let activity_path = scratch.0.join("activity.toml");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let db = crate::activity::ActivityDb {
        last_decay_ts: now,
        apps: [(1, 100.0), (2, 200.0), (3, 2.0)]
            .into_iter()
            .map(|(id, score)| {
                (
                    format!("owned-{id}.exe"),
                    crate::activity::AppRecord {
                        total_focus_secs: score,
                        total_switches: 0,
                        last_focus_ts: now,
                        category: "Other".into(),
                        last_title: String::new(),
                    },
                )
            })
            .collect(),
    };
    crate::persistence::save_toml(&activity_path, &db).unwrap();
    app.activity = Arc::new(Mutex::new(ActivityTracker::new(7.0, Some(activity_path))));
    app.refresh_windows();
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|w| w.hwnd)
            .collect::<Vec<_>>(),
        [2, 1, 3]
    );
    let state = worker_state(&app);
    dispatch_tray_action(
        tray::TrayAction::ApplyLayout(tray::LayoutChoice::Preset(LayoutPreset::Columns(3))),
        &mut desktop.clone(),
        &egui::Context::default(),
        444,
        &state,
    );
    assert_eq!(desktop.0.lock().unwrap().captures, [2, 1, 3]);
    app.config.defaults.manual_order = true;
    app.managed_windows = [3, 1].into_iter().map(window).collect();
    app.refresh_windows();
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|w| w.hwnd)
            .collect::<Vec<_>>(),
        [3, 1, 2]
    );
    app.config.defaults.manual_order = false;
    poison(&app.activity);
    app.refresh_windows();
    assert_eq!(
        app.managed_windows
            .iter()
            .map(|w| w.hwnd)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    let mut events = Events {
        steps: [
            (tray::TrayAction::ApplyCurrent, false),
            (tray::TrayAction::Quit, false),
        ]
        .into_iter()
        .collect(),
        active: None,
        quitting: app.quitting.clone(),
        stop_during_wait: false,
        waits: 0,
    };
    run_action_worker(
        &mut events,
        &mut desktop.clone(),
        &egui::Context::default(),
        444,
        &worker_state(&app),
    );
    assert_eq!(events.waits, 2);
    // The poisoned tracker is recovered only for destruction so its private poller
    // stops and saves before the scratch directory is removed.
    app.activity.clear_poison();
}

#[test]
fn saved_tray_layout_resolution_reports_stale_identity_and_preserves_previous_undo() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.config.saved_grid.push(config::SavedGrid {
        name: "Owned grid".into(),
        cols: 1,
        rows: 1,
        col_weights: vec![1.0],
        row_weights: vec![1.0],
        disabled_cells: vec![],
    });
    app.sync_live();
    let action = tray::TrayAction::ApplyLayout(tray::LayoutChoice::Saved {
        index: 0,
        name: "Owned grid".into(),
    });
    let state = worker_state(&app);
    dispatch_tray_action(
        action.clone(),
        &mut desktop.clone(),
        &egui::Context::default(),
        444,
        &state,
    );
    assert!(app.history.lock().unwrap().can_undo());
    app.config.saved_grid.clear();
    app.sync_live();
    dispatch_tray_action(
        action,
        &mut desktop.clone(),
        &egui::Context::default(),
        444,
        &state,
    );
    assert_eq!(
        app.history.lock().unwrap().pending_status.as_deref(),
        Some("Saved layout is unavailable. Reopen the layout menu.")
    );
    assert!(app.history.lock().unwrap().can_undo());
    assert_eq!(desktop.0.lock().unwrap().moved.len(), 1);
}

#[test]
fn app_hidden_state_restores_from_desktop_and_delayed_refresh_keeps_status_delivery() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1, 2]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    let ctx = egui::Context::default();
    let _ = tick(&mut app, &ctx);
    assert!(!app.theme_dirty);
    app.hide_window();
    assert!(!app.gui_visible);
    assert_eq!(desktop.0.lock().unwrap().hidden, [444]);
    let output = tick(&mut app, &ctx);
    assert!(!app.gui_visible);
    assert!(!output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .any(|cmd| matches!(cmd, egui::ViewportCommand::Close)));
    desktop.0.lock().unwrap().app_visible = true;
    app.history.lock().unwrap().pending_status = Some("Owned worker status".into());
    let _ = tick(&mut app, &ctx);
    assert!(app.gui_visible);
    assert_eq!(app.status, "Owned worker status");
    assert_eq!(app.managed_windows.len(), 2);
    desktop.0.lock().unwrap().fresh.push(window(3));
    app.last_refresh = Instant::now() - std::time::Duration::from_secs(4);
    let _ = tick(&mut app, &ctx);
    assert_eq!(app.managed_windows.len(), 3);
    poison(&app.activity);
    poison(&app.history);
    let _ = tick(&mut app, &ctx);
    assert_eq!(app.status, "Could not access layout history.");
}

#[test]
fn malformed_settings_weights_and_preset_rebuilds_recover_to_consistent_live_and_saved_state() {
    let mut config = Config::default();
    config.defaults.ui_scale = f32::NAN;
    config.defaults.custom_cols = 0;
    config.defaults.custom_rows = 99;
    config.defaults.selected_preset = usize::MAX;
    config.defaults.disabled_cells = vec![0, 4, 63, usize::MAX];
    config.layout = vec![config::LayoutDef {
        name: "invalid".into(),
        grid: Some("bad".into()),
        style: None,
        count: None,
    }];
    config.layout.push(config::LayoutDef {
        name: "Configured columns".into(),
        grid: None,
        style: Some("columns".into()),
        count: Some(3),
    });
    let mut app = PsmApp::preview(config);
    assert_eq!(app.config.defaults.ui_scale, 1.0);
    assert_eq!((app.custom_cols, app.custom_rows), (1, 8));
    assert_eq!(app.selected_preset, 0);
    assert_eq!(app.config.defaults.selected_preset, 0);
    assert_eq!(app.disabled_cells, [0].into_iter().collect());
    assert_eq!(app.active_preset(), LayoutPreset::Grid { cols: 1, rows: 2 });
    app.rebuild_presets();
    assert!(app.presets.iter().any(|(name, preset)| {
        name == "Configured columns" && *preset == LayoutPreset::Columns(3)
    }));
    assert_eq!(app.selected_preset, 0);
    assert_eq!(app.config.defaults.selected_preset, 0);
    assert_eq!(app.live.lock().unwrap().config.defaults.selected_preset, 0);
    app.custom_cols = 3;
    app.custom_rows = 2;
    app.ensure_weights();
    assert_eq!(app.col_weights, vec![1.0 / 3.0; 3]);
    assert_eq!(app.row_weights, vec![0.5; 2]);
    assert!(app.weights_are_uniform());
    app.row_weights = vec![0.3, 0.7];
    assert!(!app.weights_are_uniform());
    app.col_weights = vec![0.2, 0.4, 0.4];
    assert!(!app.weights_are_uniform());
    for values in [
        vec![],
        vec![1.0],
        vec![f32::INFINITY, 1.0],
        vec![f32::NAN, 1.0],
        vec![0.0, 0.0],
        vec![-1.0, 2.0],
        vec![0.0, 1.0],
    ] {
        assert_eq!(normalized_weights(&values, 2), vec![0.5, 0.5]);
    }
    assert_eq!(normalized_weights(&[2.0, 6.0], 2), vec![0.25, 0.75]);
    assert_eq!(normalized_weights(&[], 0), vec![1.0]);
    assert_eq!(normalized_weights(&[], 99), vec![0.125; 8]);
}

#[test]
fn imported_extreme_weight_ratios_remain_positive_after_normalization() {
    let mut config = Config::default();
    config.defaults.col_weights = vec![f32::MIN_POSITIVE, f32::MAX];
    config.defaults.use_custom = true;
    let mut app = PsmApp::preview(config);
    app.ensure_weights();
    assert!(app
        .col_weights
        .iter()
        .all(|weight| weight.is_finite() && *weight > 0.0));
    assert_eq!(app.col_weights.iter().sum::<f32>(), 1.0);
    let ctx = egui::Context::default();
    let _ = tick(&mut app, &ctx);
    assert!(app.col_weights.iter().all(|weight| *weight > 0.0));
}

#[test]
fn builtin_apply_uses_equal_slots_and_preserves_broad_and_unavailable_pins() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1, 2]);
    let mut config = Config::default();
    config.pin = vec![
        config::PinRule {
            process: Some("owned-1.exe".into()),
            title_exact: None,
            title_contains: None,
            bound_hwnd: None,
            slot: 0,
        },
        config::PinRule {
            process: Some("unavailable.exe".into()),
            title_exact: None,
            title_contains: None,
            bound_hwnd: None,
            slot: 1,
        },
    ];
    let mut app = live_app(config, desktop.clone(), &scratch);
    app.selected_preset = app
        .presets
        .iter()
        .position(|(_, preset)| *preset == LayoutPreset::LeftRight)
        .unwrap();
    app.use_custom = false;
    app.col_weights = vec![0.95, 0.05];
    app.apply_current_layout();
    assert_eq!(app.config.pin[0].bound_hwnd, Some(1));
    assert!(app.config.pin[0].title_exact.is_none());
    assert!(app.config.pin[1].bound_hwnd.is_none());
    let slots = LayoutPreset::LeftRight.compute_slots(&app.monitors[0].work_area, 4);
    let moved = desktop.0.lock().unwrap().moved.clone();
    assert_eq!(moved.len(), 2);
    for (movement, slot) in moved.iter().zip(&slots) {
        assert_eq!(
            (movement.1, movement.2, movement.3, movement.4),
            (slot.x, slot.y, slot.w, slot.h)
        );
    }
    app.reorder_window(1, 2, true);
    assert_eq!(app.config.pin[0].slot, 1);
    assert_eq!(app.config.pin[1].slot, 1);
    assert_eq!(app.managed_windows[1].hwnd, 1);
    assert_eq!(desktop.0.lock().unwrap().moved, moved);
    app.use_custom = true;
    app.custom_cols = 1;
    app.custom_rows = 1;
    app.ensure_weights();
    app.reorder_window(1, 2, false);
    assert_eq!(app.config.pin[0].slot, 0);
    app.reorder_window(1, 2, true);
    assert_eq!(app.config.pin[0].slot, 0);
    assert_eq!(app.assignment().slots, [Some(1)]);
    assert_eq!(desktop.0.lock().unwrap().moved, moved);
}

#[test]
fn close_without_a_tray_is_not_intercepted_and_hidden_preview_has_no_desktop_effects() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.refresh_windows();
    let ctx = egui::Context::default();
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1080.0, 800.0),
        )),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let output = ctx.run(input, |ctx| app.update_view(ctx));
    assert!(!output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .any(|command| matches!(command, egui::ViewportCommand::CancelClose)));
    assert!(desktop.0.lock().unwrap().hidden.is_empty());
    app.native_enabled = false;
    app.gui_visible = false;
    let _ = tick(&mut app, &ctx);
    assert!(!app.gui_visible);
    let desktop = desktop.0.lock().unwrap();
    assert!(desktop.shown.is_empty());
    assert!(desktop.hidden.is_empty());
    assert!(desktop.moved.is_empty());
}

#[test]
fn actual_gap_control_saves_its_value_without_moving_windows() {
    use crate::ui_tests::{edit_text_at, frame, visible_text_position};
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1, 2]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.refresh_windows();
    let ctx = egui::Context::default();
    let size = egui::vec2(1080.0, 1400.0);
    let _ = frame(&ctx, &mut app, size, vec![]);
    let output = frame(&ctx, &mut app, size, vec![]);
    let position = visible_text_position(&ctx, &output, "4 px").unwrap();
    edit_text_at(&ctx, &mut app, size, position, "17");
    assert_eq!(app.config.defaults.gap, 17);
    let saved: Config = crate::persistence::load_toml([app.settings_path.clone().unwrap()]);
    assert_eq!(saved.defaults.gap, 17);
    assert!(desktop.0.lock().unwrap().moved.is_empty());
}

#[test]
fn empty_live_inventory_disables_apply_without_disabling_refresh() {
    use crate::ui_tests::{click_text, frame, visible_text_position};
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.refresh_windows();
    let ctx = egui::Context::default();
    let size = egui::vec2(1080.0, 800.0);
    let _ = frame(&ctx, &mut app, size, vec![]);
    let output = frame(&ctx, &mut app, size, vec![]);
    assert!(app.actions_available());
    assert!(visible_text_position(
        &ctx,
        &output,
        "No matching windows found. Try All windows or Refresh."
    )
    .is_some());
    click_text(&ctx, &mut app, size, "Apply layout");
    assert!(desktop.0.lock().unwrap().captures.is_empty());
    assert!(desktop.0.lock().unwrap().moved.is_empty());
    let queries = desktop.0.lock().unwrap().queries.len();
    click_text(&ctx, &mut app, size, "Refresh");
    assert_eq!(desktop.0.lock().unwrap().queries.len(), queries + 1);
}

#[test]
fn disabling_every_slot_disables_apply_until_a_slot_is_enabled() {
    use crate::ui_tests::{click_text, frame};
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([10]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.use_custom = true;
    app.refresh_windows();
    app.disabled_cells = (0..app.active_preset().slot_count()).collect();
    let ctx = egui::Context::default();
    let size = egui::vec2(1080.0, 800.0);
    let _ = frame(&ctx, &mut app, size, vec![]);
    click_text(&ctx, &mut app, size, "Apply layout");
    assert!(desktop.0.lock().unwrap().captures.is_empty());
    assert!(desktop.0.lock().unwrap().moved.is_empty());
    click_text(&ctx, &mut app, size, "Enable all");
    assert!(app.disabled_cells.is_empty());
    click_text(&ctx, &mut app, size, "Apply layout");
    assert_eq!(desktop.0.lock().unwrap().moved.len(), 1);
}

#[test]
fn actual_apply_and_undo_buttons_share_history_and_restore_the_owned_windows() {
    use crate::ui_tests::{click_text, frame};
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([10, 20]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.refresh_windows();
    let ctx = egui::Context::default();
    let size = egui::vec2(1080.0, 800.0);
    let _ = frame(&ctx, &mut app, size, vec![]);
    click_text(&ctx, &mut app, size, "Apply layout");
    assert_eq!(desktop.0.lock().unwrap().moved.len(), 2);
    assert!(app.history.lock().unwrap().can_undo());
    click_text(&ctx, &mut app, size, "Undo layout");
    assert_eq!(desktop.0.lock().unwrap().restored, [10, 20]);
    assert!(!app.history.lock().unwrap().can_undo());
    let status = app.status.clone();
    click_text(&ctx, &mut app, size, "Undo layout");
    assert_eq!(
        app.status, status,
        "empty history disables the same Undo button"
    );
    assert_eq!(desktop.0.lock().unwrap().restored, [10, 20]);
}

#[test]
fn unavailable_or_non_windows_native_handle_disables_actions_before_any_startup_effects() {
    use raw_window_handle::{RawWindowHandle, Win32WindowHandle, XlibWindowHandle};
    assert_eq!(native_hwnd(None), None);
    assert_eq!(
        native_hwnd(Some(RawWindowHandle::Xlib(XlibWindowHandle::new(1)))),
        None
    );
    assert_eq!(
        native_hwnd(Some(RawWindowHandle::Win32(Win32WindowHandle::new(
            std::num::NonZeroIsize::new(42).unwrap()
        )))),
        Some(42)
    );
    let cc = eframe::CreationContext::_new_kittest(egui::Context::default());
    let app = PsmApp::new(&cc, Config::default());
    assert!(!app.native_enabled);
    assert!(!app.actions_available());
    assert!(app.action_worker.is_none());
    assert!(app._tray_icon.is_none());
    assert!(app.settings_path.is_none());
    assert_eq!(
        app.status,
        "Native window is unavailable. Layout actions are disabled."
    );
}

#[test]
fn deleting_selected_grid_saves_the_recovered_selection_and_pin_failures_do_not_create_rules() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1]);
    let mut app = live_app(Config::default(), desktop, &scratch);
    app.config.saved_grid.push(config::SavedGrid {
        name: "Delete me".into(),
        cols: 1,
        rows: 1,
        col_weights: vec![1.0],
        row_weights: vec![1.0],
        disabled_cells: vec![],
    });
    // Build the catalogue without native tray effects for this storage regression.
    app.native_enabled = false;
    app.rebuild_presets();
    app.selected_preset = app.presets.len() - 1;
    app.config.defaults.selected_preset = app.selected_preset;
    app.delete_saved_grid("Delete me");
    assert_eq!(app.selected_preset, 0);
    assert_eq!(app.config.defaults.selected_preset, 0);
    app.native_enabled = true;
    app.save_config();
    let saved: Config = crate::persistence::load_toml([app.settings_path.clone().unwrap()]);
    assert_eq!(saved.defaults.selected_preset, 0);
    app.managed_windows = vec![window(1)];
    app.toggle_pin(99);
    assert!(app.config.pin.is_empty());
    app.disabled_cells = (0..app.active_preset().slot_count()).collect();
    app.toggle_pin(1);
    assert!(app.config.pin.is_empty());
    assert_eq!(app.status, "No available slot to pin. Enable a slot first.");
    app.disabled_cells.clear();
    app.toggle_pin(1);
    assert_eq!(app.config.pin.len(), 1);
    app.toggle_pin(1);
    assert!(app.config.pin.is_empty());
    app.reorder_window(99, 1, true);
    app.reorder_window(1, 1, false);
    assert!(!app.config.defaults.manual_order);
}

#[derive(Default)]
struct Keys {
    fail: bool,
    messages: std::cell::RefCell<VecDeque<i32>>,
    released: std::cell::Cell<usize>,
}
impl crate::hotkey::HotkeyApi for Keys {
    fn register(
        &self,
        id: i32,
        _: windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS,
        key: u32,
    ) -> Result<(), String> {
        assert_eq!((id, key), (1, 0x47));
        if self.fail {
            Err("owned test key unavailable".into())
        } else {
            Ok(())
        }
    }
    fn unregister(&self, _: i32) -> Result<(), String> {
        self.released.set(self.released.get() + 1);
        Ok(())
    }
    fn receive(&self) -> Option<i32> {
        self.messages.borrow_mut().pop_front()
    }
}

#[test]
fn native_event_adapter_handles_missing_or_poisoned_menus_and_failed_registration() {
    crate::action_tests::init_logging();
    let ids = Mutex::new(None);
    let keys = Keys {
        messages: std::cell::RefCell::new([99, 1].into_iter().collect()),
        ..Default::default()
    };
    let mut events = NativeActionEvents::new(&ids, &keys);
    assert_eq!(events.menu(), tray::TrayAction::None);
    assert!(events.hotkey());
    assert!(!events.hotkey());
    events.wait();
    poison(&ids);
    assert_eq!(events.menu(), tray::TrayAction::None);
    drop(events);
    assert_eq!(keys.released.get(), 1);
    let unavailable = Keys {
        fail: true,
        ..Default::default()
    };
    let mut events = NativeActionEvents::new(&ids, &unavailable);
    assert!(!events.hotkey());
    drop(events);
    assert_eq!(unavailable.released.get(), 0);
}

#[test]
#[ignore = "Creates only owned tray resources; app actions use controlled desktop and local storage"]
fn native_audit_app_tray_rebuilds_menu_binding_and_close_interception_use_owned_resources() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.rebuild_tray();
    assert!(app._tray_icon.is_some());
    app.install_theme(&egui::Context::default());
    let keys = Keys::default();
    let mut events = NativeActionEvents::new(&app.tray_ids, &keys);
    assert_eq!(events.menu(), tray::TrayAction::None);
    drop(events);
    let ctx = egui::Context::default();
    app.sync_tray_state();
    app.refresh_windows();
    app.sync_tray_state();
    app.use_custom = true;
    app.sync_tray_state();
    app.disabled_cells = (0..4).collect();
    app.sync_tray_state();
    app.disabled_cells.clear();
    app.monitors.clear();
    app.sync_tray_state();
    app.refresh_windows();
    app.use_custom = false;
    app.selected_preset = usize::MAX;
    app.sync_tray_state();
    app.selected_preset = 0;
    app.native_enabled = false;
    app.sync_tray_state();
    app.native_enabled = true;
    app.history.lock().unwrap().record(vec![snapshot(1)]);
    let mut input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1080.0, 800.0),
        )),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let output = ctx.run(input.clone(), |ctx| app.update_view(ctx));
    assert!(!app.gui_visible);
    assert!(!app.quitting.load(Ordering::Relaxed));
    assert!(app.history.lock().unwrap().can_undo());
    assert!(app._tray_icon.is_some());
    assert_eq!(desktop.0.lock().unwrap().hidden, [444]);
    assert!(output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .any(|cmd| matches!(cmd, egui::ViewportCommand::CancelClose)));
    let state = worker_state(&app);
    dispatch_tray_action(
        tray::TrayAction::ShowGui,
        &mut desktop.clone(),
        &ctx,
        444,
        &state,
    );
    let _ = tick(&mut app, &ctx);
    assert!(app.gui_visible);
    assert!(app.history.lock().unwrap().can_undo());
    dispatch_tray_action(
        tray::TrayAction::Quit,
        &mut desktop.clone(),
        &ctx,
        444,
        &state,
    );
    assert!(app.quitting.load(Ordering::Relaxed));
    let output = ctx.run(input, |ctx| app.update_view(ctx));
    assert!(!output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .any(|cmd| matches!(cmd, egui::ViewportCommand::CancelClose)));
    poison(&app.tray_ids);
    app.rebuild_tray();
    assert_eq!(
        app.status,
        "Could not update tray: action state is unavailable."
    );
    assert!(app._tray_icon.is_some());
    println!("NATIVE APP TRAY PASS: owned icon/menu, shared adapter, theme install, close interception and poisoned rebuild preserve original binding; desktop effects controlled; local settings only");
}

#[test]
fn update_versions_reject_malformed_and_prerelease_tags() {
    for latest in ["9.invalid.0", "1.0.0-rc.1", "9.0", "9.0.0.1", "09.0.0"] {
        assert!(
            !crate::updates::version_newer(latest, "0.4.2"),
            "invalid stable release: {latest}"
        );
    }
    assert!(crate::updates::version_newer("0.4.10", "0.4.2"));
    assert!(!crate::updates::version_newer("0.4.2+build.1", "0.4.2"));
}

#[test]
fn native_startup_uses_explicit_storage_and_services_and_update_failure_is_nonfatal() {
    crate::action_tests::init_logging();
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1, 2]);
    let server = crate::updates::tests::Fixture::json(r#"{"tag_name":"v9.0.0"}"#);
    let ctx = egui::Context::default();
    let mut app = PsmApp::build(
        Config::default(),
        444,
        true,
        AppRuntime {
            desktop: Box::new(desktop.clone()),
            settings_path: Some(scratch.0.join("config.toml")),
            activity_path: Some(scratch.0.join("activity.toml")),
            update_client: Some(server.client()),
        },
        &ctx,
    );
    assert_eq!(app.monitors.len(), 1);
    assert_eq!(app.managed_windows.len(), 2);
    app.update_worker.take().unwrap().join().unwrap();
    assert_eq!(
        app.update_info
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .latest_version,
        "9.0.0"
    );
    app.set_update_worker(Err(std::io::Error::other(
        "owned update thread start failure",
    )));
    assert_eq!(app.status, "Ready. Choose a layout, then apply.");
    assert!(app.actions_available());
    assert!(desktop.0.lock().unwrap().moved.is_empty());
    drop(app);
    assert!(scratch.0.join("config.toml").is_file());
    assert!(scratch.0.join("activity.toml").is_file());

    let runtime = AppRuntime::native();
    assert!(runtime
        .settings_path
        .unwrap()
        .ends_with(".powershellmanager/config.toml"));
    assert!(runtime
        .activity_path
        .unwrap()
        .ends_with(".powershellmanager/activity.toml"));
    assert!(runtime.update_client.is_some());
}

#[derive(Default)]
struct TestGui {
    requests: Vec<crate::launch::GuiRequest>,
    error: Option<String>,
}

impl crate::launch::GuiRunner for TestGui {
    fn run(&mut self, request: crate::launch::GuiRequest) -> Result<(), String> {
        self.requests.push(request);
        match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

fn arguments(arguments: &[&str]) -> crate::cli::Cli {
    use clap::Parser;
    crate::cli::Cli::try_parse_from(
        std::iter::once("powershellmanager").chain(arguments.iter().copied()),
    )
    .unwrap()
}

#[test]
fn command_inventory_uses_configured_or_explicit_filter_without_any_layout_effects() {
    let mut gui = TestGui::default();
    for (args, expected, json) in [
        (vec!["--list"], TargetFilter::Terminals, false),
        (
            vec!["--list", "--json", "--target", "all"],
            TargetFilter::Universal,
            true,
        ),
    ] {
        let mut config = Config::default();
        config.defaults.target = "terminals".into();
        config.categories.exclude = vec!["EXCLUDED.EXE".into()];
        let mut desktop = TestDesktop::new([1, 2]);
        let output = crate::launch::dispatch(arguments(&args), config, &mut desktop, &mut gui);
        assert_eq!(output.exit_code, 0);
        assert!(output.stderr.is_empty());
        if json {
            let inventory: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
            assert_eq!(inventory["count"], 2);
            assert_eq!(inventory["windows"][0]["hwnd"], 1);
        } else {
            assert!(output.stdout.contains("2 windows"));
            assert!(output.stdout.contains("Owned app test 1"));
        }
        let desk = desktop.0.lock().unwrap();
        assert_eq!(desk.queries, [(expected, 0, vec!["excluded.exe".into()])]);
        assert!(desk.captures.is_empty());
        assert!(desk.moved.is_empty());
        assert!(desk.restored.is_empty());
        assert!(gui.requests.is_empty());
    }
}

#[test]
fn command_layout_uses_display_gap_order_and_pins_with_all_requested_cells_enabled() {
    let mut desktop = TestDesktop::new([1, 2, 3]);
    desktop.0.lock().unwrap().monitors.push(MonitorInfo {
        index: 1,
        is_primary: false,
        work_area: Rect {
            x: 100,
            y: 200,
            w: 1000,
            h: 600,
        },
    });
    let mut config = Config::default();
    config.defaults.monitor = "1".into();
    config.defaults.gap = 10;
    config.defaults.manual_order = true;
    config.defaults.disabled_cells = vec![0, 1];
    config.defaults.col_weights = vec![0.9, 0.1];
    config.window_order = [3, 2, 1]
        .map(|handle| crate::order::WindowKey::from_window(&window(handle)))
        .to_vec();
    config.pin = vec![config::PinRule {
        title_exact: Some(window(1).title),
        bound_hwnd: None,
        process: None,
        title_contains: None,
        slot: 1,
    }];
    let mut gui = TestGui::default();
    // This calls the same dispatcher used by the executable, with a controlled
    // desktop. The real --headless command is never invoked as a test.
    let output = crate::launch::dispatch(
        arguments(&["--headless", "columns:2"]),
        config,
        &mut desktop,
        &mut gui,
    );
    assert_eq!(output.exit_code, 0);
    assert!(output.stdout.starts_with("Arranged 2 windows"));
    assert!(output.stdout.contains("Skipped 1 windows"));
    assert!(output.stderr.is_empty());
    assert_eq!(
        desktop.0.lock().unwrap().moved,
        [(3, 100, 200, 495, 600), (1, 605, 200, 495, 600)]
    );
    assert!(gui.requests.is_empty());
}

#[test]
fn command_errors_report_invalid_layout_no_display_pin_warnings_and_partial_apply() {
    let mut gui = TestGui::default();
    let mut desktop = TestDesktop::new([1, 2]);
    let output = crate::launch::dispatch(
        arguments(&["--headless", "unknown"]),
        Config::default(),
        &mut desktop,
        &mut gui,
    );
    assert_eq!(output.exit_code, 1);
    assert!(output.stderr.contains("Unknown layout: 'unknown'"));
    assert!(output.stderr.contains("Examples:"));
    assert!(output.stdout.is_empty());
    assert!(desktop.0.lock().unwrap().queries.is_empty());
    desktop.0.lock().unwrap().monitors.clear();
    let output = crate::launch::dispatch(
        arguments(&["--headless", "2x1"]),
        Config::default(),
        &mut desktop,
        &mut gui,
    );
    assert_eq!(output.exit_code, 2);
    assert_eq!(output.stderr, "Error: No monitors found\n");
    assert!(output.stdout.contains("Skipped 2 windows\n"));
    assert!(!output.stdout.contains("not enough slots"));
    assert!(desktop.0.lock().unwrap().captures.is_empty());

    let mut desktop = TestDesktop::new([1, 2]);
    desktop.0.lock().unwrap().position_error.insert(2);
    let mut config = Config::default();
    config.pin.push(config::PinRule {
        title_exact: Some(window(1).title),
        bound_hwnd: None,
        process: None,
        title_contains: None,
        slot: 99,
    });
    let output = crate::launch::dispatch(
        arguments(&["--headless", "2x1"]),
        config,
        &mut desktop,
        &mut gui,
    );
    assert_eq!(output.exit_code, 2);
    assert!(output
        .stderr
        .contains("Warning: Pin slot 100 unavailable; window uses the queue"));
    assert!(output.stderr.contains("Error: position denied: 2"));
    assert!(output.stdout.starts_with("Arranged 1 windows"));
    assert!(!output.stdout.contains("Skipped"));
    assert_eq!(desktop.0.lock().unwrap().moved[0].0, 1);
    assert!(gui.requests.is_empty());
}

#[test]
fn command_gui_prepares_theme_and_window_limits_and_reports_startup_errors() {
    let mut desktop = TestDesktop::new([1]);
    let mut gui = TestGui::default();
    for preview in [false, true] {
        let mut config = Config::default();
        let theme = ThemeSettings {
            accent: [240, 20, 30],
            ..Default::default()
        };
        config.defaults.theme_code = Some(theme.encode());
        let args: &[&str] = if preview { &["--preview"] } else { &[] };
        let output = crate::launch::dispatch(arguments(args), config, &mut desktop, &mut gui);
        assert_eq!(output, crate::launch::CommandOutput::default());
        let request = gui.requests.pop().unwrap();
        assert_eq!(request.preview, preview);
        assert_eq!(request.title.ends_with(" (preview)"), preview);
        assert_eq!(
            request.options.viewport.title.as_ref(),
            Some(&request.title)
        );
        assert_eq!(
            request.options.viewport.inner_size,
            Some(egui::vec2(1080.0, 800.0))
        );
        assert_eq!(
            request.options.viewport.min_inner_size,
            Some(egui::vec2(280.0, 300.0))
        );
        assert_eq!(
            request.options.viewport.icon.unwrap().rgba,
            crate::branding::icon(theme, 64).rgba
        );
    }
    let mut config = Config::default();
    config.defaults.theme_code = Some("bad theme".into());
    gui.error = Some("owned GUI runner failure".into());
    let output = crate::launch::dispatch(arguments(&[]), config, &mut desktop, &mut gui);
    assert_eq!(output.exit_code, 1);
    assert_eq!(
        output.stderr,
        "Failed to start GUI: owned GUI runner failure\n"
    );
    assert!(output.stdout.is_empty());
    assert_eq!(
        gui.requests
            .pop()
            .unwrap()
            .options
            .viewport
            .icon
            .unwrap()
            .rgba,
        crate::branding::icon(ThemeSettings::default(), 64).rgba
    );
    assert!(desktop.0.lock().unwrap().queries.is_empty());
}

#[test]
fn command_app_factory_keeps_preview_read_only_and_handles_unavailable_native_context() {
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext::_new_kittest(ctx);
    let mut desktop = TestDesktop::new([1, 2]);
    let mut app = crate::launch::create_app(&cc, Config::default(), true, &mut desktop);
    assert!(!app.native_enabled);
    assert_eq!(app.managed_windows.len(), 2);
    assert_eq!(app.monitors.len(), 1);
    assert!(app.show_theme_studio);
    assert_eq!(
        app.status,
        "Preview mode. Layout actions and saving are disabled."
    );
    app.apply_current_layout();
    app.save_config();
    assert!(desktop.0.lock().unwrap().moved.is_empty());
    let mut desktop = TestDesktop::new([]);
    let app = crate::launch::create_app(&cc, Config::default(), false, &mut desktop);
    assert!(!app.native_enabled);
    assert!(desktop.0.lock().unwrap().queries.is_empty());
    assert_eq!(
        app.status,
        "Native window is unavailable. Layout actions are disabled."
    );
}

#[test]
fn configured_layout_styles_defaults_limits_and_grid_precedence_are_consistent() {
    for (style, expected) in [
        ("columns", Some(LayoutPreset::Columns(2))),
        ("rows", Some(LayoutPreset::Rows(2))),
        ("left-right", Some(LayoutPreset::LeftRight)),
        ("top-bottom", Some(LayoutPreset::TopBottom)),
        ("main-side", Some(LayoutPreset::MainSide { side_count: 2 })),
        ("focus", Some(LayoutPreset::Focus { side_count: 2 })),
        ("unknown", None),
    ] {
        let mut definition = config::LayoutDef {
            name: style.into(),
            grid: None,
            style: Some(style.into()),
            count: None,
        };
        assert_eq!(definition.to_preset(), expected);
        definition.count = Some(2);
        assert_eq!(definition.to_preset(), expected);
        for count in [0, 9, u32::MAX] {
            definition.count = Some(count);
            assert_eq!(definition.to_preset(), None);
        }
        definition.grid = Some("3x2".into());
        assert_eq!(
            definition.to_preset(),
            Some(LayoutPreset::Grid { cols: 3, rows: 2 })
        );
        definition.grid = Some("invalid".into());
        assert_eq!(definition.to_preset(), None);
    }
    let definition = config::LayoutDef {
        name: "empty".into(),
        grid: None,
        style: None,
        count: None,
    };
    assert_eq!(definition.to_preset(), None);
}

#[test]
fn actual_inventory_controls_use_shared_desktop_and_refresh_window_state_immediately() {
    let scratch = Scratch::new();
    let desktop = TestDesktop::new([1, 2]);
    let mut app = live_app(Config::default(), desktop.clone(), &scratch);
    app.refresh_windows();
    let ctx = egui::Context::default();
    let size = egui::vec2(1080.0, 1200.0);
    crate::ui_tests::click_text(&ctx, &mut app, size, "Minimize all");
    assert_eq!(desktop.0.lock().unwrap().minimized, [1, 2]);
    assert!(app.managed_windows.iter().all(|window| window.is_minimized));
    crate::ui_tests::click_text(&ctx, &mut app, size, "Restore all");
    assert_eq!(desktop.0.lock().unwrap().restored_controls, [1, 2]);
    assert_eq!(desktop.0.lock().unwrap().focused, [444]);
    assert!(app
        .managed_windows
        .iter()
        .all(|window| !window.is_minimized));
    let mut commands = desktop.clone();
    commands.minimize_window(2);
    app.refresh_windows();
    crate::ui_tests::click_text(&ctx, &mut app, size, "Focus");
    assert_eq!(desktop.0.lock().unwrap().focused, [444, 2]);
    assert!(app
        .managed_windows
        .iter()
        .all(|window| !window.is_minimized));
    app.focus_window(999);
    assert_eq!(desktop.0.lock().unwrap().focused, [444, 2]);
    assert_eq!(
        app.status,
        "Selected window is no longer available. Refresh the window list."
    );
    assert!(desktop.0.lock().unwrap().moved.is_empty());
    assert!(desktop.0.lock().unwrap().captures.is_empty());
}

#[test]
fn preview_inventory_controls_keep_rows_and_cannot_call_desktop_effects() {
    let ctx = egui::Context::default();
    let desktop = TestDesktop::new([1, 2]);
    let mut app = PsmApp::preview(Config::default());
    app.desktop = Box::new(desktop.clone());
    app.managed_windows = desktop.0.lock().unwrap().fresh.clone();
    let size = egui::vec2(1080.0, 1200.0);
    for (button, message) in [
        ("Minimize all", "Preview only. No windows minimized."),
        ("Restore all", "Preview only. No windows restored."),
        ("Focus", "Preview only. No windows focused."),
    ] {
        let before = app.status.clone();
        crate::ui_tests::click_text(&ctx, &mut app, size, button);
        assert_eq!(app.status, before, "preview controls must stay disabled");
        match button {
            "Minimize all" => app.minimize_all(),
            "Restore all" => app.restore_all(),
            "Focus" => app.focus_window(1),
            _ => unreachable!(),
        }
        assert_eq!(app.status, message);
        assert_eq!(app.managed_windows.len(), 2);
        let desk = desktop.0.lock().unwrap();
        assert!(desk.minimized.is_empty());
        assert!(desk.restored_controls.is_empty());
        assert!(desk.focused.is_empty());
        assert!(desk.queries.is_empty());
    }
}

#[test]
#[ignore = "Tray failure is exercised on an owned non-input desktop with local app storage"]
fn native_audit_tray_unavailable_on_private_desktop_reports_setup_failure() {
    crate::native_audit::on_private_desktop(|| {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let scratch = Scratch::new();
        let owned = crate::native_audit::OwnedWindow(
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    windows::core::w!("STATIC"),
                    windows::core::w!("Owned private PSM startup audit"),
                    WS_POPUP,
                    0,
                    0,
                    100,
                    100,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .unwrap(),
        );
        let mut app = PsmApp::start_native(
            Config::default(),
            owned.0 .0 as isize,
            &egui::Context::default(),
            AppRuntime {
                desktop: Box::new(TestDesktop::new([])),
                settings_path: Some(scratch.0.join("config.toml")),
                activity_path: Some(scratch.0.join("activity.toml")),
                update_client: None,
            },
            Box::new(Keys::default()),
        );
        assert!(app._tray_icon.is_none());
        assert!(app.action_worker.is_none());
        assert!(
            app.status
                .starts_with("Could not create tray: Notification icon registration failed:"),
            "{}",
            app.status
        );
        app.install_theme(&egui::Context::default());
        println!("NATIVE TRAY FAILURE PASS: private desktop has no Explorer notification registration; setup failure reaches app status; local storage only");
    });
}

/// Runs after the eframe idle audit on its existing thread-local event loop.
pub(crate) fn audit_native_creation_context_on_current_event_loop() {
    use std::sync::atomic::AtomicUsize;
    #[derive(Clone, Default)]
    struct StartupKeys {
        registered: Arc<AtomicUsize>,
        released: Arc<AtomicUsize>,
    }
    impl crate::hotkey::HotkeyApi for StartupKeys {
        fn register(
            &self,
            id: i32,
            _: windows::Win32::UI::Input::KeyboardAndMouse::HOT_KEY_MODIFIERS,
            key: u32,
        ) -> Result<(), String> {
            assert_eq!((id, key), (1, 0x47));
            self.registered.fetch_add(1, Ordering::Release);
            Ok(())
        }
        fn unregister(&self, id: i32) -> Result<(), String> {
            assert_eq!(id, 1);
            self.released.fetch_add(1, Ordering::Release);
            Ok(())
        }
        fn receive(&self) -> Option<i32> {
            None
        }
    }
    struct StartupApp {
        app: PsmApp,
        keys: StartupKeys,
        frames: Arc<AtomicUsize>,
        started: Instant,
    }
    impl eframe::App for StartupApp {
        fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
            eframe::App::update(&mut self.app, ctx, frame);
            let frames = self.frames.fetch_add(1, Ordering::Relaxed) + 1;
            if frames >= 3 && self.keys.registered.load(Ordering::Acquire) == 1 {
                self.app.quitting.store(true, Ordering::Release);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                assert!(
                    self.started.elapsed() < std::time::Duration::from_secs(5),
                    "owned action worker never started"
                );
                ctx.request_repaint_after(std::time::Duration::from_millis(20));
            }
        }
    }
    crate::action_tests::init_logging();
    let scratch = Scratch::new();
    let keys = StartupKeys::default();
    let worker_keys = keys.clone();
    let update_keys = keys.clone();
    let frames = Arc::new(AtomicUsize::new(0));
    let counted = frames.clone();
    let runtime = AppRuntime {
        desktop: Box::new(TestDesktop::new([])),
        settings_path: Some(scratch.0.join("config.toml")),
        activity_path: Some(scratch.0.join("activity.toml")),
        update_client: None,
    };
    let mut config = Config::default();
    // Even an unexpected menu event could discover only this nonexistent target,
    // never a user's application. Hotkeys stay within the controlled provider.
    config.defaults.target = "powershellmanager-startup-audit-nonexistent.exe".into();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(false)
            .with_active(false)
            .with_taskbar(false)
            .with_inner_size([640.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PSM owned startup audit",
        options,
        Box::new(move |cc| {
            let app = PsmApp::new_with_runtime(cc, config, runtime, Box::new(worker_keys));
            assert!(app.native_enabled);
            assert_ne!(app.app_hwnd, 0);
            assert!(app._tray_icon.is_some(), "{}", app.status);
            assert!(app.action_worker.is_some());
            assert!(app.update_worker.is_none());
            assert!(!unsafe {
                windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(
                    windows::Win32::Foundation::HWND(app.app_hwnd as *mut _),
                )
            }
            .as_bool());
            Ok(Box::new(StartupApp {
                app,
                keys: update_keys,
                frames: counted,
                started: Instant::now(),
            }))
        }),
    )
    .unwrap();
    assert!(frames.load(Ordering::Relaxed) >= 3);
    assert_eq!(keys.registered.load(Ordering::Acquire), 1);
    assert_eq!(keys.released.load(Ordering::Acquire), 1);
    assert!(scratch.0.join("config.toml").is_file());
    assert!(scratch.0.join("activity.toml").is_file());
    println!("NATIVE STARTUP PASS: real creation context and PsmApp update; owned hidden viewport/tray; action worker started/joined and controlled hotkey released; only local settings/activity saved");
}
