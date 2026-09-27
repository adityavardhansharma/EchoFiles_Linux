# ContextMenu

The right-click menu: actions for the selection, grouped and with shortcuts.

**Provide** `items`: `{icon, label, kbd?, hint?, submenu?, danger?, disabled?}`, `"-"` for separators, `{heading}` for group labels.

- On items — Open (Open in new tab / other pane for a folder, Open with… for a file) · Cut, Copy, Paste into folder, **Move to ▸** / **Copy to ▸** (other pane, Home folders, pinned folders, mounted drives), Rename, Copy path, Copy as Windows path (NTFS only), Pin to sidebar · Move to Trash, Delete permanently · Properties. In the Trash: Restore, Delete permanently.
- On empty space — New folder, New file, Paste, Select all · View as grid/list, Show hidden files · Open terminal here, Copy path, Pin to sidebar, Properties. In the Trash: **Empty Trash**.
- Right-clicking an unselected item selects it first. The Menu key opens it for the cursor item. Arrow keys move, Enter picks, hovering an item with ▸ opens its submenu beside it.
- Highlight follows Omarchy's menu: `state-hover` ground, 1px `line` border, `accent-ink` label and glyph. Panel `bg-raised`, 1px `line-strong`, square corners.
- Opens with a 4px drop in `dur-base`; closes instantly.
