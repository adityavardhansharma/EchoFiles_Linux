# CloudConnectDialog

**Connect Google Drive** (the Cloud `+`, **Add Google Drive**, the command palette, Settings → Google Drive): choose access, then sign in on Google's page in the browser through rclone.

**Provide** `step` (`start` default, `waiting`, `missing`), `access` (`full` | `read`), `error`, `again` (signing in again after a client ID change), `old` + `need` (rclone too old), `command` (install command for this distro).

- **start**: one sentence that sign-in happens in the browser through rclone and EchoFiles never sees the password. **Access** `SegmentedControl` Full · Read-only with what each means beside it. **Sign in with browser** (primary, Enter) and **Cancel** (Esc).
- **waiting**: title "Finish signing in in your browser" (or "Sign in again for your app"), a `Spinner`, what to do, **Copy sign-in link** (the URL rclone printed, for when no browser opened) and **Cancel**, which stops rclone and removes the half-made account. After 5 minutes it closes with "Sign-in timed out."
- **missing**: rclone isn't installed (or is older than 1.64): the install command in a `bg-deep` well with a copy button, and **Check again**. Never installs anything itself.
- Errors stay in the dialog in a `danger-soft` box, in words that say what to do: "Another sign-in is using port 53682. Finish or close it, then try again." / "Google didn't allow access. Try again and choose Allow." / "Couldn't reach Google. Check the internet connection."
