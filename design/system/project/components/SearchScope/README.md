# SearchScope

Where the search box looks: **Folder** filters the open folder as you type; **Everywhere** searches every indexed folder through the index.

**Provide** `value` (`folder` | `everywhere`), `onChange`.

- A `SegmentedControl` just left of the search field. Ctrl+E switches it; the field's placeholder follows ("Search this folder" / "Search everywhere").
- The default comes from Settings → Search & index → Search looks in.
