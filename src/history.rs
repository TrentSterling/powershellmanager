//! Session-local undo. Only windows successfully moved by PSM enter the history.
use crate::{layout::Slot, windows::WindowSnapshot};
use std::collections::VecDeque;

pub const MAX_UNDO_STEPS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreStatus {
    Restored,
    Closed,
}

/// The native implementation and isolated tests execute the same action pipeline.
pub trait WindowBackend {
    fn capture(&mut self, hwnd: isize) -> Result<WindowSnapshot, String>;
    fn position(&mut self, snapshot: &WindowSnapshot, slot: &Slot) -> Result<(), String>;
    fn restore(&mut self, snapshot: &WindowSnapshot) -> Result<RestoreStatus, String>;
}

#[derive(Default)]
pub struct LayoutHistory {
    steps: VecDeque<Vec<WindowSnapshot>>,
    pub pending_status: Option<String>,
    /// Sticky slots share this lock, so GUI, tray and auto mode never race.
    pub slots: crate::sticky::SlotMemory,
}

#[derive(Debug, Default)]
pub struct UndoResult {
    pub restored: usize,
    pub closed: usize,
    pub errors: Vec<String>,
}

impl UndoResult {
    pub fn status(&self) -> String {
        format!(
            "Restored {} windows. {} closed or replaced. {} errors.",
            self.restored,
            self.closed,
            self.errors.len()
        )
    }
}

impl LayoutHistory {
    pub fn record(&mut self, snapshots: Vec<WindowSnapshot>) {
        if snapshots.is_empty() {
            return;
        }
        self.steps.push_back(snapshots);
        if self.steps.len() > MAX_UNDO_STEPS {
            self.steps.pop_front();
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.steps.is_empty()
    }

    pub fn undo(&mut self, backend: &mut dyn WindowBackend) -> UndoResult {
        let mut result = UndoResult::default();
        let Some(snapshots) = self.steps.pop_back() else {
            return result;
        };
        let mut retry = Vec::new();
        for snapshot in snapshots {
            // Restored windows sit where they were; their position decides their slot again.
            self.slots.remove(&snapshot.hwnd);
            match backend.restore(&snapshot) {
                Ok(RestoreStatus::Restored) => result.restored += 1,
                Ok(RestoreStatus::Closed) => result.closed += 1,
                Err(error) => {
                    result.errors.push(error);
                    retry.push(snapshot);
                }
            }
        }
        // A temporary access/positioning failure remains available for another try.
        if !retry.is_empty() {
            self.steps.push_back(retry);
        }
        result
    }
}
