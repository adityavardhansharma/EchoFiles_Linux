#!/usr/bin/env python3
"""Assemble the EchoFiles design system (design/system/project/) from sources.

Run after build_icons.py and build_tokens.py.
"""
import json, pathlib, re, shutil

ROOT = pathlib.Path(__file__).resolve().parent.parent
SYS = ROOT / "design/system"
PRJ = SYS / "project"
SRC = SYS / "src"

# name: (group, height, extra marker attrs, preview JSX-ish body, README)
C = {}

def comp(name, group, height, body, readme, extra=""):
    C[name] = (group, height, extra, body, readme)

# ------------------------------------------------------------------ Iconography
comp("Icon", "Iconography", 196, """
var names = ["arrow-left","arrow-right","arrow-up","home","history","grid","list","columns","sidebar","sort","filter","more-vertical","search","eye","eye-off","copy","cut","paste","rename","trash","undo","redo","plus","close","check","folder-plus","file-plus","download","upload","move","swap","external","link","share","sync","refresh","cloud","star","tag","pin","info","alert","error","clock","lock","key","shield","settings","sliders","terminal","keyboard","drive","usb","sd-card","phone","network","image","music","video","play","pause","cancel","archive","code"];
return h("div", {className:"ef", style:{display:"grid", gridTemplateColumns:"repeat(auto-fill, 36px)", gap:4, padding:16, color:"var(--ink-muted)"}},
  names.map(function(n){ return h("span", {key:n, title:n, style:{width:36,height:36,display:"grid",placeItems:"center"}}, h(E.Icon, {name:n, size:18})); }));
""", """# Icon

A 24-unit, 2px-stroke, round-capped glyph that inherits `currentColor`; used for every UI action, never for file types.

**Provide** `name` (one of the 86 glyphs in `assets/icons/glyph/`), optional `size` (default 16 = `icon-glyph`), `label` when the glyph stands alone with meaning.

- Colour comes from the parent: `ink-muted` at rest, `ink-strong` on hover, `accent-ink` when active or pressed, a semantic `*-ink` inside banners and toasts.
- Sizes: 16 in toolbars, menus and sidebar; 14 inside buttons and fields; 12 in chevrons, sort arrows and badges.
- Never fill a glyph, never mix it with a colour icon in the same slot, never use it for a file type (that is `FileIcon`).
- The Rust app rasterises the same SVGs with `resvg`, substituting `currentColor` with the resolved token, and caches one bitmap per (glyph, size, scale, colour).
""")

comp("FileIcon", "Iconography", 300, """
var names = ["folder","folder-open","folder-image","folder-music","folder-video","folder-download","folder-lock","folder-link","folder-share","folder-users","home","trash","trash-full","drive","drive-external","usb","sd-card","phone","network","server","cloud","file","file-text","file-doc","file-sheet","file-slides","file-pdf","file-code","file-config","file-script","file-image","file-video","file-audio","file-vector","file-font","file-archive","file-archive-alt","file-package","file-verified","file-error","file-new","lock","shield","star","tag","info","alert","sync","send","eye"];
return h("div", {className:"ef"},
  h("div", {style:{display:"grid", gridTemplateColumns:"repeat(auto-fill, 44px)", gap:6, padding:"16px 16px 8px"}},
    names.map(function(n){ return h("span", {key:n, title:n, style:{width:44,height:44,display:"grid",placeItems:"center"}}, h(E.FileIcon, {name:n, size:36})); })),
  h("div", {className:"ef-row-flex", style:{paddingTop:4}},
    ["link","cloud","lock","broken","ads"].map(function(b){ return h("span", {key:b, style:{display:"inline-flex",gap:8,alignItems:"center",marginRight:12}}, h(E.FileIcon, {name: b==="link" ? "folder" : "file-doc", size:32, badge:b}), h("span", {className:"ef-muted", style:{fontSize:11}}, b)); })));
""", """# FileIcon

A 48-unit full-colour icon for places, devices, file types and status; its colours come from the active Omarchy theme.

**Provide** `name` (one of the 71 icons in `assets/icons/color/`), `size` (18 in rows, 48 in tiles, 32 in dialogs, 96 in the preview pane) and an optional `badge`: `link` (symlink or NTFS junction), `cloud` (OneDrive placeholder), `lock` (EFS-encrypted), `broken` (dangling link or `.lnk`), `ads` (alternate data streams).

- Colours are slots, not hexes: `{folder}` → `icon-folder` (Omarchy `yellow`), `{blue}` → `icon-blue`, `{purple}` → `icon-purple` (Omarchy `magenta`), and so on. The app substitutes slot hexes when it rasterises, so switching Omarchy themes recolours every icon.
- Map extensions with `iconFor(name, kind)`; unknown types fall back to `file`. Images and videos swap to a thumbnail once decoded (see `FileTile`).
- Badges render only at 18px and up. One badge per icon; priority `broken` > `lock` > `cloud` > `link` > `ads`.
- Never recolour a single icon by hand, never put UI glyphs on files, never scale below 16px (use `Icon` instead).
""")

# ------------------------------------------------------------------ Actions
comp("Button", "Actions", 120, """
return h("div", {className:"ef ef-stack"},
  h("div", {style:{display:"flex",gap:8,flexWrap:"wrap"}},
    h(E.Button, {variant:"primary", kbd:["Enter"]}, "Copy here"),
    h(E.Button, {icon:"folder-plus"}, "New folder"),
    h(E.Button, {variant:"ghost", icon:"undo"}, "Undo"),
    h(E.Button, {variant:"danger", icon:"trash"}, "Delete permanently"),
    h(E.Button, {disabled:true}, "Paste")),
  h("div", {style:{display:"flex",gap:8,flexWrap:"wrap"}},
    h(E.Button, {size:"sm", variant:"primary"}, "Mount"), h(E.Button, {size:"sm"}, "Show in folder"), h(E.Button, {size:"sm", variant:"ghost"}, "Dismiss")));
""", """# Button

A 28px text button with an optional leading glyph and trailing key hint; the label says exactly what will happen.

**Provide** `children` (a verb phrase: "Copy here", "Delete permanently", never "OK"), optional `variant` (`primary`, `danger`, `ghost`; default is the bordered secondary), `icon`, `kbd` (key hint shown inside), `size="sm"` (24px, for toasts, banners and panes).

- One `primary` per surface: the action Enter triggers. `danger` only for irreversible actions (permanent delete, clear dirty flag, format), and it is never the Enter default.
- Buttons live in dialog footers, banners, toasts, empty states and panes. The toolbar uses `IconButton`.
- Hover adds `state-hover`, press `state-press` (Omarchy's 8% / 22% foreground layers); primary brightens instead. Focus draws a 2px `focus-ring` outline, 1px off the edge.
- Disabled uses `opacity-disabled` and keeps its label, so people can still read what is unavailable.
""")

comp("IconButton", "Actions", 76, """
return h("div", {className:"ef ef-row-flex"},
  h(E.IconButton, {icon:"arrow-left", label:"Back (Alt+Left)"}),
  h(E.IconButton, {icon:"arrow-right", label:"Forward", disabled:true}),
  h(E.IconButton, {icon:"arrow-up", label:"Parent folder"}),
  h(E.IconButton, {icon:"sidebar", label:"Preview pane", pressed:true}),
  h(E.IconButton, {icon:"eye-off", label:"Show hidden files", pressed:false}),
  h(E.IconButton, {icon:"more-vertical", label:"More"}));
""", """# IconButton

A 28px square glyph button for the toolbar, tab strip, toasts and panes.

**Provide** `icon`, `label` (becomes the tooltip and accessible name; include the shortcut: "Back (Alt+Left)"), optional `pressed` for toggles (preview pane, show hidden, dual pane).

- Rest `ink-muted`, hover `ink-strong` on `state-hover`, pressed toggle `accent-ink` on `state-active`.
- Toolbar order is fixed: Back, Forward, Up, Reload · PathBar · Search · View ▾ · Command palette · Settings.
- Never use for destructive actions without a confirm step or undo toast.
""")

comp("SegmentedControl", "Actions", 76, """
return h("div", {className:"ef ef-row-flex"},
  h(E.SegmentedControl, {label:"View", options:[{value:"list",icon:"list",label:"List"},{value:"grid",icon:"grid",label:"Grid"},{value:"columns",icon:"columns",label:"Dual pane"}]}),
  h(E.SegmentedControl, {label:"Density", options:[{value:"c",text:"Compact"},{value:"d",text:"Default"},{value:"f",text:"Comfy"}], value:"d"}));
""", """# SegmentedControl

Two to four mutually exclusive options in one well: view mode, density, sort scope.

**Provide** `options` (`{value, icon?, text?, label}`), `value` or uncontrolled default, `onChange`, `label` for the group.

- The selected segment lifts to `bg-raised` with `accent-ink` content; others stay `ink-muted`. The switch is instant (`dur-fast` colour only, no sliding pill).
- Icon-only segments must carry `label`. More than four options → a menu.
""")

comp("Switch", "Actions", 130, """
return h("div", {className:"ef ef-stack", style:{maxWidth:360}},
  h(E.Switch, {id:"sw1", defaultChecked:true}, "Show hidden files"),
  h(E.Switch, {id:"sw2"}, "Show Windows system files"),
  h(E.Switch, {id:"sw3", defaultChecked:true}, "Mount C: read-only"));
""", """# Switch

An on/off setting that applies immediately, with its label on the left.

**Provide** `children` (the setting, phrased as the on state), `checked`/`defaultChecked`, `onChange`.

- Track `bg-deep` → `accent` when on; thumb slides 14px in `dur-base`. Use in Settings and the More menu, never inside dialogs that have a confirm button (use `Checkbox` there).
""")

comp("Checkbox", "Actions", 118, """
return h("div", {className:"ef ef-stack"},
  h(E.Checkbox, {id:"c1", defaultChecked:true}, "Apply to all 12 conflicts"),
  h(E.Checkbox, {id:"c2"}, "Hidden"),
  h(E.Checkbox, {id:"c3", checked:"mixed"}, "Read-only (2 of 5 files)"));
""", """# Checkbox

A 16px box for choices confirmed later (dialogs) and Windows attributes in Properties.

**Provide** `children`, `checked` (`true`, `false` or `"mixed"` for multi-selection), `onChange`.

- Checked fills `accent` with an `on-accent` tick that draws in `dur-fast`. Mixed shows a dash. The whole label is the hit target.
""")

comp("Kbd", "Actions", 64, """
return h("div", {className:"ef ef-row-flex"},
  h(E.Kbd, {keys:["Ctrl","L"]}), h(E.Kbd, {keys:["F2"]}), h(E.Kbd, {keys:["Shift","Del"]}), h(E.Kbd, {keys:["Space"]}), h(E.Kbd, {keys:["Ctrl","K"]}));
""", """# Kbd

A key-hint chip; EchoFiles teaches its shortcuts wherever an action appears.

**Provide** `keys` in press order, Linux names (`Ctrl`, `Shift`, `Alt`, `Super`, `Del`, `Enter`), never macOS symbols.

- Appears in menus, the command palette, tooltips, primary buttons and empty states. `caption` type, `ink-muted` on `bg-deep`, 2px bottom border.
""")

