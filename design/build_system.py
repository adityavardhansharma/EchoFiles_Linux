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
- Toolbar order is fixed: Back, Forward, Up · PathBar · Search · View · Preview · More.
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
""")

# ------------------------------------------------------------------ Navigation
comp("TabStrip", "Navigation", 64, """
return h("div", {className:"ef"}, h(E.TabStrip, {tabs:[{label:"Work",icon:"folder"},{label:"Downloads",icon:"download"},{label:"IMG_2026 exports for the tax office",icon:"image"}], active:0}));
""", """# TabStrip

Tabs across the top of the window on the `bg-deep` strip.

**Provide** `tabs` (`{label, icon}`), `active`.

- The active tab joins the toolbar (`bg`) and carries a 2px `accent` top bar — the same signal as Hyprland's active border. Close buttons appear on hover and on the active tab.
- Ctrl+T new, Ctrl+W close, Ctrl+Tab cycle, middle-click a folder to open it in a tab. Tabs never animate position.
""")

comp("Toolbar", "Navigation", 64, """
return h("div", {className:"ef"}, h(E.Toolbar, {segments:[{label:"Home",icon:"home"},{label:"Pictures"},{label:"2026"}]}));
""", """# Toolbar

The 40px navigation row: history, path, search, view and panes.

**Provide** `segments` for the PathBar, optional `scope`, `view`, `preview`.

- Fixed order, no customisation in v0.1. Everything here is also in the command palette.
- No window controls: EchoFiles is tiled by Hyprland and never draws client-side decorations.
""")

comp("Sidebar", "Navigation", 600, """
return h("div", {className:"ef", style:{display:"flex", height:590}},
  h(E.Sidebar, null,
    h(E.SidebarSection, {title:"Linux", world:"linux"},
      h(E.SidebarItem, {icon:"home", label:"Home", active:true}),
      h(E.SidebarItem, {icon:"file", label:"Documents"}),
      h(E.SidebarItem, {icon:"download", label:"Downloads", trail:"3 new"}),
      h(E.SidebarItem, {icon:"image", label:"Pictures", dropTarget:true}),
      h(E.SidebarItem, {icon:"music", label:"Music"}),
      h(E.SidebarItem, {icon:"trash", label:"Trash"})),
    h(E.SidebarSection, {title:"Windows", world:"windows", count:3},
      h(E.DriveItem, {name:"Windows (C:)", state:"readonly", used:76, world:"windows", meta:"NTFS", free:"88 GB free"}),
      h(E.DriveItem, {name:"AVS (D:)", state:"mounted", used:58, world:"windows", meta:"NTFS", free:"66 GB free"}),
      h(E.DriveItem, {name:"AVS (E:)", state:"unmounted", meta:"295 GB · click to mount"}),
      h(E.SidebarItem, {icon:"user", label:"Aditya (C:\\\\Users)"})),
    h(E.SidebarSection, {title:"Pinned"},
      h(E.SidebarItem, {icon:"star", label:"Work", trail:"D:"}))));
""", """# Sidebar

The place list on `bg-sunken`, grouped into worlds: Linux, Windows, later Phone, then Pinned.

**Provide** `Sidebar` > `SidebarSection` (`title`, optional `world`: `linux`, `windows`, `phone`, optional `count`) > `SidebarItem` (`icon`, `label`, `active`, `trail`, `dropTarget`) or `DriveItem`.

- A world is marked by a 6px square in `world-linux`, `world-windows` or `world-phone` — the only place world colours appear besides drive usage bars.
- Active item: `state-active` ground, `ink-strong` bold label, `accent-ink` glyph. No side rail.
- Sections are data-driven (the phone section plugs in later). Drag a file onto an item → `accent-soft` with a 1px `accent` inset.
- Width `sidebar-width` (236px), resizable 180–360, collapses with Ctrl+B in `dur-base`.
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

- Mounted drives show no pill (it is the normal state). Read-only and Needs check use `warning`, Locked `danger`, Not mounted is neutral and dims the name.
- Clicking an unmounted drive mounts it and opens it; a spinner replaces the pill while udisks works. Usage bars grow once on mount (400ms).
- Usage above 90% turns the bar `warning`, above 97% `danger`.
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
  h(E.Toast, {title:"Moved 3 items to Trash", tone:"success", actions:[h(E.Button, {key:1, size:"sm", kbd:["Ctrl","Z"]}, "Undo")]}),
  h(E.Toast, {title:"Couldn't copy “Q3:final.xlsx”", tone:"danger", actions:[h(E.Button, {key:1, size:"sm"}, "Rename to “Q3-final.xlsx”"), h(E.Button, {key:2, size:"sm", variant:"ghost"}, "Skip")]}, "Windows names can't contain “:”."));
""", """# Toast

A floating notice in the bottom-right corner for results of actions.

**Provide** `title` (what happened, past tense), `tone` (`success`, `danger`, or default accent), `children` (why, for errors), `actions`.

