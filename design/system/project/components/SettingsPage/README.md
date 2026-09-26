# SettingsPage

Settings is its own screen (Ctrl+, or the gear): a top bar with a ghost **← Files** button and `Esc` `Kbd`, the title and a live **Saved** mark; `SettingsNav` on the left; one page of `SettingsGroup`s in a centred 720px column. Esc or ← Files returns to the same folder.

**Provide** optional `page`; the nav is live in the preview.

- **General** — Running: Keep running in the background (close hides the window; later launches reuse it) · Start at login (disabled unless running in the background). Windows: New windows open at Home / Last folder · Show hidden files.
- **Search & index** — Index: Search index switch (off deletes the index) + `IndexStatus`. Search box: Search looks in (This folder / Everywhere). Indexed folders and Never show in search (`PathListEditor`). Skip contents of: Skip cache folders + `NameChips`.
- **AI agents** — Allow EchoFiles commands (when off, `ef` exits 3 with "turned off"), ef location, Teach AI agents about ef (links the skill into ~/.claude/skills; off removes only that link), `CommandList` of examples. A `warning` note appears if `ef` is allowed but the index is off.
- **Appearance** — the Omarchy theme's name and swatches (it follows the system); Row height Compact / Default / Comfortable (24 / 28 / 34px).
- **About** — version, settings file and index folder with **Show**, keyboard shortcuts.
- Everything saves to `~/.config/echofiles/settings.toml` the moment it changes (shared with `ef`). Only settings that work today appear.
