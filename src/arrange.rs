use crate::layout::LayoutPreset;
use crate::monitor::{enumerate_monitors, resolve_monitor};
use crate::windows::{TargetFilter, find_terminal_windows};
use std::collections::HashSet;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos};

#[derive(Debug)]
pub struct ArrangeResult {
    pub arranged: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

pub fn arrange_masked(
    preset: &LayoutPreset,
    filter: TargetFilter,
    monitor_spec: &str,
    gap: i32,
    disabled: &HashSet<usize>,
) -> ArrangeResult {
    let monitors = enumerate_monitors();
    if monitors.is_empty() {
        return ArrangeResult {
            arranged: 0,
            skipped: 0,
            errors: vec!["No monitors detected".into()],
        };
    }

    let monitor = resolve_monitor(&monitors, monitor_spec);
    let all_slots = preset.compute_slots(&monitor.work_area, gap);

    // Only use enabled slots
    let slots: Vec<_> = all_slots
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !disabled.contains(i))
        .map(|(_, slot)| slot)
        .collect();

    let windows = find_terminal_windows(filter);

    let mut arranged = 0;
    let mut errors = Vec::new();

    for (i, win) in windows.iter().enumerate() {
        if i >= slots.len() {
            break;
        }
        let slot = &slots[i];

        let result = unsafe {
            SetWindowPos(
                HWND(win.hwnd as *mut _),
                None,
                slot.x,
                slot.y,
                slot.w,
                slot.h,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        };

        match result {
            Ok(()) => arranged += 1,
            Err(e) => errors.push(format!("Failed to position '{}': {}", win.title, e)),
        }
    }

    let skipped = if windows.len() > slots.len() {
        windows.len() - slots.len()
    } else {
        0
    };

    ArrangeResult {
        arranged,
        skipped,
        errors,
    }
}
