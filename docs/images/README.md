# Screenshots Directory

This directory contains screenshots for documentation.

## Required Screenshots

### TUI Screenshots
- `tui-overview.png` - Main TUI overview tab with system stats
- `tui-gpu.png` - GPU monitoring tab showing GPU metrics
- `tui-agent.png` - AI Agent tab showing natural language queries

### GUI Screenshots  
- `gui-overview.png` - Main GUI window with system overview
- `gui-gpu.png` - GPU monitoring panel
- `gui-themes.png` - Theme selector showing different color themes

## Capturing Screenshots

### TUI Screenshots
```bash
# Run the TUI
cargo run --release --features cli --example tui

# Use your terminal's screenshot feature or a tool like:
# - Windows: Win+Shift+S (Snipping Tool)
# - macOS: Cmd+Shift+4
# - Linux: gnome-screenshot or flameshot
```

### GUI Screenshots
```bash
# Run the GUI
cargo run --release --features gui

# Same screenshot tools as above
```

## Image Guidelines
- PNG format preferred
- 1920x1080 or similar aspect ratio
- Show realistic data (not empty/zeroed)
- Dark theme recommended for TUI screenshots
