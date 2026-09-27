# PreviewPane

The Space-toggled right pane: large preview, facts, Windows attributes and quick actions.

**Provide** `file` (`name`, `thumb` or colour icon), `props` (label/value pairs), `attrs` (Windows attributes when on NTFS), `windowsPath`.

- 280px on `bg-sunken`, slides in 12px over `dur-slow`. Text files show their first 200 lines, images their thumbnail, folders their size and item count (computed in the background).
- Windows files show `Copy Windows path` and their DOS attributes; Linux files show permissions and owner instead.
- Facts: Size (exact bytes too), Modified, Created, Opened, Location, Points to (links, "missing" when broken), Permissions `rw-r--r-- (644)`, Owner · group. Folders: whole-tree size and file count, filling in with a `+` until counted, plus how many items are directly inside.
- On NTFS the Read-only, Hidden, System and Archive attributes are `Checkbox`es that change the file; Compressed, Encrypted (EFS), Sparse, Online-only and Junction are listed.
- With several items selected the header adds "3 selected · 12 MB + folders". Alt+Enter (Properties) opens the pane if it's closed. Nothing selected: "Select something to see its details."
