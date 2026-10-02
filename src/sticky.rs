//! Placed windows keep their slots: remembered this session, or recognized by position.
//! Only grabber drags (manual order) reassign windows that already have a slot.
use crate::{layout::Slot, monitor::Rect, windows::ManagedWindow};
use std::collections::{HashMap, HashSet};

/// Window handle to physical slot index, for the current session.
pub type SlotMemory = HashMap<isize, usize>;

// Invisible resize borders make a snapped window slightly larger than its slot.
const SIZE_TOLERANCE: i64 = 32;

/// The slot a window currently sits in: centered inside it and about the same size.
pub fn occupied_slot(rect: &Rect, slots: &[Slot]) -> Option<usize> {
    let (x, y, w, h) = (rect.x as i64, rect.y as i64, rect.w as i64, rect.h as i64);
    let (cx, cy) = (x + w / 2, y + h / 2);
    slots.iter().position(|s| {
        let (sx, sy, sw, sh) = (s.x as i64, s.y as i64, s.w as i64, s.h as i64);
        (sx..sx + sw).contains(&cx)
            && (sy..sy + sh).contains(&cy)
            && (w - sw).abs() <= SIZE_TOLERANCE
            && (h - sh).abs() <= SIZE_TOLERANCE
    })
}

/// Remembered slots win, so a window moved by hand returns home on Apply.
/// Minimized windows report no usable rectangle and rely on memory alone.
pub fn preferred(
    windows: &[ManagedWindow],
    slots: &[Slot],
    memory: &SlotMemory,
) -> Vec<Option<usize>> {
    windows
        .iter()
        .map(|w| match memory.get(&w.hwnd) {
            Some(&slot) => Some(slot),
            None if w.is_minimized => None,
            None => occupied_slot(&w.rect, slots),
        })
        .collect()
}

/// Record successful placements. A slot belongs to one window, so older claims drop.
pub fn remember(memory: &mut SlotMemory, placed: &[(isize, usize)]) {
    let slots: HashSet<usize> = placed.iter().map(|&(_, slot)| slot).collect();
    memory.retain(|_, slot| !slots.contains(slot));
    memory.extend(placed.iter().copied());
}

#[cfg(test)]
mod tests;
