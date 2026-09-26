# FileGrid

Tiles for picture-heavy folders: 104px cells, 88px thumbnails or 48px colour icons, two-line names.

**Provide** `files` with `thumb` (decoded thumbnail URL/handle), `sub` (size or item count), selection flags.

- The app switches to grid automatically in Pictures, Videos and camera folders, and remembers the choice per folder.
- Thumbnails fade in over the icon (`dur-base`) with no layout shift; they come from the freedesktop cache (128px) and are drawn at 88px.
- Same selection, cursor and drop states as rows.