# ------------------------------------------------------------------ Inputs
comp("TextField", "Inputs", 176, """
return h("div", {className:"ef ef-stack", style:{maxWidth:380}},
  h(E.TextField, {id:"t1", label:"Folder name", defaultValue:"Invoices 2026"}),
  h(E.TextField, {id:"t2", label:"Name on AVS (D:)", defaultValue:"notes: draft?.md", error:"Windows can't store “:” or “?”. Rename to “notes- draft-.md”?"}));
""", """# TextField

A single-line field on the `bg-deep` well with an optional label and inline error.

**Provide** `id`, `label`, value props, optional `icon`, `trailing` element, `error` (a sentence that says what is wrong and the fix).

- Focus swaps the border to `focus-ring` plus a 1px ring; errors use `danger` border and `danger-ink` message.
- Validate Windows names live when the destination is NTFS (reserved names, `: ? * " < > |`, trailing dot or space) and offer the fixed name in the message.
""")

comp("SearchField", "Inputs", 80, """
return h("div", {className:"ef ef-row-flex"},
  h(E.SearchField, {id:"s1"}),
  h(E.SearchField, {id:"s2", defaultValue:"invoice", scope:"in AVS (D:)"}));
""", """# SearchField

Filter-as-you-type for the current folder, with a scope chip once search widens.

**Provide** `id`, `placeholder`, optional `scope` ("in AVS (D:)", "everywhere").

- Typing filters the visible listing instantly (no debounce under 10k items). Press `/` or Ctrl+F to focus; Esc clears then blurs.
- The key hint shows until there is a scope; the scope chip is `accent-soft`.
""")

comp("PathBar", "Inputs", 110, """
return h("div", {className:"ef ef-stack"},
  h(E.PathBar, {segments:[{label:"AVS (D:)",icon:"drive"},{label:"Work"},{label:"2026"},{label:"Invoices"}]}),
  h(E.PathBar, {id:"pb", editing:true, path:"/run/media/aditya/AVS/Work/2026/Invoices"}));
""", """# PathBar

The breadcrumb: clickable segments that turn into a typed path on Ctrl+L or double-click.

**Provide** `segments` (`{label, icon?}`; the first carries the volume's display name and icon), or `editing` with `path`.

- Windows volumes show their drive-letter name ("AVS (D:)"), never `/run/media/…`; the typed mode shows the real path and accepts Windows paths (`D:\\Work`) too.
- Only the newest segment animates (slides 6px in `dur-base`); navigation itself is instant.
- Last segment is `ink-strong` bold; the rest `ink-muted` with hover layers. Overflow collapses middle segments into a `…` menu.
- Ctrl+L or a double-click on the empty part of the bar switches to the typed path (text selected); Enter goes there — a file's path opens its folder with the file selected — and Esc goes back. `D:\\Work` works once EchoFiles knows the letters (they're read from Windows the first time C: is mounted).
- During a drag, segments are drop targets like sidebar places.
""")

# ------------------------------------------------------------------ Navigation
comp("TabStrip", "Navigation", 64, """
return h("div", {className:"ef"}, h(E.TabStrip, {tabs:[{label:"Work",icon:"folder"},{label:"Downloads",icon:"download"},{label:"IMG_2026 exports for the tax office",icon:"image"}], active:0}));
""", """# TabStrip

Tabs across the top of the window on the `bg-deep` strip.

**Provide** `tabs` (`{label, icon}`), `active`.

- The active tab joins the toolbar (`bg`) and carries a 2px `accent` top bar — the same signal as Hyprland's active border. Close buttons appear on hover and on the active tab.
- Ctrl+T new, Ctrl+W close, Ctrl+Tab / Ctrl+Shift+Tab cycle, middle-click a folder (or a sidebar place) to open it in a tab; middle-click a tab to close it; right-click a tab for New tab / Close tab. Tabs never animate position.
- A tab split into two panes carries a small `columns` glyph after its name. Switching to a tab refreshes its folders.
""")

comp("Toolbar", "Navigation", 330, """
return h("div", {className:"ef"}, React.createElement(React.Fragment, null,
  h(E.Toolbar, {segments:[{label:"Home",icon:"home"},{label:"Pictures"},{label:"2026"}], grid:true, viewOpen:true}),
  h("div", {style:{display:"flex", justifyContent:"flex-end", padding:"2px 76px 0 0"}}, h(E.ViewMenu, {grid:true, preview:true}))));
""", """# Toolbar

The 40px navigation row: history, reload, path, search, the View dropdown, command palette and settings.

**Provide** `segments` for the PathBar, optional `scope` (`folder` | `everywhere`), `query`, `grid`, `viewOpen`, `settings`.

- Fixed order: Back, Forward, Up, Reload · PathBar · `SearchScope` · Search · View ▾ · Command palette (⌘ glyph, Ctrl+K) · Settings (gear, Ctrl+,).
- **View ▾** (`ViewButton`) shows only the current layout's glyph (list or grid) and a chevron. It opens `ViewMenu`, right-aligned under it: **Layout** — List (Ctrl+1), Grid (Ctrl+2); **Show** — Dual pane (F3), Preview pane (Space), Hidden files (Ctrl+H). A trailing `accent-ink` check marks what's on; picking an item applies it and closes the menu. The keys keep working without the menu.
- Every button is an `IconButton`: same 28px square, `state-hover` layer and `ink-strong` glyph on hover, `state-press` when pressed, 45% and no hover when disabled (Forward with no history). Every one has a tooltip with its key as a `Kbd`. PathBar segments use the same hover.
- While Everywhere results are showing, the PathBar ends with a bold **Search results** segment; clicking any earlier segment leaves the results.
- No window controls: EchoFiles is tiled by Hyprland and never draws client-side decorations.
""")

comp("Sidebar", "Navigation", 700, """
return h("div", {className:"ef", style:{display:"flex", height:690}},
  h(E.Sidebar, null,
    h(E.SidebarSection, {title:"Linux", world:"linux"},
      h(E.SidebarItem, {icon:"home", label:"Home", active:true}),
      h(E.SidebarItem, {icon:"file", label:"Documents"}),
      h(E.SidebarItem, {icon:"download", label:"Downloads", trail:"3 new"}),
      h(E.SidebarItem, {icon:"image", label:"Pictures", dropTarget:true}),
      h(E.SidebarItem, {icon:"music", label:"Music"}),
      h(E.SidebarItem, {icon:"trash", label:"Trash"})),
    h(E.SidebarSection, {title:"Pinned"},
      h(E.SidebarItem, {icon:"pin", label:"Work"}),
      h(E.SidebarItem, {icon:"pin", label:"EchoFiles_Linux"})),
    h(E.SidebarSection, {title:"Windows", world:"windows", count:3, collapsible:true, defaultOpen:true},
      h(E.DriveItem, {name:"Windows (C:)", state:"readonly", used:76, world:"windows", meta:"NTFS", free:"88 GB free"}),
      h(E.DriveItem, {name:"AVS (D:)", state:"mounted", used:58, world:"windows", meta:"NTFS", free:"66 GB free"}),
      h(E.DriveItem, {name:"AVS (E:)", state:"unmounted", meta:"295 GB · click to mount"}),
      h(E.SidebarItem, {icon:"sliders", label:"All drives"})),
    h(E.SidebarSection, {title:"Phone", world:"phone", collapsible:true, defaultOpen:true},
      h(E.PhoneItem, {name:"Galaxy S24", state:"connected", battery:72, charging:true, keep:true}),
      h(E.SidebarItem, {icon:"folder", label:"Files", indent:true}),
      h(E.SidebarItem, {icon:"image", label:"Photos", indent:true, trail:"32 new"})),
    h(E.SidebarSection, {title:"Network", world:"network", count:1, collapsible:true, action:h(E.IconButton, {icon:"plus", label:"Connect to server (Ctrl+Shift+S)"})},
      h(E.NetworkItem, {name:"Media", protocol:"SMB", where:"nas.local", state:"connected", keep:true}),
      h(E.NetworkItem, {name:"build-box", protocol:"SFTP", where:"me@build-box"}),
      h(E.SidebarItem, {icon:"plus", label:"Connect to server…"}))));
""", """# Sidebar

The place list on `bg-sunken`: Linux, Pinned (when anything is pinned), Windows, **All drives**, Phone, then Network.

**Provide** `Sidebar` > `SidebarSection` (`title`, optional `world`: `linux`, `windows`, `network`, `phone`, optional `count`, optional `action` at the end of the head, `collapsible` + `defaultOpen`) > `SidebarItem` (`icon`, `label`, `active`, `trail`, `dropTarget`, `indent`), `DriveItem`, `NetworkItem` or `PhoneItem` (`keep` = still shown while collapsed).

- **Windows** and **Network** fold. Their head is a 24px row — chevron (`chevron-right` / `chevron-down`, 12px `ink-muted`), world mark, title, count, and Network's `+` — with the standard hover; clicking it opens or closes the section. Both start **collapsed**, and the open/closed state is remembered (`settings.toml` `[sidebar] windows_open`, `network_open`). Collapsed, the section still shows the one place you're in (the drive or share holding the open folder, or All drives while that page is open), so the sidebar always says where you are. Everything the section held (All drives, Connect to server…) folds with it.
- **Settings → Appearance → Sidebar** hides either section entirely (*Windows drives*, *Network places*; both on by default). Hidden, the section's head goes too; Ctrl+Shift+D and Ctrl+Shift+S still reach drives and servers.

- A world is marked by a 6px square in `world-linux`, `world-windows`, `world-network` or `world-phone` — the only place world colours appear besides drive usage bars and network glyphs.
- **Network** lists saved servers (in the order saved) and then live connections made elsewhere (Nautilus, `gio mount`, a link). Its head counts live connections and ends with a `+` that opens `ConnectDialog`; a **Connect to server…** row always closes the section. With nothing saved it says "Windows shares, SSH and FTP servers you connect to appear here."
- Active item: `state-active` ground, `ink-strong` bold label, `accent-ink` glyph. No side rail.
- **Phone** (`world-phone`) holds the paired phone's `PhoneItem` — or **Connect phone** before pairing — with its **Files** and **Photos** indented under it while connected. It folds like Windows and Network but starts open; collapsed, the phone row stays. Settings → Appearance → Sidebar can hide it.
- Sections are data-driven. Drag a file onto an item → `accent-soft` with a 1px `accent` inset; onto the phone row, it's sent to the phone.
- Width `sidebar-width` (236px), resizable 180–360 by dragging its right edge (the edge turns `accent` while dragged), hidden with Ctrl+B; both are remembered in settings.
- **Pinned** folders come from *Pin to sidebar* in any folder's menu and show a `pin` glyph; right-click to unpin.
- Drives are listed on their own — no Windows user folders under them (a PC with several accounts would bury the list). A thin scrollbar sits beside the rows, never over them.
- Every place: click opens, middle-click opens in a new tab, right-click for Open / Open in new tab / Paste into folder / Pin, and files dragged onto it move or copy there. Trash's menu has **Empty Trash**.
""")

