# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**PowerShell Manager** — System tray app for organizing multiple PowerShell/terminal windows into grid layouts. Lives in the tray, right-click to pick a layout preset (2x3 grid, portrait columns, landscape rows, square tiles, etc.) and snap all open terminals into place. Headless CLI mode available via `--headless`.

**Owner:** Trent Sterling (tront.xyz)

## Tech Stack

- **Language:** Rust (edition 2021)
- **GUI:** `egui` with `eframe` — lightweight immediate-mode GUI for the popup window
- **Tray:** `tray-icon` + `winit` for system tray icon and right-click menu
- **Win32:** `windows` crate for window enumeration, positioning, resizing
- **Config:** TOML for layout presets and user settings

## Build & Run

```bash
cargo run                      # launch tray app (default)
cargo run -- --headless 2x3    # headless: arrange and exit
cargo run --release            # release build
cargo test                     # run tests
```

## Key Win32 APIs

All window management goes through the `windows` crate (same patterns as `C:\trontstack\pettoy\src\platform\win32.rs`):

- `EnumWindows` — find all PowerShell/Terminal windows
- `SetWindowPos` — move and resize windows into grid cells
- `GetWindowRect` — read current window positions
- `GetWindowTextW` — identify windows by title
- `IsWindowVisible` — skip hidden windows
- `MonitorFromWindow` / `GetMonitorInfoW` — multi-monitor work area detection

Required `windows` crate features:
```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Gdi",
    "Win32_UI_Shell",           # Shell_NotifyIconW for tray
] }
```

## Architecture

```
src/
  main.rs        — entry point, arg parsing, tray vs headless mode
  app.rs         — eframe App impl, tray event loop, GUI state
  tray.rs        — system tray icon, right-click context menu
  gui.rs         — egui popup window (layout picker, preview, settings)
  windows.rs     — Win32 window enumeration and filtering
  layout.rs      — layout presets and grid math
  arrange.rs     — apply layouts to discovered windows
  config.rs      — TOML config loading/saving
  monitor.rs     — multi-monitor work area detection
```

**Two modes:**
- **Tray mode** (default) — lives in system tray, right-click for layout menu, left-click opens GUI popup with visual preview
- **Headless mode** (`--headless <layout>`) — arrange windows and exit immediately, no GUI

## Layout System

A layout is a named arrangement of N window slots on a monitor's work area (excludes taskbar).

**Built-in presets:**
- `grid NxM` — equal-size grid (e.g., 2x3 = 6 cells)
- `columns N` — N equal vertical columns (portrait)
- `rows N` — N equal horizontal rows (landscape)
- `left-right` — 50/50 split
- `main-side` — one large left pane + stacked right panes (IDE-style)

Each slot is `(x, y, width, height)` in screen pixels. Windows are assigned to slots in z-order (front-to-back) or creation order.

## Window Filtering

Target windows are identified by process name and/or title substring:
- `powershell.exe`, `pwsh.exe` — PowerShell
- `WindowsTerminal.exe` — Windows Terminal
- `cmd.exe` — Command Prompt (optional)

The tool should handle Windows Terminal tabs (single HWND with multiple tabs) vs separate windows.

## Conventions

- `snake_case` everywhere except types/traits (`PascalCase`)
- No `unwrap()` in production — use `expect("reason")` or propagate errors
- Comments explain *why*, not *what*
- Keep modules small, one concern per file
- Win32 unsafe blocks should be as narrow as possible with safe wrappers

## Config File

`~/.powershellmanager/config.toml` or `powershellmanager.toml` in CWD:

```toml
[defaults]
target = "powershell"    # or "terminal", "all"
monitor = "primary"      # or monitor index

[[layout]]
name = "dev"
grid = "2x3"

[[layout]]
name = "monitor"
style = "columns"
count = 4
```

## Tray Behavior

- App starts minimized to tray (no main window on launch)
- **Right-click tray icon** — context menu with layout presets + Quit
- **Left-click tray icon** — toggle popup GUI window (layout picker with visual grid preview, detected windows list, settings)
- Popup window closes on focus loss (or ESC), app stays in tray
- Tray icon tooltip shows "PowerShell Manager — N windows detected"

## Cross-References

- **Win32 patterns:** `C:\trontstack\pettoy\src\platform\win32.rs` — window enumeration, HWND extraction, Win32 interop
- **Tray icon reference:** `C:\trontstack\pettoy\src\tray.rs` — system tray integration in Rust
- **egui desktop app:** pettoy uses `egui 0.33` for debug overlay — same crate for this GUI
- **Root monorepo:** `C:\trontstack\CLAUDE.md` — full project catalog
