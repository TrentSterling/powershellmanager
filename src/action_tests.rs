//! Action failures are exercised without real window enumeration or user data.
use crate::{
    arrange,
    history::{LayoutHistory, RestoreStatus, WindowBackend, MAX_UNDO_STEPS},
    layout::Slot,
    windows::WindowSnapshot,
};
use std::collections::{BTreeMap, HashSet};
use windows::Win32::{Foundation::RECT, UI::WindowsAndMessaging::WINDOWPLACEMENT};

pub(crate) fn init_logging() {
    let _ = env_logger::Builder::new()
        .filter_level(log::LevelFilter::Warn)
        .filter_module("powershellmanager", log::LevelFilter::Trace)
        .is_test(true)
        .try_init();
}

#[test]
fn hotkey_cannot_replace_menu_actions_and_is_applied_once_when_idle() {
    use crate::tray::{LayoutChoice, TrayAction};
    for action in [
        TrayAction::ShowGui,
        TrayAction::ApplyCurrent,
        TrayAction::UndoLayout,
        TrayAction::ApplyLayout(LayoutChoice::Preset(crate::layout::LayoutPreset::LeftRight)),
    ] {
        let mut pending = true;
        let expected = action.clone();
        let keep_hotkey = action == TrayAction::ShowGui;
        assert_eq!(crate::app::next_tray_action(action, &mut pending), expected);
        assert_eq!(pending, keep_hotkey);
        assert_eq!(
            crate::app::next_tray_action(TrayAction::None, &mut pending),
            if keep_hotkey {
                TrayAction::ApplyCurrent
            } else {
                TrayAction::None
            }
        );
        assert!(!pending);
        assert_eq!(
            crate::app::next_tray_action(TrayAction::None, &mut pending),
            TrayAction::None
        );
    }
    for queued in [true, false] {
        let mut pending = queued;
        assert_eq!(
            crate::app::next_tray_action(TrayAction::Quit, &mut pending),
            TrayAction::Quit
        );
        assert!(!pending);
    }
}

#[test]
fn app_shutdown_signals_and_joins_its_action_worker_before_returning() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let mut app = crate::app::PsmApp::preview(crate::config::Config::default());
    let quitting = app.quit_signal();
    let stopped = Arc::new(AtomicBool::new(false));
    let worker_stopped = stopped.clone();
    app.set_action_worker(Ok(std::thread::spawn(move || {
        while !quitting.load(Ordering::Relaxed) {
            std::thread::yield_now();
        }
        worker_stopped.store(true, Ordering::SeqCst);
    })));
    drop(app);
    assert!(stopped.load(Ordering::SeqCst));
}

#[test]
fn action_worker_start_failure_is_visible_and_panicked_shutdown_remains_safe() {
    init_logging();
    let mut app = crate::app::PsmApp::preview(crate::config::Config::default());
    app.set_action_worker(Err(std::io::Error::other("isolated spawn failure")));
    assert_eq!(
        app.status,
        "Tray actions unavailable: worker could not start: isolated spawn failure"
    );
    app.set_action_worker(Ok(std::thread::spawn(|| {
        panic!("isolated action worker failure")
    })));
    drop(app);
}

