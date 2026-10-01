# ConnectDialog

**Connect to server** (Ctrl+Shift+S, the Network `+`, the command palette): pick a protocol, type or paste an address, connect.

**Provide** `protocol`, `value`, `parsed` (`ok` | `error`), `describe` (the parse, in words), `state` (`connecting`), `error` (why the last try failed), `recent`, `nearby`.

- **Protocol** `SegmentedControl`: SMB · SFTP · FTP · FTPS, with what it is beside it ("Windows share"). A scheme typed or pasted (`sftp://…`) switches it; switching rewrites a typed scheme.
- **Address**: one field that takes every form people have — `nas.local/Media`, `\\nas\Media` (UNC), `smb://WORK;alice@nas:4445/Media`, `me@host:/srv` (scp style), `ftp://[::1]:2121`. Without a scheme a chip shows the one that will be used. Passwords in an address are ignored; the sign-in dialog asks.
- Under the field, live: a `success-ink` check and the parse in words ("Windows share “Media” on nas.local · port 4445 · as alice"), or a `warning-ink` reason ("“99999” isn't a port number (1–65535)"). **Connect** is disabled until it parses.
- **Add to sidebar** (on by default) saves the server — never a password — to `settings.toml` `[[network.servers]]`.
- **Recent** (up to 4, one per server) and **On this network** (mDNS/Avahi: `_smb._tcp`, `_sftp-ssh._tcp`, `_ftp._tcp`, with a rescan button): one click connects.
- Connecting: the button reads **Connecting…**; if the server asks for a password the dialog steps aside for `SignInDialog`. Failures stay in the dialog in a `danger-soft` box, in words that say what to do: "Nothing answered on nas:4445. Check the address and port, and that the server is running." / "Couldn't find a server called “nas”…" / "nas has no share called “Media”. Leave the share out to see the ones it has."
- If `gvfs-smb` isn't installed, choosing SMB says how to get it. Esc cancels (a connection already on its way finishes quietly).
