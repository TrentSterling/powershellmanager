# PowerShell Manager — Feature Brainstorm

## High Priority (next session)

| Feature | Description | Complexity |
|---------|-------------|-----------|
| **Auto-arrange on detect** | Watch for new terminal windows, auto-snap them into the next empty slot | Medium |
| **Gap slider in GUI** | Adjust pixel gap between windows from the GUI instead of config only | Low |
| **Minimize to slot** | Right-click a cell to minimize/restore just that window | Low |
| **Cascade mode** | New layout type: cascade/staircase windows offset by N pixels | Low |
| **Drag-to-reorder** | Drag windows in the preview to swap which terminal goes in which cell | High |
| **Save/load custom layouts** | Name and save weighted grid configs, recall from tray menu | Low |
| **Window pinning** | Pin specific windows to specific cells (by title/process match) | Medium |

## Backlog

| Feature | Description | Complexity |
|---------|-------------|-----------|
| **Hotkey support** | Global hotkeys to apply layouts without opening GUI (e.g. Ctrl+Alt+1 for 2x2) | Medium |
| **Multi-monitor** | Dropdown to pick which monitor to arrange on, or spread across monitors | Medium |
| **Per-monitor layouts** | Different default layout per monitor | Low |
| **Profile system** | Multiple named profiles (e.g. "dev", "monitoring", "streaming") switchable from tray | Medium |
| **Window title filters** | Filter by title substring too (e.g. only terminals with "ssh" in title) | Low |
| **Startup with Windows** | Toggle to add/remove from Windows startup (registry or startup folder) | Low |
| **Layout animation** | Animate windows sliding into position instead of instant snap | Medium |
| **Tray tooltip live count** | Show "PSM — 6 windows" in tray tooltip, update dynamically | Low |
| **Export/import config** | Share layouts as TOML snippets or import from clipboard | Low |
| **Undo last arrange** | Remember previous window positions, one-click undo | Medium |
| **Taskbar-aware gaps** | Detect taskbar position/size and avoid overlapping it on non-primary monitors | Low |
| **Opacity/always-on-top** | Set arranged windows as always-on-top or semi-transparent | Low |
| **CLI list command** | `--list` to print detected windows without arranging | Low |
