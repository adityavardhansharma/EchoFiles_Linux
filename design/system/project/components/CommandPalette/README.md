# CommandPalette

Ctrl+K: every action, folder, drive and setting, fuzzy-matched — the fastest way to do anything.

**Provide** `groups` of `{icon, label, hint?, kbd?}` and the current `query`.

- Built like Omarchy's launcher: `>` prompt in `accent-ink`, results with matched letters in `accent-ink` bold, selected row on `state-hover` with a 1px `line` border.
- Groups: **Actions** (new folder, paste, undo, tabs, panes, views, mount / unmount each drive, "Mount Windows (C:) read-write", Empty Trash…), **Go to** (places, pinned, recent folders, mounted drives), **Settings** pages. Within a group the best fuzzy match leads.
- Typing a path (`/` or `~`) switches to folder completion; Enter opens it.
- Drops 8px and fades in over `dur-slow`; Esc closes instantly.
