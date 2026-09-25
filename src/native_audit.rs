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
struct OwnedWindow(HWND);
impl Drop for OwnedWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
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
    let result = arrange::arrange_ordered(&preset, "primary", 4, &disabled, weights, &queue, &pins);
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
}
