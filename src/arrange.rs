use crate::config::PinRule;
use crate::layout::LayoutPreset;
use crate::monitor::{enumerate_monitors, resolve_monitor};
use crate::windows::ManagedWindow;
use std::collections::HashSet;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};

#[derive(Debug)]
pub struct ArrangeResult {
    pub arranged: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Apply the exact manual queue shown in the UI; never enumerate or rank it again.
pub fn arrange_ordered(
    preset: &LayoutPreset,
    monitor_spec: &str,
    gap: i32,
    disabled: &HashSet<usize>,
    weights: Option<(&[f32], &[f32])>,
    windows: &[ManagedWindow],
    pins: &[PinRule],
) -> ArrangeResult {
    let monitors = enumerate_monitors();
    if monitors.is_empty() {
        return ArrangeResult {
            arranged: 0,
            skipped: windows.len(),
            errors: vec!["No monitors found".into()],
            warnings: Vec::new(),
        };
    }
    let area = resolve_monitor(&monitors, monitor_spec).work_area;
    let (placements, warnings) =
        crate::order::placements(preset, &area, gap, disabled, weights, windows, pins);
    let mut result = ArrangeResult {
        arranged: 0,
        skipped: windows.len().saturating_sub(placements.len()),
        errors: Vec::new(),
        warnings,
    };
    for (hwnd, slot) in placements {
        let positioned = unsafe {
            SetWindowPos(
                HWND(hwnd as *mut _),
                None,
                slot.x,
                slot.y,
                slot.w,
                slot.h,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };
        match positioned {
            Ok(()) => result.arranged += 1,
            Err(error) => result
                .errors
                .push(format!("Could not position window {hwnd}: {error}")),
        }
    }
    result
}
