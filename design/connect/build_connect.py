#!/usr/bin/env python3
"""Assemble the EchoConnect design system (design/connect/project/) from sources.

EchoConnect is the EchoFiles Android app. It shares EchoFiles' tokens, colour themes and
icons (design/system/), re-sized for touch. Run after design/build_icons.py.
"""
import copy, json, pathlib, shutil

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
EF = ROOT / "design/system"
HERE = ROOT / "design/connect"
PRJ = HERE / "project"
SRC = HERE / "src"

# ------------------------------------------------------------------ tokens
def tokens():
    t = json.loads((EF / "project/tokens.json").read_text())
    t["name"] = "EchoConnect"
    t["meta"] = {"source": "EchoFiles tokens (design/system/project/tokens.json): same colours and themes; type and sizes re-scaled for touch by design/connect/build_connect.py"}
    usage = {
        "bg": "Screen ground: app bar, lists' surroundings, the active bottom-nav item. Omarchy `background` of the laptop's theme.",
        "bg-sunken": "The stage behind the laptop drawing on Home. Omarchy `dark_background`.",
        "bg-deep": "Bottom navigation, gesture bar, field wells, the ringing screen. Omarchy `darker_background`.",
        "bg-raised": "List groups, action tiles, cards, sheets, dialogs, snackbars. Omarchy `lighter_background`.",
        "world-linux": "The laptop's mark: the 6px square by its name, quick-action glyphs, clipboard items going to the laptop.",
        "world-phone": "This phone's mark: clipboard items arriving from the laptop.",
    }
    for tok in t["color"]["tokens"]:
        if tok["name"] in usage:
            tok["usage"] = usage[tok["name"]]
    t["type"] = {
        "fonts": [],
        "families": {"mono": "\"JetBrains Mono\", \"JetBrainsMono Nerd Font\", ui-monospace, monospace",
                     "system": "Inter, \"SamsungOne\", \"Google Sans\", Roboto, system-ui, sans-serif"},
        "groups": [
            {"name": "Display", "family": "mono", "styles": [
                {"name": "display", "fontSize": "28px", "lineHeight": "34px", "fontWeight": 800, "letterSpacing": "-0.02em", "sample": "Your phone, on your laptop", "usage": "Onboarding and the ringing screen. One per screen."},
                {"name": "title", "fontSize": "20px", "lineHeight": "26px", "fontWeight": 700, "letterSpacing": "-0.01em", "sample": "Clipboard", "usage": "App bar titles and the laptop's name on Home."},
                {"name": "headline", "fontSize": "17px", "lineHeight": "24px", "fontWeight": 700, "sample": "Send to Aditya's laptop", "usage": "Sheet and dialog titles."}]},
            {"name": "Interface", "family": "mono", "styles": [
                {"name": "body", "fontSize": "15px", "lineHeight": "22px", "fontWeight": 400, "sample": "IMG_20261002_1031.jpg", "usage": "Row titles, clipboard text, fields. The default."},
                {"name": "body-strong", "fontSize": "15px", "lineHeight": "22px", "fontWeight": 600, "sample": "Send clipboard now", "usage": "Buttons, the current step, emphasised values."},
                {"name": "meta", "fontSize": "13px", "lineHeight": "18px", "fontWeight": 400, "sample": "2.5 of 4.1 MB · 18 MB/s", "usage": "Row subtitles, explanations under settings, transfer progress. Tabular figures."},
                {"name": "label", "fontSize": "11px", "lineHeight": "16px", "fontWeight": 700, "letterSpacing": "0.08em", "sample": "SHARED WITH THE LAPTOP", "usage": "Section headers above list groups. Uppercase."},
                {"name": "caption", "fontSize": "12px", "lineHeight": "16px", "fontWeight": 500, "sample": "Automatic", "usage": "Pills, bottom-nav labels, clipboard meta."}]},
            {"name": "Android", "family": "system", "styles": [
                {"name": "system", "fontSize": "14px", "lineHeight": "20px", "fontWeight": 400, "sample": "Allow one-time access", "usage": "Only inside pictures of Android's own UI (notification, tile, text menu, system dialogs). Never in EchoConnect's screens."}]}]}
    t["size"] = {"tokens": [
        {"name": "touch", "value": "48px", "usage": "Minimum touch target: icon buttons, large buttons, fields, switch hit area."},
        {"name": "control-touch", "value": "40px", "usage": "Default button and segmented-control height (hit area padded to 48)."},
        {"name": "appbar", "value": "56px", "usage": "Top app bar."},
        {"name": "navbar", "value": "64px", "usage": "Bottom navigation."},
        {"name": "sysbar", "value": "30px", "usage": "Android status bar in screen mock-ups."},
        {"name": "row-one", "value": "56px", "usage": "One-line list row."},
        {"name": "row-two", "value": "72px", "usage": "Two-line list row, clipboard item."},
        {"name": "screen", "value": "360px", "usage": "Reference screen width (dp) for mock-ups; layouts stretch to any width."},
        {"name": "icon-glyph", "value": "20px", "usage": "Glyphs in rows, buttons and app bar actions."},
        {"name": "icon-nav", "value": "22px", "usage": "Glyphs in the bottom navigation."},
        {"name": "icon-file", "value": "36px", "usage": "Colour file icons in transfer and file rows."}]}
    t["spacing"]["tokens"] = [dict(x, usage={"space-5": "Screen side gutter and card padding.", "space-4": "Row padding, gap inside cards.", "space-3": "Gap between action tiles.", "space-6": "Space below the last section."}.get(x["name"], x.get("usage", ""))) for x in t["spacing"]["tokens"]]
    t["zIndex"]["tokens"] = [{"name": "z-sheet", "value": "40", "usage": "Bottom sheets over the scrim."}, {"name": "z-dialog", "value": "50", "usage": "Dialogs."}, {"name": "z-snack", "value": "60", "usage": "Snackbars, above everything."}]
    t["opacity"]["tokens"] = [{"name": "opacity-disabled", "value": "0.45", "usage": "Disabled buttons, rows and actions; label kept."}]
    return t

