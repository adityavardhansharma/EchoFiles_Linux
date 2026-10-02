# EchoConnect — the EchoFiles Android app

Decided 2 Oct 2026. Design system: [EchoConnect](https://claude.ai/artifact/V8jwyq1KvEvTJwWqJrHiq3) (sources in `design/connect/`).

## Why our own app

The laptop already speaks the KDE Connect protocol itself (`crates/phone`, no `kdeconnectd`). The phone still runs the stock KDE Connect app, and it can't send the clipboard until you press **Send clipboard**. That isn't a KDE bug. Since Android 10, a normal app can only read the clipboard when it's the app on screen or the current keyboard. Writing our own app doesn't lift that rule on its own, but it lets us:

- automate phone → laptop clipboard with the one-time setup below;
- own pairing (QR), the look (same tokens as EchoFiles), and features KDE Connect doesn't have;
- keep one protocol implementation for both ends.

## Decisions

| Topic | Decision |
| --- | --- |
| Name | **EchoConnect** |
| Platform | Android only |
| Fork KDE Connect Android? | **No.** Own app. Their source is reference only (GPL; we're GPL-3.0-or-later too) |
| Protocol | KDE Connect protocol 8 messages where they exist; our own `echofiles.*` messages for new features. Stock KDE Connect keeps working with EchoFiles as a fallback |
| Core | **Rust**: `crates/phone` shared by laptop and phone, built for Android with `cargo-ndk` and exposed to Kotlin with UniFFI |
| Android shell | Kotlin + Jetpack Compose, drawn from the EchoConnect design system (flat, Omarchy tokens, mono type) |
| Connections | **Wi-Fi for everything.** **Bluetooth only for calls** (laptop as hands-free headset) **and clipboard** |
| Clipboard phone → laptop | **Automatic** via a one-time developer-mode setup (main path). **Tap to send** for anyone who declines |
| Clipboard laptop → phone | Always automatic; Android allows background clipboard writes |
| Distribution | APK on GitHub Releases, installable and updatable through Obtainium; F-Droid later. Not the Play Store, which restricts the SMS, log and accessibility permissions this needs |
| Test phone | Samsung (One UI) |

## Clipboard, phone → laptop

### Automatic (default offer)

One-time, about 2 minutes, run together with EchoFiles:

1. Phone: Settings → About phone → Software information → tap **Build number** 7 times (Samsung path).
2. Phone: Developer options → **Wireless debugging** → **Pair device with QR code**.
3. Laptop: EchoFiles → Settings → Phone → **Make clipboard automatic** shows a QR code. EchoFiles pairs over wireless `adb` and runs only `pm grant <app> android.permission.READ_LOGS`. Needs `android-tools` (add to `optdepends`).
4. Phone: allow **Display over other apps**. This is a normal Settings toggle, and it lets the app open its invisible window from the background.
5. Phone: **turn Developer options off.** EchoConnect checks this and says so. The permission survives turning them off, reboots and app updates; only uninstalling removes it.

How it works: the app watches the system log for the single line Android writes when a background clipboard read is denied. It then opens a transparent window for a moment, which counts as being on screen, reads the clipboard and sends it. It never stores or sends anything else from the log. The app says this in plain words and points at the open-source code.

Costs and caveats:
- Android 13+ asks "Allow EchoConnect to access all device logs?" once **after each restart**. Until you tap Allow, the mode shows **Paused** with a Resume button.
- Banking apps only check whether Developer options are on *now*, so they're fine once step 5 is done.
- Battery: well under 1% a day (one blocking log reader; it sleeps with the CPU). The reader runs only while the laptop is connected and pauses with the screen off.
- If a future Android version breaks this, the app detects it, says so, and Tap to send keeps working.

### Tap to send (no setup; also available in Automatic mode)

- **Send to laptop** in the text-selection menu (official `PROCESS_TEXT` intent)
- the quick-settings tile
- the **Send clipboard** action on the always-on notification
- Share → EchoConnect
- the clipboard is sent whenever the app opens

Not used: an accessibility service (some Indian banking apps refuse to run while an unknown one is on, and it only guesses at copies) and a custom keyboard (too heavy).

## Protocol

### Reused KDE Connect messages

| Feature | Messages | Laptop today | Phone (Kotlin glue) |
| --- | --- | --- | --- |
| Connection | identity, `kdeconnect.pair`, TLS, payload ports 1739+ | done | foreground service (connected-device type), network changes |
| Ping | `kdeconnect.ping` | done | — |
| Battery | `kdeconnect.battery`, `.request` | done | BatteryManager |
| Ring | `kdeconnect.findmyphone.request` | done | full volume through DND, full-screen Stop |
| Clipboard | `kdeconnect.clipboard`, `.connect` | done | set clipboard; detection as above |
| Notifications | `kdeconnect.notification`, `.request`, `.reply`, `.action` | `.action` missing | NotificationListenerService, RemoteInput |
| SMS | `kdeconnect.sms.*` incl. attachments | attachments missing | SMS content provider, SmsManager |
| Share | `kdeconnect.share.request`, `.update` | done | Downloads, share target |
| Browse files | `kdeconnect.sftp`, `.request` | done (GVfs mount) | SFTP server in Rust (`russh` + `russh-sftp`) |
| Contacts | `kdeconnect.contacts.*` | to add | ContactsContract |
| Calls | `kdeconnect.telephony`, `.request_mute` | to add | call state, ringer mute |
| Media | `kdeconnect.mpris` | later | MediaSession |
| Signal | `kdeconnect.connectivity_report` | later | TelephonyManager |

The clipboard message gets an optional `"sensitive": true` field (stock apps ignore unknown fields).

### Our own messages and features

| Feature | Shape |
| --- | --- |
| Theme match | `echofiles.theme`: laptop sends its derived Omarchy tokens; the phone recolours to match |
| Photos timeline | `echofiles.photos.list` (paged by date), `echofiles.photos.thumb` (payload) |
| Fast file access (later) | `echofiles.files.list` / `.read` / `.write` over the existing link, replacing the GVfs mount and its FUSE hangs |
| QR pairing | not a message: the QR carries laptop id, address, Bluetooth address and certificate fingerprint; pairing auto-approves on a match |
| Image clipboard | clipboard payload with a MIME type; small images over Bluetooth, large ones over Wi-Fi only |
| One-time codes | the laptop recognises OTPs in SMS and notifications, copies them and says so (opt-in) |
| Take photo / Scan document into a folder | laptop asks; phone opens the camera or document scanner; the file lands in the open EchoFiles folder |
| Phone files in search | phone file list added to the EchoFiles index, so `ef find` covers it even when the phone is away |
| Camera-roll backup | new photos to a laptop folder, on Wi-Fi and while charging, deduplicated |
| Do Not Disturb sync, auto-lock when the phone leaves, open link on the other device | small messages |

Not possible on Android for a normal app: hotspot control, screen mirroring without adb, reading WhatsApp or RCS beyond notifications.

## Bluetooth

- **Calls:** the laptop is a Bluetooth hands-free unit. This machine's PipeWire 1.6.9 already registers the Handsfree profile and exposes call control on D-Bus (`org.pipewire.Telephony`: answer, hang up, dial, caller ID). EchoFiles shows the call UI; the phone app adds the contact name.
- **Call routing (open):** connect the headset link **only when you press Answer on laptop** (recommended; ~1–2 s), or keep it connected while the laptop is unlocked.
- **Clipboard:** an RFCOMM channel (`bluer` on Linux, Android's Bluetooth API on the phone, bytes handed to the Rust core), with the same TLS as Wi-Fi. Each clip carries an id, so one arriving over both links is applied once.
- Everything else is Wi-Fi only.
- The Rust core gets a transport layer (any byte stream) in phase 1 so Bluetooth slots in later.

## Phone permissions

Notification access · SMS · Contacts · Phone · Nearby devices (Bluetooth) · All files access · Display over other apps · Notifications (for the foreground notification) · Battery: Unrestricted. On Samsung also: Settings → Battery → Background usage limits → **Never sleeping apps** → add EchoConnect.

Each one is asked for in context, says what it unlocks, and stops being used when its feature is turned off.

## Tech stack

- **Rust core** (`crates/phone`): discovery, TLS (rustls + rcgen), pairing, message routing, payloads, transports (TCP, Bluetooth stream), SFTP server.
- **Android:** Kotlin, Jetpack Compose, `cargo-ndk`, UniFFI, CameraX + ML Kit barcode (QR), ML Kit document scanner, NsdManager, NotificationListenerService, MediaStore.
- **Laptop:** EchoFiles (iced), `bluer`, PipeWire telephony over `zbus`, `adb` for the clipboard setup.
- **CI:** GitHub Actions builds the signed APK next to the Arch package.

## Roadmap

| Phase | Work | Done when |
| --- | --- | --- |
| 0 | Laptop: debug log for the phone link (`EF_PHONE_LOG=1`); stop the `wl-paste` watcher when EchoFiles exits (it currently outlives it); fix laptop → phone clipboard with the stock app on the Samsung | Laptop → phone clipboard works with stock KDE Connect |
| 1 Core | Transport layer in `crates/phone`; Android build + UniFFI; app skeleton: foreground service, discovery, QR pairing, battery, ring | EchoConnect pairs with EchoFiles and rings |
| 2 Clipboard | Both directions; Automatic setup flow on both ends; Tap to send surfaces; image clipboard; sensitive clips | Copy on the phone, paste on the laptop, no taps |
| 3 Messages | Notifications + replies + actions; SMS + attachments; contacts; one-time codes | Stock KDE Connect not needed for these |
| 4 Calls | Telephony messages; PipeWire call UI in EchoFiles; Bluetooth headset on demand; Bluetooth clipboard | Answer a call on the laptop |
| 5 Files | Share both ways; SFTP server in Rust; photos timeline; take photo / scan into folder | Uninstall stock KDE Connect |
| 6 Extras | Phone files in search and `ef`; camera-roll backup; DND sync; auto-lock | — |
| 7 Hardening | Reconnects across network changes, One UI battery guidance, signed CI builds, design-system pass | Daily use |

Every UI phase updates both design systems (EchoFiles for the laptop side, EchoConnect for the phone).

## Open questions

1. Call routing: headset link on demand (recommended) or always while unlocked.
2. Samsung model and One UI version, to test the setup flow and background limits.
3. Whether one-time codes should also cross over Bluetooth (currently Wi-Fi only).
