# PowerShellManager 0.4.1 release audit

Requested by Trent after a Pin action matched all Windows Terminal windows.

## Findings fixed

| Finding | Result |
| --- | --- |
| Pin matched process OR title | Conditions now combine; a rule reserves one window. UI pins use exact title plus a live HWND binding. |
| Multiple pins overwrote a slot and lost windows | First reservation wins; conflicts return to the queue with a warning. |
| Disabled cells renumbered pin destinations | Slots keep their physical grid numbers. |
| Pins required activity sorting | Reservations apply independently of ranking, with one shared planner. |
| Manual order was unavailable | Actual egui drag handles set and persist order; refresh reconciles existing HWNDs and new windows. |
| Tray/hotkey read startup settings | They share current config/order/activity. Saved-grid menu entries rebuild and retain masks. |
| Fixed-height list rows overlapped | Child content no longer advances the parent cursor. Both wide and narrow rows are tested. |
| Grid rounding left edge seams | Cumulative boundaries consume the complete monitor area. |
| Malformed dimensions/weights could panic or allocate excessively | 1..8 bounds; normalized finite weights and positive dimensions. |
| Hidden eframe viewport could leave Poll active | Small vendored Windows repaint patch; hidden native loop tested. |
| Interrupted saves could truncate settings | Write/sync a temporary file, then replace the complete TOML. |
| Branding became faint in extreme themes | Theme colors with separate dark and bright edges, tested at 16..64 pixels. |

## Evidence

`cargo test --locked --offline`: 31 passed; 4 opt-in tests excluded by default.
`cargo clippy --locked --offline --all-targets -- -D warnings`: passed.
Native tests explicitly run: exact pin, manual queue, weighted slots, disabled
physical slot and exact resulting GetWindowRect values. Only three owned hidden
HWNDs are moved. The eframe test creates its own hidden viewport: 26 frames over
5.19 seconds, 0.922 seconds total process CPU including initialization, then clean
exit. This is a bounded regression check, not a long-term idle benchmark.

Actual egui pointer tests cover Pin, drag reordering, grid toggles/dividers and
movable Theme Studio. Planner adversarial cases cover 64 disabled masks and nine
pin destinations with collisions. Theme tests cover generated palettes, imports,
contrast and typography. Offscreen images reviewed at wide, compact and light
settings. Public screenshots use actual enumerated windows with anonymized titles;
capture has no native actions or settings writes.

## Limits

This is a scoped source and behavior audit, not independent security certification.
No real user windows are moved by automated tests. Native menu clicks, mixed DPI,
elevated applications, clean machines and a long tray-idle soak still need broader
acceptance. HWND bindings are session-local; restart matching uses process/title,
so identical titles are resolved by list order. Applications can enforce minimum
sizes. Minimized windows remain minimized. The Windows binary is unsigned.
