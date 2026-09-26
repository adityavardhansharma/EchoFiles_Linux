# ContextMenu

The right-click menu: actions for the selection, grouped and with shortcuts.

**Provide** `items`: `{icon, label, kbd?, hint?, submenu?, danger?, disabled?}`, `"-"` for separators, `{heading}` for group labels.

- Groups: Open · Clipboard & organise · Trash · Properties. Windows-specific actions (Copy as Windows path, Show alternate streams) appear only on NTFS items.
- Highlight follows Omarchy's menu: `state-hover` ground, 1px `line` border, `accent-ink` label and glyph. Panel `bg-raised`, 1px `line-strong`, square corners.
- Opens with a 4px drop in `dur-base`; closes instantly.
