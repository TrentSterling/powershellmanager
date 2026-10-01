use super::*;
use crate::{
    history::{RestoreStatus, WindowBackend},
    layout::Slot,
    monitor::MonitorInfo,
    windows::{AppCategory, WindowSnapshot},
};
use windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT;

// Two 200x100 work areas; a 2x1 grid with no gap gives 100x100 slots.
const MAIN: Rect = Rect {
    x: 0,
    y: 0,
    w: 200,
    h: 100,
};
const SIDE: Rect = Rect {
    x: -200,
    y: 0,
    w: 200,
    h: 100,
};

#[derive(Default)]
struct Desk {
    windows: Vec<ManagedWindow>,
    monitors: Vec<MonitorInfo>,
    moved: Vec<(isize, i32)>,
    fail: HashSet<isize>,
}

impl WindowBackend for Desk {
    fn capture(&mut self, hwnd: isize) -> Result<WindowSnapshot, String> {
        Ok(WindowSnapshot {
            hwnd,
            process_id: 1,
            placement: WINDOWPLACEMENT::default(),
            visible: true,
        })
    }
    fn position(&mut self, snapshot: &WindowSnapshot, slot: &Slot) -> Result<(), String> {
        if self.fail.contains(&snapshot.hwnd) {
            return Err(format!("denied {}", snapshot.hwnd));
        }
        let window = self
            .windows
            .iter_mut()
            .find(|w| w.hwnd == snapshot.hwnd)
            .unwrap();
        window.rect = Rect {
            x: slot.x,
            y: slot.y,
            w: slot.w,
            h: slot.h,
        };
        self.moved.push((snapshot.hwnd, slot.x));
        Ok(())
    }
    fn restore(&mut self, _: &WindowSnapshot) -> Result<RestoreStatus, String> {
        Ok(RestoreStatus::Restored)
    }
}

impl Desktop for Desk {
    fn monitors(&mut self) -> Vec<MonitorInfo> {
        self.monitors.clone()
    }
    fn windows(&mut self, _: &TargetFilter, _: isize, _: &[String]) -> Vec<ManagedWindow> {
        self.windows.clone()
    }
    fn visible(&self, _: isize) -> bool {
        true
    }
    fn show_app(&mut self, _: isize) {}
    fn hide_app(&mut self, _: isize) {}
    fn wake_for_close(&mut self, _: isize) {}
    fn focus_window(&mut self, _: isize) {}
    fn minimize_window(&mut self, _: isize) {}
    fn restore_window(&mut self, _: isize) {}
}

fn window(hwnd: isize, x: i32) -> ManagedWindow {
    ManagedWindow {
        hwnd,
        title: format!("T{hwnd}"),
        process_name: "pwsh.exe".into(),
        category: AppCategory::Terminal,
        rect: Rect {
            x,
            y: 0,
            w: 100,
            h: 100,
        },
        is_minimized: false,
    }
}

fn desk(windows: Vec<ManagedWindow>, monitors: &[Rect]) -> Desk {
    Desk {
        windows,
        monitors: monitors
            .iter()
            .enumerate()
            .map(|(index, &work_area)| MonitorInfo {
                index,
                is_primary: index == 0,
                work_area,
            })
            .collect(),
        ..Default::default()
    }
}

fn config(slide: bool, overflow: bool) -> Config {
    let mut config = Config::default();
    let d = &mut config.defaults;
    d.auto_arrange = true;
    d.slide_to_fill = slide;
    d.overflow_display = overflow;
    d.use_custom = true;
    d.custom_cols = 2;
    d.custom_rows = 1;
    d.gap = 0;
    config
}

fn run(
    auto: &mut AutoState,
    desk: &mut Desk,
    config: &Config,
    h: &mut LayoutHistory,
) -> Option<String> {
    scan(auto, desk, 0, config, h)
}