comp("DriveItem", "Navigation", 250, """
return h("div", {className:"ef", style:{width:236, padding:8, background:"var(--bg-sunken)"}},
  h(E.DriveItem, {name:"Windows (C:)", state:"readonly", used:76, world:"windows", meta:"NTFS", free:"88 GB free"}),
  h(E.DriveItem, {name:"AVS (D:)", state:"mounted", used:93, world:"windows", meta:"NTFS", free:"11 GB free", active:true}),
  h(E.DriveItem, {name:"AVS (E:)", state:"mounting", meta:"295 GB"}),
  h(E.DriveItem, {name:"Backup", state:"locked", meta:"BitLocker · unlock in v0.2"}),
  h(E.DriveItem, {name:"AVS (F:)", state:"dirty", used:40, world:"windows", meta:"Fast Startup on", free:"Read-only"}));
""", """# DriveItem

A drive in the sidebar: name, state, usage bar and free space in two lines.

**Provide** `name` (display name: alias, else drive letter + label — never the mount path or a raw UUID), `state` (`mounted`, `readonly`, `dirty`, `locked`, `unmounted`, `mounting`), `used` (percent), `world`, `meta`, `free`.

- The state pill leads the second line, before the size, so the two never collide in the 236px sidebar.
- Mounted drives show no pill (it is the normal state). Read-only and Needs check use `warning`, Locked `danger`, Not mounted is neutral and dims the name.
- Clicking an unmounted drive mounts it and opens it; a spinner replaces the pill while udisks works. Usage bars grow once on mount (400ms).
- Usage above 90% turns the bar `warning`, above 97% `danger`.
- With a pill, the second line shows only the free space ("38 GB free"); without one, "61 GB free of 295 GB". The total is always in the tooltip.
- Pills: **Mounting…** (`info`) while udisks works — the password dialog appears if Linux asks for one; **Read-only** (`warning`) for C: and anything mounted read-only; **Needs check** (`warning`) when Windows didn't shut down fully and the drive fell back to read-only; **Locked** (`danger`) for BitLocker; **Not mounted** (neutral).
- Tooltip: the mount path and driver (`/run/media/…/AVS · ntfs3`), or the device when unmounted. Right-click: Open, Open in new tab, **Allow writing…** (C: only, `danger`), **Unmount**, All drives.
""")

comp("NetworkItem", "Navigation", 190, """
return h("div", {className:"ef", style:{width:236, padding:8, background:"var(--bg-sunken)"}},
  h(E.NetworkItem, {name:"Media", protocol:"SMB", where:"nas.local", state:"connected", active:true}),
  h(E.NetworkItem, {name:"Photos", protocol:"SMB", where:"nas.local", state:"connecting"}),
  h(E.NetworkItem, {name:"build-box", protocol:"SFTP", where:"me@build-box"}),
  h(E.NetworkItem, {name:"ftp.example.com", protocol:"FTP", where:"ftp.example.com:2121"}));
""", """# NetworkItem

A network place in the sidebar: its short name, then how and where — "SMB · nas.local", "SFTP · me@build-box".

**Provide** `name` (the saved name, else the share, else the host — never GVfs' `smb-share:server=…` folder), `protocol`, `where` (`user@host:port`, the port only when it isn't the usual one), `state` (`connected`, `connecting`, or nothing for not connected), `active`.

- Connected: the glyph turns `world-network` and a 24px **Disconnect** (`arrow-up`) button sits at the row's end, inside the row so the highlight covers it. Not connected: glyph and name are `ink-muted` and there is no pill — the row itself says it.
- Connecting: a `Connecting…` `info` pill replaces the second line while GVfs works; the sign-in dialog appears if the server asks.
- Click: connected → opens it (SFTP lands in the home folder on the server); not connected → connects and opens; a bare SMB server → `SharesPage`. Right-click: Open / Open in new tab / Disconnect, or Connect; Copy address; Edit address…; Remove from sidebar (saved) or Keep in sidebar (live).
- Glyphs: `network` for SMB, `server` for SFTP and FTP. The tooltip is the full address and its state.
- When a server goes away, GVfs drops the connection: panes inside it go home and a toast says "Lost the connection to Media".
""")

comp("ConnectDialog", "Overlays", 600, """
return h("div", {className:"ef"}, h(E.ConnectDialog, {value:"nas.local:4445/Media", parsed:"ok", describe:"Windows share “Media” on nas.local · port 4445", recent:[["Media on nas.local","smb://nas.local/Media/","network"],["me on build-box","sftp://me@build-box/","server"]], nearby:[["Living Room NAS","SMB · nas.local","network"]]}));
""", """# ConnectDialog

**Connect to server** (Ctrl+Shift+S, the Network `+`, the command palette): pick a protocol, type or paste an address, connect.

**Provide** `protocol`, `value`, `parsed` (`ok` | `error`), `describe` (the parse, in words), `state` (`connecting`), `error` (why the last try failed), `recent`, `nearby`.

- **Protocol** `SegmentedControl`: SMB · SFTP · FTP · FTPS, with what it is beside it ("Windows share"). A scheme typed or pasted (`sftp://…`) switches it; switching rewrites a typed scheme.
- **Address**: one field that takes every form people have — `nas.local/Media`, `\\\\nas\\Media` (UNC), `smb://WORK;alice@nas:4445/Media`, `me@host:/srv` (scp style), `ftp://[::1]:2121`. Without a scheme a chip shows the one that will be used. Passwords in an address are ignored; the sign-in dialog asks.
- Under the field, live: a `success-ink` check and the parse in words ("Windows share “Media” on nas.local · port 4445 · as alice"), or a `warning-ink` reason ("“99999” isn't a port number (1–65535)"). **Connect** is disabled until it parses.
- **Add to sidebar** (on by default) saves the server — never a password — to `settings.toml` `[[network.servers]]`.
- **Recent** (up to 4, one per server) and **On this network** (mDNS/Avahi: `_smb._tcp`, `_sftp-ssh._tcp`, `_ftp._tcp`, with a rescan button): one click connects.
- Connecting: the button reads **Connecting…**; if the server asks for a password the dialog steps aside for `SignInDialog`. Failures stay in the dialog in a `danger-soft` box, in words that say what to do: "Nothing answered on nas:4445. Check the address and port, and that the server is running." / "Couldn't find a server called “nas”…" / "nas has no share called “Media”. Leave the share out to see the ones it has."
- If `gvfs-smb` isn't installed, choosing SMB says how to get it. Esc cancels (a connection already on its way finishes quietly).
""")

comp("SignInDialog", "Overlays", 500, """
return h("div", {className:"ef"}, h(E.SignInDialog, {retry:true}));
""", """# SignInDialog

The server wants a user name and password — asked in EchoFiles' own dialog (EchoFiles serves GVfs' `MountOperation`), never a terminal prompt.

**Provide** `title` ("Sign in to Media on nas.local"), `detail` (GVfs' sentence), `user`, `domain` (SMB only), `anonymous` (the server allows guests), `guest`, `retry`.

- **Registered user / Guest** `SegmentedControl` only when the server allows guests; Guest hides the fields and the button reads **Connect as guest**.
- Fields as the server needs them: User name (pre-filled from the address, else the login name) and Domain (`WORKGROUP`) side by side for SMB; Password below. Focus starts in the first empty field; Tab moves between them, Enter signs in.
- **Remember password in the keyring** (off by default): on saves it permanently through GVfs and the Secret Service; off keeps it until logout, so reconnecting in the same session doesn't ask again.
- A wrong password reopens the dialog with the field in `danger` and "That didn't work. Check the user name and password and try again."
- Questions go through the same channel: an unknown SSH host key shows GVfs' message in a `bg-deep` well with its choices as buttons (**Log In Anyway** primary, **Cancel Login** ghost). Nothing is ever accepted on Enter.
""")

# ------------------------------------------------------------------ Files
comp("FileList", "Files", 400, """
return h("div", {className:"ef"}, h(E.FileList, {files:E.DEMO_FILES, cursor:4}));
""", """# FileList

The virtualised details view: sticky column header and fixed-height rows — the heart of the app.

**Provide** `files` (`{name, kind, size, type, modified, items, selected, cut, hidden, badge, drop}`), `sort` + `desc`, `density` (`compact` 24px, default 28px, `comfortable` 34px), `loading` (skeleton row count), `renaming` (row index).

- Rows never change height at runtime: the Rust list renders only visible rows at a fixed `row` height.
- Selection is `selection` (Omarchy's own); the keyboard cursor is a 1px inset `focus-ring`; both change instantly (`dur-instant`).
- Names are `ink`, extensions `ink-muted`, metadata `meta` style in `ink-muted` with tabular figures. Folders sort first; natural order (`file2` before `file10`).
- Cut items use `opacity-cut`; hidden and Windows Hidden/System files use `opacity-hidden-file` when shown.
- Keyboard: arrows move, Shift extends, Ctrl toggles, type-to-jump, Enter opens, F2 renames, Del trashes, Shift+Del asks, Space previews.
""")

comp("FileRow", "Files", 250, """
var f = function(o){ return h(E.FileRow, {file:o}); };
return h("div", {className:"ef", style:{padding:"8px 0"}},
  f({name:"Invoices", kind:"folder", items:57, type:"Folder", modified:"Today", drop:true}),
  h(E.FileRow, {file:{name:"Report-Q3.xlsx", size:"84 KB", type:"Spreadsheet", modified:"Today 18:22", selected:true, cursor:true}, renaming:true}),
  f({name:"draft-old.md", size:"3 KB", type:"Markdown", modified:"2 Sep", cut:true}),
  f({name:".bashrc", size:"4 KB", type:"Shell script", modified:"12 Jan", hidden:true}),
  f({name:"Taxes-2025.pdf", size:"1.1 MB", type:"PDF", modified:"2 Sep", badge:"cloud"}),
  f({name:"Old Launcher.lnk", size:"1 KB", type:"Shortcut", modified:"4 Mar", badge:"broken"}),
  f({name:"old-notes.txt", size:"2 KB", type:"Text", modified:"1 Jan", removing:true}));
""", """# FileRow

One row of `FileList`, shown here in every state it can take.

**Provide** `file` (see FileList), `renaming`.

- While renaming, a problem with the name (empty, `/`, already exists, or a character Windows can't store on NTFS) shows in a `danger-soft` strip at the bottom of the pane, naming the fix: "Windows doesn't allow ? in names. Press Enter to use “Q3_final.xlsx”." Enter then applies the fixed name.
- Clicking anywhere else in the list commits the rename; Esc cancels it.

- Drop target: `accent-soft` + 1px `accent` inset, the icon pops (`ease-spring`), and a 2px bar fills over 700ms — when full, the folder opens (spring-loading).
- Rename: inline field on `bg-deep` with the stem pre-selected and the extension left out; Enter commits, Esc cancels, Tab moves to the next row.
- Removing: collapses in `dur-base` with `ease-exit`, then an Undo toast appears.
""")

