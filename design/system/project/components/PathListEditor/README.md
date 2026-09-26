# PathListEditor

Rows of folder paths inside a `SettingsGroup`, each with a remove ×, then an add row: field, **Add**, and ghost **Add current folder** (the fastest way to add what you're looking at).

**Provide** `paths` (shown `~`-relative), `icon`, `empty`, `placeholder`, `current`, `error`, `note` (per-row pill, e.g. "Missing").

- Validate on Add and Enter: a full path (`/` or `~/`), an existing folder, not a duplicate. The error sits under the field with an `error` glyph in `danger-ink`, and the field border turns `danger`.
- Indexed folders can't go empty: removing the last one says "Keep at least one folder — or turn the search index off instead."
- Changes save immediately and schedule a re-index 1.5 s later.
