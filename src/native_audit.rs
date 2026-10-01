//! Native tests own every HWND they touch. Never enumerate and move the desktop.
use crate::{
    arrange,
    layout::LayoutPreset,
    monitor, order,
    windows::{AppCategory, ManagedWindow},
};
use windows::Win32::{
    Foundation::{HWND, RECT},
    UI::WindowsAndMessaging::*,
};
pub(crate) struct OwnedWindow(pub(crate) HWND);
impl Drop for OwnedWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

/// Run native controls on an owned desktop without making it the input desktop.
/// All windows created by the callback must be released before the callback exits.
pub(crate) fn on_private_desktop(action: impl FnOnce() + Send + 'static) {
    use std::sync::atomic::{AtomicU64, Ordering};
    use windows::Win32::System::StationsAndDesktops::*;
    struct OwnedDesktop(HDESK);
    impl Drop for OwnedDesktop {
        fn drop(&mut self) {
            unsafe { CloseDesktop(self.0) }
                .expect("close owned private desktop after its thread exits");
        }
    }
    static NEXT_DESKTOP: AtomicU64 = AtomicU64::new(0);
    let name = format!(
        "PSM-native-audit-{}-{}",
        std::process::id(),
        NEXT_DESKTOP.fetch_add(1, Ordering::Relaxed)
    );
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    // Deliberately omit DESKTOP_SWITCHDESKTOP. This is a private UI surface,
    // never the visible/input desktop and never a target for physical input.
    let access = DESKTOP_READOBJECTS.0
        | DESKTOP_CREATEWINDOW.0
        | DESKTOP_CREATEMENU.0
        | DESKTOP_ENUMERATE.0
        | DESKTOP_WRITEOBJECTS.0;
    let desktop = OwnedDesktop(
        unsafe {
            CreateDesktopW(
                windows::core::PCWSTR(wide.as_ptr()),
                None,
                None,
                DESKTOP_CONTROL_FLAGS::default(),
                access,
                None,
            )
        }
        .unwrap(),
    );
    let handle = desktop.0 .0 as usize;
    // Windows can create thread-owned helper windows during focus/restore. Wait
    // for thread teardown instead of switching a thread that may still own them.
    // The parent remains on its original desktop and owns the handle throughout.
    let result = std::thread::spawn(move || {
        unsafe { SetThreadDesktop(HDESK(handle as *mut _)) }.unwrap();
        action();
    })
    .join();
    drop(desktop);
    result.unwrap();
}

#[test]
#[ignore = "Exercises owned window controls on a private desktop that is never made visible"]
fn native_controls_use_a_private_desktop_and_never_switch_the_input_desktop() {
    on_private_desktop(|| {
        use crate::desktop::Desktop;
        let owned = OwnedWindow(
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    windows::core::w!("STATIC"),
                    windows::core::w!("Owned private PSM window"),
                    WS_OVERLAPPEDWINDOW,
                    120,
                    140,
                    400,
                    300,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .unwrap(),
        );
        let hwnd = owned.0 .0 as isize;
        let mut desktop = crate::desktop::NativeDesktop;
        assert!(!desktop.visible(hwnd));
        desktop.wake_for_close(hwnd);
        assert!(desktop.visible(hwnd));
        desktop.focus_window(hwnd);
        desktop.minimize_window(hwnd);
        assert!(unsafe { IsIconic(owned.0) }.as_bool());
        desktop.focus_window(hwnd);
        assert!(!unsafe { IsIconic(owned.0) }.as_bool());
        desktop.minimize_window(hwnd);
        desktop.restore_window(hwnd);
        assert!(!unsafe { IsIconic(owned.0) }.as_bool());
        desktop.hide_app(hwnd);
        assert!(!desktop.visible(hwnd));
        desktop.show_app(hwnd);
        assert!(desktop.visible(hwnd));
        assert!(!desktop.monitors().is_empty());
        let process = crate::windows::get_process_name_for_hwnd(hwnd).unwrap();
        let filter = crate::windows::TargetFilter::from_str(&process);
        let found = desktop.windows(&filter, 0, &[]);
        assert!(found.iter().any(|window| window.hwnd == hwnd));
        assert!(found.iter().all(|window| window.process_name == process));
        assert!(desktop
            .windows(&filter, hwnd, &[])
            .iter()
            .all(|window| window.hwnd != hwnd));
        desktop.hide_app(hwnd);
        assert!(!desktop.visible(hwnd));
        let _ = crate::windows::get_foreground_window();
        drop(owned);
        println!("NATIVE PRIVATE DESKTOP PASS: show, hide, focus, minimize, restore, wake, discovery and monitor adapter on owned HWND; input desktop never switched; no desktop input");
    });
}

