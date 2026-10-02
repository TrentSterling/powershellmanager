# PowerShellManager 0.5.0 release audit

Requested by Trent: new terminals should land in the grid on their own, and Apply
or Restore should stop reshuffling windows that already have a place.

## Changes audited

| Area | 0.5.0 result |
| --- | --- |
| Sticky slots | Apply keeps each window in the slot it occupies, remembered this session or recognized by position after a restart. Activity ranking only orders windows without a slot. Grabber drags are the only reorder. |
| Manual moves | A window dragged away by its title bar keeps its slot reservation and returns on Apply. |
| Auto-arrange | Off by default. A new matching window moves into the first free slot once its size stops changing (two scans about half a second apart, capped at six). Turning it on moves nothing. |
| Closed windows | The hole stays for the next arrival unless Slide to fill gaps is on. |
| Full grid | Arrivals stay where they opened unless Overflow to other display is on, which uses the same grid on another display and keeps its disabled cells. |
| Pins and Undo | Pins still win over remembered slots; automatic placements enter session Undo. |
| Window chrome | The brand header replaces the OS caption: minimize, maximize/restore, hide to tray, drag to move, double-click to maximize, eight resize edges. |
| Buttons | One padding and height for every button, including caption buttons. Narrow tall windows give Theme and About their own row. |

## Evidence

`cargo test --locked --offline`: 187 passed; 12 opt-in tests excluded by default.
`cargo clippy --locked --offline --all-targets -- -D warnings`: passed. `cargo fmt --check`: passed.

`scripts/coverage.ps1 -EnforceFullCoverage`: 198 instrumented tests passed, including
10 native audits and the GPU offscreen review (the public gallery capture is skipped).
Coverage across all production source files: lines 5647/5647, functions 573/573,
regions 6422/6422, branches 1106/1106. Test-only modules are the only exclusions.

Auto mode is tested against a controlled desktop: first enable, settling, holes,
slide to fill, overflow with occupied and disabled cells, minimized and off-grid
windows, pins, failed moves and missing displays. Caption buttons, the drag gap,
double-click maximize and the resize edges are exercised through real egui pointer
input, including the maximized and fullscreen cases.

Offscreen captures reviewed: workspace, light theme, 480 px compact, 340 x 900
narrow and the 280 x 300 minimum. Release binary smoke checks (`--version`,
`--help`, `--list --json`) passed, and the final PE imports were checked for the
static MSVC runtime; `build.json` in the ZIP records the result. Trent ran the
0.5.0 development build on his desktop before release.

## Limits

This is a scoped source and behavior audit, not independent security certification.
Auto mode moves real windows; its automated tests use a controlled desktop, so
unusual applications (windows that resize for several seconds, or reopen at
remembered positions) rely on the settle cap. Slot memory is session-local;
minimized windows recover their slot after restore and Apply. Without the OS
caption, the Windows 11 snap flyout on maximize hover is unavailable; drag-to-edge
snapping still works. Mixed DPI, elevated applications, clean machines and a long
tray-idle soak still need broader acceptance. The Windows binary is unsigned.