# ------------------------------------------------------------------ components
C = {}
def comp(name, group, height, body, readme, extra=""):
    C[name] = (group, height, extra, body, readme)

PAD = 'h("div", {className:"ec-pad", style:{maxWidth:%d}}, %s)'

# Iconography
comp("Icon", "Iconography", 210, """
var names = ["home","clipboard","swap","settings","laptop","phone","phone-ring","wifi","bluetooth","battery","bolt","upload","download","send","copy","external","lock","unlock","unlink","qr","scan","camera","call","mic","speaker","bell","bell-off","moon","message","users","image","folder","shield","key","terminal","code","eye-off","check","close","refresh","arrow-up","arrow-down","arrow-left","chevron-right","more-vertical","trash","info","alert","cursor-text","flashlight"];
return h("div", {className:"ec-pad"}, h("div", {style:{display:"grid", gridTemplateColumns:"repeat(auto-fill, 44px)", gap:4, color:"var(--ink-muted)"}},
  names.map(function(n){ return h("span", {key:n, title:n, style:{width:44,height:44,display:"grid",placeItems:"center"}}, h(E.Icon, {name:n, size:22})); })));
""", """# Icon

The EchoFiles glyph set (24-unit grid, 2px stroke, round caps, `currentColor`) plus ten EchoConnect additions, drawn larger for touch.

**Provide** `name`, optional `size` (20 in rows and buttons = `icon-glyph`, 22 in the bottom nav = `icon-nav`, 16 in pills and small buttons), `label` when it stands alone with meaning.

- Additions for the phone: `bluetooth`, `laptop`, `camera`, `mic`, `speaker`, `call`, `scan`, `moon`, `cursor-text`, `flashlight`. They follow the EchoFiles drawing rules and move to `assets/icons/glyph/` when the Android app is built.
- Colour comes from the parent: `ink-muted` at rest, `ink-strong` pressed, `accent-ink` when active, a `*-ink` inside banners.
- Glyphs are for actions and UI only; files use `FileIcon`.
""")

comp("FileIcon", "Iconography", 150, """
var names = ["folder","folder-image","folder-download","file-image","file-video","file-audio","file-pdf","file-doc","file-sheet","file-archive","file-package","file-code","phone","drive","network","send"];
return h("div", {className:"ec-pad"}, h("div", {style:{display:"grid", gridTemplateColumns:"repeat(auto-fill, 48px)", gap:6}},
  names.map(function(n){ return h("span", {key:n, title:n}, h(E.FileIcon, {name:n, size:44})); })));
""", """# FileIcon

The EchoFiles 48-unit colour icons for files and places, recoloured per theme through the `icon-*` tokens.

**Provide** `name` (or map a file name with `iconFor(name)`), `size` (36 in rows = `icon-file`, 48 in tiles).

- Same slots, same theme recolouring as on the laptop, so a PDF looks the same on both screens.
- Photos and videos show a thumbnail once loaded; the icon is the placeholder.
""")

# Actions
comp("Button", "Actions", 250, """
return """ + PAD % (380, """
  h("div", {className:"ec-row-flex"}, h(E.Button, {variant:"primary", icon:"send"}, "Send"), h(E.Button, {icon:"upload"}, "Send files"), h(E.Button, {variant:"ghost"}, "Cancel")),
  h("div", {className:"ec-row-flex"}, h(E.Button, {size:"sm"}, "Retry"), h(E.Button, {size:"sm", variant:"ghost", icon:"unlink"}, "Forget"), h(E.Button, {variant:"danger", size:"sm"}, "Forget laptop"), h(E.Button, {disabled:true, size:"sm"}, "Send")),
  h(E.Button, {variant:"primary", size:"lg", block:true, icon:"scan"}, "Scan the laptop's code"),
  h(E.Button, {block:true, icon:"external"}, "Open Developer options")""") + """;
""", """# Button

A text button whose label says exactly what happens: 40px tall, 48px (`lg`) for the main action at the bottom of a screen, 32px (`sm`) inside rows.

**Provide** `children` (a verb phrase: "Send 3 photos", "Stop ringing", never "OK"), optional `variant` (`primary`, `danger`, `ghost`; default is bordered), `icon`, `size` (`sm`, `lg`), `block` for full width.

- One `primary` per screen or sheet. Full-width `lg` buttons sit in the flow footer or at the end of a step.
- `ghost` buttons are `accent-ink` text: secondary choices like "Use Tap to send instead".
- The hit area is never under 48px, even for `sm`.
- Same colours and states as EchoFiles' Button: hover `state-hover`, press `state-press`, focus a 2px `focus-ring` outline.
""")

