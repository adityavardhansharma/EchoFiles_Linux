# Toolbar

The 40px navigation row: history, reload, path, search, view toggles and settings.

**Provide** `segments` for the PathBar, optional `scope` (`folder` | `everywhere`), `query`, `grid`, `dual`, `preview`, `hidden`, `settings`.

- Fixed order: Back, Forward, Up, Reload · PathBar · `SearchScope` · Search · List (Ctrl+1), Grid (Ctrl+2), Dual pane (F3), Preview pane (Space) · Hidden files (eye, Ctrl+H) · Command palette (Ctrl+K) · Settings (gear, Ctrl+,). Toggles show their on state as `pressed`.
- Every button is an `IconButton`: same 28px square, `state-hover` layer and `ink-strong` glyph on hover, `state-press` when pressed, 45% and no hover when disabled (Forward with no history). Every one has a tooltip with its key as a `Kbd`. PathBar segments use the same hover.
- While Everywhere results are showing, the PathBar ends with a bold **Search results** segment; clicking any earlier segment leaves the results.
- No window controls: EchoFiles is tiled by Hyprland and never draws client-side decorations.
