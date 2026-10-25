# StatusBar

The 24px footer: counts, selection size, background task and volume facts.

**Provide** `count`, `selected`, `selectedSize`, `task`, `volume` (name + driver), `state`, `free`.

- Numbers in `ink` bold, words in `ink-muted`. The volume's state pill repeats here so read-only is never a surprise.
- Selection size for folders is computed in the background and fills in when ready ("12.4 MB+" until done).
- Also says what's on the clipboard ("3 cut on the clipboard") and how many transfers are running. The right side names the volume: "AVS (D:) · ntfs3 · 66 GB free", or "Linux · btrfs · …".
- Inside Google Drive the right side reads "Google Drive · 11 GB free" (from Drive's quota), and pending uploads show as the task: "Uploading 3 files to Google Drive · 12 MB left", then "Uploaded to Google Drive" for a few seconds, or "Waiting for connection" while offline.
