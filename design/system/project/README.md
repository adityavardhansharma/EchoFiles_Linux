EchoFiles is a native file manager for Omarchy that treats Linux folders and dual-boot Windows drives as one place. Its design has one job: make the fastest file manager on Linux *feel* instant, while looking like it shipped with Omarchy.

## Principles

1. **Omarchy is the design system above this one.** Colours, font, corner radius and borders come from the running Omarchy theme and Hyprland config. EchoFiles adds roles (selection, drop target, world marks), never a competing look. Switching Omarchy themes restyles the app live.
2. **Speed is the design.** Anything that could make a frame late is not allowed: no blur, no per-row shadows, no animated layout in lists, no variable row heights. Navigation, selection and typing never animate; only the UI's *response* does.
3. **One window for every world.** Linux, Windows, Phone and Network (SMB, SFTP and FTP servers) are told apart by a 6px square world mark and names people recognise — drive names that match what Windows shows ("AVS (D:)"), shares by their name ("Media") — never by different styling or raw mount paths.
4. **Keyboard first, mouse welcome.** Every action has a shortcut, and every surface that offers an action shows its key hint (`Kbd`). The command palette (Ctrl+K) reaches everything.
5. **Honest states.** Read-only, dirty, locked, cloud-only, broken link and "flushed to disk" are always visible — as a word *and* a mark, never colour alone.

## Omarchy integration

EchoFiles reads `~/.local/state/omarchy/current/theme/colors.toml` at startup and again on Omarchy's `theme-set` hook, then derives every colour token with the rules below (reference implementation: `design/build_tokens.py`). The themes in this system are real Omarchy themes run through those rules, plus **Echo**, the brand theme used outside Omarchy.

| Omarchy key | EchoFiles tokens | Rule |
| --- | --- | --- |
| `background` | `bg` | as is |
| `dark_background` | `bg-sunken` | as is (sidebar, preview, inactive pane) |
| `darker_background` | `bg-deep`, `scrim` | as is; scrim at 55% |
| `lighter_background` | `bg-raised` | as is on dark themes; light themes lift `background` 60% toward white |
| `foreground` | `ink`, `ink-muted`, `line`, `line-strong`, `state-*` | `ink` pushed to 7:1 on every surface; `ink-muted` = mix 45% toward bg, then pushed to 4.5:1; lines at 14% / 3:1; state layers at Omarchy's alphas (8% hover, 18% active, 22% press) |
| `bright_foreground` | `ink-strong` | pushed to 7:1 |
| `dark_foreground` | `ink-faint` | as is — disabled and decorative only |
| `accent` | `accent`, `accent-ink`, `focus-ring`, `accent-soft`, `on-accent` | text use pushed to 4.5:1, rings to 3:1, soft = 16% over bg |
| `selection` | `selection` | as is, adjusted only if `ink` falls below 7:1 on it |
| `green` `yellow` `red` `cyan` | `success` `warning` `danger` `info` (+ `-soft`, `-ink`, `on-`) | soft = 14% over bg; ink pushed to 4.5:1 on bg, raised and soft |
| `orange` `blue` `magenta` `green` | `world-linux` `world-windows` `world-network` `world-phone` | pushed to 3:1 on the sidebar |
| `yellow` `blue` `green` `red` `orange` `magenta` | `icon-*` slots | colour icons recolour per theme |

Beyond colour:

- **Font** follows `omarchy-font-set` (default JetBrains Mono Nerd Font). The type scale sits on Omarchy's `[font] base-size` (12): `body` is base + 1.
- **Corners** follow Hyprland `general:rounding` (Omarchy default 0): the window, panes and every floating surface use `radius-0`. Only controls (4px), rows (2px) and thumbnails (6px) round, so the UI stays crisp even when windows are square.
- **Borders mirror Hyprland.** The window's only chrome is Hyprland's 2px active border. Popups that ask for attention — dialogs, toasts — carry the same 2px `accent` border, the way Omarchy's notifications and polkit prompts do. Menus, tooltips and the palette use a 1px `line-strong` border, like Omarchy's menus.
- **No client-side decorations.** No title bar, no window buttons: Hyprland tiles the window.
- **Menus and the palette behave like Omarchy's launcher:** the highlighted row gets `state-hover` with a 1px `line` border, and its label turns `accent-ink`.
- **Animation** follows Hyprland `animations:enabled`; when it is off, or the system asks for reduced motion, every duration token becomes 0.

## Content fundamentals

