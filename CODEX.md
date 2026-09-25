# PowerShellManager UI pass

User asked for the Trontop treatment, then clarified that the app UI needs work,
ColorMagic is essential, and the existing layout tools are its strongest part.
This is a hand-driven session, not an overnight-loop task.

The 0.4 implementation preserves layout computation and Win32 action code. The
weighted preview has moved to `src/gui/preview.rs`; the first drag is now hit-tested
against the press origin so fast movements acquire dividers reliably. Disabled
cells persist, and malformed custom dimensions/weights recover safely.

Theme settings, ColorMagic, contrast, gradient math, theme JSON and fonts are
adapted from owner-authored Trontop. The theme format remains `trontop-theme` v4
for interop, with v3/legacy migration. This is shared behavior, not a new shared
crate or a claim of parity across every Rust app.

## Validation

- `cargo test --locked --offline -- --include-ignored`: includes GPU offscreen
  review, actual grid clicks/drags, movable/closable Studio, settings migration,
  saved grids/pins, palette undo, fonts, and contrast across generated palettes.
- `cargo build --release --locked --offline`: Windows executable.
- `cargo fmt --check` and `git diff --check`.
- `cargo clippy --offline --all-targets`: existing arrange/tray/config/window
  style warnings remain; no native window actions are needed for these checks.

Do not use `--headless` as a test mode: it actually arranges the owner's windows.
Use the inert UI harness or `--preview`, which has no tray, hotkey, focus worker,
settings writes or native window actions. Do not synthesize global desktop input.
Native arrangement/tray acceptance is separate from UI validation.

## Remaining product work

- Tray and hotkey settings are startup snapshots; dynamic updates need their own
  native acceptance pass. README previously described Ctrl+Alt+G as opening the
  popup, but the current implementation applies a layout.
- Pin rules are conditional on activity ranking and index the enabled-slot list.
  The new UI states those semantics rather than changing the placement engine.
- Review this app pass before new public screenshots, product-page changes or
  a new binary release. UI review captures contain labeled test window titles.

## Local acceptance receipt (2026-09-25 UTC)

22 tests passed, including the ignored GPU review explicitly enabled. Release
build, formatting and diff checks passed. Inspected dark/light, compact, preset
and Studio PNGs. Preview PID 255320 launched responding with HWND 955058116;
config and activity file hashes were unchanged after launch. Receipt is in
`target/ui-review/launch.json`. Binary SHA-256:
`A273B6AB4391AEC18546151F6B99117249F1F9BEE66471576A598BD3FFEA1C69`.

This confirms isolated UI rendering and preview startup, not native placement,
hotkey/tray acceptance or a runtime soak. No public page or release was changed.
