# Sidebar

The place list on `bg-sunken`: Linux, Pinned (when anything is pinned), Windows, **All drives**, then Network (later Phone).

**Provide** `Sidebar` > `SidebarSection` (`title`, optional `world`: `linux`, `windows`, `network`, `phone`, optional `count`, optional `action` at the end of the head) > `SidebarItem` (`icon`, `label`, `active`, `trail`, `dropTarget`), `DriveItem` or `NetworkItem`.

- A world is marked by a 6px square in `world-linux`, `world-windows`, `world-network` or `world-phone` — the only place world colours appear besides drive usage bars and network glyphs.
- **Network** lists saved servers (in the order saved) and then live connections made elsewhere (Nautilus, `gio mount`, a link). Its head counts live connections and ends with a `+` that opens `ConnectDialog`; a **Connect to server…** row always closes the section. With nothing saved it says "Windows shares, SSH and FTP servers you connect to appear here."
- Active item: `state-active` ground, `ink-strong` bold label, `accent-ink` glyph. No side rail.
- Sections are data-driven (the phone section plugs in later). Drag a file onto an item → `accent-soft` with a 1px `accent` inset.
- Width `sidebar-width` (236px), resizable 180–360 by dragging its right edge (the edge turns `accent` while dragged), hidden with Ctrl+B; both are remembered in settings.
- **Pinned** folders come from *Pin to sidebar* in any folder's menu and show a `pin` glyph; right-click to unpin.
- Drives are listed on their own — no Windows user folders under them (a PC with several accounts would bury the list). A thin scrollbar sits beside the rows, never over them.
- Every place: click opens, middle-click opens in a new tab, right-click for Open / Open in new tab / Paste into folder / Pin, and files dragged onto it move or copy there. Trash's menu has **Empty Trash**.
