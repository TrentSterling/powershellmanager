use crate::monitor::Rect;
use windows::Win32::Foundation::{CloseHandle, BOOL, HANDLE, HWND, LPARAM, RECT, TRUE};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClassNameW, GetWindowLongPtrW, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    SetForegroundWindow, ShowWindow, GWL_EXSTYLE, SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW,
    WS_EX_TOOLWINDOW,
};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AppCategory {
    Terminal,
    Browser,
    Editor,
    Chat,
    Media,
    Game,
    DevTool,
    System,
    Other,
}

impl AppCategory {
    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Terminal => "T",
            Self::Browser => "B",
            Self::Editor => "E",
            Self::Chat => "C",
            Self::Media => "M",
            Self::Game => "G",
            Self::DevTool => "D",
            Self::System => "S",
            Self::Other => "?",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Browser => "Browser",
            Self::Editor => "Editor",
            Self::Chat => "Chat",
            Self::Media => "Media",
            Self::Game => "Game",
            Self::DevTool => "DevTool",
            Self::System => "System",
            Self::Other => "Other",
        }
    }
}

pub fn categorize_process(name: &str) -> AppCategory {
    match name.to_lowercase().as_str() {
        // Terminals
        "powershell.exe"
        | "pwsh.exe"
        | "cmd.exe"
        | "windowsterminal.exe"
        | "alacritty.exe"
        | "wezterm-gui.exe"
        | "hyper.exe"
        | "mintty.exe"
        | "conhost.exe"
        | "conemu64.exe"
        | "conemu.exe"
        | "tabby.exe"
        | "terminus.exe"
        | "kitty.exe"
        | "rio.exe"
        | "warp.exe" => AppCategory::Terminal,

        // Browsers
        "chrome.exe" | "firefox.exe" | "msedge.exe" | "brave.exe" | "vivaldi.exe" | "opera.exe"
        | "arc.exe" | "waterfox.exe" | "librewolf.exe" => AppCategory::Browser,

        // Editors / IDEs
        "code.exe" | "devenv.exe" | "rider64.exe" | "idea64.exe" | "sublime_text.exe"
        | "notepad++.exe" | "notepad.exe" | "zed.exe" | "cursor.exe" | "windsurf.exe" => {
            AppCategory::Editor
        }

        // Chat / Communication
        "discord.exe" | "slack.exe" | "teams.exe" | "telegram.exe" | "signal.exe"
        | "element.exe" | "zoom.exe" => AppCategory::Chat,

        // Media
        "spotify.exe" | "vlc.exe" | "obs64.exe" | "obs.exe" | "audacity.exe" | "foobar2000.exe"
        | "mpv.exe" => AppCategory::Media,

        // Games
        "steam.exe" | "epicgameslauncher.exe" | "gogalaxy.exe" => AppCategory::Game,

        // Dev Tools
        "unity.exe" | "unrealengine.exe" | "blender.exe" | "gimp-2.10.exe" | "gimp.exe"
        | "figma.exe" | "postman.exe" | "gitextensions.exe" | "sourcetree.exe" | "fork.exe"
        | "filezilla.exe" | "docker.exe" | "winscp.exe" | "putty.exe" => AppCategory::DevTool,

        // System
        "explorer.exe" | "taskmgr.exe" | "mmc.exe" | "regedit.exe" | "control.exe"
        | "perfmon.exe" | "resmon.exe" => AppCategory::System,

        _ => AppCategory::Other,
    }
}

#[derive(Debug, Clone)]
pub struct ManagedWindow {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub category: AppCategory,
    pub rect: Rect,
    pub is_minimized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowSnapshot {
    pub hwnd: isize,
    pub process_id: u32,
    pub placement: windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT,
    pub visible: bool,
}

trait PlacementApi {
    fn owner_process_id(&self, hwnd: isize) -> u32;
    fn window_visible(&self, hwnd: isize) -> bool;
    fn read_placement(
        &self,
        hwnd: isize,
    ) -> Result<windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT, String>;
    fn move_to(&self, hwnd: isize, slot: &crate::layout::Slot) -> Result<(), String>;
    fn write_placement(
        &self,
        hwnd: isize,
        placement: &windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT,
    ) -> Result<(), String>;
}

impl PlacementApi for NativeQueries {
    fn owner_process_id(&self, hwnd: isize) -> u32 {
        self.process_id(hwnd)
    }

    fn window_visible(&self, hwnd: isize) -> bool {
        self.visible(hwnd)
    }

