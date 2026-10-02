# PhoneHub

The phone's home page, opened from its sidebar row. Everything about the phone starts here.

**Provide** `state` (`connected`, `away`), `wallpaper`, `phone` (`{name, battery, charging, network, storage: [free, total]}`), `off` (`{messages, notifications, clipboard}` — features turned off in settings), `received`, `ringing`.

- **Hero** — one flat `bg-raised` box with a 1px `line` border and `radius-2`, like a settings group: no glows, gradients or shadows. `PhoneDevice` (188px) on the left.
- Facts, top to bottom: a `StatePill` — **Connected** (`success`) or "Not nearby · last seen 2 h ago" (neutral) — with the gear (Settings → Phone) opposite; the name in `display` (28/34, 800); a meta line — Wi-Fi name, KDE Connect, paired date; three **`Gauge`** cards on `bg` — **Battery** (ring in `success`, `warning` at 20%; a bolt while charging), **Storage** (free of total, read in the background; hidden when the phone's SFTP server doesn't report it), **New photos** (the newest three thumbnails side by side, the count and **Import →**, which opens `PhotosPage`).
- Three standard bordered buttons: **Ring phone** (turns `accent`-filled **Stop ringing** while it rings, with a one-line note under the row), **Send files** (a picker; same as dropping on the sidebar row), **Get files** (opens the phone's storage in a new tab).
- The hero follows its pane width: gauges go 2+1 under 940px, the phone moves above the facts under 700px.
- **Feature tiles**: Files, Photos ("32 new"), Messages (last text, unread count), Notifications (apps with something new, count). The count badge is `accent`. A feature turned off shows a dashed tile "Off · turn on in settings" instead of disappearing, so people can find it again.
- **Received from phone** (`ReceivedList`) under the tiles; **Shared clipboard** (`ClipboardCard`) in the right column.
- **Not nearby**: screen-off device, "Not nearby · last seen 2 h ago · showing what EchoFiles saw then", buttons disabled; tiles open cached views greyed. Reconnects on its own when the phone returns.
- Breadcrumb and tab read the phone's name with the `phone` glyph; the status bar says "Galaxy S24 · 72% · charging".
