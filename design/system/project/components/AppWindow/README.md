# AppWindow

The whole app assembled: tabs, toolbar, sidebar, banner, list, preview pane, status bar and a running transfer.

**Provide** nothing; this is the reference composition every screen is checked against.

- Window chrome is only a 2px `accent` border — Hyprland's active border. No title bar, no window buttons.
- Surfaces step from `bg-deep` (tabs, status) → `bg` (toolbar, list) → `bg-sunken` (sidebar, preview) → `bg-raised` (floating things). Nothing else is layered.
