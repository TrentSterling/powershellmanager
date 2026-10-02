//! Auto mode: new matching windows move into a free slot. Windows that already have
//! a slot never move, except when "Slide to fill" closes the gap a closed window left.
use crate::{
    arrange::{self, ArrangeSettings},
    config::Config,
    desktop::Desktop,
    history::LayoutHistory,
    monitor::{resolve_monitor, Rect},
    order, sticky,
    windows::{ManagedWindow, TargetFilter},
};
use std::collections::{HashMap, HashSet};

/// The action worker ticks every 100 ms; auto mode scans on every fifth tick.
pub(crate) const SCAN_TICKS: u32 = 5;
// A window that keeps resizing itself is placed anyway after this many scans.
const MAX_SETTLE_SCANS: u8 = 6;

#[derive(Default)]
pub(crate) struct AutoState {
    active: bool,
    known: HashSet<isize>,
    pending: HashMap<isize, (Rect, u8)>,
}

fn same_rect(a: &Rect, b: &Rect) -> bool {
    (a.x, a.y, a.w, a.h) == (b.x, b.y, b.w, b.h)
}

impl AutoState {
    /// Terminals size themselves after opening; wait until the rectangle stops changing.
    fn settled(&mut self, window: &ManagedWindow) -> bool {
        let Some((rect, scans)) = self.pending.get_mut(&window.hwnd) else {
            self.pending.insert(window.hwnd, (window.rect, 0));
            return false;
        };
        *scans += 1;
        let stable = same_rect(rect, &window.rect) || *scans >= MAX_SETTLE_SCANS;
        *rect = window.rect;
        stable
    }
}

/// One auto-mode pass. Returns a status line when something moved or was left alone.
pub(crate) fn scan(
    auto: &mut AutoState,
    desktop: &mut dyn Desktop,
    app_hwnd: isize,
    config: &Config,
    history: &mut LayoutHistory,
) -> Option<String> {
    let d = &config.defaults;
    if !d.auto_arrange {
        *auto = AutoState::default();
        return None;
    }
    let fresh = desktop.windows(
        &TargetFilter::from_str(&d.target),
        app_hwnd,
        &config.categories.excluded_lower(),
    );
    let alive: HashSet<isize> = fresh.iter().map(|w| w.hwnd).collect();
    if !auto.active {
        // Turning auto mode on never moves windows that are already open.
        auto.active = true;
        auto.known = alive;
        return None;
    }
    let mut freed = false;
    for hwnd in auto.known.difference(&alive) {
        freed |= history.slots.remove(hwnd).is_some();
    }
    auto.known.retain(|hwnd| alive.contains(hwnd));
    auto.pending.retain(|hwnd, _| alive.contains(hwnd));
    let monitors = desktop.monitors();
    if monitors.is_empty() {
        return None;
    }
    let area = resolve_monitor(&monitors, &d.monitor).work_area;
    let request = crate::tray::current_request(config);
    let weights = request
        .weights
        .as_ref()
        .map(|(c, r)| (c.as_slice(), r.as_slice()));
    let slots = order::grid_slots(&request.preset, &area, d.gap, weights);
    let mut notes = Vec::new();

    if freed && d.slide_to_fill {
        // Keep the slot order and close the gaps toward slot 1.
        let mut queue: Vec<(usize, &ManagedWindow)> = fresh
            .iter()
            .zip(sticky::preferred(&fresh, &slots, &history.slots))
            .filter(|(w, _)| auto.known.contains(&w.hwnd))
            .filter_map(|(w, slot)| slot.map(|slot| (slot, w)))
            .collect();
        queue.sort_by_key(|&(slot, _)| slot);
        let queue: Vec<ManagedWindow> = queue.into_iter().map(|(_, w)| w.clone()).collect();
        let result = arrange::arrange_with(
            desktop,
            &monitors,
            &ArrangeSettings {
                preset: &request.preset,
                monitor_spec: &d.monitor,
                gap: d.gap,
                disabled: &request.disabled,
                weights,
                pins: &config.pin,
                memory: None,
            },
            &queue,
        );
        sticky::remember(&mut history.slots, &result.placed);
        notes.push(format!(
            "Auto: slid {} windows to fill the gap.",
            result.arranged
        ));
        history.record(result.snapshots);
    }

    let mut arrivals = Vec::new();
    for window in &fresh {
        if auto.known.contains(&window.hwnd) {
            continue;
        }
        if window.is_minimized || auto.settled(window) {
            auto.known.insert(window.hwnd);
            auto.pending.remove(&window.hwnd);
            if !window.is_minimized {
                arrivals.push(window.clone());
            }
        }
    }
    if arrivals.is_empty() {
        return (!notes.is_empty()).then(|| notes.join(" "));
    }

    // Residents keep their slots; arrivals take what pins and the queue leave free.
    let arriving: HashSet<isize> = arrivals.iter().map(|w| w.hwnd).collect();
    let mut windows = Vec::new();
    let mut preferred = Vec::new();
    for (window, slot) in fresh
        .iter()
        .zip(sticky::preferred(&fresh, &slots, &history.slots))
    {
        if slot.is_some() && auto.known.contains(&window.hwnd) && !arriving.contains(&window.hwnd) {
            windows.push(window.clone());
            preferred.push(slot);
        }
    }
    let residents = windows.len();
    // An arrival that opened exactly in a free slot keeps it.
    preferred.extend(sticky::preferred(&arrivals, &slots, &history.slots));
    windows.extend(arrivals.iter().cloned());
    let plan = order::assign_with(
        &windows,
        &config.pin,
        slots.len(),
        &request.disabled,
        &preferred,
    );
    let moves: Vec<(isize, usize)> = plan
        .slots
        .iter()
        .enumerate()
        .filter_map(|(slot, index)| {
            index
                .filter(|&i| i >= residents)
                .map(|i| (windows[i].hwnd, slot))
        })
        .collect();
    let placed = moves.len();
    let result = arrange::execute_slots(desktop, &slots, moves, placed, Vec::new());
    sticky::remember(&mut history.slots, &result.placed);
    if result.arranged > 0 {
        notes.push(format!("Auto: placed {} new windows.", result.arranged));
    }
    notes.extend(result.errors);
    history.record(result.snapshots);

    let assigned: HashSet<isize> = plan
        .slots
        .iter()
        .flatten()
        .map(|&i| windows[i].hwnd)
        .collect();
    let waiting: Vec<&ManagedWindow> = arrivals
        .iter()
        .filter(|w| !assigned.contains(&w.hwnd))
        .collect();
    if !waiting.is_empty() {
        notes.push(overflow(
            desktop, config, &monitors, &area, &fresh, &waiting, history,
        ));
    }
    Some(notes.join(" "))
}

