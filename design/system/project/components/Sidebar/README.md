# Sidebar

The place list on `bg-sunken`: Linux, Pinned (when anything is pinned), Windows, **All drives**, then Network (later Phone).

**Provide** `Sidebar` > `SidebarSection` (`title`, optional `world`: `linux`, `windows`, `network`, `phone`, optional `count`, optional `action` at the end of the head, `collapsible` + `defaultOpen`) > `SidebarItem` (`icon`, `label`, `active`, `trail`, `dropTarget`), `DriveItem` or `NetworkItem` (`keep` = still shown while collapsed).

- **Windows** and **Network** fold. Their head is a 24px row — chevron (`chevron-right` / `chevron-down`, 12px `ink-muted`), world mark, title, count, and Network's `+` — with the standard hover; clicking it opens or closes the section. Both start **collapsed**, and the open/closed state is remembered (`settings.toml` `[sidebar] windows_open`, `network_open`). Collapsed, the section still shows the one place you're in (the drive or share holding the open folder, or All drives while that page is open), so the sidebar always says where you are. Everything the section held (All drives, Connect to server…) folds with it.
- **Settings → Appearance → Sidebar** hides either section entirely (*Windows drives*, *Network places*; both on by default). Hidden, the section's head goes too; Ctrl+Shift+D and Ctrl+Shift+S still reach drives and servers.

- A world is marked by a 6px square in `world-linux`, `world-windows`, `world-network` or `world-phone` — the only place world colours appear besides drive usage bars and network glyphs.
- **Network** lists saved servers (in the order saved) and then live connections made elsewhere (Nautilus, `gio mount`, a link). Its head counts live connections and ends with a `+` that opens `ConnectDialog`; a **Connect to server…** row always closes the section. With nothing saved it says "Windows shares, SSH and FTP servers you connect to appear here."
- Active item: `state-active` ground, `ink-strong` bold label, `accent-ink` glyph. No side rail.
- Sections are data-driven (the phone section plugs in later). Drag a file onto an item → `accent-soft` with a 1px `accent` inset.
- Width `sidebar-width` (236px), resizable 180–360 by dragging its right edge (the edge turns `accent` while dragged), hidden with Ctrl+B; both are remembered in settings.
- **Pinned** folders come from *Pin to sidebar* in any folder's menu and show a `pin` glyph; right-click to unpin.
- Drives are listed on their own — no Windows user folders under them (a PC with several accounts would bury the list). A thin scrollbar sits beside the rows, never over them.
- Every place: click opens, middle-click opens in a new tab, right-click for Open / Open in new tab / Paste into folder / Pin, and files dragged onto it move or copy there. Trash's menu has **Empty Trash**.
