EchoConnect is the Android app for EchoFiles. It connects your phone to your laptop for files, photos, clipboard, texts, notifications and calls. The design has one job: **make the phone look and feel like it belongs to the same laptop**. It uses the same Omarchy colours, the same mono type, the same flat boxes, hairlines and pills, sized for thumbs.

EchoFiles' design system is the parent. Everything below either copies it or says how the phone differs.

## Principles

1. **One look on two screens.** The colour tokens, themes, icons, pills, stepper, pair code and the 2px accent edge are EchoFiles'. Someone moving from the laptop's Phone hub to this app should see the same object, not a new brand.
2. **The phone is mostly invisible.** EchoConnect works in the background: clipboard, notifications, texts and calls just cross. Screens exist to set things up, show state honestly and recover from problems. There's no feed, no engagement and no badges for their own sake.
3. **Flat Omarchy, never Material gloss.** No elevation tints, gradients, glows, blur or rounded hero cards. Grounds step `bg-deep` → `bg` → `bg-raised` and nothing else is layered. Shadows only lift things that float (sheets, dialogs, snackbars).
4. **Honest states, in words.** Connected, Not nearby, Automatic, Paused, Tap to send, Hidden: each is a pill with a word, never colour alone.
5. **Say what Android will ask, before it asks.** Every permission and every Settings detour is explained in plain words first, with the exact path on the phone, using Samsung's names on Samsung.

## Theme

- **Match laptop** (default): EchoFiles sends its current Omarchy theme over the connection (`echofiles.theme`), and EchoConnect applies the same derived tokens. Switching Omarchy themes on the laptop recolours the phone too.
- **Phone**: without a laptop, or if chosen, **Echo** in dark mode and **Catppuccin Latte** in light mode, following Android's dark-mode setting.
- The six themes in this system are EchoFiles' own: Echo, Tokyo Night, Catppuccin Latte, Gruvbox, Rosé Pine, Everforest. Check every new screen in Echo, Catppuccin Latte and Gruvbox.
- Android's dynamic colour (Material You) is ignored. The accent is always the laptop theme's `accent`.

## Content fundamentals

- **Voice:** plain, specific, calm, the same as EchoFiles. Say what happened and what to do next. No apologies, no exclamation marks.
- **Name both devices the way people know them:** the laptop by its name ("Aditya's laptop"), the phone as "this phone". Never an IP address in a title, never "device" or "peer".
- **Buttons are verbs that say exactly what happens:** "Send 3 photos", "Stop ringing", "Resume automatic", "Forget laptop". Never "OK" or "Continue" when something specific happens.
- **Paths on the phone use Android's own words, with arrows:** "Settings → About phone → Software information → Build number". On Samsung use One UI's names.
- **Explain permissions by what they unlock:** "Shows your notifications on the laptop", not "Required for NotificationListenerService".
- **Privacy statements are exact:** "Only the one line Android writes when a clipboard read is blocked." Wherever we ask for trust, add that EchoConnect is open source, with the repository: github.com/adityavardhansharma/EchoFiles_Linux.
- **Case:** sentence case. UPPERCASE only for `label` (section headers).
- **Numbers:** sizes base-1024 (`4.1 MB`), real transfer rates and time left, relative times within a day ("12 min ago"), then "Yesterday 18:22".
- **Emoji:** never.

## Visual foundations

### Colour

All colour tokens are EchoFiles'. EchoConnect spends colour on the same few things:

