# FileList

The virtualised details view: sticky column header and fixed-height rows — the heart of the app.

**Provide** `files` (`{name, kind, size, type, modified, items, selected, cut, hidden, badge, drop}`), `sort` + `desc`, `density` (`compact` 24px, default 28px, `comfortable` 34px), `loading` (skeleton row count), `renaming` (row index).

- Rows never change height at runtime: the Rust list renders only visible rows at a fixed `row` height.
- Selection is `selection` (Omarchy's own); the keyboard cursor is a 1px inset `focus-ring`; both change instantly (`dur-instant`).
- Names are `ink`, extensions `ink-muted`, metadata `meta` style in `ink-muted` with tabular figures. Folders sort first; natural order (`file2` before `file10`).
- Cut items use `opacity-cut`; hidden and Windows Hidden/System files use `opacity-hidden-file` when shown.
- Keyboard: arrows move, Shift extends, Ctrl toggles, type-to-jump, Enter opens, F2 renames, Del trashes, Shift+Del asks, Space previews.