comp("FileGrid", "Files", 330, """
var files = [
 {name:"Screenshots", kind:"folder", icon:"folder-image", sub:"382 items"},
 {name:"IMG_20260914_182233.jpg", selected:true, cursor:true, sub:"4.2 MB", thumb:"data:image/svg+xml;utf8," + encodeURIComponent('<svg xmlns=\\"http://www.w3.org/2000/svg\\" viewBox=\\"0 0 88 66\\"><rect width=\\"88\\" height=\\"66\\" fill=\\"#1f3b64\\"/><circle cx=\\"66\\" cy=\\"18\\" r=\\"8\\" fill=\\"#ffc857\\"/><path d=\\"M0 66 30 30l18 20 12-12 28 28z\\" fill=\\"#2e7d5b\\"/></svg>')},
 {name:"Taxes-2025.pdf", sub:"1.1 MB", badge:"cloud"},
 {name:"main.rs", sub:"12 KB"},
 {name:"Backup.7z", sub:"2.4 GB"},
 {name:"song.flac", sub:"31 MB"},
 {name:"clip.mp4", sub:"88 MB"}];
return h("div", {className:"ef"}, h(E.FileGrid, {files:files}));
""", """# FileGrid

Tiles for picture-heavy folders: 104px cells, 88px thumbnails or 48px colour icons, two-line names.

**Provide** `files` with `thumb` (decoded thumbnail URL/handle), `sub` (size or item count), selection flags.

- The app switches to grid automatically in Pictures, Videos and camera folders, and remembers the choice per folder.
- Thumbnails fade in over the icon (`dur-base`) with no layout shift; they come from the freedesktop cache (128px) and are drawn at 88px.
- Same selection, cursor and drop states as rows.
""")

comp("StatusBar", "Files", 64, """
return h("div", {className:"ef"}, h(E.StatusBar, {count:"1,284", selected:3, selectedSize:"12.4 MB", volume:"Windows (C:) · ntfs3", state:"readonly", free:"88 GB free", task:"Copying 42%"}));
""", """# StatusBar

The 24px footer: counts, selection size, background task and volume facts.

**Provide** `count`, `selected`, `selectedSize`, `task`, `volume` (name + driver), `state`, `free`.

- Numbers in `ink` bold, words in `ink-muted`. The volume's state pill repeats here so read-only is never a surprise.
- Selection size for folders is computed in the background and fills in when ready ("12.4 MB+" until done).
- Also says what's on the clipboard ("3 cut on the clipboard") and how many transfers are running. The right side names the volume: "AVS (D:) · ntfs3 · 66 GB free", or "Linux · btrfs · …".
""")

comp("EmptyState", "Files", 330, """
return h("div", {className:"ef"}, h(E.EmptyState, {title:"This folder is empty", body:"Drop files here, paste with Ctrl+V, or make a folder to get started.", actions:[h(E.Button, {key:1, icon:"folder-plus", kbd:["Ctrl","Shift","N"]}, "New folder"), h(E.Button, {key:2, icon:"paste", variant:"ghost"}, "Paste")]}));
""", """# EmptyState

What a view says when there is nothing to show: an empty folder, no search results, a locked or unmounted drive.

**Provide** `icon` (colour icon, 64px), `title` (`display` style, states the fact), `body` (one or two sentences on what to do next), `actions` (at most two buttons with key hints).

- Variants: empty folder, "No matches for “invoice” in AVS (D:)" with *Search everywhere*, "Windows (C:) is locked with BitLocker", "AVS (E:) isn't mounted" with *Mount*.
- Never decorative illustrations; the colour icon is the only picture.
""")

comp("Skeleton", "Files", 210, """
return h("div", {className:"ef", style:{paddingTop:8}}, h(E.Skeleton, {rows:6}));
""", """# Skeleton

Placeholder rows for a folder that takes longer than 150ms to list (cold NTFS, huge directories).

**Provide** `rows` (fill the viewport).

- Never shown before 150ms — fast folders paint straight away with no flash. Names from the first batch replace skeleton rows in place.
- Shimmer loops once per `dur-ambient` (1.4s) and stops under reduced motion.
""")

# ------------------------------------------------------------------ Drives & status
comp("DriveCard", "Drives & status", 180, """
return h("div", {className:"ef ef-row-flex", style:{alignItems:"stretch"}},
  h(E.DriveCard, {name:"Windows (C:)", icon:"drive", fs:"NTFS · ntfs3 · Windows 11", used:76, world:"windows", free:"88 GB free", total:"376 GB", state:"readonly"}),
  h(E.DriveCard, {name:"AVS (D:)", icon:"drive", fs:"NTFS · ntfs3", used:58, world:"windows", free:"66 GB free", total:"156 GB"}),
  h(E.DriveCard, {name:"Home", icon:"home", fs:"btrfs · zstd:3", used:41, world:"linux", free:"312 GB free", total:"530 GB"}));
""", """# DriveCard

A drive on the Drives overview (Ctrl+Shift+D): large icon, filesystem facts and a 6px usage bar.

**Provide** `name`, `icon`, `fs` (filesystem · driver · detail), `used`, `world`, `free`, `total`, `state`.

- Cards sit in a wrapping row, Linux volumes first, then Windows by drive letter. Hover lifts the border to `line-strong`; no shadow.
""")

comp("UsageBar", "Drives & status", 116, """
return h("div", {className:"ef ef-stack", style:{maxWidth:320}},
  h(E.UsageBar, {value:41, world:"linux"}), h(E.UsageBar, {value:58, world:"windows"}),
  h(E.UsageBar, {value:92, world:"windows"}), h(E.UsageBar, {value:98, world:"windows", large:true}));
""", """# UsageBar

Used space as a thin meter: 3px in the sidebar, 6px (`large`) on cards and in Properties.

**Provide** `value` (0–100), `world` (fill colour) or `color`, `label`.

- Fill `world-linux` / `world-windows`; ≥90% `warning`, ≥97% `danger`. Grows from zero once when a drive mounts.
""")

comp("StatePill", "Drives & status", 76, """
return h("div", {className:"ef ef-row-flex"},
  ["mounted","readonly","dirty","locked","unmounted","mounting","cloud","ads","hidden","system"].map(function(s){ return h(E.StatePill, {key:s, state:s}); }));
""", """# StatePill

A short status word with a 6px square mark; the mark and the word both carry meaning, never colour alone.

**Provide** `state` (`mounted`, `readonly`, `dirty`, `locked`, `unmounted`, `mounting`, `cloud`, `ads`, `hidden`, `system`) or `tone` + `children`.

- Tones map to `success`, `warning`, `danger`, `info`, `accent`, neutral. Text uses the `*-ink` token on its `*-soft` ground (4.5:1 in every theme).
""")

# ------------------------------------------------------------------ Feedback
comp("Banner", "Feedback", 170, """
return h("div", {className:"ef"},
  h(E.Banner, {tone:"warning", actions:[h(E.Button, {key:1, size:"sm"}, "How to fix"), h(E.Button, {key:2, size:"sm", variant:"ghost"}, "Keep read-only")]}, h("strong", null, "Windows (C:) was not shut down fully."), " Opened read-only to protect your files. Restart Windows, or turn off Fast Startup."),
  h(E.Banner, {tone:"info", icon:"cloud"}, h("strong", null, "2 files are OneDrive placeholders."), " They open once Windows has synced them."),
  h(E.Banner, {tone:"danger", icon:"lock", actions:h(E.Button, {size:"sm"}, "Unlock…")}, h("strong", null, "Backup is locked with BitLocker."), " Unlocking arrives in v0.2."));
""", """# Banner

A full-width strip at the top of a folder view for facts about the whole location.

**Provide** `tone` (`info`, `warning`, `danger`), `icon`, `children` (bold fact, then what to do), `actions` (small buttons).

- Used for: dirty / hibernated NTFS (warning), BitLocker (danger), cloud placeholders and WSL hints (info), writing to C: enabled (warning).
- Ground is the `*-soft` token; the glyph is `*-ink`. Banners push content down — they never cover files.
""")

comp("Toast", "Feedback", 230, """
return h("div", {className:"ef ef-stack", style:{alignItems:"flex-end"}},
  h(E.Toast, {title:"AVS (D:) opened read-only"}, "Windows didn't shut down fully (Fast Startup or hibernation)."),
  h(E.Toast, {title:"Couldn't copy “Q3:final.xlsx”", tone:"danger", actions:[h(E.Button, {key:1, size:"sm"}, "Rename to “Q3-final.xlsx”"), h(E.Button, {key:2, size:"sm", variant:"ghost"}, "Skip")]}, "Windows names can't contain “:”."));
""", """# Toast

A floating notice in the bottom-right corner for results of actions.

**Provide** `title` (what happened, past tense), `tone` (`danger`, or default accent), `children` (why), `actions`.

- Border is 2px — `accent`, `success` or `danger` — the Omarchy notification look (Hyprland's active border). Radius follows Hyprland `rounding` (0).
- Enter: rise 12px with `ease-spring` in `dur-slow`; exit: fade at 70%. Stack newest at the bottom, at most three; older collapse into "+2 more".
- Toasts are for news and problems only. A finished action — trash, delete, copy, move, rename, undo, restore, unmount — never announces itself: the files on screen already show it, and Ctrl+Z walks back through rename, move, copy, trash and new folder in order.
- Accent toasts (news: a drive opened read-only, nothing to paste, a cancelled transfer) leave after 4s, paused on hover; error toasts stay until dismissed.
""")

comp("TransferToast", "Feedback", 170, """
return h("div", {className:"ef ef-stack", style:{alignItems:"flex-end"}},
  h(E.TransferToast, {title:"Copying 1,204 files to AVS (D:)", detail:"From ~/Pictures/2026 · IMG_4471.HEIC", value:42, animate:true, meta:"812 MB of 2.4 GB · 94 MB/s", eta:"17 s left", doneTitle:"Copied 1,204 files to AVS (D:)", doneMeta:"Flushed to disk"}));
""", """# TransferToast

Progress for copy, move, delete and extraction: title, current file, bar, throughput and time left.

**Provide** `title`, `detail` (source and current file), `value` (0–100) or `indeterminate` (during planning), `meta` (bytes done · speed), `eta`.

- The bar is linear and fed from the engine's atomic counters at frame rate — never a message per chunk. It appears only for work that takes longer than ~0.4s.
- It leaves the moment the batch `syncfs` completes — the drive is then safe to unplug or reboot from. Only a transfer with failures stays, turned `danger`, until dismissed.
- Pause and Cancel (Esc) are always there; cancel undoes a partial move.
""")

