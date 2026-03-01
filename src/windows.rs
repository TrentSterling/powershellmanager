use crate::monitor::Rect;
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM, TRUE};
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, GWL_EXSTYLE, GetWindowLongPtrW,
    SW_HIDE, SW_MINIMIZE, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
    WS_EX_TOOLWINDOW,
};

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
            Self::Browser  => "B",
            Self::Editor   => "E",
            Self::Chat     => "C",
            Self::Media    => "M",
            Self::Game     => "G",
            Self::DevTool  => "D",
            Self::System   => "S",
            Self::Other    => "?",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Terminal => "Terminal",
            Self::Browser  => "Browser",
            Self::Editor   => "Editor",
            Self::Chat     => "Chat",
            Self::Media    => "Media",
            Self::Game     => "Game",
            Self::DevTool  => "DevTool",
            Self::System   => "System",
            Self::Other    => "Other",
        }
    }
}

pub fn categorize_process(name: &str) -> AppCategory {
    match name.to_lowercase().as_str() {
        // Terminals
        "powershell.exe" | "pwsh.exe" | "cmd.exe"
        | "windowsterminal.exe" | "alacritty.exe" | "wezterm-gui.exe"
        | "hyper.exe" | "mintty.exe" | "conhost.exe"
        | "conemu64.exe" | "conemu.exe" | "tabby.exe"
        | "terminus.exe" | "kitty.exe" | "rio.exe"
        | "warp.exe" => AppCategory::Terminal,

        // Browsers
        "chrome.exe" | "firefox.exe" | "msedge.exe"
        | "brave.exe" | "vivaldi.exe" | "opera.exe"
        | "arc.exe" | "waterfox.exe" | "librewolf.exe" => AppCategory::Browser,

        // Editors / IDEs
        "code.exe" | "devenv.exe" | "rider64.exe"
        | "idea64.exe" | "sublime_text.exe" | "notepad++.exe"
        | "notepad.exe" | "zed.exe" | "cursor.exe"
        | "windsurf.exe" => AppCategory::Editor,

        // Chat / Communication
        "discord.exe" | "slack.exe" | "teams.exe"
        | "telegram.exe" | "signal.exe" | "element.exe"
        | "zoom.exe" => AppCategory::Chat,

        // Media
        "spotify.exe" | "vlc.exe" | "obs64.exe"
        | "obs.exe" | "audacity.exe" | "foobar2000.exe"
        | "mpv.exe" => AppCategory::Media,

        // Games
        "steam.exe" | "epicgameslauncher.exe"
        | "gogalaxy.exe" => AppCategory::Game,

        // Dev Tools
        "unity.exe" | "unrealengine.exe"
        | "blender.exe" | "gimp-2.10.exe" | "gimp.exe"
        | "figma.exe" | "postman.exe"
        | "gitextensions.exe" | "sourcetree.exe"
        | "fork.exe" | "filezilla.exe"
        | "docker.exe" | "winscp.exe" | "putty.exe" => AppCategory::DevTool,

        // System
        "explorer.exe" | "taskmgr.exe" | "mmc.exe"
        | "regedit.exe" | "control.exe"
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

#[derive(Debug, Clone, PartialEq)]
pub enum TargetFilter {
    Terminals,
    Universal,
    Custom(Vec<String>),
}

impl TargetFilter {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "powershell" | "ps" | "terminal" | "wt" | "terminals" => Self::Terminals,
            "all" | "universal" => Self::Universal,
            _ => {
                // Could be comma-separated list of process names
                let names: Vec<String> = s.split(',')
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
                matches!(lower.as_str(),
                    "powershell.exe" | "pwsh.exe"
                    | "windowsterminal.exe" | "cmd.exe"
                    | "alacritty.exe" | "wezterm-gui.exe"
                    | "hyper.exe" | "mintty.exe"
                    | "conhost.exe" | "conemu64.exe" | "conemu.exe"
                    | "tabby.exe" | "terminus.exe"
                    | "kitty.exe" | "rio.exe" | "warp.exe"
                )
            }
            Self::Universal => true, // Accept all — filtering done elsewhere
            Self::Custom(names) => names.iter().any(|n| lower == *n),
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

pub fn find_windows(filter: &TargetFilter, app_hwnd: isize, extra_exclude: &[String]) -> Vec<ManagedWindow> {
    struct EnumState {
        filter: TargetFilter,
        app_hwnd: isize,
        extra_exclude: Vec<String>,
        results: Vec<ManagedWindow>,
    }

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut EnumState);

        // Skip own window
        if hwnd.0 as isize == state.app_hwnd {
            return TRUE;
        }

        if !IsWindowVisible(hwnd).as_bool() {
            return TRUE;
        }

        // Skip tool windows
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if (ex_style as u32) & WS_EX_TOOLWINDOW.0 != 0 {
            return TRUE;
        }

        // Get window rect
        let mut rect = windows::Win32::Foundation::RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return TRUE;
        }
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w <= 0 || h <= 0 {
            return TRUE;
        }

        // Get process name
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return TRUE;
        }

        let process_name = get_process_name(pid).unwrap_or_default();
        if process_name.is_empty() {
            return TRUE;
        }

        let lower = process_name.to_lowercase();

        // Check user-configured exclusions
        if state.extra_exclude.iter().any(|ex| lower == *ex) {
            return TRUE;
        }

        // Universal mode: exclude system windows
        if state.filter == TargetFilter::Universal {
            // Check excluded processes
            if EXCLUDED_PROCESSES.iter().any(|&p| lower == p) {
                return TRUE;
            }

            // Check excluded window classes
            let mut class_buf = [0u16; 256];
            let class_len = GetClassNameW(hwnd, &mut class_buf);
            if class_len > 0 {
                let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);
                if EXCLUDED_CLASSES.iter().any(|&c| c == class_name) {
                    return TRUE;
                }
            }

            // Special case: explorer.exe windows that aren't File Explorer
            // Only allow explorer.exe if it has the CabinetWClass (File Explorer window)
            if lower == "explorer.exe" {
                let mut class_buf2 = [0u16; 256];
                let class_len2 = GetClassNameW(hwnd, &mut class_buf2);
                if class_len2 > 0 {
                    let class_name = String::from_utf16_lossy(&class_buf2[..class_len2 as usize]);
                    if class_name != "CabinetWClass" {
                        return TRUE;
                    }
                } else {
                    return TRUE;
                }
            }
        }

        // Check filter match (for Terminals/Custom modes)
        if !state.filter.matches(&process_name) {
            return TRUE;
        }

        // Get window title
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, &mut buf);
        let title = if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        };

        let is_minimized = IsIconic(hwnd).as_bool();
        let category = categorize_process(&process_name);

        state.results.push(ManagedWindow {
            hwnd: hwnd.0 as isize,
            title,
            process_name,
            category,
            rect: Rect {
                x: rect.left,
                y: rect.top,
                w,
                h,
            },
            is_minimized,
        });

        TRUE
    }

    let mut state = EnumState {
        filter: filter.clone(),
        app_hwnd,
        extra_exclude: extra_exclude.to_vec(),
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
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(HWND(hwnd as *mut _), Some(&mut pid));
        if pid == 0 {
            return None;
        }
        get_process_name(pid)
    }
}

/// Get the title of a window by handle.
pub fn get_window_title(hwnd: isize) -> String {
    unsafe {
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(HWND(hwnd as *mut _), &mut buf);
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }
}

fn get_process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;
        let mut buf = [0u16; 260];
        let len = K32GetModuleFileNameExW(handle, HMODULE::default(), &mut buf);
        let _ = CloseHandle(handle);

        if len == 0 {
            return None;
        }

        let full_path = String::from_utf16_lossy(&buf[..len as usize]);
        full_path
            .rsplit('\\')
            .next()
            .map(|s| s.to_string())
    }
}