#[test]
#[ignore = "Exercises real snapshot and undo APIs on owned hidden windows only"]
fn native_undo_restores_owned_windows_and_skips_closed_or_changed_owners() {
    use crate::history::{LayoutHistory, WindowBackend};
    let mut owned = Vec::new();
    let mut backend = crate::desktop::NativeDesktop;
    for i in 0..2 {
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::w!("STATIC"),
                windows::core::w!("PSM isolated undo audit"),
                WS_POPUP,
                120 + i * 40,
                140 + i * 40,
                200,
                160,
                None,
                None,
                None,
                None,
            )
        }
        .unwrap();
        owned.push(OwnedWindow(hwnd));
    }
    let snapshots: Vec<_> = owned
        .iter()
        .map(|window| backend.capture(window.0 .0 as isize).unwrap())
        .collect();
    let placements = snapshots
        .iter()
        .enumerate()
        .map(|(i, snapshot)| {
            (
                snapshot.hwnd,
                crate::layout::Slot {
                    x: 600 + i as i32 * 300,
                    y: 300,
                    w: 260,
                    h: 380,
                },
            )
        })
        .collect();
    let result = arrange::execute_plan(&mut backend, placements, 2, vec![]);
    assert_eq!(result.arranged, 2);
    let mut history = LayoutHistory::default();
    history.record(result.snapshots);
    assert_eq!(history.undo(&mut backend).restored, 2);
    for original in &snapshots {
        let restored = backend.capture(original.hwnd).unwrap();
        assert_eq!(
            restored.placement.rcNormalPosition,
            original.placement.rcNormalPosition
        );
        assert_eq!(restored.placement.showCmd, original.placement.showCmd);
        assert!(!restored.visible);
    }
    let mut changed_owner = snapshots[0];
    changed_owner.process_id = changed_owner.process_id.wrapping_add(1);
    history.record(vec![changed_owner, snapshots[1]]);
    drop(owned.pop().unwrap());
    let result = history.undo(&mut backend);
    assert_eq!(
        (result.restored, result.closed, result.errors.len()),
        (0, 2, 0)
    );
    assert!(backend.capture(0).is_err());
    assert!(backend
        .position(
            &changed_owner,
            &crate::layout::Slot {
                x: 0,
                y: 0,
                w: 100,
                h: 100
            }
        )
        .is_err());
    println!("NATIVE UNDO PASS: exact original placements and hidden state restored; closed/replaced owners skipped; invalid snapshots rejected; only owned HWNDs touched");
}
#[test]
#[ignore = "Creates hidden test windows only; explicitly run on Windows"]
fn native_placement_moves_only_owned_hidden_windows() {
    let mut owned = Vec::new();
    let mut queue = Vec::new();
    for i in 0..3 {
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                windows::core::w!("STATIC"),
                windows::core::w!("PSM isolated placement audit"),
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
        .unwrap();
        owned.push(OwnedWindow(hwnd));
        queue.push(ManagedWindow {
            hwnd: hwnd.0 as isize,
            title: format!("Audit {i}"),
            process_name: "owned-test.exe".into(),
            category: AppCategory::Other,
            rect: monitor::Rect {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
            is_minimized: false,
        });
    }
    queue.swap(0, 2);
    let preset = LayoutPreset::Grid { cols: 3, rows: 2 };
    let disabled = [1].into_iter().collect();
    let pins = vec![crate::config::PinRule {
        process: Some("owned-test.exe".into()),
        title_exact: Some("Audit 1".into()),
        title_contains: None,
        bound_hwnd: Some(queue[1].hwnd),
        slot: 4,
    }];
    let monitors = monitor::enumerate_monitors();
    let area = monitor::resolve_monitor(&monitors, "primary").work_area;
    let weights = Some((&[0.4, 0.35, 0.25][..], &[0.5, 0.5][..]));
    let (plan, _) = order::placements(&preset, &area, 4, &disabled, weights, &queue, &pins);
    let result = arrange::arrange_with(
        &mut crate::desktop::NativeDesktop,
        &monitors,
        &arrange::ArrangeSettings {
            preset: &preset,
            monitor_spec: "primary",
            gap: 4,
            disabled: &disabled,
            weights,
            pins: &pins,
        },
        &queue,
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.arranged, 3);
    for (hwnd, slot) in plan {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(HWND(hwnd as *mut _), &mut rect) }.unwrap();
        assert_eq!(
            (
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top
            ),
            (slot.x, slot.y, slot.w, slot.h)
        );
        assert!(!unsafe { IsWindowVisible(HWND(hwnd as *mut _)) }.as_bool());
    }
    println!("NATIVE PASS: reordered queue, exact pin, disabled physical slot, weighted bounds; only 3 owned hidden HWNDs moved");
}