comp("IconButton", "Actions", 100, """
return h("div", {className:"ec-pad"}, h("div", {className:"ec-row-flex"},
  h(E.IconButton, {icon:"arrow-left", label:"Back"}), h(E.IconButton, {icon:"qr", label:"Pair another laptop"}), h(E.IconButton, {icon:"copy", label:"Copy again"}),
  h(E.IconButton, {icon:"bell-off", label:"Mute", pressed:true}), h(E.IconButton, {icon:"trash", label:"Clear history"}), h(E.IconButton, {icon:"close", label:"Stop", disabled:true})));
""", """# IconButton

A 48px square touch target with a 22px glyph for app bars and row ends.

**Provide** `icon`, `label` (the accessible name and long-press tooltip), optional `pressed` for toggles, `size` for the glyph (18 at row ends).

- Rest `ink-muted`, pressed `state-press`, toggled on `state-active` + `accent-ink`.
- App bar: Back on the left, at most two actions on the right.
""")

comp("Switch", "Actions", 120, """
return """ + PAD % (380, """h(E.ListGroup, null,
  h(E.ListRow, {icon:"bell", title:"Notifications", sub:"Shown on the laptop", trailing:h(E.Switch, {defaultChecked:true, label:"Notifications"})}),
  h(E.ListRow, {icon:"message", title:"Messages", sub:"Read and send texts", trailing:h(E.Switch, {label:"Messages"})}))""") + """;
""", """# Switch

On/off for a setting that applies at once: 44 × 24 with a 48px hit area, always at the end of a `ListRow`.

**Provide** `label` (the setting's name), `checked`/`defaultChecked`, `onChange`.

- On is `accent`, off is `bg-deep` with a `line-strong` edge. The knob slides in `dur-base`.
- Phrase the row title as the on state ("Accept files automatically").
""")

comp("Checkbox", "Actions", 120, """
return """ + PAD % (380, """h(E.Checkbox, {defaultChecked:true}, "Open on the laptop when done"), h(E.Checkbox, null, "Delete from phone after sending")""") + """;
""", """# Checkbox

A 20px box with a 48px row for choices inside sheets and multi-select.

**Provide** `children`, `checked`/`defaultChecked`, `onChange`. Settings use `Switch` instead.
""")

comp("SegmentedControl", "Actions", 130, """
return """ + PAD % (380, """h(E.SegmentedControl, {label:"Show", block:true, options:[{value:"all",text:"All"},{value:"to",text:"To laptop"},{value:"from",text:"From laptop"}]}),
  h(E.SegmentedControl, {label:"Theme", value:"laptop", options:[{value:"laptop",text:"Match laptop"},{value:"system",text:"Phone"}]})""") + """;
""", """# SegmentedControl

Two to four exclusive options in one 40px well: list filters, the theme choice.

**Provide** `options` (`{value, text?, icon?, label?}`), `value`, `onChange`, `label`, `block` to fill the width.

- Selected lifts to `bg-raised` with `accent-ink`, exactly as in EchoFiles. Colour change only, no sliding pill.
""")

# Inputs
comp("TextField", "Inputs", 200, """
return """ + PAD % (380, """h(E.TextField, {id:"ip", label:"Laptop address", placeholder:"192.168.1.2", icon:"laptop", hint:"Shown in EchoFiles → Connect phone"}),
  h(E.TextField, {id:"ip2", label:"Laptop address", defaultValue:"192.168.1", icon:"laptop", error:"That isn't a full address. It looks like 192.168.1.2."})""") + """;
""", """# TextField

A 48px field on `bg-deep` with its label above and a hint or error below.

**Provide** `id`, `label`, input props, optional `icon`, `hint`, `error` (names the problem and the fix).

- Focus draws `focus-ring`; an error draws `danger` and says how to fix it.
- Rare in EchoConnect: typing a laptop's address is the main use.
""")

# Feedback
comp("StatePill", "Feedback", 90, """
return h("div", {className:"ec-pad"}, h("div", {className:"ec-row-flex"},
  h(E.StatePill, {tone:"success"}, "Connected"), h(E.StatePill, {tone:"success"}, "Automatic"), h(E.StatePill, {tone:"warning"}, "Paused"),
  h(E.StatePill, null, "Tap to send"), h(E.StatePill, {tone:"danger"}, "Failed"), h(E.StatePill, {tone:"accent"}, "Recommended"), h(E.StatePill, {icon:"eye-off"}, "Hidden")));
""", """# StatePill

A state in a word with a 6px square mark — the same pill as EchoFiles, 22px tall.

**Provide** `children` (one or two words), `tone` (`success`, `warning`, `danger`, `info`, `accent`, none for neutral), or `icon` instead of the square.

- Never colour alone: the word carries the state.
""")

comp("ProgressBar", "Feedback", 120, """
return """ + PAD % (380, """h(E.ProgressBar, {value:62}), h(E.ProgressBar, {value:100, done:true}), h(E.ProgressBar, null)""") + """;
""", """# ProgressBar

A 4px bar for transfers: `accent` while moving, `success` when done, a sweep when the size isn't known.

**Provide** `value` (0–100, omit for indeterminate), `done`.

- Progress is linear and real; never fake a slow start.
""")

comp("Spinner", "Feedback", 80, """
return h("div", {className:"ec-pad"}, h("div", {className:"ec-row-flex"}, h(E.Spinner, null), h(E.Spinner, {large:true}), h("span", {className:"ec-wait"}, h(E.Spinner, null), "Waiting for Aditya's laptop…")));
""", """# Spinner

Waiting on the other device. Always with a line saying what for.

**Provide** optional `large`, `label`.
""")

