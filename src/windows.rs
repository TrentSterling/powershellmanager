use crate::monitor::Rect;
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, LPARAM, TRUE};
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    GWL_EXSTYLE, GetWindowLongPtrW, WS_EX_TOOLWINDOW,
};

#[derive(Debug, Clone)]
pub struct TerminalWindow {
    pub hwnd: isize,
    pub title: String,
    pub process_name: String,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetFilter {
    PowerShell,
    Terminal,
    All,
}

impl TargetFilter {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "powershell" | "ps" => Self::PowerShell,
            "terminal" | "wt" => Self::Terminal,
            "all" => Self::All,
            _ => Self::All,
        }
    }

    fn matches(&self, process_name: &str) -> bool {
        let lower = process_name.to_lowercase();
        match self {
            Self::PowerShell => {
                lower == "powershell.exe" || lower == "pwsh.exe"
            }
            Self::Terminal => {
                lower == "windowsterminal.exe"
            }
            Self::All => {
                lower == "powershell.exe"
                    || lower == "pwsh.exe"
                    || lower == "windowsterminal.exe"
                    || lower == "cmd.exe"
                    || lower == "alacritty.exe"
                    || lower == "wezterm-gui.exe"
                    || lower == "hyper.exe"
                    || lower == "mintty.exe"
                    || lower == "conhost.exe"
                    || lower == "conemu64.exe"
                    || lower == "conemu.exe"
                    || lower == "tabby.exe"
                    || lower == "terminus.exe"
                    || lower == "kitty.exe"
                    || lower == "rio.exe"
            }
        }
    }
}

pub fn find_terminal_windows(filter: TargetFilter) -> Vec<TerminalWindow> {
    struct EnumState {
        filter: TargetFilter,
        results: Vec<TerminalWindow>,
    }

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut EnumState);

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
        if process_name.is_empty() || !state.filter.matches(&process_name) {
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

        state.results.push(TerminalWindow {
            hwnd: hwnd.0 as isize,
            title,
            process_name,
            rect: Rect {
                x: rect.left,
                y: rect.top,
                w,
                h,
            },
        });

        TRUE
    }

    let mut state = EnumState {
        filter,
        results: Vec::with_capacity(16),
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_callback),
            LPARAM(&mut state as *mut EnumState as isize),
        );
    }

    state.results
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
