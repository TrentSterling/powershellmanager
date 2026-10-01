# PowerShellManager 0.4.2 release audit

Goal for this release: make every production line reachable by a test, keep the tray
and hotkey honest about current state, and show the author credit without opening About.

## Changes audited

| Area | 0.4.2 result |
| --- | --- |
| Header credit | A "by tront.xyz" link sits under the title at every window size, down to 280 x 300. |
| Tray state | The tray reports current layout, window/slot counts, last result and visibility; unchanged frames do not rewrite the tooltip. |
| Undo | Shared session Undo covers Apply, tray layouts and the hotkey: 20 steps, original window states and partial-failure recovery. |
| Close vs Quit | Closing hides to the tray and keeps Undo; only Quit exits. Show, Hide and Refresh work while hidden. |
| Interrupted divider drags | Returning from a hidden viewport completes the drag and saves its widths. |
| Activity ranking | Session data is counted once and live focus time appears immediately; foreground loss, clock rollback and invalid samples are handled. |
| Shutdown | Workers are joined, queued events drain before the final save, and hotkey ownership is released on exit and unwinding. |
| Settings saves | Exclusive temporary files, flush and replace; failures leave the original document intact and appear in the status line. |
| Update checks | Semantic version precedence; malformed tags, drafts and prereleases are ignored; replies have size and time limits; failures stay nonfatal. |
| Out-of-range pins | The Pins tab keeps the reservation and reports the unavailable slot. |
| Extreme grid and gradient values | Weights stay positive and representable; nonfinite geometry and peg edits keep safe values. |
| Read-only inventory | `--list`, `--json` and `--target` print the inventory without changing windows or settings. |

The CHANGELOG lists every 0.4.2 change.

## Evidence

`cargo test --locked --offline`: 170 passed; 12 opt-in tests excluded by default.
`cargo clippy --locked --offline --all-targets -- -D warnings`: passed. `cargo fmt --check`: passed.

`scripts/coverage.ps1 -EnforceFullCoverage`: 181 instrumented tests passed, including
10 native audits and the GPU offscreen review (the public gallery capture is skipped).
Coverage across all 27 production source files: lines 5120/5120, functions 513/513,
regions 5809/5809, branches 1008/1008. Test-only modules are the only exclusions.

Native audits create and move only their own windows, on a private Win32 desktop that
is never switched into view. No global desktop input is synthesized. Real user windows
are not moved by any automated test.

Release binary smoke checks (`--version`, `--help`, `--list --json`) passed for both the
built and the packaged executable. Offscreen captures reviewed: workspace, light theme,
compact and the 280 x 300 minimum window. The final PE imports were checked for the
static MSVC runtime before packaging; `build.json` in the ZIP records the result.

## Limits

This is a scoped source and behavior audit, not independent security certification.
Native menu clicks, mixed DPI, elevated applications, clean machines and a long
tray-idle soak still need broader acceptance. HWND bindings are session-local; restart
matching uses process/title, so identical titles are resolved by list order.
Applications can enforce minimum sizes. Minimized windows remain minimized. Session
Undo does not survive a restart. The Windows binary is unsigned.
