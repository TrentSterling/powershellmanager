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
        };
    }
    let area = resolve_monitor(monitors, settings.monitor_spec).work_area;
    let (placements, warnings) = crate::order::placements(
        settings.preset,
        &area,
        settings.gap,
        settings.disabled,
        settings.weights,
        windows,
        settings.pins,
    );
    execute_plan(backend, placements, windows.len(), warnings)
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