- `accent`: the one primary button per screen, the active bottom-nav bar, the 2px edge on sheets, dialogs and snackbars, progress, the QR corner brackets.
- **Semantic** `success` / `warning` / `danger` / `info`: states and results, always with a word.
- **World marks:** `world-linux` is **the laptop** (its name's 6px square, quick-action glyphs, clipboard items going to it). `world-phone` is **this phone** (clipboard items arriving). It's the same pairing as EchoFiles' sidebar, seen from the other side.
- `brand-*` only on the cover and onboarding art, never as state.

### Surfaces

```
bg-deep    bottom nav · gesture bar · field wells · the ringing screen
bg         status bar · app bar · screen ground · active nav item
bg-sunken  the stage behind the laptop drawing on Home
bg-raised  list groups · action tiles · cards · sheets · dialogs · snackbars   (+ scrim behind sheets)
```

### Type

One family: the Omarchy font, JetBrains Mono by default, the same as the laptop. Mono keeps file names, codes and OTPs legible (`l` `1` `I`, `O` `0`) and gives tabular figures for free.

- `body` 15/22 for rows, clipboard text and fields; `body-strong` 15/22 for buttons and the current step.
- `meta` 13/18 for subtitles and explanations, always `ink-muted`.
- `label` 11/16 bold uppercase, 0.08em tracking, for section headers (the same as EchoFiles).
- `caption` 12/16 for pills and nav labels.
- `title` 20/26 for app bars and the laptop's name; `headline` 17/24 for sheets and dialogs; `display` 28/34 once per screen at most (onboarding, ringing).
- **`system`** (Inter or the phone's own font) **only inside pictures of Android's UI**: the notification, quick-settings tile, text-selection menu and system dialogs. Our own screens are always mono.

### Space, size and layout

- 4px base, the same steps as EchoFiles (2 · 4 · 6 · 8 · 12 · 16 · 24 · 32). `space-5` (16px) is the screen gutter.
- Touch: every target is at least `touch` (48px). Buttons are `control-touch` (40px) with a padded hit area, `lg` buttons are 48px. Switches are 44 × 24.
- Fixed geometry: `appbar` 56 · `navbar` 64 · `row-one` 56 · `row-two` 72. Mock-ups use the 360dp `screen` width; layouts stretch to any width.

```
┌ status bar 30 ──────────────── 10:42 ● ᛒ ▾ 72% ┐
├ app bar 56 ─ Title ……………………………… [action] [action] ┤
│ gutter 16                                      │
│  LABEL                                See all  │
│  ┌ bg-raised group ─────────────────────────┐  │
│  │ [36] Title                    [trailing] │  │  row 56 / 72
│  ├──────────────────────────────────────────┤  │
│  └──────────────────────────────────────────┘  │
├ bottom nav 64 ─ Home │ Clipboard │ Transfers │ Settings ┤
└ gesture bar 18 ────────────────────────────────┘
```

### Borders, radius, elevation

- `line` hairlines between rows and around groups; `line-strong` outlines controls.
- Radii are EchoFiles': `radius-1` pills, `radius-2` buttons, fields, groups and tiles, `radius-3` thumbnails, `radius-full` switch and avatars. Sheets, dialogs and snackbars are **square**, like Hyprland's windows.
- **The accent edge:** sheets get a 2px `accent` top edge; dialogs and snackbars a 2px `accent` border (`danger` / `success` by tone). It's Omarchy's active-border language, as on EchoFiles' dialogs and toasts.
- `shadow-float` only on floating things. Never on rows, groups or tiles.

### States

| State | Treatment |
| --- | --- |
| Pressed row / tile / button | `state-press` layer (22%), `dur-fast` |
| Hover (mouse, stylus) | `state-hover` layer (8%) |
| Toggled icon button | `state-active` + `accent-ink` |
| Current nav item | `bg` ground + 2px `accent` top bar + `accent-ink` label |
| Focus (keyboard, switch access) | 2px `focus-ring` outline, 1px offset |
| Disabled | `opacity-disabled`, label kept; dependent rows say what they depend on |
| Paused / needs action | `warning` border on the card + a primary fix button |

### Iconography

- **Glyphs** (`Icon`): the EchoFiles set, plus ten phone additions (`bluetooth`, `laptop`, `camera`, `mic`, `speaker`, `call`, `scan`, `moon`, `cursor-text`, `flashlight`) drawn to the same 24-unit, 2px, round-cap rules. Sizes 20 (rows, buttons), 22 (nav), 16 (pills, small buttons).
- **Colour icons** (`FileIcon`): the EchoFiles 48-unit icons for files and places, recoloured per theme through `icon-*` slots.
- **Devices are drawn flat** from icon slots: the phone frame (`icon-slate-deep` body) and `LaptopDevice`, which shows EchoFiles open on its screen in live tokens. Each app draws the other device: EchoFiles draws the phone, EchoConnect draws the laptop.
- **Logo:** EchoConnect has no mark of its own yet. The launcher icon uses the EchoFiles logo (Logos group) until one is drawn. Never inside the working UI.

### Motion

The same durations and easings as EchoFiles: `dur-fast` 90 · `dur-base` 140 · `dur-slow` 220 · `dur-ambient` 1400.

| Moment | Motion |
| --- | --- |
| Tab switch, list scroll, typing | none |
| Press | colour layer, `dur-fast` |
| Sheet | rises 24px + scrim fade, `dur-slow` |
| Dialog | scale 0.98 → 1, `dur-slow` |
| Snackbar | rises 12px, `ease-spring` |
| Transfer | linear progress from real bytes; success tick at 100% |
| Ringing | two flat outlines fade in and out, `dur-ambient` |

Android's "Remove animations" and `prefers-reduced-motion` set every duration to 0.

### Accessibility

- Contrast comes from EchoFiles' derivation: `ink` 7:1, `ink-muted` and every `*-ink` 4.5:1, borders and marks 3:1, in every theme.
- 48px touch targets, TalkBack labels on every icon-only button, states in words.
- Text scales with Android's font size; rows grow and nothing truncates the meaning (subtitles wrap to two lines).

## Navigation

Four tabs in `BottomNav`: **Home**, **Clipboard**, **Transfers**, **Settings**. Pushed screens (Permissions, Automatic clipboard) have Back in the app bar. First run is `PairingScreen`.

- **Home** (`HomeScreen`): `LaptopCard`, `CallBar` during a call, `ActionGrid` (Send files, Send clipboard, Open on laptop, Lock laptop), the last two clipboard items, the last two transfers.
- **Clipboard** (`ClipboardScreen`): `ClipModeCard`, Send clipboard now, history both ways for 24 hours.
- **Transfers** (`TransfersScreen`): moving now, then by day.
- **Settings** (`SettingsScreen`): laptop and links, features shared with the laptop (mirrored with EchoFiles → Settings → Phone), receiving, theme, permissions, About with the open-source line.

## How the phone reaches the laptop

- **Wi-Fi** carries everything: files, photos, texts, notifications, battery, ring.
- **Bluetooth** carries only **calls** (the laptop is a hands-free headset; Answer on laptop connects the audio link on demand) and **clipboard** (works off Wi-Fi too).
- `LinkPills` shows both; Settings explains both in one line each.

## Clipboard, both ways

- **Laptop → phone:** always automatic. Android lets any app write the clipboard.
- **Phone → laptop: Automatic** (recommended). A one-time setup done with EchoFiles (`ClipboardSetupScreen`): turn on Developer options and Wireless debugging, scan the laptop's code (EchoFiles grants the log permission), allow Display over other apps, **turn Developer options off again**. After each restart Android asks once to allow log access (`SystemDialog`); until then the mode shows **Paused**.
- **Phone → laptop: Tap to send** for anyone who skips setup: Send to laptop in the text-selection menu (`SelectionMenu`), the quick-settings tile (`QuickTile`), the notification's Send clipboard action, Share → EchoConnect, and sending when the app opens. These also work in Automatic mode.
- Sensitive clips (from password managers) cross but are never kept in history and show as dots.

## How the pieces come together

`AppMap` shows every main screen side by side and is the reference composition; `SystemSurfaces` shows where EchoConnect appears inside Android. Build any new screen from `AppBar` + `ListGroup`s of `ListRow`s + at most one primary button. Check it in Echo, Catppuccin Latte and Gruvbox, and next to EchoFiles' `PhoneWindow`, before shipping.