#[test]
#[ignore = "Runs a private hidden eframe viewport for five seconds; no tray or global input"]
fn hidden_eframe_repaints_idle_and_exits_without_poll_spin() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use winit::platform::windows::EventLoopBuilderExtWindows;
    struct Audit {
        frames: Arc<AtomicUsize>,
    }
    impl eframe::App for Audit {
        fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
            self.frames.fetch_add(1, Ordering::Relaxed);
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
    fn cpu_ticks() -> u64 {
        use windows::Win32::{
            Foundation::FILETIME,
            System::Threading::{GetCurrentProcess, GetProcessTimes},
        };
        let (mut c, mut e, mut k, mut u) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) }.unwrap();
        ((k.dwHighDateTime as u64) << 32 | k.dwLowDateTime as u64)
            + ((u.dwHighDateTime as u64) << 32 | u.dwLowDateTime as u64)
    }
    let frames = Arc::new(AtomicUsize::new(0));
    let counted = frames.clone();
    let before = cpu_ticks();
    let start = std::time::Instant::now();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(false)
            .with_inner_size([200.0, 120.0]),
        event_loop_builder: Some(Box::new(|builder| {
            builder.with_any_thread(true);
        })),
        ..Default::default()
    };
    eframe::run_native(
        "PSM hidden audit",
        opts,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(5));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                ctx.request_repaint();
            });
            Ok(Box::new(Audit { frames: counted }))
        }),
    )
    .unwrap();
    let cpu = (cpu_ticks() - before) as f64 / 10_000_000.0;
    let elapsed = start.elapsed().as_secs_f64();
    let count = frames.load(Ordering::Relaxed);
    println!("HIDDEN NATIVE: {count} frames; {elapsed:.2}s wall; {cpu:.3}s process CPU (includes startup)");
    assert!(
        (5..100).contains(&count),
        "hidden repaint must advance and remain bounded"
    );
    assert!(
        cpu < elapsed * 0.60,
        "a hidden window must not spin a CPU core"
    );
    crate::app::tests::audit_native_creation_context_on_current_event_loop();
    audit_native_gui_runner_on_current_event_loop();
}

fn audit_native_gui_runner_on_current_event_loop() {
    use crate::launch::{GuiRequest, GuiRunner, NativeGuiRunner};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let title = format!("PSM owned preview runner audit {}", std::process::id());
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let done = Arc::new(AtomicBool::new(false));
    let closing = done.clone();
    let closer = std::thread::spawn(move || {
        let started = std::time::Instant::now();
        while !closing.load(Ordering::Acquire) {
            let hwnd = unsafe { FindWindowW(None, windows::core::PCWSTR(wide.as_ptr())) };
            if let Ok(hwnd) = hwnd {
                let mut process_id = 0;
                unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
                assert_eq!(
                    process_id,
                    std::process::id(),
                    "close only this process's owned viewport"
                );
                assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());
                // Let initialization and a real PsmApp::update finish before
                // closing this owned window; this is not synthesized input.
                std::thread::sleep(std::time::Duration::from_millis(250));
                unsafe { PostMessageW(hwnd, WM_CLOSE, None, None) }.unwrap();
                return;
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(6),
                "owned preview viewport never appeared"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    let mut config = crate::config::Config::default();
    config.defaults.target = "powershellmanager-preview-audit-nonexistent.exe".into();
    let request = GuiRequest {
        title,
        options: eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_visible(false)
                .with_active(false)
                .with_taskbar(false)
                .with_inner_size([640.0, 600.0]),
            ..Default::default()
        },
        config,
        preview: true,
    };
    let result = NativeGuiRunner.run(request);
    done.store(true, Ordering::Release);
    closer.join().unwrap();
    result.unwrap();
    // winit forbids a second event loop. A different thread has no eframe cache,
    // so this produces an actual framework error before creating any window.
    let error = std::thread::spawn(|| {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        NativeGuiRunner
            .run(GuiRequest {
                title: "PSM intentionally rejected second event loop".into(),
                options: eframe::NativeOptions {
                    viewport: egui::ViewportBuilder::default().with_visible(false),
                    event_loop_builder: Some(Box::new(|builder| {
                        builder.with_any_thread(true);
                    })),
                    ..Default::default()
                },
                config: Default::default(),
                preview: true,
            })
            .unwrap_err()
    })
    .join()
    .unwrap();
    assert!(!error.is_empty());
    println!("NATIVE GUI RUNNER PASS: actual read-only preview and PsmApp frame on owned hidden viewport; real framework startup error propagated; no desktop input or home settings writes");
}
