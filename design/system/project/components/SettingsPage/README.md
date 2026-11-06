# SettingsPage

Settings is its own screen (Ctrl+, or the gear): a top bar with a ghost **← Files** button and `Esc` `Kbd`, the title and a live **Saved** mark; `SettingsNav` on the left; one page of `SettingsGroup`s in a centred 720px column. Esc or ← Files returns to the same folder.

**Provide** optional `page`; the nav is live in the preview.

- **General** — Running: Keep running in the background (close hides the window; later launches reuse it) · Start at login (disabled unless running in the background). Windows: New windows open at Home / Last folder · Show hidden files.
- **Search & index** — Index: Search index switch (off deletes the index) + `IndexStatus`, and a muted line "Google Drive has its own index switch in Settings → Google Drive." (it never changes that switch). Search box: Search looks in (This folder / Everywhere). Indexed folders and Never show in search (`PathListEditor`). Skip contents of: Skip cache folders + `NameChips`.
- **AI agents** — Allow EchoFiles commands (when off, `ef` exits 3 with "turned off"), ef location, Teach AI agents about ef (links the skill into ~/.claude/skills; off removes only that link), `CommandList` of examples. A `warning` note appears if `ef` is allowed but the index is off. **Let ef search Google Drive** (off by default) is its own switch: the Drive index never gives agents access and this never turns the index on.
- **Google Drive** — Account: one row per account (usage, **Sign out…**, and a ⋮ menu with **Rename…** and **Change access…**) and **Connect Google Drive**; a `warning` `Banner` with the install command when rclone is missing. Search: *Include Google Drive in search index* (off by default, separate from every other index switch) + `IndexStatus`, *Search Google Drive live* (on). Storage: cache size limit (1–500 GB), cached size and **Clear cache** (disabled while uploads are pending). Advanced: *Use my own client ID* — Client ID and secret fields, **Save and sign in again**, the five setup steps, and **Use rclone's app** when one is set. rclone: version, path and **Open log**.
- **Phone** — see `PhoneSettings`: this phone and Forget, phone picture, feature switches, Accept files automatically + Save to, notifications Off / In app / Desktop too and muted apps.
- **Appearance** — the Omarchy theme's name and swatches (it follows the system); Row height Compact / Default / Comfortable (24 / 28 / 34px); Folders open as Automatic / List / Grid (resets per-folder choices). Sidebar: show or hide the Windows drives, Phone, Network places and Cloud sections.
- **About** — version, settings file and index folder with **Show**, keyboard shortcuts.
- Everything saves to `~/.config/echofiles/settings.toml` the moment it changes (shared with `ef`). Only settings that work today appear.
