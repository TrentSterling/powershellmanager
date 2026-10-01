# Changelog

## 0.5.0 (2026-10-01)

- Auto-arrange new windows (off by default): a new matching window moves into the
  first free slot about a second after it opens, once it stops resizing. Windows
  that already have a slot never move, and turning auto mode on moves nothing.
- Sticky slots: Apply keeps each window in the slot it already occupies, remembered
  this session or recognized by position after a restart. Focus changes and
  activity ranking no longer reshuffle the grid; ranking only orders windows that
  do not have a slot yet. Only the grabbers reorder placed windows. A window moved
  by hand keeps its slot and returns to it on Apply.
- Slide to fill gaps (off by default): when a window closes, later windows move up.
  Otherwise the hole stays for the next new window.
- Overflow to other display (off by default): with every slot full, new windows use
  the same grid on another display; otherwise they stay where they opened.
- Auto placements enter session Undo, and pins still win over remembered slots.

## 0.4.2 (2026-09-30)

- Remove the header tagline. A small "by tront.xyz" link sits under the title at
  every window size, so the credit shows without opening About. About is always
  available in the header and opens a separate page with a return button, version,
  author, source link and credits.
- Focus and Pin/Unpin share measured dimensions and a baseline. Rows reserve
  space for status text at every UI scale, including the last row when scrolled
  to the bottom. Apply, Undo and Refresh also share a button height.
- The strict local suite reaches 100% production line, function, region and branch
  coverage across all 27 Rust source files: 181 tests pass, including native audits
  and GPU review. Ordinary tests, Clippy with denied warnings and formatting pass.
- The tray reports current layout, window/slot counts, last result and visibility.
  Apply and Undo follow available windows and history. Show, Hide to tray and
  Refresh work while the window is hidden; single-click and double-click reopen it.
  Closing keeps session Undo alive; only Quit exits. Unchanged frames do not rewrite
  the icon tooltip, and Unicode tips stay within the Windows buffer limit.
- Returning from a hidden viewport completes an interrupted divider drag and saves
  its widths. Hover and press share the same handle boundaries; clicks and keyboard
  activation during resizing keep slot selections intact.
- Theme Studio fits the minimum viewport, including expanded saved-theme controls.
  Saved-theme rows wrap so Save and Delete remain reachable in narrow windows.
- Empty grid geometry renders a clear message; disabled slots and an empty window
  inventory keep Apply disabled while Refresh remains available.
- Preview UI tests use the same action availability as production; action guards
  and shared storage/focus worker failure paths have dedicated regression checks.
- Viewing the Pins tab preserves out-of-range reservations and reports the
  unavailable slot instead of silently moving the pin into the current layout.
- UI checks verify queue scrolling at both edges, outside-release order and the
  insertion marker when dropping after a row.
- Gradient bands clip in local coordinates, preserving full coverage and shared
  boundaries when large translated coordinates lose floating-point precision.
- Extreme imported custom-grid ratios keep positive, representable weights.
  Divider release uses the same normalization as loaded settings.
- Actual UI checks verify saved gap edits, UI scale changes, peg clicks and pointer
  loss during dragging; monitor-read failure keeps prior enumeration results.
- Layout parsing rejects repeated prefixes and delimiters while retaining aliases,
  default side counts and the documented one-to-eight limits.
- Gradient rendering rejects nonfinite or overflowing rectangle geometry; invalid
  peg edits and nonfinite phases keep safe values.
- Invalid saved themes show a visible error and keep the current appearance.
  JSON and legacy import tests validate required fields, color channels and numbers.
- Actual UI tests cover grid dimension edits, title/process search, gradient peg
  typing and dragging, duplicate pin warnings and invalid saved themes.
- Update notices use semantic version precedence and ignore malformed tags, drafts
  and prereleases. HTTP replies have size and time limits; failures stay nonfatal.
- Update publication checks shutdown and repaints the app when a newer release
  arrives. Network latency cannot delay closing the app.
- Native startup accepts explicit storage and service dependencies for isolated
  verification. Invalid saved preset indices recover before the first frame.
- Real hidden creation-context and GUI-runner audits cover startup, owned worker
  shutdown and launch errors without global desktop input.
- Minimize, restore and focus use the shared desktop service and refresh window
  state immediately. Preview keeps its rows and rejects those actions safely.
