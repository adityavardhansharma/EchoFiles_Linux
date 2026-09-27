# Toolbar

The 40px navigation row: history, reload, path, search, the View dropdown, command palette and settings.

**Provide** `segments` for the PathBar, optional `scope` (`folder` | `everywhere`), `query`, `grid`, `viewOpen`, `settings`.

- Fixed order: Back, Forward, Up, Reload · PathBar · `SearchScope` · Search · View ▾ · Command palette (⌘ glyph, Ctrl+K) · Settings (gear, Ctrl+,).
- **View ▾** (`ViewButton`) shows only the current layout's glyph (list or grid) and a chevron. It opens `ViewMenu`, right-aligned under it: **Layout** — List (Ctrl+1), Grid (Ctrl+2); **Show** — Dual pane (F3), Preview pane (Space), Hidden files (Ctrl+H). A trailing `accent-ink` check marks what's on; picking an item applies it and closes the menu. The keys keep working without the menu.
- Every button is an `IconButton`: same 28px square, `state-hover` layer and `ink-strong` glyph on hover, `state-press` when pressed, 45% and no hover when disabled (Forward with no history). Every one has a tooltip with its key as a `Kbd`. PathBar segments use the same hover.
- While Everywhere results are showing, the PathBar ends with a bold **Search results** segment; clicking any earlier segment leaves the results.
- No window controls: EchoFiles is tiled by Hyprland and never draws client-side decorations.