comp("Snackbar", "Feedback", 220, """
return """ + PAD % (380, """h(E.Snackbar, {icon:"check", tone:"success"}, "Sent to Aditya's laptop"), h(E.Snackbar, {icon:"clipboard", action:"Undo"}, "Clipboard sent"), h(E.Snackbar, {icon:"error", tone:"danger", action:"Retry"}, "Couldn't reach the laptop")""") + """;
""", """# Snackbar

A short result at the bottom of the screen, with at most one action: `bg-raised` with the 2px accent edge EchoFiles' toasts use.

**Provide** `children` (what happened), optional `icon`, `tone` (`success`, `danger`), `action`.

- Rises 12px with `ease-spring`; leaves after 4 seconds, or 8 with an action.
- Errors name the fix in the action ("Retry").
""")

comp("Banner", "Feedback", 230, """
return """ + PAD % (380, """h(E.Banner, {title:"On Samsung", icon:"battery"}, " Settings → Battery → Background usage limits → Never sleeping apps → add EchoConnect."),
  h(E.Banner, {tone:"warning", title:"Automatic clipboard paused."}, " Allow log access once to switch it back on.")""") + """;
""", """# Banner

An inline note in the flow of a screen: soft semantic ground, glyph, bold lead-in.

**Provide** `children`, optional `title` (bold lead-in), `tone` (`info` default, `warning`, `danger`, `success`), `icon`, `action`.

- For facts the reader must know to succeed (brand-specific steps, what a permission really reads). Not for marketing.
""")

# Navigation & structure
comp("AppBar", "Navigation", 170, """
return h("div", {className:"ec", style:{maxWidth:380, display:"flex", flexDirection:"column", gap:12, padding:12}},
  h(E.AppBar, {title:"EchoConnect", actions:[{icon:"qr", label:"Pair another laptop"}]}),
  h(E.AppBar, {title:"Automatic clipboard", back:true}));
""", """# AppBar

The 56px top bar: Back or nothing, the screen's title in `title` style, up to two `IconButton`s.

**Provide** `title`, optional `back`, `sub`, `actions` (`{icon, label}`).

- `bg` with a `line` hairline under it. No colour, no elevation, no collapsing hero.
- Tab screens have no Back; pushed screens (Permissions, Automatic clipboard) do.
""")

comp("BottomNav", "Navigation", 110, """
return h("div", {className:"ec", style:{maxWidth:380, padding:12}}, h(E.BottomNav, {active:0, badges:{2:1}}));
""", """# BottomNav

Four destinations — Home, Clipboard, Transfers, Settings — on a 64px `bg-deep` bar.

**Provide** `active` index, optional `badges` (`{index: count}`), `onChange`.

- The current item gets the `bg` ground, a 2px `accent` bar on its top edge and `accent-ink` — the same mark as EchoFiles' active tab.
- Labels always show. A badge counts things waiting (a transfer in progress).
""")

comp("ListGroup", "Navigation", 260, """
return """ + PAD % (380, """h(E.ListGroup, {title:"Shared with the laptop", foot:"Turning one off stops it on both devices."},
  h(E.ListRow, {icon:"bell", title:"Notifications", sub:"Shown on the laptop; reply from there", trailing:h(E.Switch, {defaultChecked:true, label:"Notifications"})}),
  h(E.ListRow, {icon:"clipboard", title:"Clipboard", sub:"Automatic · set up 2 Oct", chevron:true}))""") + """;
""", """# ListGroup

A titled box of rows: uppercase `label` header, a `bg-raised` box with a `line` border and hairlines between rows, an optional muted foot.

**Provide** `title`, `children` (rows), optional `action` (header link, like "See all"), `foot`.

- The phone version of EchoFiles' SettingsGroup. Every list in the app is one of these.
""")

comp("ListRow", "Navigation", 330, """
return """ + PAD % (380, """h(E.ListGroup, null,
  h(E.ListRow, {icon:"wifi", title:"Wi-Fi", sub:"Files, photos, texts, notifications", trailing:h(E.StatePill, {tone:"success"}, "home")}),
  h(E.ListRow, {file:"file-pdf", title:"boarding-pass.pdf", sub:"212 KB · from laptop · 12 min ago", trailing:h(E.IconButton, {icon:"external", label:"Open", size:18})}),
  h(E.ListRow, {avatar:"P", title:"Priya", sub:"See you at 7?"}),
  h(E.ListRow, {icon:"shield", title:"Permissions", sub:"8 of 9 allowed", chevron:true}),
  h(E.ListRow, {icon:"image", title:"Files and photos", sub:"Turn on Wi-Fi first", disabled:true}))""") + """;
""", """# ListRow

One row: a lead (glyph tile, file icon, avatar or any node), title, up to two lines of subtitle, and a trailing control or chevron. 56px, 72px with a subtitle.

**Provide** `title`, optional `sub`, one lead (`icon` + `tint`, `file`, `avatar` + `avatarColor`, `lead`), `trailing`, `chevron`, `onClick`, `below` (extra content under the text), `disabled`.

- A row with `onClick` or `chevron` is a button: `state-hover` and `state-press` layers.
- Dependent rows show disabled with the dependency as their subtitle, never hidden.
""")

