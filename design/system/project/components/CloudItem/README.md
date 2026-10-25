# CloudItem

A Google Drive account in the sidebar's Cloud section: the name the person gave it, a usage bar, and the usage in words ("4.2 GB of 15 GB").

**Provide** `name` ("Google Drive", "Google Drive 2", … renamable; never an email or a mount path), `state` (`connected` default, `connecting`, `uploading` + `uploads`, `offline`, `signin`, `notmounted`), `used` (percent), `usage`, `active`.

- Glyph `cloud` in `world-cloud`, like a connected network place. No Google logo, ever — brand marks stay out of the working UI.
- Normal state has no pill. **Connecting…** (`info`) with a spinner while rclone mounts; **Uploading 3** (`info`) while files copied in are still going up; **Offline** (neutral) after two failed calls in a row; **Sign in again** (`warning`) when Google revoked the token; **Not mounted** (neutral) when FUSE isn't available. The pill leads the second line, like `DriveItem`.
- The usage bar follows `UsageBar` (grows once, 400ms; `warning` above 90%, `danger` above 97%), coloured `world-cloud`.
- Click: opens the Drive root in the pane (mounts first if needed). Middle-click: new tab. Right-click: Open · Open in new tab · Open in browser · Refresh · Rename… · Disconnect · Sign out… · Google Drive settings.
- Tooltip: the name, access (Full / Read-only) and usage.