- Actual UI tests cover display/preset menus, saved weights, pin edits/removal,
  Activity, theme fonts/presets, gradient controls and bundled notices.
- Manual native CI verification pins its coverage tools and enforces all four
  100% targets on an interactive Windows runner.
- Inventory, layout actions and GUI launch share a tested command dispatcher.
  Missing displays no longer produce a misleading capacity-only skip message.
- The About download link is verified through actual egui controls without opening
  a browser. UI stroke widths use explicit types for the nightly coverage compiler.
- GUI and tray actions share the same desktop and ordering pipeline, with complete
  worker-loop tests for command priority, live settings, partial failures and Undo.
- Stale saved-layout commands and unavailable live state report actionable status;
  poisoned history prevents moves and appears in the GUI.
- Deleting a selected grid saves the recovered preset selection consistently.
- Real focus, minimize, restore and window discovery are audited on an owned private
  Win32 desktop that is never switched into view. No desktop input is synthesized.
- Settings saves use an explicit destination; failure tests write only local folders.
- Missing native window handles disable actions before workers, tray or storage start.
- Tray clicks are bound to the current icon; retired events and hover traffic
  cannot open the app or delay menu commands indefinitely.
- Simultaneous hotkey events preserve menu actions, including Quit and Undo.
- Tray action shutdown joins its worker; hotkey ownership is released on normal
  exit and unwinding, with bounded event draining and reported start failures.
- Saved-grid tray commands preserve duplicate-name identities by index and name.
- Tray setup and theme-update errors are reported instead of silently discarded.
- Native discovery accepts padded target aliases, handles exclusions consistently,
  reads full bounded Unicode titles, and queries process identity with limited access.
- Tray Win32 calls initialize the documented structure size and failed registration
  releases its owned hidden window; upstream licenses remain intact.
- Read-only text and JSON window inventory through `--list`, `--json` and `--target`.
- Builtin presets keep their identity when a saved grid uses the same display name.
- Compact workspace keeps the grid and primary actions visible at the 280x300
  minimum size, with condensed headings and a one-line status with full-text hover.
- Activity ranking no longer counts session data twice. Live focus time appears
  immediately; foreground loss, clock rollback and invalid samples are handled.
- Activity shutdown joins the poller and drains queued events before the final save.
  Decay retains the currently focused app and ranking ties use stable ordering.
- Shared storage keeps original documents intact through serialization, write,
  flush and replacement failures. Concurrent saves use exclusive temporary files;
  failed writes clean up, and settings errors appear in the status line.
- Shared session undo for Apply, tray layouts and the hotkey, with 20 steps,
  original window states, partial-failure recovery and closed-owner checks.
- All presets consume remainder pixels through the shared weighted grid math.
- Reproducible production and branch coverage reports, stale-executable cleanup,
  read-only CLI smoke checks and a strict coverage gate.
- Preview uses the selected display's work area and preserves its aspect ratio,
  including portrait and ultrawide displays. Display discovery updates on refresh.
- Custom dividers track the pointer with the actual grid gaps and pixel boundaries.
  Resizing preserves adjacent cells' combined size and enforces their minimum weights.
- Pointer regression tests cover display geometry, large gaps and divider limits.
- Activity save and decay timing tests advance an injected clock, so they pass on
  freshly booted machines such as CI runners.

## 0.4.1 (2026-09-24)

- ColorMagic Theme Studio, gradient pips, frost, contrast, fonts and shared theme JSON.
- Drag handles for manual window ordering; Apply and slot labels share one plan.
- Individual-window pins, collision recovery and stable physical slot numbers.
- Current settings and ordering for tray/hotkey actions; saved grids update the menu.
- Aligned zebra rows, grid boundaries without remainder seams and a smaller window.
- Theme-responsive four-square app/tray icon; author portrait in About.
- Static MSVC runtime for the portable Windows executable.
- Bounded layout parsing, resilient weights, atomic settings writes and a hidden
  eframe repaint fix to prevent a Windows idle-loop CPU spin.
- 31 ordinary tests plus explicit native placement, hidden-window and image reviews.

## 0.4.0 (local UI preview)

Initial grid workspace and Theme Studio redesign, reviewed locally before release.
