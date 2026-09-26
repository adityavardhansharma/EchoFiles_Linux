# Sidebar

The place list on `bg-sunken`, grouped into worlds: Linux, Windows, later Phone, then Pinned.

**Provide** `Sidebar` > `SidebarSection` (`title`, optional `world`: `linux`, `windows`, `phone`, optional `count`) > `SidebarItem` (`icon`, `label`, `active`, `trail`, `dropTarget`) or `DriveItem`.

- A world is marked by a 6px square in `world-linux`, `world-windows` or `world-phone` — the only place world colours appear besides drive usage bars.
- Active item: `state-active` ground, `ink-strong` bold label, `accent-ink` glyph. No side rail.
- Sections are data-driven (the phone section plugs in later). Drag a file onto an item → `accent-soft` with a 1px `accent` inset.
- Width `sidebar-width` (236px), resizable 180–360, collapses with Ctrl+B in `dur-base`.
