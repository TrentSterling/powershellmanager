# Named workspaces, local 0.6.0

Save visible managed window positions under a name, then restore matching open
windows without rebuilding a grid. The preview lists the number of available
matches and the saved window titles. Reusing a name explicitly replaces its saved
positions. Removing a workspace removes only the saved arrangement.

Positions are stored as fractions of monitor work areas. A disconnected display
falls back to the primary display; rectangles are kept inside the available work
area. Matching requires the process name and exact title, so missing windows or
duplicate titles are skipped. Workspaces do not launch applications.

Restore uses the existing native position and identity checks, records only
successful moves for Undo, and pauses Auto arrange. Save excludes minimized
windows. Preview mode cannot save or restore positions.

Verification: 189 ordinary tests passed, 10 explicitly invoked native checks
passed on test-owned windows, strict Clippy and formatting passed, release build
passed, and both workspace themes were reviewed from offscreen captures. The first
native run was sandbox-blocked; the approved run outside that sandbox passed.
