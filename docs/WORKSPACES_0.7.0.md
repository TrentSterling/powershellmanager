# Workspace matching and compact UI, local 0.7.0

Saved workspaces show numbered display destinations and live matching titles.
Expand a rule to edit its process and select Exact title, Title contains or
Process only. Fragments are case-insensitive and cannot be blank. The process
must always match. Old saves retain exact matching; rules persist without moving
windows. Preview and Restore use the same resolver. Ambiguous matches and two
unique rules resolving to the same HWND are skipped.

Existing monitor fallback, geometry validation, window identity checks, Auto
arrange pause and Undo remain active. Save excludes minimized windows.

Buttons, header, panels and rail use tighter spacing. The proportional grid
preview is shorter and branding scales with the header. Small-window scrolling
and controls remain available.

Verification: 191 tests, 10 explicitly invoked native audits using owned windows
and private desktops, strict Clippy, formatting, GPU UI review and release build
passed. Reviewed dark/light workspace previews and compact layouts. No global
pointer/keyboard injection or user-window movements were used.

Evidence: target/compact-tests.log, compact-native.log, compact-clippy.log,
compact-render.log, compact-build.log and target/ui-review/.

This is a local update. Public downloads and the product site are unchanged.