comp("BottomSheet", "Navigation", 330, """
return h("div", {className:"ec", style:{maxWidth:380, position:"relative", background:"var(--bg)"}}, h("div", {className:"ec-sheet-static"}, h(E.BottomSheet, {title:"Send to Aditya's laptop", icon:"laptop",
  footer:[h(E.Button, {key:"c", variant:"ghost"}, "Cancel"), h(E.Button, {key:"s", variant:"primary", icon:"send"}, "Send 3 photos")]},
  h(E.ListRow, {icon:"folder", title:"Lands in ~/Downloads/Phone", sub:"11.8 MB · over Wi-Fi"}),
  h(E.ListRow, {icon:"check", title:"Open on the laptop when done", trailing:h(E.Switch, {label:"Open when done"})}))));
""", """# BottomSheet

A task over the current screen: `bg-raised`, a 2px `accent` top edge (Hyprland's border, as on EchoFiles dialogs), a handle, title, rows, and a footer with the one primary action on the right.

**Provide** `title`, optional `icon`, `children`, `footer` (buttons).

- Over a `scrim`; rises 24px in `dur-slow`. Swipe down or Cancel closes it.
- Square corners: the flat Omarchy edge, not a rounded card.
""")

comp("Dialog", "Navigation", 260, """
return h("div", {className:"ec-pad", style:{maxWidth:380}}, h(E.Dialog, {title:"Forget Aditya's laptop?", tone:"danger",
  actions:[h(E.Button, {key:"c", variant:"ghost"}, "Keep"), h(E.Button, {key:"f", variant:"danger"}, "Forget laptop")]},
  h("p", null, "The laptop stops seeing this phone. Pair again with its code to undo.")));
""", """# Dialog

A decision that blocks: title as a question, one or two lines saying the consequence, actions on the right.

**Provide** `title`, `children`, `actions`, optional `tone="danger"` (danger border and button).

- 2px `accent` border (`danger` for destructive), `bg-raised`, square corners.
- Buttons name the outcome ("Forget laptop", "Keep"), never Yes/No.
""")

# Device
comp("PhoneFrame", "Device", 820, """
return h("div", {className:"ec-pad", style:{alignItems:"flex-start"}}, h(E.PhoneFrame, {height:720}, h(E.HomeScreen, null)));
""", """# PhoneFrame

A flat Android phone around a whole screen for mock-ups: `icon-slate-deep` body, side keys, a pin-hole camera in the status bar, a gesture bar.

**Provide** `children` (a screen), optional `height` (default 760; width is the 360dp reference), `time`, `battery`, `bluetooth`, `dim` (scrim for system dialogs).

- Drawn like the EchoFiles phone: flat fills from icon slots, no gloss, no shadow.
- The status bar uses the system font; everything inside the screen is EchoConnect.
""", extra=" page")

comp("LaptopDevice", "Device", 240, """
return h("div", {className:"ec-pad"}, h("div", {className:"ec-row-flex", style:{alignItems:"flex-end"}},
  h(E.LaptopDevice, {width:220}), h(E.LaptopDevice, {width:160, state:"locked"}), h(E.LaptopDevice, {width:120, state:"away"}), h(E.LaptopDevice, {width:52})));
""", """# LaptopDevice

The laptop drawn flat, with EchoFiles open on its screen in the active theme's tokens: tab strip with the accent bar, sidebar with the four world marks, a selected row.

**Provide** optional `width`, `state` (`connected`, `locked`, `away`).

- The counterpart of EchoFiles' PhoneDevice: each app draws the other device.
- `locked` after Lock laptop; `away` dims it when the laptop isn't reachable. Under 64px it is an avatar for rows.
""")

comp("BatteryMeter", "Device", 80, """
return h("div", {className:"ec-pad"}, h("div", {className:"ec-row-flex"}, h(E.BatteryMeter, {level:64, charging:true}), h(E.BatteryMeter, {level:72}), h(E.BatteryMeter, {level:18}), h(E.BatteryMeter, {level:8})));
""", """# BatteryMeter

The laptop's battery: `success` when charging (with a bolt), `warning` at 20% and below, `danger` at 10%.

**Provide** `level`, optional `charging`, `label={false}` to hide the number.
""")

comp("LinkPills", "Device", 90, """
return h("div", {className:"ec-pad"}, h(E.LinkPills, null), h(E.LinkPills, {wifi:false}));
""", """# LinkPills

How the phone reaches the laptop right now: Wi-Fi (everything) and Bluetooth (calls and clipboard).

**Provide** optional `wifi`, `bluetooth` (false = off, dashed), `network` name.

- Bluetooth alone keeps calls and clipboard working; everything else waits for Wi-Fi.
""")

# Connect
comp("LaptopCard", "Connect", 470, """
var st = React.useState("connected");
return """ + PAD % (380, """h(E.SegmentedControl, {label:"State", value:st[0], onChange:st[1], options:[{value:"connected",text:"Connected"},{value:"locked",text:"Locked"},{value:"away",text:"Not nearby"}]}),
  h(E.LaptopCard, {key:st[0], state:st[0] === "away" ? "away" : "connected", locked:st[0] === "locked"})""") + """;
""", """# LaptopCard

The top of Home: the laptop drawing on a `bg-sunken` stage, the connection pill, the laptop's battery, its name with the laptop's world mark, and how it's reached.

**Provide** `state` (`connected`, `away`), optional `locked`, `battery`, `charging`, `wifi`, `bluetooth`, `network`.

- Not nearby says what to do ("Open the laptop on the same Wi-Fi…"), never just "Offline".
- The 6px square is `world-linux`, the laptop's colour in EchoFiles' sidebar.
""")

