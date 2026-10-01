use super::*;
use crate::windows::AppCategory;

fn slot(x: i32) -> Slot {
    Slot {
        x,
        y: 0,
        w: 100,
        h: 100,
    }
}

fn window(hwnd: isize, x: i32, minimized: bool) -> ManagedWindow {
    ManagedWindow {
        hwnd,
        title: format!("T{hwnd}"),
        process_name: "pwsh.exe".into(),
        category: AppCategory::Terminal,
        rect: Rect {
            x,
            y: -7,
            w: 114,
            h: 114,
        },
        is_minimized: minimized,
    }
}

#[test]
fn position_matches_need_the_center_inside_and_a_similar_size() {
    let slots = [slot(0), slot(100)];
    let at = |x, w| Rect { x, y: 0, w, h: 100 };
    assert_eq!(occupied_slot(&at(100, 100), &slots), Some(1));
    assert_eq!(occupied_slot(&at(-7, 114), &slots), Some(0));
    assert_eq!(occupied_slot(&at(150, 100), &slots), None);
    assert_eq!(occupied_slot(&at(0, 200), &slots), None);
    assert_eq!(occupied_slot(&at(10, 60), &slots), None);
    let tall = Rect {
        x: 0,
        y: -100,
        w: 100,
        h: 300,
    };
    assert_eq!(occupied_slot(&tall, &slots), None);
    let below = Rect {
        x: 0,
        y: 200,
        w: 100,
        h: 100,
    };
    assert_eq!(occupied_slot(&below, &slots), None);
    let extreme = Rect {
        x: i32::MAX,
        y: i32::MAX,
        w: i32::MAX,
        h: i32::MAX,
    };
    assert_eq!(occupied_slot(&extreme, &slots), None);
}

#[test]
fn memory_beats_position_and_minimized_windows_use_memory_only() {
    let slots = [slot(0), slot(100)];
    let windows = [
        window(1, -7, false),
        window(2, 93, false),
        window(3, 93, true),
        window(4, 500, false),
        window(5, 500, true),
    ];
    let memory = SlotMemory::from([(2, 0), (5, 1)]);
    assert_eq!(
        preferred(&windows, &slots, &memory),
        [Some(0), Some(0), None, None, Some(1)]
    );
}

#[test]
fn remembering_a_slot_releases_older_claims_on_it() {
    let mut memory = SlotMemory::from([(1, 0), (2, 1), (3, 2)]);
    remember(&mut memory, &[(4, 1), (1, 3)]);
    assert_eq!(memory, SlotMemory::from([(1, 3), (3, 2), (4, 1)]));
}

#[test]
fn preferred_slots_follow_pins_and_skip_unavailable_or_taken_slots() {
    let windows: Vec<_> = (1..=6).map(|h| window(h, 0, false)).collect();
    let pins = [crate::config::PinRule {
        process: None,
        title_exact: Some("T1".into()),
        title_contains: None,
        bound_hwnd: None,
        slot: 2,
    }];
    let disabled = HashSet::from([3]);
    let preferred = [Some(0), Some(4), Some(9), Some(3), Some(4), None];
    let plan = crate::order::assign_with(&windows, &pins, 5, &disabled, &preferred);
    assert_eq!(plan.slots, [Some(2), Some(3), Some(0), None, Some(1)]);
}
