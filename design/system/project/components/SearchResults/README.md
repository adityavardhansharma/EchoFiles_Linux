# SearchResults

Everywhere results in place of the file list: name, the folder it lives in (`~`-relative), size and date.

**Provide** `hits` (`{name, kind, location, size, date}`); omit for the demo set.

- Answered from the index in milliseconds; with the index off, a live walk of the indexed folders fills the same view. The status bar says which ("from the index" / "live search") and "N matches · showing first 300" when capped.
- ↑/↓ move, Enter opens (folders open in place, files in their app), Alt+Enter / **Show in folder** opens the parent folder with the file selected. Double-click opens.
- Location clips on the right; it never pushes into Size. Rows use the same hover and active layers as `FileRow`.
- No matches: a centred search glyph, "No matches for “…”", and when the index answered, how fresh it is ("updated 20 s ago; new files elsewhere can take up to a minute").