#[test]
fn enabling_never_moves_open_windows_and_disabling_forgets_them() {
    let mut desk = desk(vec![window(1, 300)], &[MAIN]);
    let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
    let cfg = config(false, false);
    assert_eq!(run(&mut auto, &mut desk, &cfg, &mut history), None);
    assert_eq!(run(&mut auto, &mut desk, &cfg, &mut history), None);
    assert!(desk.moved.is_empty());
    let mut off = cfg.clone();
    off.defaults.auto_arrange = false;
    assert_eq!(run(&mut auto, &mut desk, &off, &mut history), None);
    assert!(!auto.active && auto.known.is_empty());
}

#[test]
fn a_new_window_waits_to_settle_then_takes_the_hole_without_moving_residents() {
    let mut desk = desk(vec![window(1, 100)], &[MAIN]);
    let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
    let cfg = config(false, false);
    run(&mut auto, &mut desk, &cfg, &mut history);
    desk.windows.push(window(2, 900));
    assert_eq!(run(&mut auto, &mut desk, &cfg, &mut history), None);
    assert_eq!(
        run(&mut auto, &mut desk, &cfg, &mut history).as_deref(),
        Some("Auto: placed 1 new windows.")
    );
    assert_eq!(desk.moved, [(2, 0)]);
    assert_eq!(history.slots.get(&2), Some(&0));
    assert!(history.can_undo());
    assert_eq!(run(&mut auto, &mut desk, &cfg, &mut history), None);
}

#[test]
fn a_window_that_keeps_resizing_is_placed_after_the_settle_limit() {
    let mut desk = desk(vec![], &[MAIN]);
    let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
    let cfg = config(false, false);
    run(&mut auto, &mut desk, &cfg, &mut history);
    desk.windows.push(window(5, 900));
    for scan in 0..=MAX_SETTLE_SCANS {
        desk.windows[0].rect.x = 900 + i32::from(scan);
        let status = run(&mut auto, &mut desk, &cfg, &mut history);
        assert_eq!(status.is_some(), scan == MAX_SETTLE_SCANS, "scan {scan}");
    }
    assert_eq!(desk.moved, [(5, 0)]);
}

#[test]
fn minimized_arrivals_and_off_grid_residents_are_left_alone() {
    let mut desk = desk(vec![window(1, 700)], &[MAIN]);
    let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
    let cfg = config(false, false);
    run(&mut auto, &mut desk, &cfg, &mut history);
    let mut minimized = window(2, 900);
    minimized.is_minimized = true;
    desk.windows.push(minimized);
    assert_eq!(run(&mut auto, &mut desk, &cfg, &mut history), None);
    desk.windows.push(window(3, 900));
    run(&mut auto, &mut desk, &cfg, &mut history);
    run(&mut auto, &mut desk, &cfg, &mut history);
    assert_eq!(desk.moved, [(3, 0)]);
}

#[test]
fn closing_leaves_the_hole_unless_slide_to_fill_is_on() {
    for slide in [false, true] {
        let mut desk = desk(vec![window(1, 0), window(2, 100)], &[MAIN]);
        let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
        history.slots.extend([(1, 0), (2, 1)]);
        let cfg = config(slide, false);
        run(&mut auto, &mut desk, &cfg, &mut history);
        desk.windows.remove(0);
        let status = run(&mut auto, &mut desk, &cfg, &mut history);
        assert!(!history.slots.contains_key(&1));
        if slide {
            assert_eq!(
                status.as_deref(),
                Some("Auto: slid 1 windows to fill the gap.")
            );
            assert_eq!(desk.moved, [(2, 0)]);
            assert_eq!(history.slots.get(&2), Some(&0));
        } else {
            assert_eq!(status, None);
            assert!(desk.moved.is_empty());
        }
    }
    // A closed window that never had a slot frees nothing and slides nothing.
    let mut desk = desk(vec![window(1, 0), window(2, 700)], &[MAIN]);
    let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
    let cfg = config(true, false);
    run(&mut auto, &mut desk, &cfg, &mut history);
    desk.windows.remove(1);
    assert_eq!(run(&mut auto, &mut desk, &cfg, &mut history), None);
    assert!(desk.moved.is_empty());
}