    fn read_placement(
        &self,
        hwnd: isize,
    ) -> Result<windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT, String> {
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowPlacement, WINDOWPLACEMENT};
        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        unsafe { GetWindowPlacement(HWND(hwnd as *mut _), &mut placement) }
            .map_err(|error| error.to_string())?;
        Ok(placement)
    }

    fn move_to(&self, hwnd: isize, slot: &crate::layout::Slot) -> Result<(), String> {
        use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};
        unsafe {
            SetWindowPos(
                HWND(hwnd as *mut _),
                None,
                slot.x,
                slot.y,
                slot.w,
                slot.h,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|error| error.to_string())
    }

    fn write_placement(
        &self,
        hwnd: isize,
        placement: &windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT,
    ) -> Result<(), String> {
        use windows::Win32::UI::WindowsAndMessaging::SetWindowPlacement;
        unsafe { SetWindowPlacement(HWND(hwnd as *mut _), placement) }
            .map_err(|error| error.to_string())
    }
}

pub struct NativeWindowBackend;

impl crate::history::WindowBackend for NativeWindowBackend {
    fn capture(&mut self, hwnd: isize) -> Result<WindowSnapshot, String> {
        capture_with(&NativeQueries, hwnd)
    }

    fn position(
        &mut self,
        snapshot: &WindowSnapshot,
        slot: &crate::layout::Slot,
    ) -> Result<(), String> {
        position_with(&NativeQueries, snapshot, slot)
    }

    fn restore(
        &mut self,
        snapshot: &WindowSnapshot,
    ) -> Result<crate::history::RestoreStatus, String> {
        restore_with(&NativeQueries, snapshot)
    }
}

fn capture_with(api: &dyn PlacementApi, hwnd: isize) -> Result<WindowSnapshot, String> {
    let process_id = api.owner_process_id(hwnd);
    if process_id == 0 {
        return Err(format!(
            "Could not snapshot window {hwnd}: window is closed"
        ));
    }
    let placement = api
        .read_placement(hwnd)
        .map_err(|error| format!("Could not snapshot window {hwnd}: {error}"))?;
    let snapshot = WindowSnapshot {
        hwnd,
        process_id,
        placement,
        visible: api.window_visible(hwnd),
    };
    if !snapshot.still_owned_with(api) {
        return Err(format!(
            "Window {hwnd} closed or changed owner while taking its snapshot"
        ));
    }
    Ok(snapshot)
}

fn position_with(
    api: &dyn PlacementApi,
    snapshot: &WindowSnapshot,
    slot: &crate::layout::Slot,
) -> Result<(), String> {
    if !snapshot.still_owned_with(api) {
        return Err(format!(
            "Window {} closed or changed owner before Apply",
            snapshot.hwnd
        ));
    }
    api.move_to(snapshot.hwnd, slot)
        .map_err(|error| format!("Could not position window {}: {error}", snapshot.hwnd))
}

fn restore_with(
    api: &dyn PlacementApi,
    snapshot: &WindowSnapshot,
) -> Result<crate::history::RestoreStatus, String> {
    use crate::history::RestoreStatus;
    if !snapshot.still_owned_with(api) {
        return Ok(RestoreStatus::Closed);
    }
    let mut placement = snapshot.placement;
    // Hidden audit/utility windows must remain hidden when restoring placement.
    if !snapshot.visible {
        placement.showCmd = SW_HIDE.0 as u32;
    }
    api.write_placement(snapshot.hwnd, &placement)
        .map_err(|error| format!("Could not restore window {}: {error}", snapshot.hwnd))?;
    Ok(RestoreStatus::Restored)
}

impl WindowSnapshot {
    fn still_owned_with(&self, api: &dyn PlacementApi) -> bool {
        let process_id = api.owner_process_id(self.hwnd);
        process_id != 0 && process_id == self.process_id
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum TargetFilter {
    Terminals,
    Universal,
    Custom(Vec<String>),
}

impl TargetFilter {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "powershell" | "ps" | "terminal" | "wt" | "terminals" => Self::Terminals,
            "all" | "universal" => Self::Universal,
            _ => {
                // Could be comma-separated list of process names
                let names: Vec<String> = s
                    .split(',')
                    .map(|n| n.trim().to_lowercase())
                    .filter(|n| !n.is_empty())
                    .collect();
                if names.is_empty() {
                    Self::Universal
                } else {
                    Self::Custom(names)
                }
            }
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Terminals => "Terminals",
            Self::Universal => "Universal",
            Self::Custom(_) => "Custom",
        }
    }

