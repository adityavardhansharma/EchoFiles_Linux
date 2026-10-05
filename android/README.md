# EchoConnect

EchoFiles’ Android companion. Primary physical test target: **Samsung Galaxy S24+, Android 15 (API 35)**. The exact One UI build still needs recording from that phone.

The app uses the shared Rust protocol and SFTP crates through UniFFI. Kotlin supplies Android permissions, services, providers and a Compose UI generated from [the EchoConnect design system](../design/connect/project/README.md).

## Build and install

Requirements: Java 21, Rust with `aarch64-linux-android` and `x86_64-linux-android`, `cargo-ndk`, Android SDK platform `android-37.0`, build tools 36, and NDK `30.0.16248370`. The app runs on API 29+; its target SDK is 36. Java 27 is not supported by the current Gradle tooling.

```sh
export ANDROID_HOME="$HOME/Android/Sdk"
export JAVA_HOME="/path/to/jdk-21"
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk --locked
cd android
./gradlew assembleDebug lintDebug
"$ANDROID_HOME/platform-tools/adb" install -r app/build/outputs/apk/debug/app-debug.apk
```

The verified local package is `android/dist/EchoConnect-debug.apk` (about 71 MiB), with `SHA256SUMS`, source hashes and build/test reports alongside it.

Gradle builds both native ABIs and generates the Kotlin bindings. The debug APK is signed with the local Android debug key. It is suitable for device testing; it is not a production release signed with the project’s release identity.

Regenerate checked-in design assets after editing the source design:

```sh
python android/tools/tokens.py
python android/tools/glyphs.py
```

## Pair and enable features

Open EchoFiles → Connect phone and scan its QR from EchoConnect. The QR pins the laptop’s TLS certificate; manual discovery uses a matching code on both screens. Keep both devices on the same Wi-Fi. Allow TCP/UDP 1714–1764 through the laptop firewall where required.

Use Settings → Permissions and battery to enable only the features you need. SMS, contacts, notifications, files and photos have separate Android grants. On the S24+, set EchoConnect’s battery usage to Unrestricted and add it to Samsung’s Never sleeping apps. Bluetooth calls require an OS Bluetooth bond and PipeWire’s hands-free profile; the EchoConnect RFCOMM clipboard link uses the same certificate trust as Wi-Fi.

Automatic clipboard needs the one-time READ_LOGS grant from EchoFiles and Display over other apps. The laptop supports Android’s wireless-debugging QR and the six-digit pairing-code fallback. Turn Developer options off afterwards. Android can ask for one-time log access again when the process restarts; allow that prompt, or use Tap to send. EchoConnect filters for its own clipboard-access denial line and does not record unrelated log output. Automatic clipboard pauses while disconnected or the screen is off. Text selection, quick tile, notification action and Android Share remain available.

Photo sharing supports selected photos and videos; Settings → Photos → Change shared photos lets you revise that selection. Camera-roll backup pauses unless all photos and videos are allowed, so its chronological cursor cannot skip hidden items.

Camera-roll backup requires opt-in, full photo and video access, an unmetered network, charging, and the laptop online. WorkManager schedules it periodically, so it is not immediate. A cursor advances only after the laptop acknowledges saved content; retries are deduplicated by verified SHA-256 content. Backup goes to `~/Pictures/Phone/Backup`.

DND integration on the laptop uses Mako. Calls use BlueZ and PipeWire; Answer connects the hands-free profile and two audio streams, and Use phone releases them without changing default audio devices. Camera capture needs a camera app; document scanning also needs Google Play services and its downloaded scanner module.

## Checks

```sh
cargo test -p echofiles-phone -p echoconnect-core -p echofiles-index --lib --tests -- --test-threads=1
cargo test -p echofiles --bin echofiles
cd android && ./gradlew assembleDebug lintDebug
```

The interactive integration fixture uses disposable pairing credentials:

```sh
cargo run -p echofiles-phone --example android_smoke
```

For an API 35 emulator, forward its link port with `adb forward tcp:18716 tcp:1716`, then enter `connect` and `pair` in the fixture and accept the code in EchoConnect. The fixture prints the matching laptop code. `clip`, `sensitive`, `file`, `ring`, and `theme` exercise incoming features. `adb reverse tcp:1739 tcp:1739` lets the phone receive a fixture file. To test phone → fixture uploads with that reverse rule active, forward `tcp:1740` to emulator `tcp:1740`; remove that forward before fixture uploads need the port. On normal Wi-Fi these emulator-only port translations are unnecessary.

Verified on the API 35 emulator: pairing, battery, reconnect after process restart, text both ways, automatic clipboard copied from another foreground app after granting access, and file contents saved correctly in both directions. The native callback bridge dispatches events on a serial Java thread so feature replies cannot recursively detach a JNA callback thread.

The [scope and device checklist](../design/docs/echoconnect-build-plan.md) separates implemented features from physical-device verification.

## Releases

Releases are manual: **Actions → EchoConnect release → Run workflow** with a version runs the shared-core tests, then signs the release APK and attaches `EchoConnect.apk` and `SHA256SUMS` to the `echoconnect-v<version>` GitHub release (a draft unless you untick **draft**). **Actions → Rust regression tests** runs the whole workspace's tests and Clippy on demand. Nothing runs on pushes or pull requests. Configure these repository secrets before publishing:

- `ECHOCONNECT_KEYSTORE_BASE64`
- `ECHOCONNECT_KEYSTORE_PASSWORD`
- `ECHOCONNECT_KEY_ALIAS`
- `ECHOCONNECT_KEY_PASSWORD`

Keep the signing key backed up privately: upgrades require the same key. Release assets are named `EchoConnect.apk` for Obtainium. F-Droid distribution is deferred in the product plan. No release has been published by this implementation session.