comp("ActionGrid", "Connect", 230, """
return """ + PAD % (380, """h(E.ActionGrid, null)""") + """;
""", """# ActionGrid

Four quick actions on Home, two by two: Send files, Send clipboard, Open on laptop, Lock laptop.

**Provide** optional `items` (`{icon, label, sub}`), `disabled` (laptop not nearby).

- Tiles are `bg-raised` boxes; their glyph square tints toward `world-linux` because each one acts on the laptop. Hover and focus draw a `world-linux` border, like EchoFiles' FeatureTile does with `world-phone`.
""")

comp("ClipItem", "Connect", 420, """
return """ + PAD % (380, """h(E.ListGroup, null,
  h(E.ClipItem, {dir:"to", text:"https://maps.app.goo.gl/x7KdQ2", when:"1 min ago"}),
  h(E.ClipItem, {dir:"from", text:"ssh aditya@build-box -p 2222", when:"6 min ago"}),
  h(E.ClipItem, {dir:"to", sensitive:true, when:"22 min ago"}),
  h(E.ClipItem, {dir:"to", image:1, when:"40 min ago"}),
  h(E.ClipItem, {dir:"from", text:"cargo run -p echofiles-phone", when:"1 h ago", via:"bt"}))""") + """;
""", """# ClipItem

One thing that crossed the clipboard: direction, the text (or a screenshot), where and when, Copy again.

**Provide** `dir` (`to` laptop, `from` laptop), `text` or `image`, `when`, optional `sensitive`, `via="bt"`.

- Up arrows in `world-linux` go to the laptop; down arrows in `world-phone` arrive on the phone.
- Sensitive clips (marked by a password manager) show dots and a Hidden pill and are never kept in history.
- Clips that crossed over Bluetooth say so.
""")

comp("ClipModeCard", "Connect", 600, """
return """ + PAD % (380, """h(E.ClipModeCard, {mode:"auto"}), h(E.ClipModeCard, {mode:"paused"}), h(E.ClipModeCard, {mode:"manual"})""") + """;
""", """# ClipModeCard

How phone → laptop clipboard works on this phone, and what to do about it.

**Provide** `mode`: `auto` (set up, working), `paused` (after a restart, waiting for the log prompt), `manual` (Tap to send).

- Paused takes a `warning` border and a primary Resume button; Tap to send offers Make it automatic.
- The foot always says laptop → phone is automatic, so nobody thinks both directions need setup.
""")

comp("TransferRow", "Connect", 330, """
return """ + PAD % (380, """h(E.ListGroup, null,
  h(E.TransferRow, {name:"VID_20261002_0912.mp4", progress:38, rate:"31 of 84 MB · 22 MB/s · 3 s left"}),
  h(E.TransferRow, {name:"boarding-pass.pdf", dir:"from", when:"12 min ago"}),
  h(E.TransferRow, {name:"voice-note.opus", size:"88 KB", failed:true}))""") + """;
""", """# TransferRow

A file moving between the devices: icon, name, progress with real numbers, or size · direction · time when done.

**Provide** `name`, and `progress` + `rate` while moving, or `size`, `dir` (`to`/`from`), `when`; `failed` for a stopped transfer.

- Moving: a Stop button. Done: Open. Failed: the reason in `danger-ink` and Retry — transfers resume where they stopped.
""")

comp("PermissionRow", "Connect", 260, """
return """ + PAD % (380, """h(E.ListGroup, null,
  h(E.PermissionRow, {icon:"bell", title:"Notification access", why:"Shows your notifications on the laptop", granted:true}),
  h(E.PermissionRow, {icon:"clipboard", title:"Display over other apps", why:"Lets automatic clipboard read what you copy", action:"Open setting"}),
  h(E.PermissionRow, {icon:"users", title:"Contacts", why:"Names instead of numbers", optional:true}))""") + """;
""", """# PermissionRow

One Android permission: the feature it unlocks in plain words, and either Allowed or the button that grants it.

**Provide** `icon`, `title` (Android's own name for it, so people find it in Settings), `why`, `granted`, optional `action` (button label), `optional` (secondary button).

- Ask in context and say why; never a wall of system prompts on first run.
""")

comp("StepList", "Connect", 520, """
return """ + PAD % (380, """h(E.StepList, {current:2, steps:[
  {title:"Turn on Developer options", done:"On"},
  {title:"Turn on Wireless debugging", done:"On"},
  {title:"Scan the code on the laptop", body:[h("p", {key:"a"}, "On the laptop: EchoFiles → Settings → Phone → Make clipboard automatic."), h("div", {key:"b", className:"ec-wait"}, h(E.Spinner, null), "Waiting for Aditya's laptop…")]},
  {title:"Allow Display over other apps"},
  {title:"Turn Developer options off"}]})""") + """;
""", """# StepList

A setup that takes several stops in Android's Settings: done steps collapse to one `success` line, the current one opens with its instructions and button, later ones wait.

**Provide** `steps` (`{title, body?, done?}`), `current`.

- Each step names the exact path on the phone ("Settings → About phone → Software information"), using Samsung's names on Samsung.
- EchoConnect checks each step itself and moves on; the person never ticks a box.
""")

comp("Stepper", "Connect", 100, """
return """ + PAD % (380, """h(E.Stepper, {current:1, count:5, label:"Scan"}), h(E.Stepper, {current:4, count:5, label:"Clipboard"})""") + """;
""", """# Stepper

Onboarding progress: one 3px bar per step — `success` done, `accent` current — and "Step 2 of 5 · Scan". The same bars as EchoFiles' PairPhone stepper.

**Provide** `current`, `count`, optional `label`.
""")