comp("Spinner", "Feedback", 60, """
return h("div", {className:"ef ef-row-flex"}, h(E.Spinner, null), h(E.Spinner, {large:true}), h("span", {className:"ef-muted", style:{display:"inline-flex",gap:8,alignItems:"center"}}, h(E.Spinner, null), "Mounting AVS (E:)…"));
""", """# Spinner

A 14px (or 24px) ring for short waits with no measurable progress: mounting, unlocking, computing a folder size.

**Provide** `label`, `large`.

- Show only after 150ms. Anything longer than a few seconds with measurable work gets a progress bar instead.
""")

comp("Tooltip", "Feedback", 64, """
return h("div", {className:"ef ef-row-flex"}, h(E.Tooltip, {kbd:["Alt","Left"]}, "Back"), h(E.Tooltip, null, "D:\\\\Work\\\\2026\\\\Invoices"), h(E.Tooltip, {kbd:["Ctrl","Shift","."]}, "Show hidden files"));
""", """# Tooltip

A one-line label with its shortcut, after 500ms hover (instant when moving between toolbar items).

**Provide** `children`, optional `kbd`.

- `bg-raised`, 1px `line-strong` border, `caption` type. Truncated filenames show their full name; drives show their full path and filesystem.
""")

# ------------------------------------------------------------------ Overlays
comp("Dialog", "Overlays", 980, """
return h("div", {className:"ef ef-stack"},
  h(E.Dialog, {tone:"danger", title:"Delete 3 items permanently?", icon:"trash-full",
    footer:[h(E.Button, {key:1, variant:"ghost", kbd:["Esc"]}, "Cancel"), h(E.Button, {key:2, icon:"trash"}, "Move to Trash"), h(E.Button, {key:3, variant:"danger", icon:"trash"}, "Delete permanently")]},
    h("p", null, "“Report-Q3.xlsx”, “IMG_20260914_182233.jpg” and 1 more will be erased from AVS (D:). Deleting permanently can't be undone.")),
  h(E.Dialog, {title:"Password needed", icon:"lock", height:260,
    footer:[h(E.Button, {key:1, variant:"ghost", kbd:["Esc"]}, "Cancel"), h(E.Button, {key:2, variant:"primary", icon:"key", kbd:["Enter"]}, "Authenticate")]},
    h("p", null, "Authentication is required to mount AVS (/dev/nvme0n1p6). Enter the password for aditya."),
    h(E.TextField, {id:"pw", type:"password", placeholder:"Password"})),
  h(E.Dialog, {tone:"danger", title:"Allow writing to Windows (C:)?", icon:"shield", height:280,
    footer:[h(E.Button, {key:1, variant:"ghost", kbd:["Esc"]}, "Cancel"), h(E.Button, {key:2, variant:"danger", icon:"unlock"}, "Allow writing")]},
    h("p", null, "This is the drive Windows runs from. Changing files here while Windows is hibernated or using Fast Startup can corrupt it — only continue if Windows was shut down with Restart. EchoFiles goes back to read-only the next time it mounts the drive.")));
""", """# Dialog

A modal for decisions that can't wait: permanent delete, conflicts, unsafe writes, unlocking.

**Provide** `title` (the question, naming the thing), `icon` (colour icon), `children` (consequences + safer alternative), `footer` (buttons: Cancel first, the action last), `tone="danger"` for irreversible actions.

- `bg-raised` panel with a 2px `accent` border (Omarchy popup style; `danger` border when destructive), square corners, `shadow-float`, over `scrim`.
- Enters with scale .98 → 1 in `dur-slow`. Esc cancels. The dangerous button is never the Enter default.
- The dialogs EchoFiles has: **Delete permanently** (Shift+Del; offers Move to Trash; if a drive has no trash it says why and offers only permanent delete), **Empty the Trash?**, **Password needed** (EchoFiles' own polkit prompt for mounting internal drives; a wrong password turns the field `danger` with "That password didn't work"), **Allow writing to Windows (C:)?**, **Properties** (Alt+Enter or the menu: the item's facts only — size, dates, location, permissions or Windows attributes — with a × in the corner, Close and Esc; for several items, how many, their total size counted live and where), `ConflictDialog`, and **Some names won't work on Windows** (lists "a:b.txt → a_b.txt", Enter renames and continues).
""")

comp("ConflictDialog", "Overlays", 440, """
return h("div", {className:"ef"}, h(E.ConflictDialog, {count:12, title:"“Report-Q3.xlsx” already exists"}));
""", """# ConflictDialog

Resolves name collisions during copy and move, side by side, once for all or file by file.

**Provide** `title`, `count` (conflicts in the batch).

- The two files are compared by modified date and size; the newer one is marked in `success-ink`.
- The pre-flight check finds every conflict before any byte is written, so this appears once at the start, never mid-copy.
- Windows-name problems (`:`, `?`, reserved names, trailing dots on NTFS) get their own dialog right after, with the fixed names listed.
- **Keep both** is the Enter default (it loses nothing): the copy becomes "Report-Q3 (2).xlsx". Replace becomes **Merge** when both are folders. Cancel drops the whole transfer. "Do this for all N conflicts" applies the choice to the rest.
""")

comp("ContextMenu", "Overlays", 430, """
return h("div", {className:"ef", style:{padding:16, display:"flex", gap:16, alignItems:"flex-start"}},
  h(E.ContextMenu, {label:"On a folder", items:[
    {icon:"external", label:"Open", kbd:["Enter"], active:true}, {icon:"plus", label:"Open in new tab", kbd:["Ctrl","Enter"]}, {icon:"columns", label:"Open in other pane", kbd:["F3"]},
    "-", {icon:"cut", label:"Cut", kbd:["Ctrl","X"]}, {icon:"copy", label:"Copy", kbd:["Ctrl","C"]}, {icon:"paste", label:"Paste into folder"},
    {icon:"move", label:"Move to", submenu:true}, {icon:"copy", label:"Copy to", submenu:true},
    {icon:"rename", label:"Rename", kbd:["F2"]}, {icon:"link", label:"Copy path", kbd:["Ctrl","Shift","C"]}, {icon:"link", label:"Copy as Windows path", hint:"D:\\\\Work"}, {icon:"pin", label:"Pin to sidebar"},
    "-", {icon:"trash", label:"Move to Trash", kbd:["Del"]}, {icon:"trash", label:"Delete permanently", kbd:["Shift","Del"], danger:true},
    "-", {icon:"info", label:"Properties", kbd:["Alt","Enter"]}]}),
  h(E.ContextMenu, {label:"On empty space", items:[
    {icon:"folder-plus", label:"New folder", kbd:["Ctrl","Shift","N"]}, {icon:"file-plus", label:"New file"}, {icon:"paste", label:"Paste", kbd:["Ctrl","V"]}, {icon:"check", label:"Select all", kbd:["Ctrl","A"]},
    "-", {icon:"grid", label:"View as grid", kbd:["Ctrl","2"]}, {icon:"eye", label:"Show hidden files", kbd:["Ctrl","H"]},
    "-", {icon:"terminal", label:"Open terminal here"}, {icon:"link", label:"Copy path"}, {icon:"pin", label:"Pin to sidebar"}, {icon:"info", label:"Properties", kbd:["Alt","Enter"]}]}));
""", """# ContextMenu

The right-click menu: actions for the selection, grouped and with shortcuts.

**Provide** `items`: `{icon, label, kbd?, hint?, submenu?, danger?, disabled?}`, `"-"` for separators, `{heading}` for group labels.

- On items — Open (Open in new tab / other pane for a folder, Open with… for a file) · Cut, Copy, Paste into folder, **Move to ▸** / **Copy to ▸** (other pane, Home folders, pinned folders, mounted drives), Rename, Copy path, Copy as Windows path (NTFS only), Pin to sidebar · Move to Trash, Delete permanently · Properties. In the Trash: Restore, Delete permanently.
- On empty space — New folder, New file, Paste, Select all · View as grid/list, Show hidden files · Open terminal here, Copy path, Pin to sidebar, Properties. In the Trash: **Empty Trash**.
- Right-clicking an unselected item selects it first. The Menu key opens it for the cursor item. Arrow keys move, Enter picks, hovering an item with ▸ opens its submenu beside it.
- Highlight follows Omarchy's menu: `state-hover` ground, 1px `line` border, `accent-ink` label and glyph. Panel `bg-raised`, 1px `line-strong`, square corners.
- Opens with a 4px drop in `dur-base`; closes instantly.
""")

comp("CommandPalette", "Overlays", 460, """
return h("div", {className:"ef", style:{padding:16}}, h(E.CommandPalette, {id:"cp", query:"mnt", groups:[
  {heading:"Actions", items:[{icon:"drive", label:"Mount AVS (E:)", hint:"295 GB NTFS"}, {icon:"lock", label:"Mount Windows (C:) read-write", hint:"unsafe"}, {icon:"eye", label:"Toggle hidden files", kbd:["Ctrl","H"]}]},
  {heading:"Go to", items:[{icon:"folder", label:"/mnt/archive/Photos", hint:"recent"}, {icon:"drive", label:"AVS (D:) \\\\ Music"}]},
  {heading:"Settings", items:[{icon:"settings", label:"NTFS driver: ntfs3 → ntfs", hint:"v7.1"}]}]}));
""", """# CommandPalette

Ctrl+K: every action, folder, drive and setting, fuzzy-matched — the fastest way to do anything.

**Provide** `groups` of `{icon, label, hint?, kbd?}` and the current `query`.

- Built like Omarchy's launcher: `>` prompt in `accent-ink`, results with matched letters in `accent-ink` bold, selected row on `state-hover` with a 1px `line` border.
- Groups: **Actions** (new folder, paste, undo, tabs, panes, views, mount / unmount each drive, "Mount Windows (C:) read-write", Empty Trash…), **Go to** (places, pinned, recent folders, mounted drives), **Settings** pages. Within a group the best fuzzy match leads.
- Typing a path (`/` or `~`) switches to folder completion; Enter opens it.
- Drops 8px and fades in over `dur-slow`; Esc closes instantly.
""")

# ------------------------------------------------------------------ Panels
comp("PreviewPane", "Panels", 560, """
return h("div", {className:"ef", style:{display:"flex", justifyContent:"flex-end", height:550}},
  h(E.PreviewPane, {file:{name:"Report-Q3.xlsx"}, props:[["Kind","Spreadsheet"],["Size","84 KB"],["Modified","Today 18:22"],["Created","2 Sep 2026"],["Where","D:\\\\Work\\\\2026"],["Driver","ntfs3"]], attrs:["Archive","Hidden"], windowsPath:true}));
""", """# PreviewPane

The Space-toggled right pane: large preview, facts, Windows attributes and quick actions.

**Provide** `file` (`name`, `thumb` or colour icon), `props` (label/value pairs), `attrs` (Windows attributes when on NTFS), `windowsPath`.

- 280px on `bg-sunken`, slides in 12px over `dur-slow`. Text files show their first 200 lines, images their thumbnail, folders their size and item count (computed in the background).
- Windows files show `Copy Windows path` and their DOS attributes; Linux files show permissions and owner instead.
- Facts: Size (exact bytes too), Modified, Created, Opened, Location, Points to (links, "missing" when broken), Permissions `rw-r--r-- (644)`, Owner · group. Folders: whole-tree size and file count, filling in with a `+` until counted, plus how many items are directly inside.
- On NTFS the Read-only, Hidden, System and Archive attributes are `Checkbox`es that change the file; Compressed, Encrypted (EFS), Sparse, Online-only and Junction are listed.
- Linux files get the matching switches under **PERMISSIONS**: Read-only, Executable (files) and Only you can open it, each a `Checkbox` that changes the mode bits; links show none. Both worlds get **Copy path**; Windows files add **Copy Windows path**.
- With several items selected the header adds "3 selected · 12 MB + folders". Nothing selected: "Select something to see its details." Properties (Alt+Enter, or the menu) opens the `Dialog` popup instead; the pane stays Space's.
""")

