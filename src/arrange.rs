use crate::config::PinRule;
use crate::history::WindowBackend;
use crate::layout::LayoutPreset;
use crate::monitor::{resolve_monitor, MonitorInfo};
use crate::windows::ManagedWindow;
use crate::windows::WindowSnapshot;
use std::collections::HashSet;

#[derive(Debug)]
pub struct ArrangeResult {
    pub arranged: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub snapshots: Vec<WindowSnapshot>,
    /// Successfully moved windows and their physical slot indices.
    pub placed: Vec<(isize, usize)>,
}

impl ArrangeResult {
    pub fn status(&self) -> String {
        let mut status = format!(
            "Arranged {} windows. {} skipped. {} errors.",
            self.arranged,
            self.skipped,
            self.errors.len()
        );
        if !self.warnings.is_empty() {
            status.push_str(&format!(" {}", self.warnings.join("; ")));
        }
        status
    }
}

pub(crate) struct ArrangeSettings<'a> {
    pub preset: &'a LayoutPreset,
    pub monitor_spec: &'a str,
    pub gap: i32,
    pub disabled: &'a HashSet<usize>,
    pub weights: Option<(&'a [f32], &'a [f32])>,
    pub pins: &'a [PinRule],
    /// Sticky slots; None lets the queue order decide (manual order, headless).
    pub memory: Option<&'a crate::sticky::SlotMemory>,
}

pub(crate) fn arrange_with(
    backend: &mut dyn WindowBackend,
    monitors: &[MonitorInfo],
    settings: &ArrangeSettings<'_>,
    windows: &[ManagedWindow],
) -> ArrangeResult {
    if monitors.is_empty() {
        return ArrangeResult {
            arranged: 0,
            skipped: windows.len(),
            errors: vec!["No monitors found".into()],
            warnings: Vec::new(),
            snapshots: Vec::new(),
            placed: Vec::new(),
        };
    }
    let area = resolve_monitor(monitors, settings.monitor_spec).work_area;
    let slots = crate::order::grid_slots(settings.preset, &area, settings.gap, settings.weights);
    let preferred = settings
        .memory
        .map(|memory| crate::sticky::preferred(windows, &slots, memory))
        .unwrap_or_default();
    let plan = crate::order::assign_with(
        windows,
        settings.pins,
        slots.len(),
        settings.disabled,
        &preferred,
    );
    let moves = plan
        .slots
        .iter()
        .enumerate()
        .filter_map(|(slot, window)| window.map(|i| (windows[i].hwnd, slot)))
        .collect();
    execute_slots(backend, &slots, moves, windows.len(), plan.warnings)
}

/// Move windows to slot indices and report which ones actually landed.
pub(crate) fn execute_slots(
    backend: &mut dyn WindowBackend,
    slots: &[crate::layout::Slot],
    moves: Vec<(isize, usize)>,
    total_windows: usize,
    warnings: Vec<String>,
) -> ArrangeResult {
    let placements = moves
        .iter()
        .map(|&(hwnd, slot)| (hwnd, slots[slot].clone()))
        .collect();
    let mut result = execute_plan(backend, placements, total_windows, warnings);
    let moved: HashSet<isize> = result.snapshots.iter().map(|s| s.hwnd).collect();
    result.placed = moves
        .into_iter()
        .filter(|(hwnd, _)| moved.contains(hwnd))
        .collect();
    result
}

pub fn execute_plan(
    backend: &mut dyn WindowBackend,
    placements: Vec<(isize, crate::layout::Slot)>,
    total_windows: usize,
    warnings: Vec<String>,
) -> ArrangeResult {
    let mut result = ArrangeResult {
        arranged: 0,
        skipped: total_windows.saturating_sub(placements.len()),
        errors: Vec::new(),
        warnings,
        snapshots: Vec::new(),
        placed: Vec::new(),
    };
    for (hwnd, slot) in placements {
        let snapshot = match backend.capture(hwnd) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                result.errors.push(error);
                continue;
            }
        };
        match backend.position(&snapshot, &slot) {
            Ok(()) => {
                result.arranged += 1;
                result.snapshots.push(snapshot);
            }
            Err(error) => result.errors.push(error),
        }
    }
    result
}