comp("QrViewfinder", "Connect", 420, """
return """ + PAD % (360, """h(E.QrViewfinder, {found:true})""") + """;
""", """# QrViewfinder

The camera pointed at the code EchoFiles shows in Connect phone: accent corner brackets, `success` when found, one status line.

**Provide** optional `found`.

- The code carries the laptop's address, Bluetooth address and certificate fingerprint, so pairing needs no typing and is approved on both at once.
""")

comp("PairCode", "Connect", 110, """
return h("div", {className:"ec-pad"}, h(E.PairCode, null));
""", """# PairCode

The four-pair code both screens show while pairing — identical to EchoFiles' code, so the two can be compared at a glance.

**Provide** optional `code` (four strings).
""")

comp("CallBar", "Connect", 110, """
return """ + PAD % (380, """h(E.CallBar, null)""") + """;
""", """# CallBar

A call running through the laptop's mic and speakers over Bluetooth: who, how long, and Use phone to move it back.

**Provide** optional `who`, `time`.

- `success-soft` ground: a live call is good news, not an alert.
""")

# Android system surfaces
comp("SystemNotification", "Android", 200, """
return h("div", {className:"ec-pad", style:{maxWidth:380, background:"var(--bg-deep)"}}, h(E.SystemNotification, null));
""", """# SystemNotification

EchoConnect's always-on notification as Android draws it: connected laptop, how, and two actions (Send clipboard, Disconnect).

**Provide** optional `title`, `text`, `actions`.

- Android requires it while EchoConnect stays connected in the background. Keep it to one quiet line; never an ad.
- Drawn in the system font (`system` style): this is Android's UI, not ours.
""")

comp("QuickTile", "Android", 120, """
return h("div", {className:"ec-pad", style:{maxWidth:380, background:"var(--bg-deep)"}}, h("div", {style:{display:"grid", gridTemplateColumns:"1fr 1fr", gap:8}}, h(E.QuickTile, {on:true}), h(E.QuickTile, {icon:"moon", label:"Do not disturb", sub:"Synced with laptop"})));
""", """# QuickTile

The Send clipboard tile in quick settings: one tap sends what you copied, no setup needed.

**Provide** optional `icon`, `label`, `sub`, `on`.
""")

comp("SelectionMenu", "Android", 100, """
return h("div", {className:"ec-pad"}, h(E.SelectionMenu, null));
""", """# SelectionMenu

Android's text-selection menu with EchoConnect's **Send to laptop** next to Copy — the no-setup way to send text.

**Provide** nothing.

- Added through Android's official text-processing intent; works in any app that lets you select text.
""")

comp("SystemDialog", "Android", 360, """
return h("div", {className:"ec-pad", style:{alignItems:"center"}}, h(E.SystemDialog, null));
""", """# SystemDialog

Android's own log-access prompt, shown once after each restart while Automatic clipboard is on.

**Provide** optional `title`, `body`, `actions`.

- We can't change its words; EchoConnect explains it beforehand (ClipboardSetupScreen) and in the Paused card.
""")

# Screens
def screen(name, body, readme, height=800, width=None):
    w = f" width={width}" if width else ""
    comp(name, "Screens", height, "return h(\"div\", {className:\"ec-pad\", style:{alignItems:\"flex-start\"}}, h(E.PhoneFrame, {height:720}, " + body + "));", readme, extra=w + " page")

screen("HomeScreen", 'h(E.HomeScreen, null)', """# HomeScreen

Home: the laptop and its state, four quick actions, the last two clipboard items and transfers.

**Provide** optional `state` (`connected`, `away`, `call`), `locked`.

- Order is fixed: LaptopCard → CallBar (during a call) → ActionGrid → Clipboard → Transfers.
- Not nearby disables the actions and keeps the history readable.
""")
screen("ClipboardScreen", 'h(E.ClipboardScreen, null)', """# ClipboardScreen

Clipboard: how phone → laptop works here (ClipModeCard), Send clipboard now, and 24 hours of history both ways.

**Provide** optional `mode` (`auto`, `paused`, `manual`); manual makes Send now the primary button.
""")
screen("TransfersScreen", 'h(E.TransfersScreen, null)', """# TransfersScreen

Transfers: what's moving now, then today and earlier, with where files land on each device in the foot.
""")
screen("SettingsScreen", 'h(E.SettingsScreen, null)', """# SettingsScreen

Settings: the laptop and how it's reached, what's shared, receiving, theme, this phone, and About with the open-source line.

- Every change saves at once and is mirrored to EchoFiles → Settings → Phone; a feature turned off stops on both.
- Theme: Match laptop (default) uses the laptop's Omarchy theme sent over the connection; Phone uses Echo or Catppuccin Latte with Android's dark mode.
""")
screen("PermissionsScreen", 'h(E.PermissionsScreen, null)', """# PermissionsScreen

Every permission, what it unlocks, and its state, plus the Samsung battery step.
""")
screen("ClipboardSetupScreen", 'h(E.ClipboardSetupScreen, null)', """# ClipboardSetupScreen

The one-time Automatic clipboard setup, done together with EchoFiles: Developer options on, Wireless debugging, scan the laptop's code, Display over other apps, Developer options off.

**Provide** optional `step` (0–5); Next and Back are live in the preview.

- Says exactly what is read from the log and that the code is open source.
- Always offers Use Tap to send instead.
""")
screen("PairingScreen", 'h(E.PairingScreen, null)', """# PairingScreen

First run, five steps: Welcome → Scan the laptop's code → Check the code → Permissions → Clipboard choice → Connected. Live in the preview.

**Provide** optional `step`, `found`.

- One primary button per step at the bottom; Back in the top left from step 2.
""")
screen("RingScreen", 'h(E.RingScreen, null)', """# RingScreen

Full screen while the laptop rings the phone: big Stop ringing, full volume even on silent, flashlight blinking.

- The pulse is two flat outlines fading (`dur-ambient`), off with reduced motion.
""")
screen("ShareSheetScreen", 'h(E.ShareSheetScreen, null)', """# ShareSheetScreen

Share → EchoConnect from the gallery: the photos going, where they land, Send.
""")

