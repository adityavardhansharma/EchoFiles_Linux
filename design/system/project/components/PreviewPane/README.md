# PreviewPane

The Space-toggled right pane: large preview, facts, Windows attributes and quick actions.

**Provide** `file` (`name`, `thumb` or colour icon), `props` (label/value pairs), `attrs` (Windows attributes when on NTFS), `windowsPath`.

- 280px on `bg-sunken`, slides in 12px over `dur-slow`. Text files show their first 200 lines, images their thumbnail, folders their size and item count (computed in the background).
- Windows files show `Copy Windows path` and their DOS attributes; Linux files show permissions and owner instead.
