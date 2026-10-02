# Icon

The EchoFiles glyph set (24-unit grid, 2px stroke, round caps, `currentColor`) plus ten EchoConnect additions, drawn larger for touch.

**Provide** `name`, optional `size` (20 in rows and buttons = `icon-glyph`, 22 in the bottom nav = `icon-nav`, 16 in pills and small buttons), `label` when it stands alone with meaning.

- Additions for the phone: `bluetooth`, `laptop`, `camera`, `mic`, `speaker`, `call`, `scan`, `moon`, `cursor-text`, `flashlight`. They follow the EchoFiles drawing rules and move to `assets/icons/glyph/` when the Android app is built.
- Colour comes from the parent: `ink-muted` at rest, `ink-strong` pressed, `accent-ink` when active, a `*-ink` inside banners.
- Glyphs are for actions and UI only; files use `FileIcon`.
