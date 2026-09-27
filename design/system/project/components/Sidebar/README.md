# Sidebar

The place list on `bg-sunken`: Linux, Pinned (when anything is pinned), Windows, later Phone, and **All drives** at the end.

**Provide** `Sidebar` > `SidebarSection` (`title`, optional `world`: `linux`, `windows`, `phone`, optional `count`) > `SidebarItem` (`icon`, `label`, `active`, `trail`, `dropTarget`) or `DriveItem`.

- A world is marked by a 6px square in `world-linux`, `world-windows` or `world-phone` — the only place world colours appear besides drive usage bars.
- Active item: `state-active` ground, `ink-strong` bold label, `accent-ink` glyph. No side rail.
- Sections are data-driven (the phone section plugs in later). Drag a file onto an item → `accent-soft` with a 1px `accent` inset.
- Width `sidebar-width` (236px), resizable 180–360 by dragging its right edge (the edge turns `accent` while dragged), hidden with Ctrl+B; both are remembered in settings.
- **Pinned** folders come from *Pin to sidebar* in any folder's menu and show a `pin` glyph; right-click to unpin.
- Windows user folders (Desktop, Documents, Downloads, Pictures, Music, Videos under `Users\<you>`) nest, indented, under the mounted drive that holds them.
- Every place: click opens, middle-click opens in a new tab, right-click for Open / Open in new tab / Paste into folder / Pin, and files dragged onto it move or copy there. Trash's menu has **Empty Trash**.
