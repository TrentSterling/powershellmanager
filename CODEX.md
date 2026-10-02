# PowerShellManager release work

User approved the new UI, normal operation, public product page/downloads and an
in-place update of the existing February blog post. Later requests add manual drag
ordering, individual pins, zebra/alignment polish, a smaller window, theme-aware
four-square branding and a red-team pass before release. User explicitly chose the
same Apache 2.0 plus Commons Clause 1.0 terms as Trontop.

The receipt is `docs/RELEASE_AUDIT_0.5.0.md`. Run cargo test, clippy -D warnings,
fmt, native_audit (explicit ignored tests), and GPU render_ui_review. The native
audit owns every window it manipulates. Never run --headless as a test and never
synthesize global desktop input. --preview is read-only; normal launch enables
Apply. Keep real terminal titles out of public captures.

The four-square icon is code-native. The original portrait remains in About.
Theme code and third-party sources retain their separate attribution and licenses.
This repo is reserved for a hand-driven session, not the overnight loop.

User requested no disruptive desktop interaction testing. Do not move the mouse,
open menus on the user desktop, or force foreground focus for tests. Use private
or offscreen test resources and read-only inspection. A requested app launch does
not authorize additional focus or interaction checks.
