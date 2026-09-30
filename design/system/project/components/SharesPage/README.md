# SharesPage

An SMB server's shares, when an address names no share (`smb://nas.local`, a server found nearby, a saved bare server).

**Provide** `host`, `shares` (`[name, state]`), `state` (`loading`).

- Shown in the pane like the Drives page: breadcrumb and tab carry the server with a `network` glyph; the status bar counts shares. Hidden `$` shares (IPC$, ADMIN$) are left out.
- Cards: `folder-share` icon, name, and **Connected** (`success`) / **Connecting…** (`info`) / "Windows share". Hover draws a `world-network` border. Click connects and opens.
- Asking the server may need a sign-in first (`SignInDialog`). Errors show in words with **Try again**.
