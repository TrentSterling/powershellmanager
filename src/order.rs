//! Manual order uses live HWNDs within a session and title/process keys on restart.
use crate::{
    layout::{compute_weighted_grid, LayoutPreset, Slot},
    monitor::Rect,
    windows::ManagedWindow,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowKey {
    pub process: String,
    pub title: String,
}

impl WindowKey {
    pub fn from_window(w: &ManagedWindow) -> Self {
        Self {
            process: w.process_name.clone(),
            title: w.title.clone(),
        }
    }
}

pub fn reconcile(
    mut fresh: Vec<ManagedWindow>,
    previous: &[ManagedWindow],
    saved: &[WindowKey],
) -> Vec<ManagedWindow> {
    let mut ordered = Vec::with_capacity(fresh.len());
    for old in previous {
        if let Some(i) = fresh.iter().position(|w| {
            w.hwnd == old.hwnd && w.process_name.eq_ignore_ascii_case(&old.process_name)
        }) {
            ordered.push(fresh.remove(i));
        }
    }
    for key in saved {
        if let Some(i) = fresh
            .iter()
            .position(|w| w.process_name.eq_ignore_ascii_case(&key.process) && w.title == key.title)
        {
            ordered.push(fresh.remove(i));
        }
    }
    ordered.extend(fresh);
    ordered
}

pub fn move_relative(
    windows: &mut Vec<ManagedWindow>,
    source: isize,
    target: isize,
    after: bool,
) -> bool {
    if source == target {
        return false;
    }
    let Some(from) = windows.iter().position(|w| w.hwnd == source) else {
        return false;
    };
    if !windows.iter().any(|w| w.hwnd == target) {
        return false;
    }
    let item = windows.remove(from);
    let to = windows
        .iter()
        .position(|w| w.hwnd == target)
        .expect("target retained");
    windows.insert(to + usize::from(after), item);
    true
}

/// Physical slot numbers never change when a cell is disabled.
#[derive(Debug)]
pub struct Assignment {
    pub slots: Vec<Option<usize>>,
    pub rule_windows: Vec<Option<usize>>,
    pub warnings: Vec<String>,
}

pub fn assign(
    windows: &[ManagedWindow],
    pins: &[crate::config::PinRule],
    count: usize,
    disabled: &HashSet<usize>,
) -> Assignment {
    let mut plan = Assignment {
        slots: vec![None; count],
        rule_windows: vec![None; pins.len()],
        warnings: Vec::new(),
    };
    let mut reserved = HashSet::new();
    let mut placed = HashSet::new();
    for (r, pin) in pins.iter().enumerate() {
        let candidate = if let Some(hwnd) = pin.bound_hwnd {
            windows.iter().position(|w| {
                w.hwnd == hwnd
                    && pin
                        .process
                        .as_ref()
                        .is_none_or(|p| w.process_name.eq_ignore_ascii_case(p))
            })
        } else {
            windows
                .iter()
                .enumerate()
                .find(|(i, w)| !reserved.contains(i) && pin.matches(&w.process_name, &w.title))
                .map(|(i, _)| i)
        };
        let Some(i) = candidate else {
            continue;
        };
        if !reserved.insert(i) {
            plan.warnings
                .push(format!("Duplicate pin for {}", windows[i].title));
            continue;
        }
        plan.rule_windows[r] = Some(i);
        if pin.slot >= count || disabled.contains(&pin.slot) {
            plan.warnings.push(format!(
                "Pin slot {} unavailable; window uses the queue",
                pin.slot.saturating_add(1)
            ));
        } else if plan.slots[pin.slot].is_some() {
            plan.warnings.push(format!(
                "Pin conflict at slot {}; window uses the queue",
                pin.slot + 1
            ));
        } else {
            plan.slots[pin.slot] = Some(i);
            placed.insert(i);
        }
    }
    let mut remaining = (0..windows.len()).filter(|i| !placed.contains(i));
    for (slot, item) in plan.slots.iter_mut().enumerate() {
        if !disabled.contains(&slot) && item.is_none() {
            *item = remaining.next();
        }
    }
    plan
}

pub fn placements(
    preset: &LayoutPreset,
    area: &Rect,
    gap: i32,
    disabled: &HashSet<usize>,
    weights: Option<(&[f32], &[f32])>,
    windows: &[ManagedWindow],
    pins: &[crate::config::PinRule],
) -> (Vec<(isize, Slot)>, Vec<String>) {
    let slots = if let (Some((cw, rw)), LayoutPreset::Grid { cols, rows }) = (weights, preset) {
        compute_weighted_grid(*cols, *rows, area, gap, cw, rw)
    } else {
        preset.compute_slots(area, gap)
    };
    let plan = assign(windows, pins, slots.len(), disabled);
    (
        slots
            .into_iter()
            .zip(plan.slots)
            .filter_map(|(s, w)| w.map(|i| (windows[i].hwnd, s)))
            .collect(),
        plan.warnings,
    )
}
