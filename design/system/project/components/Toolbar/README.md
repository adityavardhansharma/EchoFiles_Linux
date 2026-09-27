# Toolbar

The 40px navigation row: history, reload, path, search and settings.

**Provide** `segments` for the PathBar, optional `scope` (`folder` | `everywhere`), `query`, `hidden`, `settings`.

- Fixed order: Back, Forward, Up, Reload · PathBar · `SearchScope` · Search · Hidden files (eye) · Settings (gear, Ctrl+,). Grid view, dual pane and preview join when they ship — never as dead buttons.
- Every button is an `IconButton`: same 28px square, `state-hover` layer and `ink-strong` glyph on hover, `state-press` when pressed, 45% and no hover when disabled (Forward with no history). Every one has a tooltip with its key as a `Kbd`. PathBar segments use the same hover.
- While Everywhere results are showing, the PathBar ends with a bold **Search results** segment; clicking any earlier segment leaves the results.
- No window controls: EchoFiles is tiled by Hyprland and never draws client-side decorations.