# ------------------------------------------------------------------ Search
comp("SearchScope", "Search", 60, """
return h("div", {className:"ef ef-stack"}, h(E.SearchScope, {value:"everywhere"}));
""", """# SearchScope

Where the search box looks: **Folder** filters the open folder as you type; **Everywhere** searches every indexed folder through the index.

**Provide** `value` (`folder` | `everywhere`), `onChange`.

- A `SegmentedControl` just left of the search field. Ctrl+E switches it; the field's placeholder follows ("Search this folder" / "Search everywhere").
- The default comes from Settings → Search & index → Search looks in.
""")

comp("SearchResults", "Search", 300, """
return h("div", {className:"ef", style:{height:290, display:"flex"}}, h(E.SearchResults, null));
""", """# SearchResults

Everywhere results in place of the file list: name, the folder it lives in (`~`-relative), size and date.

**Provide** `hits` (`{name, kind, location, size, date}`); omit for the demo set.

- Answered from the index in milliseconds; with the index off, a live walk of the indexed folders fills the same view. The status bar says which ("from the index" / "live search") and "N matches · showing first 300" when capped.
- ↑/↓ move, Enter opens (folders open in place, files in their app), Alt+Enter / **Show in folder** opens the parent folder with the file selected. Double-click opens.
- Location clips on the right; it never pushes into Size. Rows use the same hover and active layers as `FileRow`.
- No matches: a centred search glyph, "No matches for “…”", and when the index answered, how fresh it is ("updated 20 s ago; new files elsewhere can take up to a minute").
""")

# ------------------------------------------------------------------ Phone
comp("PhoneDevice", "Phone", 520, """
var cap = function(t){ return h("span", {className:"ef-muted", style:{fontSize:11}}, t); };
var col = function(kids){ return h("div", {style:{display:"flex", flexDirection:"column", alignItems:"center", gap:14}}, kids); };
return h("div", {className:"ef", style:{display:"flex", gap:40, padding:"28px 32px", alignItems:"flex-start", flexWrap:"wrap"}},
  col([h(E.PhoneDevice, {key:1, width:190, wallpaper:"aurora"}), h("span", {key:2}, cap("EchoFiles wallpaper"))]),
  col([h(E.PhoneDevice, {key:1, width:190, wallpaper:"photo", photo:2}), h("span", {key:2}, cap("Latest photo"))]),
  col([h(E.PhoneDevice, {key:1, width:190, state:"ringing"}), h("span", {key:2}, cap("Ringing"))]),
  col([h(E.PhoneDevice, {key:1, width:190, state:"away"}), h("span", {key:2}, cap("Not nearby"))]));
""", """# PhoneDevice

The phone, drawn flat like the colour icons — a flat-sided Android phone in the `icon-slate` slots with antenna bands, volume rocker and side key both on the right, thin even bezels and a pin-hole camera — showing a One UI–style lock screen. No gradients, glare or shadow: it recolours with the Omarchy theme like every other icon. It is the hero of `PhoneHub` and the thumbnail in Settings → Phone.

**Provide** `state` (`connected`, `ringing`, `away`), `wallpaper` (`photo` = the phone's newest camera photo, `aurora` = the EchoFiles wallpaper), `photo` (thumbnail handle), `battery`, `time`, `date`, `notice`, `width` (196 on the hub, 34 in Settings).

- The protocol tells EchoFiles the phone's name and type, not its model, so this is one generic modern Android phone — never a picture of a specific brand, and never iPhone-like (no notch or island, no thick rounded chrome band, no centred thin clock).
- Lock screen, Android-style: status bar with notification icons, signal, Wi-Fi and battery; date and weather above a bold **stacked clock** (hours over minutes, left-aligned); an Android notification card ("EchoFiles · now", title, text); in-display fingerprint; phone and camera shortcuts in the corners; a short gesture handle.
- **Latest photo** is the default and makes the drawing feel like *your* phone: the newest picture in `DCIM/Camera`, read from its EXIF thumbnail (no full download), behind a top and bottom vignette so the clock always reads. **EchoFiles** is a flat wallpaper: three flat circles in `world-network`, `accent` and `world-linux` over a fixed near-black screen, so it follows the theme and the white clock always reads.
- Clock, date and battery are live from the phone. The screen text uses the phone's sans (Inter / Roboto), never the app's mono — it is a picture of a phone, not app UI.
- **Ringing** (Ring phone): the device buzzes (a short wiggle every 1.2s) and three `accent` rings pulse out; the notification becomes "Ringing from EchoFiles · Swipe to stop". **Away**: screen off, "Not nearby", 72% opacity, desaturated. All motion stops under reduced motion.
- The app draws it once per state as a vector and caches the bitmap per size; the photo is the only raster in it.
""")

comp("PhoneItem", "Phone", 260, """
var box = function(kids){ return h("div", {className:"ef", style:{width:236, padding:8, background:"var(--bg-sunken)"}}, kids); };
return h("div", {style:{display:"flex", gap:16, flexWrap:"wrap"}},
  box([h(E.PhoneItem, {key:1, state:"connected", name:"Galaxy S24", battery:72, charging:true, active:true}), h(E.SidebarItem, {key:2, icon:"folder", label:"Files", indent:true}), h(E.SidebarItem, {key:3, icon:"image", label:"Photos", indent:true, trail:"32 new"})]),
  box([h(E.PhoneItem, {key:1, state:"connected", name:"Pixel 8", battery:14}), h(E.PhoneItem, {key:2, state:"pairing", name:"Galaxy S24"}), h(E.PhoneItem, {key:3, state:"away", name:"Galaxy S24", seen:"2 h ago"}), h(E.PhoneItem, {key:4, state:"none"})]));
""", """# PhoneItem

The phone in the sidebar's **Phone** section — the one button that opens everything phone.

**Provide** `state` (`none`, `pairing`, `connected`, `away`), `name` (the name set on the phone), `battery`, `charging`, `network` (Wi-Fi name), `seen` (when it was last nearby), `active`.

- **none**: a plain row, `plus` glyph, **Connect phone** — opens `PairPhone`.
- **pairing**: `Pairing…` `info` pill on the second line while the code check runs.
- **connected**: `phone` glyph in `world-phone`, the name, a `BatteryMeter` at the end; second line "Wi-Fi · home" and the battery %. Below it, indented, **Files** and **Photos** (with "32 new" when there are unimported photos) — the phone's two places you browse and drop onto.
- **away**: glyph and name `ink-muted`, "Not nearby · 2 h ago". Files and Photos fold away; the hub still opens with the last-known facts.
- Click opens `PhoneHub`. Drop files on the row → they're sent to the phone. A **Connect another phone** row ends the open section. Files also get **Send to Galaxy S24** in their right-click menu, and the command palette has a **Phone** group (Open, Photos, Files, Messages, Notifications, Ring, Send files…, Connect).
""")

comp("BatteryMeter", "Phone", 80, """
return h("div", {className:"ef ef-row-flex", style:{gap:20}},
  h(E.BatteryMeter, {level:72, charging:true}), h(E.BatteryMeter, {level:54}), h(E.BatteryMeter, {level:18}), h(E.BatteryMeter, {level:6}), h(E.BatteryMeter, {level:100, charging:true}));
""", """# BatteryMeter

The phone's charge as a 20×10 battery with its percentage.

**Provide** `level` (0–100), `charging`, `label={false}` to hide the number (the sidebar shows it on the second line).

- Fill is `ink-muted`; `success` with a `bolt` while charging; `warning` at 20% and below; `danger` at 10% and below. The tooltip says "Battery 18%".
- **Low battery warning** (Settings → Phone) shows one accent `Toast` at 15%: "Galaxy S24 is at 15%".
""")

comp("PhoneHub", "Phone", 840, """
var st = React.useState("connected");
var s = st[0];
return h("div", {className:"ef", style:{background:"var(--bg)"}},
  h("div", {style:{padding:"12px 28px 0"}}, h(E.SegmentedControl, {label:"State", value:s, onChange:st[1], options:[{value:"connected",text:"Connected"},{value:"photo",text:"Latest photo"},{value:"away",text:"Not nearby"},{value:"off",text:"Features off"}]})),
  h(E.PhoneHub, {key:s, state: s === "away" ? "away" : "connected", wallpaper: s === "photo" ? "photo" : "aurora", off: s === "off" ? {messages:true, clipboard:true} : null}));
""", """# PhoneHub

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
""", extra=' width=1100 page')

comp("Gauge", "Phone", 140, """
return h("div", {className:"ef", style:{padding:16, display:"grid", gridTemplateColumns:"repeat(3, 230px)", gap:12, background:"var(--bg-raised)"}},
  h(E.Gauge, {icon:"battery", label:"Battery", value:72, unit:"%", pct:72, color:"var(--success)", sub:"Charging", bolt:true}),
  h(E.Gauge, {icon:"battery", label:"Battery", value:14, unit:"%", pct:14, color:"var(--warning)", sub:"On battery"}),
  h(E.Gauge, {icon:"sd-card", label:"Storage", value:41, unit:" GB free", pct:68, color:"var(--accent)", sub:"of 128 GB"}));
""", """# Gauge

A hub fact as a 56px ring gauge with label, value and one line — used for the phone's battery and storage.

**Provide** `icon`, `label`, `value`, `unit`, `pct` (ring fill), `color`, `sub`, `bolt` (charging).

- 48px ring, 5px stroke on `line`, round cap; it fills once in 400ms like `UsageBar` (none under reduced motion). The glyph sits in the middle, `ink-muted`, or a `success-ink` bolt while charging.
- Card: flat `bg` with a 1px `line` border and `radius-2` — no tint, highlight or shadow. Label in `label` style, value 22px/800, unit 12px muted.
""")