fn snapshot(hwnd: isize) -> WindowSnapshot {
    WindowSnapshot {
        hwnd,
        process_id: 42,
        visible: hwnd % 2 == 0,
        placement: WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            showCmd: if hwnd % 3 == 0 { 2 } else { 1 },
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

#[derive(Default)]
struct Backend {
    windows: BTreeMap<isize, WindowSnapshot>,
    capture_fail: HashSet<isize>,
    position_fail: HashSet<isize>,
    restore_fail: HashSet<isize>,
    positioned: Vec<isize>,
    restored: Vec<isize>,
}

impl Backend {
    fn with_windows(handles: impl IntoIterator<Item = isize>) -> Self {
        Self {
            windows: handles
                .into_iter()
                .map(|hwnd| (hwnd, snapshot(hwnd)))
                .collect(),
            ..Default::default()
        }
    }
}

impl WindowBackend for Backend {
    fn capture(&mut self, hwnd: isize) -> Result<WindowSnapshot, String> {
        if self.capture_fail.contains(&hwnd) {
            return Err(format!("snapshot failed: {hwnd}"));
        }
        self.windows
            .get(&hwnd)
            .copied()
            .ok_or_else(|| format!("closed: {hwnd}"))
    }

    fn position(&mut self, snapshot: &WindowSnapshot, slot: &Slot) -> Result<(), String> {
        if self.position_fail.contains(&snapshot.hwnd) {
            return Err(format!("position failed: {}", snapshot.hwnd));
        }
        let window = self.windows.get_mut(&snapshot.hwnd).unwrap();
        window.placement.rcNormalPosition = RECT {
            left: slot.x,
            top: slot.y,
            right: slot.x + slot.w,
            bottom: slot.y + slot.h,
        };
        self.positioned.push(snapshot.hwnd);
        Ok(())
    }

    fn restore(&mut self, snapshot: &WindowSnapshot) -> Result<RestoreStatus, String> {
        let Some(window) = self.windows.get_mut(&snapshot.hwnd) else {
            return Ok(RestoreStatus::Closed);
        };
        if window.process_id != snapshot.process_id {
            return Ok(RestoreStatus::Closed);
        }
        if self.restore_fail.contains(&snapshot.hwnd) {
            return Err(format!("restore failed: {}", snapshot.hwnd));
        }
        *window = *snapshot;
        self.restored.push(snapshot.hwnd);
        Ok(RestoreStatus::Restored)
    }
}

fn plan(handles: impl IntoIterator<Item = isize>, x: i32) -> Vec<(isize, Slot)> {
    handles
        .into_iter()
        .enumerate()
        .map(|(i, hwnd)| {
            (
                hwnd,
                Slot {
                    x: x + i as i32 * 300,
                    y: 50,
                    w: 290,
                    h: 500,
                },
            )
        })
        .collect()
}

#[test]
fn apply_and_multi_step_undo_restore_exact_positions_and_window_states() {
    let mut backend = Backend::with_windows(1..=3);
    let original = backend.windows.clone();
    let mut history = LayoutHistory::default();
    let first = arrange::execute_plan(&mut backend, plan(1..=3, 100), 4, vec![]);
    assert_eq!(
        (first.arranged, first.skipped, first.errors.len()),
        (3, 1, 0)
    );
    assert_eq!(first.status(), "Arranged 3 windows. 1 skipped. 0 errors.");
    history.record(first.snapshots);
    let after_first = backend.windows.clone();
    let second = arrange::execute_plan(
        &mut backend,
        plan([3, 1], 900),
        3,
        vec!["Pin conflict".into()],
    );
    assert!(second.status().ends_with("Pin conflict"));
    history.record(second.snapshots);
    assert!(history.can_undo());
    let undo = history.undo(&mut backend);
    assert_eq!((undo.restored, undo.closed, undo.errors.len()), (2, 0, 0));
    assert_eq!(backend.windows, after_first);
    assert_eq!(
        undo.status(),
        "Restored 2 windows. 0 closed or replaced. 0 errors."
    );
    assert_eq!(history.undo(&mut backend).restored, 3);
    assert_eq!(backend.windows, original);
    assert!(!history.can_undo());
    assert_eq!(history.undo(&mut backend).restored, 0);
}

#[test]
fn failed_snapshots_never_move_and_failed_moves_never_enter_undo() {
    let mut backend = Backend::with_windows(1..=3);
    let original = backend.windows.clone();
    backend.capture_fail.insert(1);
    backend.position_fail.insert(2);
    let result = arrange::execute_plan(&mut backend, plan(1..=3, 100), 4, vec![]);
    assert_eq!(
        (result.arranged, result.skipped, result.errors.len()),
        (1, 1, 2)
    );
    assert_eq!(backend.positioned, vec![3]);
    assert_eq!(result.snapshots, vec![original[&3]]);
    assert_eq!(backend.windows[&1], original[&1]);
    assert_eq!(backend.windows[&2], original[&2]);
    let mut history = LayoutHistory::default();
    history.record(result.snapshots);
    assert_eq!(history.undo(&mut backend).restored, 1);
    assert_eq!(backend.windows, original);
}

#[test]
fn undo_skips_closed_and_reused_handles_and_retains_temporary_failures_for_retry() {
    let mut backend = Backend::with_windows(1..=4);
    let original = backend.windows.clone();
    let result = arrange::execute_plan(&mut backend, plan(1..=4, 100), 4, vec![]);
    let mut history = LayoutHistory::default();
    history.record(result.snapshots);
    backend.windows.remove(&1);
    backend.windows.get_mut(&2).unwrap().process_id = 99;
    backend.restore_fail.insert(3);
    let replaced = backend.windows[&2];
    let result = history.undo(&mut backend);
    assert_eq!(
        (result.restored, result.closed, result.errors.len()),
        (1, 2, 1)
    );
    assert_eq!(backend.windows[&2], replaced);
    assert_eq!(backend.windows[&4], original[&4]);
    assert!(history.can_undo());
    backend.restore_fail.clear();
    assert_eq!(history.undo(&mut backend).restored, 1);
    assert_eq!(backend.windows[&3], original[&3]);
    assert_eq!(backend.restored, vec![4, 3]);
    assert!(!history.can_undo());
}

#[test]
fn empty_or_failed_apply_preserves_previous_undo_and_history_evicts_oldest_steps() {
    let mut backend = Backend::with_windows([1]);
    let mut history = LayoutHistory::default();
    for step in 0..MAX_UNDO_STEPS + 2 {
        let result = arrange::execute_plan(&mut backend, plan([1], step as i32 * 10), 1, vec![]);
        history.record(result.snapshots);
    }
    history.record(vec![]);
    let result = arrange::execute_plan(&mut backend, vec![], 3, vec![]);
    assert_eq!((result.arranged, result.skipped), (0, 3));
    history.record(result.snapshots);
    backend.position_fail.insert(1);
    let result = arrange::execute_plan(&mut backend, plan([1], 2000), 1, vec![]);
    history.record(result.snapshots);
    for _ in 0..MAX_UNDO_STEPS {
        assert_eq!(history.undo(&mut backend).restored, 1);
    }
    assert!(!history.can_undo());
    assert_eq!(backend.windows[&1].placement.rcNormalPosition.left, 10);
}

#[test]
fn inventory_flags_are_read_only_and_incompatible_modes_fail_before_dispatch() {
    use clap::{error::ErrorKind, Parser};
    let default = crate::cli::Cli::try_parse_from(["psm"]).unwrap();
    assert!(!default.list && !default.preview && default.headless.is_none());
    let list =
        crate::cli::Cli::try_parse_from(["psm", "--list", "--json", "--target", "terminals"])
            .unwrap();
    assert!(list.list && list.json);
    assert_eq!(list.target.as_deref(), Some("terminals"));
    for arguments in [
        vec!["psm", "--list", "--preview"],
        vec!["psm", "--list", "--headless", "2x2"],
        vec!["psm", "--preview", "--headless", "2x2"],
    ] {
        assert_eq!(
            crate::cli::Cli::try_parse_from(arguments)
                .unwrap_err()
                .kind(),
            ErrorKind::ArgumentConflict
        );
    }
    for arguments in [vec!["psm", "--json"], vec!["psm", "--target", "all"]] {
        assert_eq!(
            crate::cli::Cli::try_parse_from(arguments)
                .unwrap_err()
                .kind(),
            ErrorKind::MissingRequiredArgument
        );
    }
    assert_eq!(
        crate::cli::Cli::try_parse_from(["psm", "--help"])
            .unwrap_err()
            .kind(),
        ErrorKind::DisplayHelp
    );
    assert_eq!(
        crate::cli::Cli::try_parse_from(["psm", "--version"])
            .unwrap_err()
            .kind(),
        ErrorKind::DisplayVersion
    );
}

#[test]
fn inventory_json_preserves_unicode_quotes_empty_titles_and_physical_bounds() {
    let window = crate::windows::ManagedWindow {
        hwnd: 123,
        process_name: "pwsh.exe".into(),
        title: "日本語 / café / 🎨 \"quoted\"\nnew\ttab".into(),
        category: crate::windows::AppCategory::Terminal,
        is_minimized: true,
        rect: crate::monitor::Rect {
            x: -1080,
            y: 40,
            w: 1080,
            h: 1880,
        },
    };
    let mut empty = window.clone();
    empty.hwnd = 124;
    empty.title.clear();
    empty.is_minimized = false;
    let windows = [window, empty];
    let json: serde_json::Value =
        serde_json::from_str(&crate::cli::format_inventory(&windows, true)).unwrap();
    assert_eq!(json["count"], 2);
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["windows"][0]["title"], windows[0].title);
    assert_eq!(json["windows"][0]["bounds"]["x"], -1080);
    assert_eq!(json["windows"][0]["bounds"]["height"], 1880);
    assert_eq!(json["windows"][0]["minimized"], true);
    assert_eq!(json["windows"][1]["title"], "");
    let text = crate::cli::format_inventory(&windows, false);
    assert_eq!(text.lines().count(), 4);
    assert!(text.contains("0x7b\tpwsh.exe\tMinimized"));
    assert!(text.contains("0x7c\tpwsh.exe\tReady"));
    let empty: serde_json::Value =
        serde_json::from_str(&crate::cli::format_inventory(&[], true)).unwrap();
    assert_eq!(empty["count"], 0);
    assert_eq!(empty["windows"], serde_json::json!([]));
    assert_eq!(crate::cli::format_inventory(&[], false).lines().count(), 2);
}
