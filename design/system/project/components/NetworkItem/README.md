# NetworkItem

A network place in the sidebar: its short name, then how and where — "SMB · nas.local", "SFTP · me@build-box".

**Provide** `name` (the saved name, else the share, else the host — never GVfs' `smb-share:server=…` folder), `protocol`, `where` (`user@host:port`, the port only when it isn't the usual one), `state` (`connected`, `connecting`, or nothing for not connected), `active`.

- Connected: the glyph turns `world-network` and a 24px **Disconnect** (`arrow-up`) button sits at the row's end, inside the row so the highlight covers it. Not connected: glyph and name are `ink-muted` and there is no pill — the row itself says it.
- Connecting: a `Connecting…` `info` pill replaces the second line while GVfs works; the sign-in dialog appears if the server asks.
- Click: connected → opens it (SFTP lands in the home folder on the server); not connected → connects and opens; a bare SMB server → `SharesPage`. Right-click: Open / Open in new tab / Disconnect, or Connect; Copy address; Edit address…; Remove from sidebar (saved) or Keep in sidebar (live).
- Glyphs: `network` for SMB, `server` for SFTP and FTP. The tooltip is the full address and its state.
- When a server goes away, GVfs drops the connection: panes inside it go home and a toast says "Lost the connection to Media".
