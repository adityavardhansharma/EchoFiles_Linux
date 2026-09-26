# CommandPalette

Ctrl+K: every action, folder, drive and setting, fuzzy-matched — the fastest way to do anything.

**Provide** `groups` of `{icon, label, hint?, kbd?}` and the current `query`.

- Built like Omarchy's launcher: `>` prompt in `accent-ink`, results with matched letters in `accent-ink` bold, selected row on `state-hover` with a 1px `line` border.
- Results rank recent and frequent first; typing a path (`/`, `~`, `C:\`) switches to path completion; `/` searches files.
- Drops 8px and fades in over `dur-slow`; Esc closes instantly.
