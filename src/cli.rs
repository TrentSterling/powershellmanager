use crate::windows::ManagedWindow;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "powershellmanager", version)]
#[command(about = "Native Windows layouts, window inventory and ColorMagic Theme Studio")]
pub struct Cli {
    /// Apply a layout and exit (e.g., "2x3", "columns:4", "left-right")
    #[arg(long)]
    pub headless: Option<String>,
    /// Inspect the UI without moving windows, registering hotkeys or saving settings.
    #[arg(long, conflicts_with = "headless")]
    pub preview: bool,
    /// List matching windows and exit without changing them or saving settings.
    #[arg(long, conflicts_with_all = ["preview", "headless"])]
    pub list: bool,
    /// Output the window inventory as JSON.
    #[arg(long, requires = "list")]
    pub json: bool,
    /// Override the inventory filter: terminals, all, or comma-separated executables.
    #[arg(long, requires = "list")]
    pub target: Option<String>,
}

pub fn format_inventory(windows: &[ManagedWindow], json: bool) -> String {
    if json {
        let entries: Vec<_> = windows.iter().map(|window| serde_json::json!({
            "hwnd": window.hwnd,
            "process": window.process_name,
            "title": window.title,
            "category": window.category.display_name(),
            "minimized": window.is_minimized,
            "bounds": {"x": window.rect.x, "y": window.rect.y, "width": window.rect.w, "height": window.rect.h}
        })).collect();
        return serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "count": windows.len(),
            "windows": entries,
        })
        .to_string();
    }
    let mut output = format!(
        "PowerShell Manager v{}: {} windows\nHWND\tPROCESS\tSTATE\tTITLE",
        env!("CARGO_PKG_VERSION"),
        windows.len()
    );
    for window in windows {
        let title: String = window
            .title
            .chars()
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .collect();
        output.push_str(&format!(
            "\n{:#x}\t{}\t{}\t{}",
            window.hwnd,
            window.process_name,
            if window.is_minimized {
                "Minimized"
            } else {
                "Ready"
            },
            title
        ));
    }
    output
}
