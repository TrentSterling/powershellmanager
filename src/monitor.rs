use windows::Win32::Foundation::{BOOL, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub is_primary: bool,
    pub work_area: Rect,
}

pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    struct EnumState {
        monitors: Vec<(HMONITOR, Rect, bool)>,
    }

    unsafe extern "system" fn enum_callback(
        hmon: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let state = &mut *(lparam.0 as *mut EnumState);

        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

        if GetMonitorInfoW(hmon, &mut info.monitorInfo).as_bool() {
            let wa = info.monitorInfo.rcWork;
            let work_area = Rect {
                x: wa.left,
                y: wa.top,
                w: wa.right - wa.left,
                h: wa.bottom - wa.top,
            };
            let is_primary = (info.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY
            state.monitors.push((hmon, work_area, is_primary));
        }

        TRUE
    }

    let mut state = EnumState {
        monitors: Vec::with_capacity(4),
    };

    unsafe {
        let _ = EnumDisplayMonitors(
            HDC::default(),
            None,
            Some(enum_callback),
            LPARAM(&mut state as *mut EnumState as isize),
        );
    }

    state
        .monitors
        .into_iter()
        .enumerate()
        .map(|(i, (_hmon, work_area, is_primary))| MonitorInfo {
            index: i,
            is_primary,
            work_area,
        })
        .collect()
}

pub fn resolve_monitor<'a>(monitors: &'a [MonitorInfo], spec: &str) -> &'a MonitorInfo {
    match spec {
        "primary" | "" => monitors
            .iter()
            .find(|m| m.is_primary)
            .unwrap_or(&monitors[0]),
        s => {
            if let Ok(idx) = s.parse::<usize>() {
                monitors.get(idx).unwrap_or(&monitors[0])
            } else {
                monitors
                    .iter()
                    .find(|m| m.is_primary)
                    .unwrap_or(&monitors[0])
            }
        }
    }
}