comp("SystemSurfaces", "Screens", 760, "return h(E.SystemSurfaces, null);", """# SystemSurfaces

The places EchoConnect shows up inside Android: the notification shade (always-on notification, Send clipboard tile, call on laptop), the text-selection menu, and the log prompt after a restart.
""", extra=" width=1240 page")

comp("AppMap", "Screens", 1700, "return h(E.AppMap, null);", """# AppMap

Every main screen side by side — the reference composition for checking that screens agree with each other and with EchoFiles.
""", extra=" width=1680 page")

# ------------------------------------------------------------------ build
def preview(name):
    group, height, extra, body, _ = C[name]
    return (f'<!-- @dsCard group="{group}" height={height}{extra} -->\n<!doctype html>\n<html>\n<head><meta charset="utf-8"><title>{name}</title></head>\n'
            f'<body>\n<div id="root"></div>\n<script>\n(function(){{\nvar E = window.EchoConnect, h = React.createElement;\n'
            f'function App(){{{body}}}\nReactDOM.createRoot(document.getElementById("root")).render(h(App));\n}})();\n</script>\n</body>\n</html>\n')

def main():
    comps = PRJ / "components"
    if comps.exists():
        for d in comps.iterdir():
            if d.is_dir() and d.name not in C and d.name not in ("Cover", "lib"):
                shutil.rmtree(d)
    for name, (_, _, _, _, readme) in C.items():
        d = comps / name
        d.mkdir(parents=True, exist_ok=True)
        (d / "preview.html").write_text(preview(name))
        (d / "README.md").write_text(readme.lstrip())
    icons = (EF / "icons.generated.js").read_text()
    body = (SRC / "glyphs.js").read_text() + (SRC / "components.js").read_text()
    header = {"format": 4, "namespace": "EchoConnect", "components": [{"name": n} for n in C]}
    bundle = "/* @ds-bundle: " + json.dumps(header, separators=(",", ":")) + " */\n(function(){\n" + icons + "\n" + body + "\n})();\n"
    for bad in ("</script", "<!--"):
        assert bad not in bundle.lower(), bad
    (comps / "bundle.js").write_text(bundle)
    shutil.copy(SRC / "bundle.css", comps / "bundle.css")
    shutil.copy(SRC / "index.d.ts", comps / "index.d.ts")
    (PRJ / "tokens.json").write_text(json.dumps(tokens(), indent=2, ensure_ascii=False) + "\n")
    (comps / "Cover").mkdir(exist_ok=True)
    shutil.copy(SRC / "Cover.html", comps / "Cover" / "preview.html")
    print(f"{len(C)} components, bundle {len(bundle)//1024} KB")

if __name__ == "__main__":
    main()


# ------------------------------------------------------------------ index (after uploads)
def index(note):
    """design-system.json from asset-ids.txt (the ids the artifact's asset store returned)."""
    import datetime
    src = {"Logos": [ROOT / "assets/brand", EF / "uploads/Logos"], "Icons": [EF / "uploads/Icons"],
           "Glyphs": [EF / "uploads/Glyphs", HERE / "uploads/Glyphs"]}
    groups = {g: {"name": g, "tile": {"Logos": "l", "Icons": "m", "Glyphs": "s"}[g], "order": [], "files": {}} for g in ["Logos", "Icons", "Glyphs"]}
    for line in (HERE / "asset-ids.txt").read_text().split("\n"):
        if not line.strip():
            continue
        path, blob = line.split()
        g, name = path.split("/", 1)
        f = next(d / name for d in src[g] if (d / name).exists())
        groups[g]["order"].append(name)
        groups[g]["files"][name] = {"name": name, "blob": blob, "size": f.stat().st_size,
                                    "type": "image/webp" if name.endswith(".webp") else "image/svg+xml"}
    groups["Logos"]["order"].sort(key=lambda n: not n.endswith(".webp"))
    prev = PRJ / "design-system.json"
    created = json.loads(prev.read_text())["createdOnFiles"] if prev.exists() else {"v": 1, "at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")}
    now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    d = {"v": 3, "layout": "files", "createdOnFiles": created, "title": "EchoConnect", "namespace": "EchoConnect",
         "libraries": [{"name": "react", "version": "18"}, {"name": "react-dom", "version": "18"}],
         "sections": {}, "groups": ["Logos", "Icons", "Glyphs"], "assetGroups": groups, "blobs": {},
         "docs": {"readme": "project/README.md", "sections": []},
         "lastChange": {"by": "Aditya", "at": now, "via": "Claude Code", "note": note}}
    prev.write_text(json.dumps(d, indent=2) + "\n")
    print("index:", {g: len(v["order"]) for g, v in groups.items()})
