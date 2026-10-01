use super::*;

#[test]
fn failed_monitor_info_keeps_previous_results_and_continues_enumeration() {
    let area = Rect {
        x: -1200,
        y: 50,
        w: 1200,
        h: 900,
    };
    let mut state = EnumState {
        monitors: vec![(HMONITOR::default(), area, true)],
    };
    // This invokes the production callback with owned state and a real invalid
    // monitor query. It does not create, activate or move any desktop windows.
    let keep_going = unsafe {
        enum_callback(
            HMONITOR::default(),
            HDC::default(),
            std::ptr::null_mut(),
            LPARAM(&mut state as *mut EnumState as isize),
        )
    };
    assert!(keep_going.as_bool());
    assert_eq!(state.monitors.len(), 1);
    let (_, retained, primary) = state.monitors[0];
    assert_eq!(
        (retained.x, retained.y, retained.w, retained.h),
        (area.x, area.y, area.w, area.h)
    );
    assert!(primary);
}
