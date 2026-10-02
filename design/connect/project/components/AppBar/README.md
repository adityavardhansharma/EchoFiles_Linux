# AppBar

The 56px top bar: Back or nothing, the screen's title in `title` style, up to two `IconButton`s.

**Provide** `title`, optional `back`, `sub`, `actions` (`{icon, label}`).

- `bg` with a `line` hairline under it. No colour, no elevation, no collapsing hero.
- Tab screens have no Back; pushed screens (Permissions, Automatic clipboard) do.
