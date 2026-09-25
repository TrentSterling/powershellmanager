# PowerShell Manager

Your windows. Your grid. A native Windows window arranger with weighted layouts,
manual terminal ordering and ColorMagic Theme Studio.

[Product page](https://tront.xyz/powershellmanager/) | [Download](https://github.com/TrentSterling/powershellmanager/releases/latest) | [Development story](https://tront.xyz/blog/posts/powershellmanager/)

![PowerShell Manager workspace](https://tront.xyz/powershellmanager/media/electric-workspace.png)

## Use it

Extract the Windows x64 ZIP and run `powershellmanager.exe`. Normal mode enables
Apply, settings, the tray and Ctrl+Alt+G. No installer or administrator launch is
required. Quit an older PSM from its tray menu before running another version;
Windows only lets one application register the same global hotkey.

1. Choose Terminals or All windows and your target display.
2. Pick a preset, or use Custom and drag the dividers. Click a cell to leave it empty.
3. Drag the six-dot handles in the Windows list to choose the order. The numbers
   are the destination slots, and the grid shows the assigned window titles.
4. Click Apply layout. Editing the grid or queue never moves windows by itself.

Drag ordering switches off activity ranking. Pins reserve individual windows in
physical grid slots, independently of ranking. Dragging a pinned window moves its
reservation with it. Conflicting or unavailable pins fall back to the remaining
queue and report a warning, rather than discarding a window. Existing broad
process-only rules reserve one matching window each; remove them and pin an
individual window if that is what you want. Across restarts, individual pins and
manual order use executable and exact title; identical titles are resolved in
window-list order. Within a session, pins follow the bound HWND as titles change.

Closing the window hides it to the tray. Left-click its icon to reopen; right-click
for Apply current layout, presets, saved grids and Quit. Ctrl+Alt+G uses your
current layout, disabled cells, order and pins. Selecting a different preset from
the tray intentionally starts with all of that preset's cells enabled. Saved-grid
entries keep their own disabled cells and proportions.

## Make it yours

Theme Studio takes its cues from Discord's themes and the Trontop implementation:
ColorMagic palettes with undo, four draggable gradient pips, intensity, direction,
dark/light frost, surface tint, text contrast, outlines, fonts and scaling. Save
named themes or import/export Trontop-compatible theme JSON. The four-square mark
follows your colors in the app, window and tray, with dark and bright outlines.
Trent's portrait is in About. The window can shrink to 280 by 300 logical pixels.

## Build and verify

```powershell
cargo build --release --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
cargo test native_audit -- --ignored --nocapture --test-threads=1
cargo test render_ui_review -- --ignored --nocapture
```

Native audit tests create and manipulate only their own hidden windows. The inert
UI harness exercises actual egui controls without desktop input or settings writes.
See [the audit receipt](docs/RELEASE_AUDIT_0.4.1.md) for coverage and limits.

`--preview` is deliberately read-only and disables Apply. Use normal mode for work.
`--headless 3x2` or `--headless columns:3` applies that layout immediately and exits;
it is an action, not a test mode. It uses configured targets, display, gap, pins
and saved manual order, with every cell in the requested preset enabled.

## Local data and limitations

Settings and activity are stored in `~/.powershellmanager/`. Window titles remain
local. There is no account or telemetry upload. A background request checks this
repository's public GitHub releases for updates; downloads are opened explicitly.
Settings use an atomic file replacement. The executable is unsigned.

This is a development release tested on Windows 11 x64. Some applications impose
minimum sizes or require higher integrity permissions and can reject positioning.
Minimized windows keep their minimized state; use Restore all when needed. A
Windows Terminal tab is not a separate window and cannot occupy its own slot.
Broader hardware, elevated-window and mixed-DPI acceptance remain open.

## License

Copyright 2026 Trent Sterling. Source available under Apache 2.0 with Commons Clause
1.0: free for personal and workplace use, retain credits and terms when
redistributing, with restrictions on selling products or services whose value
comes entirely or substantially from this software. This is not an OSI open-source
license. Read [LICENSE](LICENSE) and [NOTICE](NOTICE) for the complete terms.
Third-party components retain their own licenses in `THIRD_PARTY_NOTICES.txt`.

The theme implementation is adapted from Trent's Trontop. The vendored eframe
0.31.1 change is documented in `vendor/eframe/PSM-PATCH.md` and retains upstream
MIT/Apache terms. Rajdhani's OFL is included in `assets/fonts/OFL.txt`.