- **Voice:** plain, specific, calm. Say what happened and what to do next. No apologies, no exclamation marks, no "Oops".
- **Person:** address the reader implicitly ("Restart Windows, or turn off Fast Startup"). Never "I" or "we".
- **Case:** Sentence case everywhere. UPPERCASE only for `label` (sidebar section headers, column headers).
- **Buttons are verbs that say exactly what happens:** "Delete permanently", "Copy here", "Mount read-only". Never "OK", "Yes" or "Submit".
- **Name things the way the reader knows them.** Volumes by their Windows name: "Windows (C:)", "AVS (D:)". Never `/run/media/aditya/AVS1`, never a UUID. Paths in Windows form on NTFS (`D:\Work`), POSIX form elsewhere.
- **Numbers:** sizes base-1024 with Explorer's labels (`4.2 MB`, `2.4 GB`), so they match what people see in Windows. Counts use thousands separators (`1,284 items`). Dates are relative within a week ("Today 18:22", "Yesterday"), then `14 Sep`, then `14 Sep 2025`; 24-hour clock unless the locale says otherwise.
- **Errors name the item, the reason and the fix:** “Couldn't copy “Q3:final.xlsx” — Windows names can't contain “:”. Rename to “Q3-final.xlsx”?”
- **Emoji:** never. Icons carry meaning; words carry the rest.

## Visual foundations

### Colour

Colour means something or it isn't there. The ground is Omarchy's own; EchoFiles spends colour on four things only:

