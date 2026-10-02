# PowerShell Manager

A native Windows window arranger with weighted layouts,
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

The preview follows the selected display's work area, including portrait and
ultrawide proportions. Custom dividers preserve neighboring cells' combined size
and stop at their minimum proportions.

Drag ordering switches off activity ranking. Pins reserve individual windows in
physical grid slots, independently of ranking. Dragging a pinned window moves its
reservation with it. Conflicting or unavailable pins fall back to the remaining
queue and report a warning, rather than discarding a window. Existing broad
process-only rules reserve one matching window each; remove them and pin an
individual window if that is what you want. Across restarts, individual pins and
manual order use executable and exact title; identical titles are resolved in
window-list order. Within a session, pins follow the bound HWND as titles change.

Activity ranking counts recorded focus once, includes the current open interval,
and uses alphabetical ordering for ties. Losing foreground access ends the focus
interval. Clock rollback cannot subtract or charge the same flushed time twice.

Closing the window hides it to the tray and keeps your session and Undo history.
Single-click or double-click its icon to reopen. Right-click for the current
layout, window/slot counts, last result, Show, Hide to tray, Refresh windows, Apply,
Undo, presets, saved grids and Quit. Apply and Undo follow the current available
windows, enabled slots and history. The tooltip shows whether the window is open
or running in the tray. Quit is the explicit exit. Ctrl+Alt+G uses your
current layout, disabled cells, order and pins. Selecting a different preset from
the tray intentionally starts with all of that preset's cells enabled. Saved-grid
entries keep their own disabled cells and proportions.

Undo layout restores the positions and window states from the previous Apply.
The UI, tray and hotkey share up to 20 undo steps for the current session. Failed
moves never enter history; closed windows or handles with a changed process owner
are skipped. Temporary restore failures remain available to retry.

## Make it yours

Theme Studio takes its cues from Discord's themes and the Trontop implementation:
ColorMagic palettes with undo, four draggable gradient pips, intensity, direction,
dark/light frost, surface tint, text contrast, outlines, fonts and scaling. Save
named themes or import/export Trontop-compatible theme JSON. The four-square mark
follows your colors in the app, window and tray, with dark and bright outlines.
About is in the header and opens a separate page with the version, author,
source link and credits. The window can shrink to 280 by 300 logical pixels.

## Build and verify

```powershell
cargo build --release --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
cargo test native_audit -- --ignored --nocapture --test-threads=1
cargo test render_ui_review -- --ignored --nocapture
./scripts/coverage.ps1
./scripts/coverage.ps1 -BranchCoverage
```

Native audit tests create and manipulate only their own windows, icons and a
private F24 hotkey chord. Focus, minimize and restore checks run on an owned
private Win32 desktop that is never switched into view. Run tray/hotkey audits on a normal Windows desktop;
the sandbox can reject notification registration. The inert
UI harness exercises actual egui controls without desktop input or settings writes.
Update checks use local HTTP fixtures; native startup tests use explicit local
storage paths, and command tests use a controlled desktop.
Display/preset menus, saved-grid weights, pin slot edits, font choices, theme
presets and bundled notices are exercised through actual egui pointer events.
Minimize, restore and focus refresh the inventory immediately after their action.
See [the audit receipt](docs/RELEASE_AUDIT_0.5.0.md) for coverage and limits.

Coverage uses cargo-llvm-cov 0.9.1 and Rust's llvm-tools-preview component. The
script measures every production source file and excludes test-only modules.
`-EnforceFullCoverage` uses a separate nightly toolchain and requires 100% coverage
of lines, functions, regions and branches. Use `-BranchCoverage` to measure branches
without enforcing the full target. The local strict suite passes all four measures
at 100%; counts, validation and portable build receipts are recorded in
`docs/QUALITY_STATUS.json`. The script clears old app
executables before measurement and runs real read-only CLI inventory checks.

The Windows CI workflow runs tests, formatting, Clippy and a release build on
every push and pull request. Its manual `native_coverage` option runs the complete
strict coverage gate on a self-hosted Windows runner labeled `psm-desktop`.
That runner needs an interactive Explorer desktop and a graphics adapter for
the native/GPU audits. This job pins cargo-llvm-cov 0.9.1 and
`nightly-2026-09-29`; `-CoverageToolchain` selects the same toolchain locally.
Its artifacts contain coverage and synthetic UI captures; CLI inventories stay local.
The local equivalent passes. The remote native job has not run; the repository
has no registered self-hosted runner at the latest check.

Read-only inventory is available without opening the app:

```powershell
./powershellmanager.exe --list --target terminals
./powershellmanager.exe --list --json --target all
```

Omit `--target` to use your configured filter and exclusions. JSON includes window
handles, process names, titles, categories, minimized state and physical bounds.
Listing does not move windows, save settings or register hotkeys.

`--preview` is deliberately read-only and disables Apply. Use normal mode for work.
`--headless 3x2` or `--headless columns:3` applies that layout immediately and exits;
it is an action, not a test mode. It uses configured targets, display, gap, pins
and saved manual order, with every cell in the requested preset enabled.
Unknown layouts return exit code 1; display or positioning errors return 2.
Partial success retains its diagnostics and pin warnings.

## Local data and limitations

Settings and activity are stored in `~/.powershellmanager/`. Window titles remain
local. There is no account or telemetry upload. A background request checks this
repository's public GitHub releases for newer stable versions. Malformed tags,
drafts and prereleases are ignored; request failures stay nonfatal and network
latency cannot delay closing the app. Downloads are opened explicitly.
Settings use an atomic file replacement with exclusive temporary files and cleanup
after failed writes. A failed settings save appears in the UI status line.
The executable is unsigned.

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
MIT/Apache terms. The tray-icon 0.19.3 Win32 structure initialization and failed
registration cleanup are documented in `vendor/tray-icon/PSM-PATCH.md`, with
upstream MIT/Apache license files preserved. Rajdhani's OFL is included in
`assets/fonts/OFL.txt`.
