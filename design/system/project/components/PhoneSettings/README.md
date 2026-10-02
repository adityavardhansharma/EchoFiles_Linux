# PhoneSettings

**Settings → Phone** (also the gear on the hub): what this phone may do.

**Provide** `background` (Keep running in the background is on).

- **This phone** — thumbnail, name, "Connected over Wi-Fi · paired 2 Oct 2026", **Forget phone** (removes the pairing on both sides); **Phone picture**: Latest photo / EchoFiles.
- **Features** — Files and photos (on), Shared clipboard (on), Messages (off until turned on; the phone asks for SMS permission), Low battery warning (on). Turning one off stops it on both devices: EchoFiles stops asking and drops anything the phone still sends.
- **Receiving files** — **Accept files automatically** (on by default; off asks each time with `ReceiveToast`), **Save to** `~/Downloads/Phone` with **Change…**.
- **Notifications** — Off / **In app** / Desktop too, with a note when Keep running in the background is off (pop-ups then stop with the window); **Never show notifications from**: chips of app names with an add field.
- With no phone paired the page shows one row: "No phone yet" and **Connect phone**.
- Saved to `settings.toml` `[phone]` (features, receive dir, notification mode, muted apps); the pairing itself lives in `~/.config/echofiles/phone/trusted/`.