- `accent` — the one active thing: the primary button, the active tab's top bar, the focus ring, progress fills, drop targets.
- `selection` — selected rows and tiles (Omarchy's own selection colour, so selection looks the same as in the terminal and launcher).
- **Semantic** `success` / `warning` / `danger` / `info` — states and results, always paired with a word or glyph.
- **World marks** `world-linux` / `world-windows` / `world-network` / `world-phone` — the 6px squares on sidebar sections and pane heads, drive usage bars, and the glyph of a connected network place. Nowhere else.

`brand-*` colours belong to the logo, About and onboarding only; they never mean state.

### Surfaces

Four grounds, always in this order — nothing else is layered:

```
bg-deep    tab strip · status bar · field wells
bg         toolbar · file list · active pane
bg-sunken  sidebar · preview pane · inactive pane
bg-raised  menus · dialogs · toasts · palette · tooltips   (+ scrim behind modals)
```

### Type

One family — the Omarchy font (JetBrains Mono by default). Mono suits file names: `l`, `1` and `I` and `O` and `0` never collide, extensions line up, and figures are tabular for free.

- `body` 13/20 for file names, menus and controls; `body-strong` for the focused file and active places.
- `meta` 12/16 for size, date and kind columns, status bar and subtitles — always `ink-muted`.
- `label` 11/16 bold uppercase, 0.08em tracking, for section and column headers.
- `caption` 11/14 for pills, key hints and tooltips.
- `title` 16/22 for dialog titles and the preview filename; `display` 28/34 for empty states and About, once per view.
- File extensions render in `ink-muted` after an `ink` stem. Long names truncate at the end in rows and wrap to two lines in tiles, and the full name is always in the tooltip.

### Space and layout

A 4px base (`space-1`) with steps 2 · 4 · 6 · 8 · 12 · 16 · 24 · 32. Fixed geometry the virtualised list depends on:

```
┌ tabbar 32 ───────────────────────────────────────────────────────────────┐
├ toolbar 40 ─ ← → ↑ │ PathBar ………………………… │ Search │ View │ Preview │ ⋮ ┤
├ sidebar 236 ┬ banner (optional) ──────────────────────────┬ preview 280 ──┤
│ LINUX       │ NAME ↑                SIZE   KIND   MODIFIED │               │
│ WINDOWS     │ row 28 (24 compact · 34 comfortable)        │               │
│ PINNED      │ …                                           │               │
├─────────────┴─────────────────────────────────────────────┴───────────────┤
└ statusbar 24 ─ counts · selection · task ………………… volume · state · free ┘
```

Rows never change height at runtime. Tiles are `tile` 104px wide with an 88px thumbnail box, so thumbnails load without layout shift.

### Borders, radius and elevation

- Hairlines (`line`) separate regions; `line-strong` outlines controls and meets 3:1. Borders separate things; shadows only lift things that float.
- `shadow-pop` for menus, tooltips and the drag ghost; `shadow-float` for dialogs, toasts and the palette. Never on rows, tiles, cards or the sidebar.
- `radius-0` for the window and anything floating (follows Hyprland), `radius-1` for rows, pills and checkboxes, `radius-2` for buttons, fields and tiles, `radius-3` for thumbnails, `radius-full` for the switch only.

### States

| State | Treatment |
| --- | --- |
| Hover | `state-hover` layer (8% foreground), `dur-fast` |
| Pressed | `state-press` layer (22%) |
| Active / checked toggle | `state-active` layer (18%) + `accent-ink` glyph |
| Selected row or tile | `selection` ground, name turns `ink-strong` |
| Keyboard cursor | 1px inset `focus-ring` on the row |
| Focus (controls) | 2px `focus-ring` outline, 1px offset |
| Drop target | `accent-soft` + 1px `accent` inset + icon pop + 700ms spring-load bar |
| Cut (on clipboard) | `opacity-cut` |
| Hidden / Windows Hidden+System | `opacity-hidden-file` when shown |
| Disabled | `opacity-disabled`, label kept |

### Iconography

Two families, never mixed in one slot. The SVG sources are in the repo (`assets/icons/`) and in this system's asset groups.

- **Glyphs** (`Icon`, 86): 24-unit grid, 2px stroke, round caps and joins, `currentColor`. For actions and UI only. Sizes 16 · 14 · 12.
- **Colour icons** (`FileIcon`, 71): 48-unit grid, flat fills, no outlines, no gradients. For places, devices, file types and status. Folders are a two-tone back and front with the glyph printed on the front; documents are a page with a folded corner. They are drawn with **palette slots** — `{folder}`, `{blue}`, `{purple}`… — and the `icon-*` tokens fill the slots, so every Omarchy theme recolours them. The Rust app substitutes slot hexes before rasterising with `resvg`.
- **Badges** mark file facts in a corner: link / junction, cloud placeholder, EFS lock, broken, +ADS.
- The logo is only for the app icon, About and onboarding — never inside the working UI.

### Motion

Motion confirms; it never makes anyone wait. Durations: `dur-instant` 0 · `dur-fast` 90 · `dur-base` 140 · `dur-slow` 220 · `dur-ambient` 1400. Easings: `ease-standard` to enter, `ease-exit` (at 70% of the duration) to leave, `ease-spring` only for toasts and drop targets, `ease-linear` for progress.

| Moment | Motion |
| --- | --- |
| Open folder, back/forward, sort, select, type-to-jump | none (0ms) |
| New breadcrumb segment | slides 6px, `dur-base` |
| Hover / press | colour layer, `dur-fast` |
| Menu, tooltip | 4px drop + fade, `dur-base`; closes instantly |
| Dialog | scrim fade `dur-base`; panel scale 0.98 → 1, `dur-slow` |
| Command palette | 8px drop + fade, `dur-slow` |
| Toast | rises 12px, `ease-spring`, `dur-slow`; exit fade |
| Drop target | icon pops 1 → 1.12 spring; spring-load bar fills in 700ms, then the folder opens |
| Drag ghost | up to three icons stacked like the logo's sheets, plus a count |
| Thumbnail ready | fades over the icon, `dur-base`, no layout shift |
| Delete | row collapses `dur-base` `ease-exit`; Undo toast follows |
| Transfer | linear progress from atomic counters at frame rate; success tick at 100% |
| Drive mounted | usage bar grows once, 400ms |
| Slow folder (>150ms) | skeleton rows, shimmer `dur-ambient`; nothing appears before 150ms |
| Omarchy theme change | whole window crossfades, `dur-slow` |

### Accessibility

- Contrast is guaranteed by the derivation, not by hand: `ink` 7:1, `ink-muted` and every `*-ink` 4.5:1, rings, control borders and world marks 3:1 — on every surface and on `selection`, in every theme, including Omarchy themes installed later.
- States are never colour alone: pills carry a word and a square mark, and badges carry a glyph.
- Full keyboard operation; focus is always visible; `prefers-reduced-motion` zeroes every duration.

## Interaction states

One set of states for everything clickable — toolbar buttons, path segments, sidebar rows, column headers, result rows, chips' ×, settings nav. The app builds all of them from one module (`crates/ui/src/widgets.rs`), so they can't drift.

- **Hover**: `state-hover` layer; glyphs and labels go to `ink-strong`. Column headers show the layer behind the hovered column and a pointer cursor.
- **Pressed**: `state-press`. **Active/current**: `state-active` + `accent-ink` glyph + bold label.
- **Disabled**: 45% opacity and no hover (Forward with no history, Start at login while background is off).
- Fields: `line-strong` border at rest, `ink-muted` on hover, `focus-ring` when focused, `danger` when invalid.
- Every icon-only button has a tooltip naming the action with its key as a `Kbd` chip.

## Settings screen

Settings is its own screen (Ctrl+, or the toolbar gear): top bar with **← Files** `Esc` and a live **Saved** mark, `SettingsNav` on the left, one page on the right. Use `SettingsPage` as the reference.

- Five pages: General · Search & index · AI agents · Appearance · About. A page is a 22px title, one sentence, then 2–5 `SettingsGroup`s in a centred 720px column.
- Every row is a `SettingRow`: what it does on the left, the control on the right. Dependent settings are shown disabled with the dependency as their description, never hidden.
- Only settings that work today. No "coming soon" rows.
- Save on every change to `~/.config/echofiles/settings.toml` (shared with `ef`). No Save/Cancel pair.
- Folder lists use `PathListEditor`; folder names use `NameChips`; the index shows `IndexStatus`; commands and shortcuts use `CommandList`.
- Validation errors name the path and the fix, in `danger-ink` under the field.

## Search

The search box has two scopes (`SearchScope`, Ctrl+E): **Folder** filters the open folder as you type ("12 of 40 match" in the status bar); **Everywhere** shows `SearchResults` from the index. Esc clears the search from anywhere, even while typing, and navigating anywhere clears it too.

## Network places

Windows shares (SMB), SSH servers (SFTP) and FTP/FTPS servers open like any folder. EchoFiles connects through GVfs (the engine Nautilus uses) and answers its sign-in and host-key questions in its own dialogs.

- **Network** is the last sidebar section, after Phone (`world-network`, Omarchy `magenta`): saved servers, then live connections made anywhere. `NetworkItem` rows name the place ("Media") and say how and where ("SMB · nas.local"); a connected row carries a Disconnect button.
- **Connect to server** (Ctrl+Shift+S, the section's `+`, the palette) is `ConnectDialog`; addresses also work in the path bar (Ctrl+L), from the command line and from `smb://`, `sftp://`, `ftp://` links.
- Sign-in is `SignInDialog`; a bare SMB server opens `SharesPage`.
- Breadcrumbs, tabs, the status bar and errors use the place's name, never GVfs' `/run/user/…/gvfs/smb-share:server=…` folders. Network files get no thumbnails and never go to the Trash (Delete asks to delete permanently). A server that stops answering closes its folders with a toast saying so.

## Phone

Your Android phone over Wi-Fi. EchoFiles speaks the KDE Connect protocol itself — nothing KDE is installed on the laptop; the phone runs the stock KDE Connect app (later, EchoFiles' own app on the same protocol).

- **Phone** sits between Windows and Network (`world-phone`, Omarchy `green`). Before pairing it holds **Connect phone**, which opens `PairPhone` (get the app → choose your phone → check the code → allow files). Paired, `PhoneItem` shows the phone's name, Wi-Fi and a `BatteryMeter`, with **Files** and **Photos** under it.
- Clicking the phone opens `PhoneHub`: a drawn `PhoneDevice` (a flat Android phone in icon slot colours, lock screen over your latest photo or a flat theme-coloured wallpaper), battery and storage, **Ring phone**, **Send files**, **Get files**, feature tiles (Files, Photos, Messages, Notifications), `ReceivedList` and `ClipboardCard`. `PhoneWindow` is the reference composition.
- Pages: `PhotosPage` (timeline, Import new, Save to), `MessagesPage` (SMS through the phone), `NotificationsPage` (grouped by app, dismiss, quick reply).
- **Settings → Phone** (`PhoneSettings`) turns each feature off on both devices. Files from the phone are accepted automatically by default (off → `ReceiveToast` asks). Notifications are Off / **In app** (default) / Desktop too.
- The phone is drawn flat, like a colour icon, and the hub uses the same flat boxes, hairlines and pills as the rest of the app; only the phone's screen text uses a sans (it's a picture of a phone, not app UI).

## How the pieces come together

`AppWindow` is the reference screen, and `DualPane` and `MotionSpec` are the other two. Build any new screen from the same grounds (`bg-deep` → `bg` → `bg-sunken` → `bg-raised`), put at most one `primary` button on each surface, keep colour for accent, selection, state and world, and check it in Echo, one light theme (Catppuccin Latte) and one warm dark theme (Gruvbox) before shipping.