comp("FeatureTile", "Phone", 150, """
return h("div", {className:"ef", style:{padding:16, display:"grid", gridTemplateColumns:"repeat(2, 260px)", gap:10}},
  h(E.FeatureTile, {icon:"image", label:"Photos", sub:"2,481 photos", badge:"32 new"}),
  h(E.FeatureTile, {icon:"message", label:"Messages", sub:"Priya: See you at 7?", badge:"3"}),
  h(E.FeatureTile, {icon:"folder", label:"Files", sub:"Camera, Downloads, WhatsApp…"}),
  h(E.FeatureTile, {icon:"bell", label:"Notifications", off:true}));
""", """# FeatureTile

One phone feature on the hub: tinted glyph, name, one live line, an optional count.

**Provide** `icon`, `label`, `sub`, `badge`, `off`.

- Glyph sits on a 36px `world-phone` 14% square. Hover draws a `world-phone` border. Off: dashed border, muted, "Off · turn on in settings".
""")

comp("ReceivedList", "Phone", 220, """
return h("div", {className:"ef", style:{padding:16, maxWidth:620}}, h(E.ReceivedList, null));
""", """# ReceivedList

Files the phone sent to this laptop, newest first: in progress with a bar and speed, or landed with when and a **Show** button.

**Provide** `items` (`{name, size, progress?, done?, when?}`), `title`.

- Files share from any phone app (Share → KDE Connect → this laptop). With **Accept files automatically** on (the default) they save straight to `~/Downloads/Phone`; name clashes keep both as "name (2).ext", never overwrite. Off, each send asks first with `ReceiveToast`.
- **Open folder** opens the save folder in a tab. Receiving needs EchoFiles running; with the window closed only if Keep running in the background is on.
""")

comp("ClipboardCard", "Phone", 220, """
return h("div", {className:"ef", style:{padding:16, maxWidth:360}}, h(E.ClipboardCard, null));
""", """# ClipboardCard

Shared clipboard on the hub — no page of its own, because syncing is automatic.

**Provide** `on`, `items` (`[direction, text, when]`; `from` = came from the phone, `to` = went to it).

- The switch is the same setting as Settings → Phone → Shared clipboard. The last three items show with an arrow for direction and a copy button to put one back on the clipboard. Text only; passwords copied from a password manager (marked sensitive) are never sent.
""")

comp("PairPhone", "Phone", 560, """
return h("div", {className:"ef"}, h(E.PairPhone, {step:1}));
""", """# PairPhone

**Connect phone**: four steps in one dialog. Nothing KDE is installed on the laptop — EchoFiles speaks KDE Connect itself; the phone runs the stock KDE Connect app.

**Provide** `step` (0–3), `nearby` (`"none"` while nothing is found), `filesOk`. The stepper is clickable in the preview.

1. **Get the app** — Google Play and F-Droid entries; "It's open on my phone".
2. **Choose your phone** — phones announcing KDE Connect on this Wi-Fi, live, one click pairs (EchoFiles re-announces itself every 5 s while the dialog is open). After 10 s with none, a help box: same Wi-Fi and app open · **Allow KDE Connect…** (only when ufw is active; asks for the password once through pkexec and opens ports 1714–1764) · or type the phone's IP and **Connect** (EchoFiles connects to it directly).
3. **Check the code** — the 8-character code in four 26px tiles; the phone shows the same. When we asked: "Waiting for Galaxy S24…" until it's accepted there; **Cancel** tells the phone. When the phone asked (the dialog opens by itself, even from the background): **Pair** (Enter) or **Codes don't match**.
4. **Allow files** — the one switch on the phone (Plugin settings → Filesystem expose, then All files access). A pill re-checks on its own: `warning` "Files not shared yet" → `success` "Files available". **Open Galaxy S24** lands on the hub.

- Pairing is stored as the phone's certificate in `~/.config/echofiles/phone/trusted/`. Several phones can be paired; the hub shows the one picked in the sidebar.
""")

comp("ReceiveToast", "Phone", 260, """
return h("div", {className:"ef ef-stack", style:{alignItems:"flex-end"}},
  h(E.ReceiveToast, null),
  h(E.TransferToast, {title:"Receiving 3 files from Galaxy S24", detail:"To ~/Downloads/Phone · IMG_2041.jpg", value:38, animate:true, icon:"download", meta:"4.6 of 12 MB · 18 MB/s", eta:"1 s left", doneTitle:"Received 3 files from Galaxy S24", doneMeta:"In ~/Downloads/Phone"}));
""", """# ReceiveToast

What appears when the phone sends files.

**Provide** `from`, `what`.

- **Accept files automatically on** (default): no question — a `TransferToast` "Receiving 3 files from Galaxy S24" for sends longer than ~0.4s, and the files appear in `ReceivedList`.
- **Off**: this toast asks first — "Galaxy S24 wants to send 3 files · 12 MB", the names and where they'll go, **Accept** (`primary`) and **Decline**. It stays until answered; declining tells the phone.
""")

comp("PhotosPage", "Phone", 760, """
return h("div", {className:"ef", style:{height:750, overflow:"auto", background:"var(--bg)"}}, h(E.PhotosPage, null));
""", """# PhotosPage

Every photo and video on the phone in one timeline, newest first, grouped by day.

**Provide** `selected` (keys); the demo is clickable.

- Gathered from Camera, Screenshots, Pictures and messaging media (WhatsApp, Telegram). The `SegmentedControl` filters: All · Camera · Screenshots · WhatsApp.
- Square tiles, 4px gaps, 118px minimum. Thumbnails come from each JPEG's embedded EXIF thumbnail (a 64 KB range read, never the full file) and HEIC's thumbnail item; videos show a placeholder with their length until a frame is grabbed. Cached per phone.
- Click selects (a round check, top left, `accent` inset ring and the photo shrinks 10%); double-click opens it. With a selection, a sticky `accent-soft` bar: "3 selected · 11.8 MB", **Save to…** (any Linux or Windows folder), **Copy**, **Clear**. Drag photos out to any folder.
- **Import 32 new** (`primary`) copies only photos not imported before (matched by name, size and date) into dated folders, `~/Pictures/Phone/2026-10/`. One-way: nothing is deleted on the phone.
""", extra=' width=1100 page')

comp("MessagesPage", "Phone", 600, """
return h("div", {className:"ef", style:{height:590, background:"var(--bg)"}}, h(E.MessagesPage, null));
""", """# MessagesPage

Texts from the phone: conversations left, the open chat right, a reply box that sends through the phone.

**Provide** nothing; the list is clickable.

- Conversation rows: initial avatar (contact colour from the icon slots), name, last message, time, unread count in `accent`. Search filters names and text.
- Bubbles: theirs `bg-raised` with a hairline, yours `accent` with `on-accent` text; 14px radius with the tail corner squared. A day chip separates days.
- **Send** (Enter) sends a normal SMS from the phone; the line under the box says so. Loaded when the page opens (the phone sends the threads on request), so it works without a background service.
- Off in Settings → Phone → Messages (the default): no texts reach the laptop at all.
""", extra=' width=1100 page')

comp("NotificationsPage", "Phone", 560, """
return h("div", {className:"ef", style:{height:550, overflow:"auto", background:"var(--bg)"}}, h(E.NotificationsPage, null));
""", """# NotificationsPage

The phone's current notifications, grouped by app.

**Provide** `mode` (`app` | `desktop`).

- Each app is one bordered group: app initial on its colour, name, count; rows with title, text, time and a × that dismisses on the phone too. Apps that allow quick replies (WhatsApp, Telegram, Messages) get a reply field on the newest one.
- **Settings → Phone → Phone notifications**: **Off** (nothing comes over), **In app** (only this page — the default; the subtitle says so), **Desktop too** (also pops up as a normal desktop notification while EchoFiles runs; with Keep running in the background on, even with the window closed). Apps in *Never show notifications from* are dropped on arrival.
- **Dismiss all** clears them here and on the phone.
""", extra=' width=1100 page')

comp("PhoneSettings", "Phone", 1180, """
return h(E.PhoneSettings, {height:1170});
""", """# PhoneSettings

**Settings → Phone** (also the gear on the hub): what this phone may do.

**Provide** `background` (Keep running in the background is on).

- **This phone** — thumbnail, name, "Connected over Wi-Fi · paired 2 Oct 2026", **Forget phone** (removes the pairing on both sides); **Phone picture**: Latest photo / EchoFiles.
- **Features** — Files and photos (on), Shared clipboard (on), Messages (off until turned on; the phone asks for SMS permission), Low battery warning (on). Turning one off stops it on both devices: EchoFiles stops asking and drops anything the phone still sends.
- **Receiving files** — **Accept files automatically** (on by default; off asks each time with `ReceiveToast`), **Save to** `~/Downloads/Phone` with **Change…**.
- **Notifications** — Off / **In app** / Desktop too, with a note when Keep running in the background is off (pop-ups then stop with the window); **Never show notifications from**: chips of app names with an add field.
- With no phone paired the page shows one row: "No phone yet" and **Connect phone**.
- Saved to `settings.toml` `[phone]` (features, receive dir, notification mode, muted apps); the pairing itself lives in `~/.config/echofiles/phone/trusted/`.
""", extra=' width=1100 page')

comp("PhoneWindow", "Phone", 780, """
var st = React.useState("hub");
return h("div", null,
  h("div", {className:"ef", style:{padding:"8px 0 10px"}}, h(E.SegmentedControl, {label:"Page", value:st[0], onChange:st[1], options:[{value:"hub",text:"Hub"},{value:"photos",text:"Photos"},{value:"messages",text:"Messages"},{value:"notifications",text:"Notifications"}]})),
  h(E.PhoneWindow, {key:st[0], view:st[0], height:760}));
""", """# PhoneWindow

The whole app with the phone open — the reference composition for every phone screen.

**Provide** `view` (`hub`, `photos`, `messages`, `notifications`), `state`, `wallpaper`.

- The **Phone** section sits between Windows and Network, marked `world-phone`, collapsible like the others (open by default; collapsed it still shows the phone row).
- Phone pages open in the pane like folders: back/forward work, the breadcrumb reads "Galaxy S24 › Photos", middle-click opens one in a new tab. Files is the normal file list rooted at the phone's important folders, with a *Show all storage* switch.
""", extra=' width=1200 page')

# ------------------------------------------------------------------ Settings
comp("SettingsGroup", "Settings", 200, """
return h("div", {className:"ef ef-stack", style:{maxWidth:720}},
  h(E.SettingsGroup, {title:"Running"},
    h(E.SettingRow, {label:"Keep running in the background", description:"Closing the window hides EchoFiles instead of quitting. The next window opens instantly and search stays up to date."}, h(E.Switch, {defaultChecked:true, label:"Keep running"})),
    h(E.SettingRow, {label:"Start at login", description:"Needs Keep running in the background.", disabled:true}, h(E.Switch, {label:"Start at login"}))));
""", """# SettingsGroup

An uppercase `label` heading above one bordered box (`bg-raised`, 1px `line`, radius 2) of rows separated by hairlines.

**Provide** `title` and rows as children: `SettingRow`, `SettingBlock`, `IndexStatus`, `PathListEditor`, `NameChips`.

- Groups are separated by 28px; a page is 2–5 groups. Never nest groups.
""")