    fn matches(&self, process_name: &str) -> bool {
        let lower = process_name.to_lowercase();
        match self {
            Self::Terminals => {
                matches!(
                    lower.as_str(),
                    "powershell.exe"
                        | "pwsh.exe"
                        | "windowsterminal.exe"
                        | "cmd.exe"
                        | "alacritty.exe"
                        | "wezterm-gui.exe"
                        | "hyper.exe"
                        | "mintty.exe"
                        | "conhost.exe"
                        | "conemu64.exe"
                        | "conemu.exe"
                        | "tabby.exe"
                        | "terminus.exe"
                        | "kitty.exe"
                        | "rio.exe"
                        | "warp.exe"
                )
            }
            Self::Universal => true, // Accept all — filtering done elsewhere
            Self::Custom(names) => names.contains(&lower),
        }
    }
}

/// System window classes to exclude in Universal mode.
const EXCLUDED_CLASSES: &[&str] = &[
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Progman",
    "WorkerW",
    "Button", // Start button
    "Windows.UI.Core.CoreWindow",
];

/// System processes to exclude in Universal mode.
const EXCLUDED_PROCESSES: &[&str] = &[
    "powershellmanager.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "textinputhost.exe",
    "applicationframehost.exe",
    "systemsettings.exe",
    "lockapp.exe",
    "screenclippinghost.exe",
    "widgets.exe",
    "gamebar.exe",
    "gamebarpresencewriter.exe",
    "runtimebroker.exe",
    "dwm.exe",
    "csrss.exe",
    "lsass.exe",
    "services.exe",
    "svchost.exe",
    "winlogon.exe",
    "sihost.exe",
    "ctfmon.exe",
    "fontdrvhost.exe",
    "dllhost.exe",
    "conhost.exe",
    "securityhealthsystray.exe",
    "crashpad_handler.exe",
    "ceftestprocess.exe",
    "msedgewebview2.exe",
    "windowsinternal.composableshell.experiences.textinput.inputapp.exe",
];

trait WindowQueries {
    fn visible(&self, hwnd: isize) -> bool;
    fn ex_style(&self, hwnd: isize) -> u32;
    fn rect(&self, hwnd: isize) -> Option<RECT>;
    fn process_id(&self, hwnd: isize) -> u32;
    fn process_name(&self, pid: u32) -> Option<String>;
    fn class_name(&self, hwnd: isize) -> String;
    fn title(&self, hwnd: isize) -> String;
    fn minimized(&self, hwnd: isize) -> bool;
}

struct NativeQueries;

impl WindowQueries for NativeQueries {
    fn visible(&self, hwnd: isize) -> bool {
        unsafe { IsWindowVisible(HWND(hwnd as *mut _)) }.as_bool()
    }

    fn ex_style(&self, hwnd: isize) -> u32 {
        unsafe { GetWindowLongPtrW(HWND(hwnd as *mut _), GWL_EXSTYLE) as u32 }
    }

    fn rect(&self, hwnd: isize) -> Option<RECT> {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(HWND(hwnd as *mut _), &mut rect) }.ok()?;
        Some(rect)
    }

    fn process_id(&self, hwnd: isize) -> u32 {
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(HWND(hwnd as *mut _), Some(&mut pid));
        }
        pid
    }

    fn process_name(&self, pid: u32) -> Option<String> {
        get_process_name(pid)
    }

    fn class_name(&self, hwnd: isize) -> String {
        let mut buffer = [0u16; 256];
        let length = unsafe { GetClassNameW(HWND(hwnd as *mut _), &mut buffer) }.max(0) as usize;
        String::from_utf16_lossy(&buffer[..length])
    }

    fn title(&self, hwnd: isize) -> String {
        get_window_title(hwnd)
    }

    fn minimized(&self, hwnd: isize) -> bool {
        unsafe { IsIconic(HWND(hwnd as *mut _)) }.as_bool()
    }
}