/// Every slot is taken: leave arrivals alone, or use the same grid on another display.
fn overflow(
    desktop: &mut dyn Desktop,
    config: &Config,
    monitors: &[crate::monitor::MonitorInfo],
    area: &Rect,
    fresh: &[ManagedWindow],
    waiting: &[&ManagedWindow],
    history: &mut LayoutHistory,
) -> String {
    let d = &config.defaults;
    let other = monitors
        .iter()
        .find(|m| !same_rect(&m.work_area, area))
        .filter(|_| d.overflow_display);
    let Some(other) = other else {
        return format!(
            "Auto: grid full; {} new windows left where they opened.",
            waiting.len()
        );
    };
    let request = crate::tray::current_request(config);
    let weights = request
        .weights
        .as_ref()
        .map(|(c, r)| (c.as_slice(), r.as_slice()));
    let slots = order::grid_slots(&request.preset, &other.work_area, d.gap, weights);
    let mut taken: HashSet<usize> = fresh
        .iter()
        .filter(|w| !w.is_minimized)
        .filter_map(|w| sticky::occupied_slot(&w.rect, &slots))
        .collect();
    taken.extend(request.disabled.iter().copied());
    let mut moves = Vec::new();
    for window in waiting {
        if let Some(slot) = (0..slots.len()).find(|slot| !taken.contains(slot)) {
            taken.insert(slot);
            moves.push((window.hwnd, slot));
        }
    }
    let result = arrange::execute_slots(desktop, &slots, moves, waiting.len(), Vec::new());
    history.record(result.snapshots);
    format!(
        "Auto: grid full; {} new windows sent to the other display, {} left where they opened.",
        result.arranged,
        waiting.len() - result.arranged
    )
}

#[cfg(test)]
mod tests;
