# IconButton

A 28px square glyph button for the toolbar, tab strip, toasts and panes.

**Provide** `icon`, `label` (becomes the tooltip and accessible name; include the shortcut: "Back (Alt+Left)"), optional `pressed` for toggles (preview pane, show hidden, dual pane).

- Rest `ink-muted`, hover `ink-strong` on `state-hover`, pressed toggle `accent-ink` on `state-active`.
- Toolbar order is fixed: Back, Forward, Up · PathBar · Search · View · Preview · More.
- Never use for destructive actions without a confirm step or undo toast.
