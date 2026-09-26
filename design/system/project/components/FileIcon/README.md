# FileIcon

A 48-unit full-colour icon for places, devices, file types and status; its colours come from the active Omarchy theme.

**Provide** `name` (one of the 71 icons in `assets/icons/color/`), `size` (18 in rows, 48 in tiles, 32 in dialogs, 96 in the preview pane) and an optional `badge`: `link` (symlink or NTFS junction), `cloud` (OneDrive placeholder), `lock` (EFS-encrypted), `broken` (dangling link or `.lnk`), `ads` (alternate data streams).

- Colours are slots, not hexes: `{folder}` → `icon-folder` (Omarchy `yellow`), `{blue}` → `icon-blue`, `{purple}` → `icon-purple` (Omarchy `magenta`), and so on. The app substitutes slot hexes when it rasterises, so switching Omarchy themes recolours every icon.
- Map extensions with `iconFor(name, kind)`; unknown types fall back to `file`. Images and videos swap to a thumbnail once decoded (see `FileTile`).
- Badges render only at 18px and up. One badge per icon; priority `broken` > `lock` > `cloud` > `link` > `ads`.
- Never recolour a single icon by hand, never put UI glyphs on files, never scale below 16px (use `Icon` instead).