/// Resolve metadata lazily so rejected system/tool windows never need title queries.
fn inspect_window(
    queries: &dyn WindowQueries,
    hwnd: isize,
    filter: &TargetFilter,
    app_hwnd: isize,
    extra_exclude: &[String],
) -> Option<ManagedWindow> {
    if hwnd == app_hwnd || !queries.visible(hwnd) {
        return None;
    }
    if queries.ex_style(hwnd) & WS_EX_TOOLWINDOW.0 != 0 {
        return None;
    }
    let rect = queries.rect(hwnd)?;
    let w = rect.right.checked_sub(rect.left)?;
    let h = rect.bottom.checked_sub(rect.top)?;
    if w <= 0 || h <= 0 {
        return None;
    }
    let pid = queries.process_id(hwnd);
    if pid == 0 {
        return None;
    }
    let process_name = queries.process_name(pid)?;
    if process_name.is_empty() {
        return None;
    }
    let lower = process_name.to_lowercase();
    if extra_exclude
        .iter()
        .any(|name| name.trim().to_lowercase() == lower)
    {
        return None;
    }
    if *filter == TargetFilter::Universal {
        if EXCLUDED_PROCESSES.contains(&lower.as_str()) {
            return None;
        }
        let class = queries.class_name(hwnd);
        if EXCLUDED_CLASSES.contains(&class.as_str()) {
            return None;
        }
        if lower == "explorer.exe" && class != "CabinetWClass" {
            return None;
        }
    }
    if !filter.matches(&process_name) {
        return None;
    }
    Some(ManagedWindow {
        hwnd,
        title: queries.title(hwnd),
        category: categorize_process(&process_name),
        process_name,
        rect: Rect {
            x: rect.left,
            y: rect.top,
            w,
            h,
        },
        is_minimized: queries.minimized(hwnd),
    })
}

pub fn find_windows(
    filter: &TargetFilter,
    app_hwnd: isize,
    extra_exclude: &[String],
) -> Vec<ManagedWindow> {
    struct EnumState<'a> {
        filter: &'a TargetFilter,
        app_hwnd: isize,
        extra_exclude: &'a [String],
        results: Vec<ManagedWindow>,
    }
    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut EnumState);
        if let Some(window) = inspect_window(
            &NativeQueries,
            hwnd.0 as isize,
            state.filter,
            state.app_hwnd,
            state.extra_exclude,
        ) {
            state.results.push(window);
        }
        TRUE
    }
    let mut state = EnumState {
        filter,
        app_hwnd,
        extra_exclude,
        results: Vec::with_capacity(32),
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut state as *mut EnumState as isize),
        );
    }
    state.results
}
pub fn focus_window(hwnd: isize) {
    unsafe {
        let h = HWND(hwnd as *mut _);
        if IsIconic(h).as_bool() {
            let _ = ShowWindow(h, SW_RESTORE);
        }
        let _ = SetForegroundWindow(h);
    }
}

pub fn minimize_window(hwnd: isize) {
    unsafe {
        let _ = ShowWindow(HWND(hwnd as *mut _), SW_MINIMIZE);
    }
}

pub fn restore_window(hwnd: isize) {
    unsafe {
        let h = HWND(hwnd as *mut _);
        let _ = ShowWindow(h, SW_RESTORE);
        let _ = SetForegroundWindow(h);
    }
}

/// Show and restore the app window via direct Win32 calls.
/// Works even when eframe's update loop is paused (hidden window).
pub fn show_app_window(hwnd: isize) {
    unsafe {
        let h = HWND(hwnd as *mut _);
        let _ = ShowWindow(h, SW_SHOW);
        let _ = ShowWindow(h, SW_RESTORE);
        let _ = BringWindowToTop(h);
        let _ = SetForegroundWindow(h);
    }
}

/// Hide the app window via direct Win32 call.
pub fn hide_app_window(hwnd: isize) {
    unsafe {
        let _ = ShowWindow(HWND(hwnd as *mut _), SW_HIDE);
    }
}

/// Get the HWND of the current foreground window.
pub fn get_foreground_window() -> Option<isize> {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() {
            None
        } else {
            Some(hwnd.0 as isize)
        }
    }
}

/// Get the process name for a given window handle.
pub fn get_process_name_for_hwnd(hwnd: isize) -> Option<String> {
    let pid = NativeQueries.process_id(hwnd);
    if pid == 0 {
        return None;
    }
    get_process_name(pid)
}

/// Get the title of a window by handle.
pub fn get_window_title(hwnd: isize) -> String {
    unsafe {
        let length = GetWindowTextLengthW(HWND(hwnd as *mut _)).clamp(0, 8192) as usize;
        let mut buf = vec![0u16; length + 1];
        let len = GetWindowTextW(HWND(hwnd as *mut _), &mut buf);
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }
}

fn get_process_name(pid: u32) -> Option<String> {
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()? };
    process_name_from_owned_handle(handle)
}

/// Consumes a process handle opened by this module, including query failure.
fn process_name_from_owned_handle(handle: HANDLE) -> Option<String> {
    unsafe {
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        result.ok()?;
        let full_path = String::from_utf16_lossy(&buf[..len as usize]);
        process_name_from_path(&full_path)
    }
}

fn process_name_from_path(path: &str) -> Option<String> {
    path.rsplit(['\\', '/'])
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
}
