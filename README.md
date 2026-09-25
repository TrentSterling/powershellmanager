# PowerShell Manager (PSM)

Tray-based window tiler for Windows. Snap PowerShell, Windows Terminal, browsers, editors, and any other open window into grid layouts with one click or a global hotkey.

## What it is

PSM started as a PowerShell/Terminal window arranger and grew into a universal window manager. It lives in the system tray, watches every visible top-level window, and can snap them into a grid, columns, rows, or IDE-style main+side layouts on the current monitor's work area.

Key features:

- **System tray**: closing the main window hides it; left-click the tray icon to reopen it, or right-click for layouts and Quit.
- **Global hotkey**: `Ctrl+Alt+G` applies the active layout captured at startup.
- **Built-in layout presets** — grids from 1x2 up to 4x4, left/right, top/bottom, main+N-side, focus+N-side, N columns, N rows.
- **Draggable custom grids** — adjustable column/row weights instead of fixed equal splits, saved as named presets in the tray menu.
- **Smart activity-based sorting** — a background focus poller tracks per-app focus time and switch count (`activity.rs`), persisted and decayed over time, so frequently-used windows can be placed in priority slots.
- **App categorization** — windows are auto-classified (Terminal, Editor, Browser, Chat, Media, Game, DevTool, System, Other) with user overrides via config.
- **Pin rules** — force specific processes/titles into specific grid slots.
- **Multi-monitor aware** — targets primary or a specific monitor's work area (excludes taskbar).
- **Headless mode** — `--headless <layout>` arranges windows and exits immediately, no GUI, for scripting.

## Build & Run

The 0.4 workspace puts the interactive grid first, with preset thumbnails,
searchable windows, editable pins, a display picker and an Apply bar that stays
visible. Custom weights, disabled cells and named grids are retained.

Theme Studio includes ColorMagic's seven palette families and undo, four draggable
gradient pips, intensity, direction, independent dark/light frost, surface tint,
text contrast, bright outlines, four fonts and UI scaling. Save named themes or
copy/import the same theme JSON used by Trontop. Font attribution and the reused
Trontop theme engine's terms are in `assets/fonts/OFL.txt` and
`assets/theme-engine-{LICENSE,NOTICE}`.

```bash
cargo build --release           # release build
cargo run                       # launch tray app (default)
cargo run -- --headless 2x3     # headless: arrange into a 2x3 grid and exit
cargo test                      # run tests
cargo run -- --preview           # inspect UI; no window actions or settings writes
cargo test render_ui_review -- --ignored --nocapture # offscreen PNG review
```

Config lives at `~/.powershellmanager/config.toml` (or `powershellmanager.toml` in the CWD), covering default target/monitor/gap, custom layout defs, category overrides, pin rules, and saved custom grids. Activity data persists to `~/.powershellmanager/activity.toml`.

`--preview` reads settings and a window-list snapshot, opens Theme Studio, and
registers no tray icon, hotkey or activity worker. It does not move, focus,
minimize or restore windows, and does not persist edits. Normal mode retains
the existing native actions.

UI tests use an inert app with clearly labeled test window titles. They exercise
production egui widgets and render to GPU textures without desktop input or a
native window. Review images are written to `target/ui-review/`.

Existing tray behavior to be aware of: tray menus and Ctrl+Alt+G use startup
configuration; restart after changing layouts if you use those entry points.
Ctrl+Alt+G currently applies the startup layout. Pin rules apply when activity
ranking is enabled, and their slot order counts enabled cells.

## Tech Stack

- **Rust** (edition 2021)
- **egui / eframe** — immediate-mode GUI for the popup window
- **tray-icon** — system tray icon and right-click context menu
- **windows crate** — Win32 window enumeration, positioning, resizing, monitor info, global hotkey registration
- **clap** — CLI arg parsing (headless mode)
- **serde + toml** — config and saved-layout persistence
- **image** — tray/window icon loading (embedded PNG)

## Project Layout

```
src/
  main.rs      entry point, arg parsing, tray vs headless mode
  app.rs       eframe App impl, tray event loop, global hotkey, GUI state
  tray.rs      system tray icon, right-click layout menu
  gui.rs       egui popup window (layout picker, preview, settings)
  windows.rs   Win32 window enumeration, filtering, app categorization
  layout.rs    layout presets and grid math (incl. weighted custom grids)
  arrange.rs   applies layouts to discovered windows
  config.rs    TOML config load/save (defaults, pins, saved grids)
  monitor.rs   multi-monitor work area detection
  activity.rs  background focus poller + persistent per-app activity DB
  theme.rs     UI theme
```

Owner: Trent Sterling (tront.xyz)