comp("SettingRow", "Settings", 90, """
return h("div", {className:"ef ef-stack", style:{maxWidth:720}}, h("div", {className:"ef-setbox"}, h(E.SettingRow, {label:"Skip cache folders", description:"Folders marked with CACHEDIR.TAG (browser, build and package caches). They're still listed by name."}, h(E.Switch, {defaultChecked:true, label:"Skip cache folders"}))));
""", """# SettingRow

One setting: label (`ink-strong`, 13px) and a one- or two-sentence description (`ink-muted`, 12px) on the left; the control on the right, vertically centred. 12px × 16px padding.

**Provide** `label`, `description`, `disabled`, and the control as `children` (`Switch`, `SegmentedControl`, a `Button`, or a status pill).

- The description says what the setting does and its consequence ("Turning it off deletes the index"), not how it works.
- A setting that depends on another is shown `disabled` (45%) with the dependency as its description ("Needs Keep running in the background.") — never hidden.
""")

comp("PathListEditor", "Settings", 230, """
return h("div", {className:"ef ef-stack", style:{maxWidth:720}},
  h(E.SettingsGroup, {title:"Indexed folders"},
    h(E.SettingBlock, {muted:true}, "Everywhere search and ef cover these folders and everything inside them."),
    h(E.PathListEditor, {id:"p1", paths:["~", "~/Work/Clients"], current:"~/Projects/EchoFiles_Linux", draft:"~/Privte", error:"“~/Privte” isn't a folder on this computer. Check the spelling, or open the folder and use Add current folder."})));
""", """# PathListEditor

Rows of folder paths inside a `SettingsGroup`, each with a remove ×, then an add row: field, **Add**, and ghost **Add current folder** (the fastest way to add what you're looking at).

**Provide** `paths` (shown `~`-relative), `icon`, `empty`, `placeholder`, `current`, `error`, `note` (per-row pill, e.g. "Missing").

- Validate on Add and Enter: a full path (`/` or `~/`), an existing folder, not a duplicate. The error sits under the field with an `error` glyph in `danger-ink`, and the field border turns `danger`.
- Indexed folders can't go empty: removing the last one says "Keep at least one folder — or turn the search index off instead."
- Changes save immediately and schedule a re-index 1.5 s later.
""")

comp("NameChips", "Settings", 170, """
return h("div", {className:"ef ef-stack", style:{maxWidth:720}}, h(E.SettingsGroup, {title:"Skip contents of"}, h(E.NameChips, {id:"n1", names:["node_modules", ".git", ".cache", "__pycache__", ".cargo", ".venv", ".gradle", ".npm"]})));
""", """# NameChips

Folder *names* (not paths) whose contents the index skips, as chips with a remove ×, then an add row with **Reset to recommended**.

**Provide** `names`.

- Chips are 24px on `bg-deep` with a 1px `line-strong` border; the × is a 24px hit target with the standard hover.
- Names never contain `/`: the field says a path belongs in **Never show in search**.
""")

comp("IndexStatus", "Settings", 260, """
return h("div", {className:"ef ef-stack", style:{maxWidth:720}}, h("div", {className:"ef-setbox"},
  ["ready","updating","building","problem","off"].map(function(s){ return h(E.IndexStatus, {key:s, state:s}); })));
""", """# IndexStatus

The search index's health in one row: a `StatePill`, what it means in numbers, and **Rebuild now**.

**Provide** `state` (`off` | `opening` | `building` | `updating` | `ready` | `problem`), optional `detail`.

- Ready: "179,581 items · 13 MB on disk · updated 21 s ago" — counts, size and freshness, live.
- Building/Updating: `info` pill, the button reads "Indexing…" and is disabled. Problem: `warning` pill and the reason with the folder named.
- Sits directly under the Search index switch in Settings → Search & index.
""")

comp("CommandList", "Settings", 170, """
return h("div", {className:"ef ef-stack", style:{maxWidth:720}}, h(E.CommandList, {items:[["ef find report","names containing “report”"],["ef find '*' --ext pdf --limit 50","every PDF, first 50"],["ef status","what's indexed, how fresh"],["ef index","update the index now"]]}));
""", """# CommandList

Commands or key bindings with what they do, on the `bg-deep` well — AI agents page and About → Keyboard.

**Provide** `items` (`[command, meaning]`).
""")

comp("SettingsNav", "Settings", 220, """
return h("div", {className:"ef", style:{display:"flex", height:210}}, h(E.SettingsNav, {page:"search"}));
""", """# SettingsNav

The Settings page list, in the sidebar's place: General · Search & index · AI agents · Phone · Appearance · About.

**Provide** `page`, `onChange`.

- 236px on `bg-sunken`, 32px rows with a glyph; the current page uses the sidebar's active style (`state-active`, `accent-ink` glyph, bold).
""")

comp("SharesPage", "Pages", 380, """
return h("div", {className:"ef"}, h(E.SharesPage, {host:"nas.local"}));
""", """# SharesPage

An SMB server's shares, when an address names no share (`smb://nas.local`, a server found nearby, a saved bare server).

**Provide** `host`, `shares` (`[name, state]`), `state` (`loading`).

- Shown in the pane like the Drives page: breadcrumb and tab carry the server with a `network` glyph; the status bar counts shares. Hidden `$` shares (IPC$, ADMIN$) are left out.
- Cards: `folder-share` icon, name, and **Connected** (`success`) / **Connecting…** (`info`) / "Windows share". Hover draws a `world-network` border. Click connects and opens.
- Asking the server may need a sign-in first (`SignInDialog`). Errors show in words with **Try again**.
""", extra=' width=1100 page')

comp("SettingsPage", "Pages", 900, """
return h(E.SettingsPage, {height:880});
""", """# SettingsPage

Settings is its own screen (Ctrl+, or the gear): a top bar with a ghost **← Files** button and `Esc` `Kbd`, the title and a live **Saved** mark; `SettingsNav` on the left; one page of `SettingsGroup`s in a centred 720px column. Esc or ← Files returns to the same folder.

**Provide** optional `page`; the nav is live in the preview.

- **General** — Running: Keep running in the background (close hides the window; later launches reuse it) · Start at login (disabled unless running in the background). Windows: New windows open at Home / Last folder · Show hidden files.
- **Search & index** — Index: Search index switch (off deletes the index) + `IndexStatus`. Search box: Search looks in (This folder / Everywhere). Indexed folders and Never show in search (`PathListEditor`). Skip contents of: Skip cache folders + `NameChips`.
- **AI agents** — Allow EchoFiles commands (when off, `ef` exits 3 with "turned off"), ef location, Teach AI agents about ef (links the skill into ~/.claude/skills; off removes only that link), `CommandList` of examples. A `warning` note appears if `ef` is allowed but the index is off.
- **Phone** — see `PhoneSettings`: this phone and Forget, phone picture, feature switches, Accept files automatically + Save to, notifications Off / In app / Desktop too and muted apps.
- **Appearance** — the Omarchy theme's name and swatches (it follows the system); Row height Compact / Default / Comfortable (24 / 28 / 34px); Folders open as Automatic / List / Grid (resets per-folder choices). Sidebar: show or hide the Windows drives, Phone and Network places sections.
- **About** — version, settings file and index folder with **Show**, keyboard shortcuts.
- Everything saves to `~/.config/echofiles/settings.toml` the moment it changes (shared with `ef`). Only settings that work today appear.
""", extra=' width=1100 page')

# ------------------------------------------------------------------ Pages
comp("AppWindow", "Pages", 700, """
return h(E.AppWindow, {height:690, animate:true});
""", """# AppWindow

The whole app assembled: tabs, toolbar, sidebar, banner, list, preview pane, status bar and a running transfer.

**Provide** nothing; this is the reference composition every screen is checked against.

- Window chrome is only a 2px `accent` border — Hyprland's active border. No title bar, no window buttons.
- Surfaces step from `bg-deep` (tabs, status) → `bg` (toolbar, list) → `bg-sunken` (sidebar, preview) → `bg-raised` (floating things). Nothing else is layered.
""", extra=' width=1200 page')

comp("DualPane", "Pages", 460, """
return h(E.DualPane, {height:450});
""", """# DualPane

Two folders side by side (F3) — the fastest way to move files between Linux and Windows.

**Provide** nothing; reference composition.

- The active pane has a 2px `accent` top edge and `bg` ground; the inactive pane drops to `bg-sunken`. Tab switches panes.
- F5 copies the selection to the other pane, F6 moves it, both through the same pre-flight and progress as drag and drop. Each pane head carries its world mark and its location; the toolbar and status bar follow the active pane.
- Clicking in a pane makes it active. Only the active pane takes keys. Narrow panes drop the Kind column first, then Modified, so names keep their room.
""", extra=' width=1100 page')

comp("MotionSpec", "Pages", 700, """
return h(E.MotionSpec, null);
""", """# MotionSpec

Every animation in EchoFiles, playable, with its duration and easing.

**Provide** nothing; reference.

- Rule 1: motion never delays input. Navigation, selection, sorting and typing are instant; only the UI's response to them animates.
- Rule 2: everything is interruptible and respects `prefers-reduced-motion` and Hyprland `animations:enabled = false` (all durations become 0).
""", extra=' page')

# ------------------------------------------------------------------ build
def preview(name):
    group, height, extra, body, _ = C[name]
    return (f'<!-- @dsCard group="{group}" height={height}{extra} -->\n<!doctype html>\n<html>\n<head><meta charset="utf-8"><title>{name}</title></head>\n'
            f'<body>\n<div id="root"></div>\n<script>\n(function(){{\nvar E = window.Echo, h = React.createElement;\n'
            f'function App(){{{body}}}\nReactDOM.createRoot(document.getElementById("root")).render(h(App));\n}})();\n</script>\n</body>\n</html>\n')

def main():
    comps = PRJ / "components"
    if comps.exists():
        for d in comps.iterdir():
            if d.is_dir() and d.name not in C and d.name != "Cover" and d.name != "lib":
                shutil.rmtree(d)
    for name, (_, _, _, _, readme) in C.items():
        d = comps / name
        d.mkdir(parents=True, exist_ok=True)
        (d / "preview.html").write_text(preview(name))
        (d / "README.md").write_text(readme.lstrip())
    icons = (SYS / "icons.generated.js").read_text()
    body = (SRC / "components.js").read_text()
    header = {"format": 4, "namespace": "Echo", "components": [{"name": n} for n in C]}
    bundle = "/* @ds-bundle: " + json.dumps(header, separators=(",", ":")) + " */\n(function(){\n" + icons + body + "\n})();\n"
    for bad in ("</script", "<!--"):
        assert bad not in bundle.lower(), bad
    (comps / "bundle.js").write_text(bundle)
    shutil.copy(SRC / "bundle.css", comps / "bundle.css")
    shutil.copy(SRC / "index.d.ts", comps / "index.d.ts")
    print(f"{len(C)} components, bundle {len(bundle)//1024} KB")

if __name__ == "__main__":
    main()