#[test]
fn a_full_grid_leaves_arrivals_or_overflows_to_the_other_display() {
    let full = || vec![window(1, 0), window(2, 100)];
    let arrive = |desk: &mut Desk, cfg: &Config| {
        let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
        run(&mut auto, desk, cfg, &mut history);
        desk.windows.push(window(3, 900));
        desk.windows.push(window(4, 950));
        run(&mut auto, desk, cfg, &mut history);
        run(&mut auto, desk, cfg, &mut history)
    };
    let mut alone = desk(full(), &[MAIN, SIDE]);
    assert_eq!(
        arrive(&mut alone, &config(false, false)).as_deref(),
        Some("Auto: grid full; 2 new windows left where they opened.")
    );
    let mut single = desk(full(), &[MAIN]);
    assert!(arrive(&mut single, &config(false, true))
        .unwrap()
        .contains("left where they opened"));
    assert!(single.moved.is_empty());

    // The side grid already holds one window, so one arrival fits and one waits.
    let mut busy = desk(full(), &[MAIN, SIDE]);
    busy.windows.push(window(9, -200));
    assert_eq!(
        arrive(&mut busy, &config(false, true)).as_deref(),
        Some("Auto: grid full; 1 new windows sent to the other display, 1 left where they opened.")
    );
    assert_eq!(busy.moved, [(3, -100)]);

    // Disabled cells stay empty on the overflow display too.
    let mut cfg = config(false, true);
    cfg.defaults.disabled_cells = vec![1];
    let mut masked = desk(vec![window(1, 0)], &[MAIN, SIDE]);
    arrive(&mut masked, &cfg);
    assert_eq!(masked.moved, [(3, -200)]);
}

#[test]
fn failed_moves_are_reported_and_missing_displays_skip_the_pass() {
    let mut desk = desk(vec![], &[MAIN]);
    let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
    let cfg = config(false, false);
    run(&mut auto, &mut desk, &cfg, &mut history);
    desk.windows.push(window(7, 900));
    desk.fail.insert(7);
    run(&mut auto, &mut desk, &cfg, &mut history);
    assert_eq!(
        run(&mut auto, &mut desk, &cfg, &mut history).as_deref(),
        Some("denied 7")
    );
    assert!(history.slots.is_empty());
    desk.monitors.clear();
    desk.windows.push(window(8, 900));
    assert_eq!(run(&mut auto, &mut desk, &cfg, &mut history), None);
}

#[test]
fn a_pinned_arrival_takes_its_pin_slot() {
    let mut desk = desk(vec![], &[MAIN]);
    let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
    let mut cfg = config(false, false);
    cfg.pin.push(crate::config::PinRule {
        process: Some("pwsh.exe".into()),
        title_exact: Some("T4".into()),
        title_contains: None,
        bound_hwnd: None,
        slot: 1,
    });
    run(&mut auto, &mut desk, &cfg, &mut history);
    desk.windows.push(window(4, 900));
    run(&mut auto, &mut desk, &cfg, &mut history);
    run(&mut auto, &mut desk, &cfg, &mut history);
    assert_eq!(desk.moved, [(4, 100)]);
}

#[test]
fn arrivals_already_in_a_free_slot_keep_it_and_unsettled_windows_are_not_residents() {
    let mut desk = desk(vec![], &[MAIN]);
    let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
    let cfg = config(false, false);
    run(&mut auto, &mut desk, &cfg, &mut history);
    desk.windows.push(window(1, 100));
    run(&mut auto, &mut desk, &cfg, &mut history);
    run(&mut auto, &mut desk, &cfg, &mut history);
    assert_eq!(desk.moved, [(1, 100)]);
    assert_eq!(history.slots.get(&1), Some(&1));

    // Window 3 is still settling in slot 0 when window 2 settles; only 2 is placed.
    let mut desk = desk_with_main(vec![]);
    let (mut auto, mut history) = (AutoState::default(), LayoutHistory::default());
    run(&mut auto, &mut desk, &cfg, &mut history);
    desk.windows.push(window(2, 900));
    run(&mut auto, &mut desk, &cfg, &mut history);
    desk.windows.push(window(3, 0));
    run(&mut auto, &mut desk, &cfg, &mut history);
    assert_eq!(desk.moved, [(2, 0)]);
}

fn desk_with_main(windows: Vec<ManagedWindow>) -> Desk {
    desk(windows, &[MAIN])
}
