# SearchResults

Everywhere results in place of the file list: name, the folder it lives in (`~`-relative), size and date.

**Provide** `hits` (`{name, kind, location, size, date}`); omit for the demo set.

- Answered from the index in milliseconds; with the index off, a live walk of the indexed folders fills the same view. The status bar says which ("from the index" / "live search") and "N matches · showing first 300" when capped.
- ↑/↓ move, Enter opens (folders open in place, files in their app), Alt+Enter / **Show in folder** opens the parent folder with the file selected. Double-click opens.
- Location clips on the right; it never pushes into Size. Rows use the same hover and active layers as `FileRow`.
- No matches: a centred search glyph, "No matches for “…”", and when the index answered, how fresh it is ("updated 20 s ago; new files elsewhere can take up to a minute").
- **Google Drive, two steps.** Saved results (local index and the Drive index if it's on) paint first. 350ms after typing stops the same search goes to Google Drive and the footer shows `live`: **searching** (`Spinner`, "Searching Google Drive…") → **done** ("Google Drive checked · 3 new", for 3 s) or **timeout** / **offline** ("…— showing saved results", `warning-ink`) or **pattern** ("Google Drive can't search by pattern — showing saved results").
- New Drive matches join the list without moving anything under the cursor: if the cursor hasn't moved since the first paint the list re-ranks in one frame (no animation); otherwise they're added below the cursor. A saved hit that turns out to be gone fades out (`dur-base`, `ease-exit`); if it's the selected row it stays, its location reads "No longer on Google Drive" in `ink-muted` and it can't be opened (`gone`).
- Drive locations read "Google Drive › Folder"; with the account offline, "Google Drive · offline" and opening offers **Connect**.
