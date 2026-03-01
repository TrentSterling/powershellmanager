use crate::activity::ActivityTracker;
use crate::config::PinRule;
use crate::layout::{LayoutPreset, compute_weighted_grid};
use crate::monitor::{enumerate_monitors, resolve_monitor};
use crate::windows::{ManagedWindow, TargetFilter, find_windows};
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
    filter: &TargetFilter,
    monitor_spec: &str,
    gap: i32,
    disabled: &HashSet<usize>,
    weights: Option<(&[f32], &[f32])>,
    app_hwnd: isize,
    extra_exclude: &[String],
    smart_sort: bool,
    activity: Option<&ActivityTracker>,
    pin_rules: &[PinRule],
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
    let all_slots = if let (Some((col_w, row_w)), LayoutPreset::Grid { cols, rows }) = (weights, preset) {
        compute_weighted_grid(*cols, *rows, &monitor.work_area, gap, col_w, row_w)
    } else {
        preset.compute_slots(&monitor.work_area, gap)
    };

    // Only use enabled slots
    let slots: Vec<_> = all_slots
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !disabled.contains(i))
        .map(|(_, slot)| slot)
        .collect();

    let mut windows = find_windows(filter, app_hwnd, extra_exclude);

    if smart_sort && !pin_rules.is_empty() {
        // Pin resolution: assign pinned windows to their slots first
        let mut pinned: Vec<(usize, ManagedWindow)> = Vec::new(); // (slot_index, window)
        let mut unpinned: Vec<ManagedWindow> = Vec::new();

        for win in windows.drain(..) {
            let mut matched_slot = None;
            for rule in pin_rules {
                if rule.matches(&win.process_name, &win.title) && rule.slot < slots.len() {
                    matched_slot = Some(rule.slot);
                    break;
                }
            }
            if let Some(slot_idx) = matched_slot {
                pinned.push((slot_idx, win));
            } else {
                unpinned.push(win);
            }
        }

        // Score and sort unpinned windows
        if smart_sort {
            if let Some(tracker) = activity {
                let scores = tracker.score_windows(&unpinned);
                // Sort by score descending
                let mut scored: Vec<_> = unpinned.into_iter().zip(scores).collect();
                scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                unpinned = scored.into_iter().map(|(w, _)| w).collect();
            }
        }

        // Build final ordered list: pinned get their exact slots, unpinned fill remaining
        let used_slots: HashSet<usize> = pinned.iter().map(|(s, _)| *s).collect();
        let mut final_windows: Vec<Option<ManagedWindow>> = vec![None; slots.len()];

        // Place pinned windows
        for (slot_idx, win) in pinned {
            if slot_idx < final_windows.len() {
                final_windows[slot_idx] = Some(win);
            }
        }

        // Fill remaining slots with unpinned windows in score order
        let mut unpinned_iter = unpinned.into_iter();
        for i in 0..slots.len() {
            if final_windows[i].is_none() && !used_slots.contains(&i) {
                if let Some(win) = unpinned_iter.next() {
                    final_windows[i] = Some(win);
                }
            }
        }

        // Position all assigned windows
        let mut arranged = 0;
        let mut errors = Vec::new();

        for (i, maybe_win) in final_windows.iter().enumerate() {
            if let Some(win) = maybe_win {
                let slot = &slots[i];
                let result = unsafe {
                    SetWindowPos(
                        HWND(win.hwnd as *mut _),
                        None,
                        slot.x, slot.y, slot.w, slot.h,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                };
                match result {
                    Ok(()) => arranged += 1,
                    Err(e) => errors.push(format!("Failed to position '{}': {}", win.title, e)),
                }
            }
        }

        let remaining: usize = unpinned_iter.count();
        return ArrangeResult {
            arranged,
            skipped: remaining,
            errors,
        };
    } else if smart_sort {
        // Smart sort without pins
        if let Some(tracker) = activity {
            let scores = tracker.score_windows(&windows);
            let mut scored: Vec<_> = windows.into_iter().zip(scores).collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            windows = scored.into_iter().map(|(w, _)| w).collect();
        }
    }

    // Standard arrangement (no pins, or no smart sort)
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
