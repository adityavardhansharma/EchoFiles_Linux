# CloudPage

Google Drive open in a pane — the reference for Drive-specific rows and states.

**Provide** nothing; reference composition.

- **Google Docs, Sheets, Slides, Forms, Drawings** show their name without rclone's `.html`, kind "Google Doc" / "Google Sheet" / "Google Slides" / …, the matching colour icon (`file-doc`, `file-sheet`, `file-slides`) and **—** for size. Enter opens them in the browser; the menu adds **Download as…** (Word / PDF / OpenDocument, Excel / CSV / PDF, PowerPoint / PDF). Dragging one out asks "Google Docs can't be copied as files. Download as Word or PDF?".
- **Honest uploads:** a copy into Drive finishes when the data reaches rclone's cache, so the transfer toast ends "Copied — uploading to Google Drive" and the status bar shows "Uploading 3 files to Google Drive · 12 MB left" until the queue is empty, then "Uploaded to Google Drive" for a few seconds. Offline it reads "Waiting for connection".
- Delete moves to **Google Drive's trash**: toast "Moved to Google Drive's trash" with **Open trash in browser**. Shift+Delete says it bypasses Google's trash.
- No thumbnails (a thumbnail would download the whole file); the status bar's free space comes from Drive's quota.
