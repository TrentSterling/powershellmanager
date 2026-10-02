//! Desktop effects are kept behind one interface; layout and app logic are shared.
use crate::{
    history::WindowBackend,
    monitor::MonitorInfo,
    windows::{ManagedWindow, TargetFilter},
};

pub(crate) trait Desktop: WindowBackend {
    fn monitors(&mut self) -> Vec<MonitorInfo>;
    fn windows(
        &mut self,
        filter: &TargetFilter,
        app_hwnd: isize,
        excluded: &[String],
    ) -> Vec<ManagedWindow>;
    fn visible(&self, hwnd: isize) -> bool;
    fn show_app(&mut self, hwnd: isize);
    fn hide_app(&mut self, hwnd: isize);
    fn wake_for_close(&mut self, hwnd: isize);
    fn focus_window(&mut self, hwnd: isize);
    fn minimize_window(&mut self, hwnd: isize);
    fn restore_window(&mut self, hwnd: isize);
}

pub(crate) struct NativeDesktop;

impl WindowBackend for NativeDesktop {
    fn capture(&mut self, hwnd: isize) -> Result<crate::windows::WindowSnapshot, String> {
        crate::windows::NativeWindowBackend.capture(hwnd)
    }
    fn position(
        &mut self,
        snapshot: &crate::windows::WindowSnapshot,
        slot: &crate::layout::Slot,
    ) -> Result<(), String> {
        crate::windows::NativeWindowBackend.position(snapshot, slot)
    }
    fn restore(
        &mut self,
        snapshot: &crate::windows::WindowSnapshot,
    ) -> Result<crate::history::RestoreStatus, String> {
        crate::windows::NativeWindowBackend.restore(snapshot)
    }
}

impl Desktop for NativeDesktop {
    fn monitors(&mut self) -> Vec<MonitorInfo> {
        crate::monitor::enumerate_monitors()
    }
    fn windows(
        &mut self,
        filter: &TargetFilter,
        app_hwnd: isize,
        excluded: &[String],
    ) -> Vec<ManagedWindow> {
        crate::windows::find_windows(filter, app_hwnd, excluded)
    }
    fn visible(&self, hwnd: isize) -> bool {
        use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindowVisible};
        unsafe { IsWindowVisible(HWND(hwnd as *mut _)) }.as_bool()
    }
    fn show_app(&mut self, hwnd: isize) {
        crate::windows::show_app_window(hwnd);
    }
    fn hide_app(&mut self, hwnd: isize) {
        crate::windows::hide_app_window(hwnd);
    }
    fn focus_window(&mut self, hwnd: isize) {
        crate::windows::focus_window(hwnd);
    }
    fn minimize_window(&mut self, hwnd: isize) {
        crate::windows::minimize_window(hwnd);
    }
    fn restore_window(&mut self, hwnd: isize) {
        crate::windows::restore_window(hwnd);
    }
    fn wake_for_close(&mut self, hwnd: isize) {
        use windows::Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE},
        };
        // An otherwise hidden eframe window needs one event to finish closing.
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut _), SW_SHOWNOACTIVATE);
        }
    }
}

pub(crate) fn ordered_windows(
    fresh: Vec<ManagedWindow>,
    previous: &[ManagedWindow],
    config: &crate::config::Config,
    activity: &std::sync::Mutex<crate::activity::ActivityTracker>,
) -> Vec<ManagedWindow> {
    if config.defaults.manual_order {
        return crate::order::reconcile(fresh, previous, &config.window_order);
    }
    if config.defaults.smart_sort {
        if let Ok(tracker) = activity.lock() {
            let scores = tracker.score_windows(&fresh);
            let mut scored: Vec<_> = fresh.into_iter().zip(scores).collect();
            scored.sort_by(|a, b| b.1.total_cmp(&a.1));
            return scored.into_iter().map(|(window, _)| window).collect();
        }
    }
    fresh
}
