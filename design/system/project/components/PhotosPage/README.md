# PhotosPage

Every photo and video on the phone in one timeline, newest first, grouped by day.

**Provide** `selected` (keys); the demo is clickable.

- Gathered from Camera, Screenshots, Pictures and messaging media (WhatsApp, Telegram). The `SegmentedControl` filters: All · Camera · Screenshots · WhatsApp.
- Square tiles, 4px gaps, 118px minimum. Thumbnails come from each JPEG's embedded EXIF thumbnail (a 64 KB range read, never the full file) and HEIC's thumbnail item; videos show a placeholder with their length until a frame is grabbed. Cached per phone.
- Click selects (a round check, top left, `accent` inset ring and the photo shrinks 10%); double-click opens it. With a selection, a sticky `accent-soft` bar: "3 selected · 11.8 MB", **Save to…** (any Linux or Windows folder), **Copy**, **Clear**. Drag photos out to any folder.
- **Import 32 new** (`primary`) copies only photos not imported before (matched by name, size and date) into dated folders, `~/Pictures/Phone/2026-10/`. One-way: nothing is deleted on the phone.
