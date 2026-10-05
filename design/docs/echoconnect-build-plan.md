# EchoConnect implementation and verification

Updated 5 October 2026. Scope source: [product plan](echoconnect-plan.md). UI source: [EchoConnect design system](../connect/project/README.md). Physical target supplied by the owner: **Samsung Galaxy S24+, Android 15**; exact One UI build not yet recorded.

## Implemented in this working tree

| Scope | Android implementation | EchoFiles / shared core |
| --- | --- | --- |
| Connection and pairing | Connected-device foreground service, network rediscovery, QR scanner, matching-code pairing, trust persistence, Bluetooth reconnect | Shared TLS protocol, QR fingerprint pinning, certificate-bound authorization and payloads, bounded protocol work, forget/revoke handling |
| Design | Compose port of the EchoConnect design system (AppBar, BottomNav, ListGroup/ListRow, Switch, SegmentedControl, StatePill, LaptopCard, ActionGrid, ClipItem, ClipModeCard, TransferRow, PermissionRow, StepList, Stepper, PairCode, CallBar, Banner, Snackbar, Dialog, BottomSheet); five-step first run; Home / Clipboard / Transfers / Settings, Permissions, Automatic clipboard steps checked live; Share sheet, scanner viewfinder and Ring screen; flat token-coloured illustrations (laptop with EchoFiles open, phone, pairing hero, empty states) and EchoFiles colour file icons; six themes and laptop theme matching | Theme token transmission and pairing/setup QR |
| Battery and ring | Battery broadcasts; full-volume alarm channel, DND bypass when granted, lock-screen notification, Stop and timeout | Battery display, low-battery warning, ring action |
| Clipboard | Foreground and automatic background capture, wireless debugging setup guide, quick tile, notification action, selected text, Android Share; image normalization; sensitive flag with hidden history | Wireless ADB QR and code setup, text and image clipboard, duplicate suppression, Bluetooth transport, private temporary image handling |
| Notifications | Notification listener, reply RemoteInput, action buttons, dismissal, per-app mute | Notification list, replies/actions, optional desktop notifications, OTP recognition |
| Texts and contacts | SMS conversations and sending, MMS metadata and attachment download, contact vCards and caller lookup | Threads, composer, attachment actions and contact names |
| Calls | Call state, mute/answer/end permissions, Use phone request, route indicator | BlueZ headset on demand, PipeWire answer/hangup, microphone/speaker loopbacks, route release on call end / Use phone |
| Files | Share target and document picker, Downloads publishing with actual save status, progress/cancel/retry; authenticated SFTP; direct list/read/write protocol | Transfers, SFTP browsing, direct reads from indexed results; no-overwrite atomic payload publication |
| Photos | Paged MediaStore images and videos, selected-photo access with reselection, thumbnails and originals | Timeline, folder categories, bounded thumbnail requests, copy/save/open/import with successful-transfer tracking |
| Capture | Notification-triggered camera and ML Kit document scanner | Capture request correlated to the folder that requested it |
| Offline names | Paged shared-storage listing | Completed index snapshots, expired/incomplete refresh handling, metadata-aware `ef find` and Everywhere results; unpair removes cached names |
| Camera-roll backup | Opt-in WorkManager, charging + unmetered network, date/ID cursor, saved-file acknowledgement | SHA-256 deduplication with original verification and stale-marker recovery; saves under Pictures/Phone/Backup |
| Extras | DND changes, lock command, open link on laptop, MediaSession controls, connectivity reports | Mako DND synchronization, away auto-lock, link handling, media controls and signal display |
| Build / distribution | Cargo NDK arm64 + x86_64, generated UniFFI Kotlin, Gradle debug APK and lint | Debug artifact CI and release signing/upload workflow; Obtainium-compatible APK asset name |

The product plan explicitly defers F-Droid and replacing all GVfs browsing with the direct protocol. Direct protocol endpoints and cached-result downloads are implemented; full folder browsing continues through SFTP/GVfs.

## Verification performed

- 5 October, API 35 emulator with the redesigned UI and the `android_smoke` fixture: first run, matching-code pairing, permission rows updating live, laptop → phone text and sensitive clipboard (hidden in history), phone → laptop clipboard (received by the fixture), laptop → phone file saved to Download/EchoConnect with the expected bytes, ring alarm, Ring screen and volume-key stop, Echo and Catppuccin Latte themes. Found and fixed a row whose action ran during composition (opening the source link whenever Settings showed).
- Rust phone/core/index regression suite, including certificate substitution, untrusted unpair/injection, file collision/symlink safety, SFTP containment, cancellation, offline search filters and index behavior.
- Desktop regression tests, including backup deduplication/stale markers, index paging and OTP context.
- Android debug build and lint for the arm64 and x86_64 native libraries. Lint has no errors; remaining warnings include dependency updates, style suggestions, compatibility attributes and launcher icon shape.
- API 35 emulator: matching-code pairing, battery, app restart/reconnect, clipboard in both directions, and file transfer in both directions with saved-byte verification. Final packaged APK was installed again: reconnect, sensitive clipboard and incoming file save passed with an empty crash buffer.
- API 35 emulator: selected-photo permission reports “Only allowed photos and videos” and exposes “Change shared photos”. Full-access requirements keep the backup cursor from skipping inaccessible items.
- API 35 emulator: automatic background clipboard copied in Android Settings, with READ_LOGS and overlay permission granted and the system log-access prompt accepted. The laptop fixture received the exact copied text.
- The emulator exposed a nested JNA callback crash during reconnect. Android event processing now runs on a serial Java executor; the same reconnect and background-clipboard scenario passed afterwards.
- The unpair regression test caught premature connection shutdown before notifying the peer. Revocation now drops queued feature data while delivering the final unpair message; the test passes.

Reproducible commands and emulator port translations are in [android/README.md](../../android/README.md). Local reports, screenshots and APKs live under `android/app/build/` and are ignored by Git. The packaged debug APK, checksum and source manifest live under `android/dist/`.

## Physical-device and release acceptance still required

These are verification or release prerequisites, not claims of completed testing:

- Install on the S24+ and record the exact One UI build. Exercise Wi-Fi changes, Bluetooth-only clipboard, screen off/on, reboot, process termination and Samsung battery restrictions.
- Place a real test call and verify both audio directions, microphone selection, Answer/Hang up/Use phone, reconnect and headset release. The emulator cannot establish a real HFP call.
- Verify SMS sending/receiving, MMS attachments, actual contact data, notification replies/actions and media playback with the phone’s chosen apps.
- Verify ring during DND and on the lock screen after granting notification, full-screen and DND access. Android/user channel settings can still suppress alerts.
- Verify photo/video permissions, a large photo library, camera and Google Play scanner output, and charging/unmetered backup including network interruption.
- Run the stock KDE Connect compatibility check on the Samsung from phase 0.
- Configure the project’s release-signing secrets, then run release CI. This session produced a locally signed debug APK; it did not publish a release or provision production signing keys.

No measured battery-life claim or assertion of Samsung hardware validation is made by this implementation.