- Border is 2px — `accent`, `success` or `danger` — the Omarchy notification look (Hyprland's active border). Radius follows Hyprland `rounding` (0).
- Enter: rise 12px with `ease-spring` in `dur-slow`; exit: fade at 70%. Stack newest at the bottom, at most three; older collapse into "+2 more".
- Success toasts auto-dismiss after 4s (paused on hover); error toasts stay until dismissed. Every destructive action gets an Undo toast.
""")

comp("TransferToast", "Feedback", 170, """
return h("div", {className:"ef ef-stack", style:{alignItems:"flex-end"}},
  h(E.TransferToast, {title:"Copying 1,204 files to AVS (D:)", detail:"From ~/Pictures/2026 · IMG_4471.HEIC", value:42, animate:true, meta:"812 MB of 2.4 GB · 94 MB/s", eta:"17 s left", doneTitle:"Copied 1,204 files to AVS (D:)", doneMeta:"Flushed to disk"}));
""", """# TransferToast

Progress for copy, move, delete and extraction: title, current file, bar, throughput and time left.

**Provide** `title`, `detail` (source and current file), `value` (0–100) or `indeterminate` (during planning), `meta` (bytes done · speed), `eta`.

- The bar is linear and fed from the engine's atomic counters at frame rate — never a message per chunk. At 100% it turns `success` and the tick draws.
- "Flushed to disk" appears only after the batch `syncfs` completes, so people know when it's safe to reboot into Windows.
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
comp("Dialog", "Overlays", 330, """
return h("div", {className:"ef"}, h(E.Dialog, {tone:"danger", title:"Delete 3 items permanently?", icon:"trash-full",
  footer:[h(E.Button, {key:1}, "Cancel"), h(E.Button, {key:2, variant:"danger", icon:"trash"}, "Delete permanently")]},
  h("p", null, "“Report-Q3.xlsx”, “IMG_20260914_182233.jpg” and 1 more will be erased from AVS (D:). This can't be undone."),
  h("p", null, "To keep a way back, press Del instead — it moves them to Trash.")));
""", """# Dialog

A modal for decisions that can't wait: permanent delete, conflicts, unsafe writes, unlocking.

**Provide** `title` (the question, naming the thing), `icon` (colour icon), `children` (consequences + safer alternative), `footer` (buttons: Cancel first, the action last), `tone="danger"` for irreversible actions.

- `bg-raised` panel with a 2px `accent` border (Omarchy popup style; `danger` border when destructive), square corners, `shadow-float`, over `scrim`.
- Enters with scale .98 → 1 in `dur-slow`. Esc cancels. The dangerous button is never the Enter default.
""")

comp("ConflictDialog", "Overlays", 440, """
return h("div", {className:"ef"}, h(E.ConflictDialog, {count:12}));
""", """# ConflictDialog

Resolves name collisions during copy and move, side by side, once for all or file by file.

**Provide** `title`, `count` (conflicts in the batch).

- The two files are compared by modified date and size; the newer one is marked in `success-ink`.
- The pre-flight check finds every conflict before any byte is written, so this appears once at the start, never mid-copy.
- Windows-name problems (`:`, `?`, case-only duplicates on NTFS) get the same dialog with a rename rule instead of Replace.
""")

comp("ContextMenu", "Overlays", 430, """
return h("div", {className:"ef", style:{padding:16}}, h(E.ContextMenu, {items:[
  {icon:"external", label:"Open", kbd:["Enter"], active:true}, {icon:"columns", label:"Open in other pane", kbd:["F3"]}, {icon:"plus", label:"Open in new tab", kbd:["Ctrl","Enter"]},
  "-", {icon:"copy", label:"Copy", kbd:["Ctrl","C"]}, {icon:"cut", label:"Cut", kbd:["Ctrl","X"]}, {icon:"move", label:"Move to AVS (D:)", submenu:true},
  {icon:"rename", label:"Rename", kbd:["F2"]}, {icon:"link", label:"Copy as Windows path", hint:"D:\\\\Work"},
  "-", {icon:"trash", label:"Move to Trash", kbd:["Del"]}, {icon:"trash", label:"Delete permanently", kbd:["Shift","Del"], danger:true},
  "-", {icon:"info", label:"Properties", kbd:["Alt","Enter"]}]}));
""", """# ContextMenu

The right-click menu: actions for the selection, grouped and with shortcuts.

**Provide** `items`: `{icon, label, kbd?, hint?, submenu?, danger?, disabled?}`, `"-"` for separators, `{heading}` for group labels.

- Groups: Open · Clipboard & organise · Trash · Properties. Windows-specific actions (Copy as Windows path, Show alternate streams) appear only on NTFS items.
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
- Results rank recent and frequent first; typing a path (`/`, `~`, `C:\\`) switches to path completion; `/` searches files.
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
""")

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
- F5 copies the selection to the other pane, F6 moves it, both through the same pre-flight and progress as drag and drop. Each pane head carries its world mark.
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
