# Google Drive in EchoFiles: system design

Status: design, not built. Written 2026-10-25.

This document describes how to build Google Drive support in EchoFiles: what each part does, how the parts talk to each other, and why each decision was made. It contains no code. Anyone implementing this should be able to do it from this document alone, and should come back and update it when reality disagrees with it.

**All UI work follows the EchoFiles design system** in `design/system/project/` (start with its `README.md`, then the component READMEs). Nothing in this feature gets its own look. Where the design system doesn't cover something yet (a fifth world, a cloud sidebar item), the design system is updated first, in `design/`, and the Rust UI follows it. Never the other way round.

---

## 1. Summary

- **Access is the goal.** People browse, open, copy, move, rename and delete Google Drive files in the normal EchoFiles window, as if Drive were another drive in the sidebar.
- **rclone does the talking to Google.** EchoFiles doesn't ship its own Google OAuth app. It runs the user's installed `rclone`, which has its own Google-approved app. Users sign in to rclone in their browser, and EchoFiles never holds Google credentials.
- **Users can bring their own client ID.** Settings has an optional "Use my own client ID" field. If it's empty, nothing changes. If filled, rclone uses that app instead, which removes the shared app's speed limits and unlocks near-live index updates.
- **Indexing is opt-in.** A Settings switch, off by default, saves Drive *metadata* (names, folders, sizes, dates, hashes, Drive IDs) on this computer so search answers instantly. File contents are never indexed.
- **Two separate switches, both off by default.** Indexing Google Drive is its own switch, separate from every other index setting (Settings → Search & index never turns it on or off). `ef` reaching Google Drive is another separate switch, in the ef / AI agents options; turning on the Drive index doesn't give `ef` access, and the other way round.
- **Search has two steps.** The local index (if on) answers in milliseconds. Then the same search goes to Google Drive live, and new matches are added to the results and the index while the user looks. Because of this, the index never has to be perfectly fresh.

## 2. Decisions already made, and why

| Decision | Why |
|---|---|
| rclone instead of GVfs/GNOME Online Accounts | GOA needs GNOME's settings app to add an account, and Omarchy/Hyprland doesn't have it. GVfs's Google backend shows files by Drive ID through FUSE and is slow. rclone has real file names, a good cache, and handles Drive's quirks. |
| rclone instead of our own Google client | A file manager needs full Drive access, which Google calls a "restricted scope". Shipping our own app means Google verification plus a yearly paid security assessment. Unverified apps stop at 100 users. We can't carry that. |
| Never copy rclone's client ID into our code | It is public in rclone's source, but using it would pass EchoFiles off as rclone. That breaks Google's terms and rclone's wishes. We only ever run the rclone program. |
| Mount Drive as a folder (FUSE), not a fully remote backend | Every existing feature (listing, ops, drag and drop, open with) works on a path. This matches how network places already work through GVfs's FUSE bridge. A real `Location::Remote` can come later if the mount feels slow. |
| Index metadata only, off by default | Privacy. Metadata is small (tens of MB for a large Drive); contents would be huge and pointless. |
| Two-step search | Removes the need for frequent background re-crawls, and the user always sees fresh results within a second. |
| `ef` stays index-only by default | `ef` exists to be faster than `fd`. A network round trip breaks that. Live results are behind a flag. |

## 3. Words used in this document

- **Account**: one connected Google account. Internally an rclone "remote" in EchoFiles' own rclone config.
- **Account ID**: a short random ID EchoFiles creates per account (for example `k3f9q2`). Used in file names, URIs and the rclone remote name. Never the email address, never a mount path.
- **rcd**: rclone's long-running daemon with a JSON API ("remote control"). EchoFiles runs exactly one.
- **Mount**: the account's Drive shown as a folder through FUSE, served by the rcd.
- **Catalog**: the saved Drive index for one account (when the index switch is on).
- **Folder map**: an in-memory table of Drive folder ID → path. It always exists while an account is connected, even with the index off. It is never written to disk when the index is off.
- **Drive URI**: how Drive files are named in search results and pins: `gdrive://<account id>/<path inside Drive>`. Same idea as the existing `phone://<device>/<path>`.
- **Live search**: step 2 of search, a query sent to Google Drive through rclone.

## 4. Big picture

```
┌──────────────────────────── EchoFiles app (one process, runs from login) ─────────────────────────────┐
│                                                                                                        │
│  UI (iced)                                                                                             │
│   ├─ Sidebar: CLOUD section ── Google Drive item(s)                                                    │
│   ├─ File list on the mount path (existing listing pipeline)                                           │
│   ├─ Search results (step 1 catalog hits → step 2 live hits merged in)                                 │
│   └─ Settings → Google Drive page                                                                      │
│                                                                                                        │
│  ef-cloud crate (new)                                                                                  │
│   ├─ rclone locator + version check                                                                    │
│   ├─ Supervisor: starts/stops/restarts the one rcd process                                             │
│   ├─ RC client: JSON over a private Unix socket                                                        │
│   ├─ Accounts: our own rclone config file, create/sign in/sign out                                     │
│   ├─ Mounts: mount/unmount per account, stale mount clean-up                                           │
│   ├─ Folder map: folder ID ↔ path, in memory                                                           │
│   ├─ Crawler: streams `rclone lsjson` into a catalog                                                   │
│   ├─ Live search: our query → Drive query → results → paths                                            │
│   └─ Upload/transfer status: polls the rcd for pending uploads                                         │
│                                                                                                        │
│  ef-index: catalog search (generalised from the phone index) ── also used by `ef`                      │
└───────────────┬──────────────────────────────────────────────┬─────────────────────────────────────────┘
                │ Unix socket, JSON (rc)                         │ child processes (crawl, sign-in)
                ▼                                                ▼
        ┌──────────────── rclone rcd ────────────────┐    ┌──── rclone lsjson / rclone config reconnect ────┐
        │ VFS cache · mount(s) at $XDG_RUNTIME_DIR/… │    │ short-lived, same config file                   │
        └───────────────────────┬────────────────────┘    └──────────────────────┬──────────────────────────┘
                                ▼                                                ▼
                                       Google Drive API (rclone's app or the user's own)
```

### 4.1 Why a new crate (`crates/cloud`, package `echofiles-cloud`, lib name `ef_cloud`)

- `ef-net` is about GVfs over D-Bus. rclone is a child process plus HTTP over a socket, a different world with different failure modes. Mixing them would make both harder to read.
- `ef-phone` sets the precedent: one crate per "world".
- It keeps any future HTTP client dependency (section 12.4) out of the UI and the other crates.
- The crate never depends on iced. It exposes plain functions, plain data types and channels, like `ef-net` does, so the UI crate decides threading and `ef` can reuse the parts it needs.
- Name it "cloud", not "gdrive". rclone speaks Dropbox, OneDrive, Box and others. Everything Drive-specific (query language, Google Docs, export formats, shared drives) lives in one clearly marked Drive module, so another provider is mostly a new module later. Only Google Drive is in scope now.

### 4.2 What lives where

- **`ef-config`**: a new `Cloud` settings struct (section 11.1), new sidebar fields, and helpers for the cloud state, cache and config directories.
- **`ef-vfs`**: a new `VolumeId::Cloud(account id)` variant (section 7.3). `Location` stays `Local` only for now.
- **`ef-index`**: the phone index module is generalised into a "catalog" module used by both phone and Drive (section 9.2). `ef`'s find and agent code learn to add Drive hits the same way they add phone hits.
- **`ef-net`**: unchanged, except that "is this a slow/remote path" checks become one shared helper that also covers cloud mounts (section 7.4).
- **`ef-core`**: the ops engine learns about server-side copies within one Drive (phase 7) and about Drive's own trash (section 8.4).
- **UI crate**: new `cloud.rs` (state, messages, update) and `cloud_view.rs` (sidebar items, dialogs), following the `network.rs` and `phone.rs` / `phone_view.rs` split. Plus a new settings page.

## 5. rclone as a dependency

### 5.1 Finding rclone

- Look for `rclone` on `PATH`, then `/usr/bin/rclone`, then `/usr/local/bin/rclone`. Remember the path for the session.
- Run `rclone version` once, in the background, the first time anything cloud-related is needed (sidebar render with a connected account, opening Settings → Google Drive, or "Connect Google Drive"). Never at startup on the UI thread.
- Parse the version and compare against a **minimum version of 1.64.0** (section 18). That floor covers rc over a Unix socket, `mount/mount` in rc, `backend/command` in rc, Drive's `query` backend command, and `lsjson` with `--hash` and IDs. It was chosen from rclone's docs, not by running older binaries.
- Don't bundle rclone. Add it to the Arch package's `optdepends` with the reason "access Google Drive". Follow the style of the existing entries in `packaging/arch/PKGBUILD.in`.
- Never install rclone for the user, and never download a binary (both are against our safety rules and against good sense).

### 5.2 States the UI must show

| State | What the user sees |
|---|---|
| rclone not found | Sidebar: no Drive item. "Connect Google Drive" opens a dialog saying rclone is needed, with the install command for the detected distro (`pacman -S rclone` on Arch) and a copy button. |
| rclone too old | Same dialog, naming the installed and the needed version. |
| rclone found, no account | Sidebar CLOUD section shows only "Add Google Drive" (respecting the section's show/hide toggle). |
| Account connected | Normal Drive item (section 7.1). |

### 5.3 FUSE

- Mounting needs FUSE 3 (`fusermount3`). Check for it next to the rclone check. If it's missing, the account can still connect, but the Drive item shows "Can't show as a folder: FUSE isn't installed", with the package name. Browsing isn't possible without the mount in this design, so it's a blocking state with a clear fix.

## 6. The rclone process

### 6.1 One rcd, owned by EchoFiles

- EchoFiles starts one `rclone rcd` child process when there's at least one connected account. It's started from a background thread after the first frame is drawn, never before.
- That one process serves every mount and every rc call. One process means one VFS cache, one token refresh loop and one place to watch.
- It's started with:
  - **Our own config file**: `~/.config/echofiles/rclone/rclone.conf` (`--config`). Never the user's own `~/.config/rclone/rclone.conf`. The user may already have remotes there with the same names. We must never edit or read their setup, and signing out of EchoFiles must not touch their rclone.
  - **Cache directory**: `~/.cache/echofiles/rclone/` (`--cache-dir`), permissions 0700, since it holds real file contents.
  - **A Unix socket for rc**: `$XDG_RUNTIME_DIR/echofiles/rclone.sock`, with the directory at 0700.
  - **No rc web GUI.** No serving of files over rc. Only the API.
  - **Logging** at NOTICE level to `~/.local/state/echofiles/rclone.log`, rotated by size (keep the last two files of 5 MB). Never at DEBUG by default, because debug logs can contain request details.

### 6.2 Why a Unix socket and not TCP

- rc on a TCP port on localhost can be reached by every process of every user on the machine. rc can read and write files and change config.
- A socket inside a 0700 directory in the user's runtime directory is reachable only by the user.
- If phase 0 finds that the minimum rclone version can't serve rc on a Unix socket, the fallback is TCP on 127.0.0.1, a random free port, plus a random username and password generated per session and passed through rclone's environment variables, not its command line (section 13.2). Prefer raising the minimum version over using the fallback.

### 6.3 The RC client

- Plain HTTP/1.1 over the Unix socket, POST with a JSON body, JSON reply. It's a small hand-written client (a request line, a few headers, `Content-Length`, read the reply). No HTTP library, because there is no TLS and no redirects. That's in line with the project's preference for its own small code over big dependencies.
- One persistent connection per worker thread, reconnecting on error.
- Every call has a timeout. Defaults: 5 s for metadata calls, 10 s for live search, no timeout for mount and unmount but with a cancel path.
- Every call returns a typed result: success with the parsed reply, or an error type that separates: rcd not running, socket error, timeout, rclone error with its message, and Google errors detected from the message (rate limited, signed out / token revoked, storage full, not found, permission denied). The UI maps these to wording (section 10).
- Calls run on background threads, never on the UI thread. The live search worker gets its own connection so it never waits behind a slow mount call.

### 6.4 Lifetime and supervision

- **Start**: lazily, as above. The supervisor waits for the socket to accept a connection, then calls rc `core/version` as a health check. If that fails within 5 s, it counts as a failed start.
- **Restart**: if the process exits unexpectedly, restart after 1 s, 2 s, 5 s, 15 s, then 30 s between tries. After 5 failures within 2 minutes, stop retrying and show a banner on the Drive item: "Google Drive stopped responding. rclone keeps exiting — see the log." with "Try again" and "Open log" buttons.
- **Restore after a restart**: the supervisor re-mounts every account that was mounted before. The list of what should be mounted lives in EchoFiles' state, not only inside rclone.
- **Health**: a cheap `core/version` ping every 60 s while any account is connected. This catches a hung rcd (the process exists but doesn't answer). After two missed pings, kill and restart it.
- **Quit**: on app quit, call rc `vfs/stats` to see if uploads are pending. If they are, ask in a Dialog: "2 files are still uploading to Google Drive. Quit anyway?" with the buttons "Wait for uploads" (default) and "Quit and upload later". After the user decides, unmount everything (`mount/unmountall`), then `core/quit`. Wait up to 5 s, then kill.
  - "Quit and upload later" is safe: rclone keeps pending uploads in its cache and uploads them the next time the rcd starts with the same cache directory. Say that in the dialog's description line.
- **Crash of EchoFiles itself**: the rcd is started with a "parent death" signal, so it dies when EchoFiles dies. That can leave a stale FUSE mount ("transport endpoint is not connected"). At the next start, before mounting, the mount manager checks each expected mount point. If it's a dead FUSE mount, it lazily unmounts it with `fusermount3 -uz`, then mounts fresh.
- **Login session**: EchoFiles runs from login (see the startup memory: load once, keep warm). The rcd follows the app and isn't a separate systemd service. One owner, one lifetime, simpler debugging.

### 6.5 Resource cost (be honest about it)

- The rcd costs roughly 50–150 MB of memory while mounted, plus cache disk space. That's acceptable for a feature the user turned on, and it costs nothing when no account is connected (no process at all).
- Measure it in phase 0 and write the real numbers here.

## 7. Accounts, mounting and the sidebar

### 7.1 Sidebar

- **New section "CLOUD"** below NETWORK, with its own show/hide setting and its own fold state, exactly like the Windows/Network/Phone sections (`sidebar.cloud`, `sidebar.cloud_open` in config; default shown, open).
- **World mark**: the design system has four worlds today (Linux, Windows, Phone, Network). Cloud becomes a fifth world with its own token `world-cloud`. Proposed mapping: Omarchy's `cyan`, pushed to 3:1 on the sidebar, by the same rule as the other world marks.
  - `cyan` also feeds the `info` semantic colour. That's acceptable, because world marks only appear in the places the README lists, never as state. The design system owner decides; if rejected, use the Network world colour and place Drive under NETWORK instead.
  - Either way, this is a **design system change first**: `design/build_tokens.py`, `tokens.json`, `README.md` principle 3 and the "Colour" section, the theme crate's `world_*` fields, and the harness.
- **Item**: modelled on the existing `NetworkItem` / `DriveItem` components. Add a cloud variant to the design system (or a `CloudItem` component if the differences warrant it). Contents:
  - Colour icon: the generic `cloud` colour icon that already exists in `design/icons.json`. **No Google logo**, ever. The design system keeps brand marks out of the working UI, and it's not our trademark. The name "Google Drive" as text is fine.
  - Name: "Google Drive" by default. With more than one account, "Google Drive", "Google Drive 2", … Renamable from the context menu, and the name is stored in our config.
  - Usage bar: from rc `operations/about` (used / total quota), refreshed when the item is shown and at most every 10 minutes. Use the existing `UsageBar` rules, including "Drive mounted: usage bar grows once, 400 ms".
  - State pill (`StatePill`), only when not normal: "Connecting", "Offline", "Sign in again", "Uploading 3", "Not mounted".
- **Click**: opens the mount folder in the current pane. If not mounted, it mounts first, with a spinner in the item, then opens.
- **Context menu** (follow `ContextMenu`, every entry with its key hint where one exists):
  - Open
  - Open in new tab
  - Open in browser (opens `https://drive.google.com/` with `xdg-open`)
  - Refresh (re-reads the open folder; with the index on, also offers "Refresh index")
  - Rename…
  - Disconnect (unmount, stop using the network, keep the account and catalog)
  - Sign out… (opens the confirm dialog in section 7.5)
  - Google Drive settings

### 7.2 Connecting an account (sign-in flow)

Steps, in order:

1. The user clicks "Add Google Drive", in the CLOUD section, the command palette ("Connect Google Drive"), or Settings → Google Drive.
2. rclone and FUSE are checked (section 5). If they're missing, show the install dialog and stop.
3. A Dialog opens: title "Connect Google Drive". It explains in one line that sign-in happens in the browser, through rclone. One choice: Access, "Full" (default) or "Read-only", as a `SegmentedControl`. Buttons: "Sign in with browser" (primary) and "Cancel".
4. EchoFiles creates a new account ID and calls rc `config/create` to add a remote of type `drive` to **our** config. Options: scope `drive` (or `drive.readonly`); the user's client ID and secret if set (section 12); everything else at rclone defaults. **No sign-in yet.** Creating the remote over rc keeps the client secret off any command line (section 13.2).
5. EchoFiles starts a short-lived `rclone config reconnect <remote>:` child process with our config file. This runs rclone's own browser sign-in: rclone starts a small local web server on 127.0.0.1:53682 and opens the Google sign-in page with `xdg-open`.
   - rclone's published `rclone config` flow for a new Drive remote asks terminal questions before the browser (client ID, scope, shared drive, "use the browser?"). Step 4 therefore sets every one of those on the remote over rc (`scope`, optional client ID and secret, `team_drive` empty, `export_formats` = `link.html`) so reconnect has nothing left to ask. Never type into a terminal prompt programmatically. Whether `config reconnect` itself still prompts was not run here (section 18).
6. While this runs, the dialog changes to "Finish signing in in your browser", with a `Spinner`, plus:
   - "Copy sign-in link". rclone prints the URL to its output. We read it from the child's output so the user can paste it if no browser opened.
   - "Cancel", which kills the child process and deletes the half-created remote.
   - A timeout of 5 minutes, then the same as Cancel, with the note "Sign-in timed out."
7. When the child exits successfully, call rc `operations/list` on the Drive root (not recursive) as a check. If it works, the account is connected.
8. Save the account in EchoFiles config (ID, name, access), mount it (section 7.3), show the sidebar item, open the Drive root in the current pane, and show a Toast: "Google Drive connected".
9. If the index switch is already on (from an earlier account), start the first crawl (section 9.3).

Failure cases to handle with clear words (section 10): port 53682 busy (another sign-in, or another app), the user denied access in the browser, the network is down, rclone failed with any other message.

### 7.3 Mounting

- One mount per account, through rc `mount/mount` inside the shared rcd.
- **Mount point**: `$XDG_RUNTIME_DIR/echofiles/cloud/<account id>`. It's per-session, invisible in home, and tmpfs-backed. That matches where GVfs puts network places (`$XDG_RUNTIME_DIR/gvfs`).
  - Mount paths are **never** stored as identity (the build plan's rule: never key anything by mount path). Pins, recent folders, "last folder", search results and the catalog all store **Drive URIs** (`gdrive://<account id>/<path>`), and a resolver turns them into the current mount path.
  - `ef-vfs` gets `VolumeId::Cloud(account id)`, so anything keyed by volume can key Drive the same way.
  - Later, possible: a setting "Also show Google Drive in other apps" that mounts at `~/Google Drive`. Not in this version.
- **Mount options** (each one is a decision):
  - VFS cache mode **full**. Needed so other apps can open, seek in and save files normally. Lower modes break many editors.
  - Cache max size: default 10 GB, configurable in Settings. Cache max age: 24 h. Unused cached files are evicted after that.
  - Write-back delay: 5 s. Uploads start 5 s after a file is closed, so a quick save-and-save-again uploads once.
  - Directory cache time: 5 minutes, with Drive change polling every 60 s. rclone's Drive backend watches Drive's change feed and invalidates cached folders, so a folder changed on the web shows correctly within about a minute even though listings are cached.
  - Google Docs/Sheets/Slides export format for the mount: **`link.html`**, so in the mount and for other apps they are small HTML files that open the document in the browser. EchoFiles itself recognises them by mime type (section 8.2) and does better.
  - Follow shortcuts: yes for the mount (rclone's default), so a shortcut folder opens like a folder. The crawler does the opposite (section 9.3).
  - Use Drive's trash on delete: yes (rclone's default). Section 8.4.
  - No `allow-other`. Only this user sees the mount.
  - File names: rclone's default encoding for Drive. Phase 0 checks how names with `/` (allowed in Drive) and other unusual characters look.
- **Unmount**: on Disconnect, Sign out, quit, or account removal. If files from the mount are open in other apps, the unmount fails. Then show: "Couldn't disconnect Google Drive — files from it are open in other apps. Close them, or disconnect anyway." Buttons: "Disconnect anyway" (lazy unmount) and "Cancel". This mirrors the network unmount's force option.

### 7.4 "Remote path" rules shared with network places

- Today, `ef_net::gvfs::is_network_path` turns off thumbnails, trash and "reopen last folder" for GVfs folders. Replace the scattered checks with one helper (for example "is this path on a slow remote place") that answers true for GVfs folders **and** cloud mounts. Each call site then decides what that means. That's a small refactor, and it should land in its own PR before the Drive UI.
- For cloud mounts specifically:
  - **Thumbnails**: off in this version. Generating one means downloading the whole file. (Drive offers thumbnail links, but they need authenticated HTTP calls we don't make; see section 12.4 for when that could change.)
  - **Reopen last folder at startup**: allowed if the mount comes back within 2 s. Otherwise open home and show a small banner: "Google Drive is still connecting." This differs from network places, which are always skipped.
  - **Free space** in the status bar: from `operations/about`, not `statvfs` (FUSE reports nothing useful).
  - **Preview pane**: section 8.3.

### 7.5 Signing out and removing an account

- Confirm Dialog: title "Sign out of Google Drive?" Body: "EchoFiles will stop using this account and delete its saved index and cached files from this computer. Files on Google Drive aren't touched." Buttons: "Sign out" (danger) and "Cancel".
- If uploads are pending, the dialog says so first, and the primary button becomes "Wait for uploads". "Sign out anyway" loses those changes, and the dialog says that plainly.
- Steps: unmount, delete the remote from our rclone config (rc `config/delete`), delete the account's cache folder, catalog and delta files, and remove it from EchoFiles config.
- rclone does not revoke the Google token. Say so in a final Toast with an action: "Signed out. To remove rclone's access from your Google account, open Google's account permissions." The action opens `https://myaccount.google.com/permissions`.

## 8. Browsing and file operations inside Drive

### 8.1 Listing

- The pane lists the mount path with the existing listing pipeline (getdents then statx), which already runs off the UI thread. The first listing of a big Drive folder can take a second or more, because rclone has to call the Drive API. Show the existing loading state (`Skeleton` rows) as for network places. No special code.
- Every folder listing we get from the mount is also fed to the folder map and, if the index is on, to the catalog as a "this folder now contains exactly these entries" update (section 9.4). This is free freshness.

### 8.2 Google Docs, Sheets, Slides and other Google-only types

- In the mount they are `.html` link files (section 7.3). In the EchoFiles file list:
  - The row shows the document's name **without** the `.html` that rclone adds, with the kind "Google Doc" / "Google Sheet" / "Google Slides" / "Google Form" / "Google Drawing" and a matching colour icon (document, spreadsheet, presentation, …) from the existing set.
  - The size column shows "—", because these have no file size.
  - Opening one (Enter or double-click) opens it in the browser.
  - The context menu adds "Download as…", with Word / PDF / OpenDocument for Docs, Excel / CSV / PDF for Sheets, and PowerPoint / PDF for Slides. It copies an exported file to a folder the user picks, using rclone's export formats. That's a normal transfer with progress.
  - Copying a Google Doc **out** of Drive by drag and drop copies the link file. The drop must warn instead: "Google Docs can't be copied as files. Download as Word or PDF?", with those buttons plus "Copy link file".
- How EchoFiles knows a file is a Google type: by name pattern alone, it can't. The folder map / catalog holds the Drive mime type from listings, and the pane asks it. For folders not yet in the folder map, it falls back to a cheap `operations/list` of that one folder, which returns mime types.

### 8.3 Preview pane

- Previewing a Drive file downloads it, at least partly. Rules:
  - Preview only on explicit selection, never on hover or while the cursor moves fast (wait for the selection to rest 300 ms).
  - Only for files under 5 MB, and only text, image and PDF kinds.
  - Show a `Spinner` in the preview while the file downloads, then the normal preview.
  - Bigger files show the normal "no preview" state with the text "Preview downloads the file. Open it to see it."
- Google types show their kind, owner-friendly facts (modified date) and an "Open in browser" button.

### 8.4 Operations

All of these go through the existing ops engine on mount paths unless stated:

- **Rename / move within one Drive**: a rename in the mount, which rclone turns into a server-side move. Fast, no data transfer.
- **Copy within one Drive**: through FUSE this would download and upload everything. In phase 7, the ops engine notices that source and destination are on the same cloud account and uses rc `operations/copyfile` (server-side copy) instead. Until then it works, just slower.
- **Copy into Drive from local**: through FUSE. The copy finishes as soon as the data is in rclone's cache, but **it is not on Google yet**. Following the design system's "honest states" principle:
  - The transfer toast finishes with "Copied — uploading to Google Drive".
  - The status bar shows "Uploading 3 files to Google Drive · 12 MB left", from rc `vfs/stats` and `vfs/queue`, polled every second while uploads are pending and every 30 s otherwise.
  - The Drive sidebar item shows the "Uploading 3" pill.
  - When the queue empties: "Uploaded to Google Drive", in the status bar for a few seconds.
- **Copy out of Drive to local**: through FUSE. The download shows as normal transfer progress.
- **Delete**: rclone moves the files to **Google Drive's trash**, not EchoFiles' Trash.
  - The network-place delete dialog ("Files on network servers can't go to the Trash") doesn't apply.
  - Drive gets its own wording: the Delete key does the normal "Move to Trash" action, with the Toast "Moved to Google Drive's trash". In phase 7, the toast gets **Undo** via rclone's Drive `untrash` backend command. Until then, it offers "Open trash in browser".
  - "Delete permanently" (Shift+Delete) needs rclone's option to skip the trash for that one call. It shows the normal permanent-delete confirmation, and the text says it bypasses Google's trash.
  - The sidebar Trash item does not show Drive's trash in this version. Drive's trash lives in the browser.
- **New folder, new file**: through FUSE.
- **Permissions, owner, chmod**: not supported on Drive. Hide those items in the properties view for Drive paths instead of letting them fail.
- **Duplicate names in one folder**: Drive allows two files with the same name in one folder. rclone shows both, which is ambiguous for operations. In this version: show both, and on any operation that touches an ambiguous name, show "Google Drive has two files named “report.pdf” here. Rename one in the browser first." Later maybe offer rclone's `dedupe`.

### 8.5 Offline

- If the network is down, the mount still shows cached folders, and cached files open. Uncached files fail to open.
- The rcd logs network errors; the RC client sees them in calls. The sidebar pill shows "Offline" once two calls in a row fail with network errors, and goes back once any call works.
- Opening an uncached file while offline: "“report.pdf” isn't downloaded yet and Google Drive is offline. Connect to the internet and try again."
- Writes while offline go to the cache and upload later. That's correct behaviour, shown with the "Uploading" state as "Waiting for connection".

## 9. The index (opt-in)

### 9.1 What is saved, where, and the privacy promise

- **Switch**: Settings → Google Drive → "Include Google Drive in search index". Off by default.
- **Separate from every other index.** This switch belongs to Google Drive alone. The Settings → Search & index page (local folders, drives, excludes, the main index on/off) never turns it on or off, and turning the main index off doesn't turn this off. The Search & index page only shows a read-only line pointing here: "Google Drive has its own index switch in Settings → Google Drive."
- **Off means**: nothing about the user's Drive is written to this computer by EchoFiles except rclone's own cache of opened files and the sign-in token. The folder map (section 9.6) exists only in memory. The Settings description says exactly this.
- **On means**: one catalog per account at `~/.local/state/echofiles/cloud-index/<account id>.jsonl` plus a delta file `<account id>.delta.jsonl`. Files 0600, directory 0700.
- **Each entry**: Drive file ID, parent folder ID, path inside Drive, folder-or-file, size, modified time, mime type, MD5 (files only; free, because Drive stores it), and whether it's a shortcut (with the target ID).
- **Not saved**: contents, owners, sharing, comments, descriptions, thumbnails, trashed files.
- **Turning the switch off**, or signing out, deletes the catalog and delta files at once. Turning it back on starts a fresh crawl.

### 9.2 Format and search engine: generalise the phone index

- The phone index (`crates/index/src/phone.rs`) already does exactly this shape of job: JSONL files of remote entries, parsed once, cached while the file's size and mtime don't change, folded names, the shared matcher and ranker, and URIs that can't be opened locally.
- Generalise it into a **catalog** module with a source type (phone or Drive) and the URI scheme. Keep the phone API working unchanged, so the phone feature doesn't regress, and cover it with its existing tests.
- The Drive catalog adds the ID, parent ID, mime type, MD5 and shortcut fields. It also gets an **ID → entry** table built at parse time, used by live search (section 10.4) and the folder map.
- **Delta file**: append-only lines of "put entry" and "delete entry by ID". Reads apply the base file, then the delta in order. Each full crawl writes a new base and empties the delta. This makes every small update an append instead of a rewrite of a large file.
- **Size**: a Drive with 200,000 entries is about 40–60 MB of JSONL and a similar amount in memory once parsed. Linear scanning of folded names over 200,000 entries takes a few milliseconds, well inside the search budget. Measure it in phase 5 and write the numbers here.
- **When to move to the fast mapped index format**: if a real Drive above about 1 million entries makes the catalog scan slower than 20 ms, add a builder to the main index that takes entries instead of a filesystem walk, and store Drive in the main format. Don't do this up front.

### 9.3 First crawl and full re-crawls

- **How**: a separate child process, `rclone lsjson` on the remote root with: recursive, fast-list (batches requests, much faster on Drive), hashes, our config file, and skip shortcuts (avoids loops and duplicates; shortcuts are recorded as link entries instead).
  - **Why a child process and not rc `operations/list`**: rc returns the whole listing as one JSON reply in memory. For a big Drive that's hundreds of MB in one reply. `lsjson` writes one entry per line to stdout, which we parse as a stream: constant memory, progress for free.
  - The output goes to a temporary file next to the catalog and is renamed into place only when the crawl completes. That's the same atomic-write pattern `ef-config` already uses. A failed or cancelled crawl leaves the old catalog untouched.
  - Polite to the shared quota: limit the crawl's request rate (rclone's transactions-per-second limit, start with 8). That leaves room for the user's interactive browsing and live search on rclone's shared app.
  - Run on the existing low-priority background pool rules (nice, idle I/O priority). Paused on battery saver, like the main indexer.
- **Two processes sharing one config file**: the rcd and the crawler can both refresh the access token and both write the config. Google refresh tokens don't rotate on use, so a lost write is harmless (the next refresh works). Phase 0 confirms this. If it's wrong, run the crawl through the rcd's `core/command` instead and accept the memory cost for very large drives, or serialise refreshes.
- **When**:
  - Right after the switch is turned on, or after connecting an account while the switch is on.
  - At app start, if the last full crawl is older than 24 hours.
  - "Refresh index now" in Settings and in the item's context menu.
  - Not on a short timer. Section 10 (live search) is why that's no longer needed.
- **Progress**: Settings uses the existing `IndexStatus` component: "Indexing Google Drive · 48,210 items". When done: "Google Drive indexed · 212,904 items · 3 minutes ago".
- **Errors**: rate limiting from the shared app (Google "user rate limit exceeded") is retried by rclone. If the crawl still fails, keep the old catalog and show the error with a Retry button. If it failed because of the shared app's limits, add: "Using your own client ID avoids this limit." with a link to the advanced setting.

### 9.4 Keeping the catalog fresh between crawls

Five sources of updates, all writing to the delta file:

1. **Our own operations.** After any rename, move, copy, delete or new folder on a Drive path succeeds, the ops engine reports it. The catalog records a put or delete right away.
2. **Folders the user opens.** Each listing from the mount replaces that folder's direct children in the catalog: puts for new and changed entries, deletes for missing ones. The listing returns names, but entries need IDs and mime types, so the update calls rc `operations/list` on that folder (one cheap call) in the background after the pane has painted. Never before.
3. **Live search results** (section 10): new matches become puts.
4. **Verification of shown results** (section 10.6): confirmed-missing entries become deletes.
5. **Drive's change feed**, only with the user's own client ID (section 12.4): near-live puts and deletes for everything.

### 9.5 Scope

- **Default**: "My Drive" only.
- **"Shared with me"** and **shared drives**: phase 8. Each is its own rclone remote (rclone's shared-with-me option, or one remote per shared drive), its own sidebar child item under the account, and its own catalog and switch. These are where privacy surprises live, since files from other people would land in the index, so they're opt-in separately even when the main switch is on.
- Settings → Search **exclude names** (like `node_modules`) apply to the Drive catalog too, with the same matching rules.
- Names starting with `.` follow the "show hidden" rules, like everywhere else.

### 9.6 The folder map (always, in memory)

- **Why it's needed**: live search returns Drive IDs and parent IDs, not paths (section 10.4). Without a way to turn a parent ID into a path, live results can't say where they are, and can't open in EchoFiles.
- **With the index on**: the catalog's ID table is the folder map.
- **With the index off**: right after mounting, a background child process lists **folders only** (recursive, fast-list, IDs, nothing else) into memory. That's typically a few thousand entries and a few seconds. It's never written to disk and discarded on disconnect or quit. It's kept fresh by sources 1, 2 and 3 of section 9.4 the same way, in memory.
- If the folder map isn't ready yet when a live result arrives, the result shows with the location "Google Drive" only, and opening it uses the browser (section 10.7).

## 10. Search: two steps

### 10.1 The flow

1. **The user types** in the search field with the Everywhere scope. Step 1 runs as today: local indexes, plus phone hits if enabled, **plus Drive catalog hits** if the index switch is on. Results render in the same frame budget as now.
2. **Debounce**: 350 ms after the last keystroke, if live search is on (Settings → Google Drive → "Search Google Drive live", default on) and the account is connected and online, step 2 starts. The `SearchResults` header shows a `Spinner` with "Searching Google Drive…".
3. **Step 2** translates the query (section 10.3), sends it through rc (section 10.4), turns the results into hits with paths (section 10.4), and **filters them through our own matcher** (section 10.5).
4. **Merge** (section 10.6): existing hits are marked confirmed; new hits are inserted; the catalog gets puts (if the index is on).
5. **Verify** (section 10.6): Drive catalog hits on screen that the live query didn't confirm are checked one by one; gone ones fade out and get deletes.
6. **Done**: the header shows "Google Drive checked · 3 new" for 3 s, then the normal header.

With the index switch off, step 1 has no Drive hits, and step 2 is the only Drive source. It's the same code path, with an empty catalog.

Search scope "This folder", when the current folder is inside Drive, uses the existing in-folder filter on the listing. No live step, since the folder's listing is already fresh.

### 10.2 Concurrency, cancellation and rate

- **One in flight**: there's never more than one live query in flight. If the user types while one runs, remember only the latest text. When the in-flight query returns, its result is dropped if stale, and the latest text is sent. This is "latest wins" with a single slot. It's simpler than cancelling HTTP requests, and it caps the load on the shared quota at one query per round trip.
- **Generations**: every search has a generation number (the search code already uses cancel flags). A live result is applied only if its generation matches the current search.
- **Minimum query**: at least one search word of 2+ characters after translation. Otherwise, skip step 2 silently.
- **Timeout**: 10 s, then "Google Drive didn't answer — showing saved results" in the header. No retry loop.
- **Result cap**: the live step keeps at most the first 200 matches from Drive. Drive's own order isn't our ranking, so after filtering they're ranked by our ranker. Phase 0 checks whether rclone's query command fetches every page of results; if so, very broad queries are why the 2-character minimum and the cap exist. If rclone can't stop early, prefer more specific translated queries (always add every word, section 10.3).

### 10.3 Translating our search language to Drive's query language

The search box language is `ef`'s (`report ext:pdf size:>1m modified:<7d in:~/Work`). Drive's query language is different and weaker. The rule: **send Drive what narrows the search safely, and apply everything else locally** (section 10.5). Never send a filter that could wrongly drop matches.

| Our query part | Sent to Drive | Applied locally |
|---|---|---|
| Plain words (`annual report`) | One "name contains 'word'" per word, joined with "and" | Our matcher, as always |
| `ext:pdf` | Nothing reliable (Drive has no extension filter in queries; mime mapping misses cases) | Yes |
| `kind:dir` / folders only | "mimeType = folder" | Yes |
| `kind:file` | "mimeType != folder" | Yes |
| `modified:<7d` and similar | "modifiedTime > '<date>'" in RFC 3339, UTC | Yes |
| `size:` | Nothing (Drive can't filter by size in queries) | Yes |
| `in:<path>` outside Drive | Skip step 2 entirely | — |
| `in:<path>` inside Drive | "'<folder id>' in parents" only gives direct children, so nothing is sent for recursive scope | Path prefix filter |
| Regex / glob | The longest literal words in the pattern, if any are 2+ characters; otherwise skip step 2 with the note "Google Drive can't search by pattern — showing saved results" | Yes |
| Hidden files | Nothing | Yes |
| Always | "trashed = false" | — |

- **Escaping**: inside Drive query strings, backslashes and single quotes must be escaped. Every word goes through one escaping function, and it has tests with quotes, backslashes and unicode.
- **Important limitation, stated plainly in this design**: Drive's `name contains` matches **prefixes of words**, not substrings. `port` finds `port-forwarding.md` but not `report.pdf`. Our index matches substrings and fuzzy. So live results can only **add** matches, never prove that a catalog hit is gone (section 10.6). Users with the index on get substring matching from step 1. With the index off, Drive-only results are limited to Drive's prefix matching. Settings says so in one line under the live search switch.
- **Later option**: "Also search inside files" (Drive's full-text search). Drive offers it; our index can't. Off by default, phase 8 or later.

### 10.4 Running the query and getting paths

- Call rc `backend/command` with the Drive backend's `query` command, the account's remote, and the translated query. Request the fields: ID, name, mime type, parents, size, modified time, MD5. Phase 0 checks the exact command name, how fields are chosen, and the reply shape.
- For each result, build the path: look up the parent ID in the folder map / catalog, take the parent's path, add the name.
  - Parent found: a normal hit, `gdrive://<account>/<path>`, which opens in EchoFiles.
  - Parent not found (folder created since the last listing): a hit with location "Google Drive" that opens in the browser. Schedule a refresh of the folder map in the background (folders only, rate-limited to once per minute).
  - Multiple parents (old Drive files can have several): use the first parent that resolves. The catalog keeps one path per ID.
  - Files with no parent visible to the user (shared files not in My Drive): skip them in this version. Phase 8 handles "Shared with me".

### 10.5 Our matcher is the final judge

- Every live result goes through the same matcher and ranker as step 1, with the same query, including extension, size, date, kind and hidden rules.
- This guarantees that step 2 never shows something step 1's rules would have rejected, and the two steps rank consistently. Drive's prefix rule makes it find *fewer* candidates. Our matcher then removes Drive's loose matches (Drive is case- and accent-insensitive in ways that may differ from our folding).

### 10.6 Merging and verification

The merge is a pure function: (the shown hits, the live hits, the verification results, whether the user has interacted) → the new list. That makes it easy to test (section 14).

- **Confirmed**: a live hit with the same Drive ID as a shown hit. No visual change. If size or date differ, update them in place.
- **New**: a live hit not shown. Insert it.
  - If the user hasn't moved the keyboard cursor or selection since the first paint, re-rank the whole list in one frame (no animation; the design system says lists never animate on sort).
  - If they have, add new hits **below** the current cursor position, in rank order among themselves, so nothing jumps under their hands.
- **Verification**: take the Drive catalog hits currently on screen (at most the first 20) that live search didn't confirm. Check each one on the mount path (a stat; rclone's folder cache is fresh within a minute because of change polling). This runs on the background pool, at most 4 at a time.
  - Exists: keep it. Drive's prefix matching just didn't find it.
  - Gone: fade the row out (`dur-base`, `ease-exit`, as the design system does for deletes) and remove it, unless it's the selected row. Then it stays, its location turns `ink-muted` and reads "No longer on Google Drive", and it can't be opened.
  - Record a delete in the catalog delta.
- **Total count**: the result count in the header updates with the merge.

### 10.7 Opening results

- **Drive URI with a mount**: resolve to the mount path. Open or "show in folder" like any local result.
- **Drive URI with the account disconnected or offline**: show the same pattern phone results already use. Location reads "Google Drive · offline". Opening it shows a toast with "Connect" as the action.
- **Result without a resolved path**: "Open in browser" with Drive's web address for that file ID.

### 10.8 `ef` (CLI and agents)

- **`ef` access to Google Drive is its own switch**, separate from the Drive index switch: Settings → AI agents (the ef options) → "Let ef search Google Drive", **off by default**, and independent. Turning on the Drive index for the user's own search never gives `ef` or agents access, and turning on `ef` access never turns on the index. When `ef` access is on but the Drive index is off, plain `ef` finds nothing on Drive and says why ("Google Drive isn't indexed"); `ef --live` still works.
- `ef` searches the Drive catalog only if **both** switches are on.
- **Output paths**: when the account is mounted, print the **mount path**, so agents can read the file with normal tools. Otherwise print the Drive URI. JSON output always includes both `uri` and, if mounted, `path`, plus `source: "gdrive"`.
- **`--live` flag** (agent extra): also run step 2, waiting up to 10 s, and include its results, marked `"live": true` in JSON. This needs the running EchoFiles app (it owns the rcd). `ef` already hands commands to the warm app over its socket. If the app isn't running, `--live` prints a note to stderr and returns index results only.
- The `why_nothing` explanations learn Drive cases: "Google Drive isn't indexed (Settings → Google Drive)" and "Google Drive results are off for agents".
- `ef`'s speed budget is unchanged without `--live`: the catalog scan is milliseconds.

## 11. Settings

### 11.1 What's stored in EchoFiles config (`config.toml`)

A new `Cloud` section, with serde defaults so older config files load unchanged:

- `accounts`: a list of account ID, display name, access (full or read-only). Never tokens, never the client secret.
- `index`: bool, default false.
- `live_search`: bool, default true.
- `cache_max_gb`: number, default 10.
- `ef` access: a separate bool in the existing Agents settings struct ("let ef search Google Drive"), default false, never derived from `index`. `index` is likewise never derived from the main search index settings.
- Sidebar: `cloud` (show section) and `cloud_open` (expanded), in the existing `Sidebar` struct.
- Whether a custom client ID is in use is **not** stored here. It's read from our rclone config, so the two can never disagree.

### 11.2 Settings page "Google Drive"

- **Page**: new entry in the settings nav with the `cloud` glyph, placed after "Phone". Built only from existing design system components: `SettingsPage`, `SettingsNav`, `SettingsGroup`, `SettingRow`, `Switch`, `SegmentedControl`, `TextField`, `Button`, `StatePill`, `IndexStatus`, `Banner`, `Kbd`. Follow the voice rules: sentence case, buttons are exact verbs, no "we", no "OK".
- **Group "Account"**, one row per account:
  - Name, a state pill, and usage ("4.2 GB of 15 GB").
  - Buttons: "Sign out…", "Rename…".
  - Access: shown as text; changing it requires signing in again, so it's a "Change access…" button that runs the sign-in flow with the new scope.
  - A row "Add another account" with the button "Connect Google Drive".
  - If rclone is missing: a warning `Banner` with the install command and a copy button, instead of the rows.
- **Group "Search"**:
  - "Include Google Drive in search index". Description: "Saves file names, folders, sizes and dates from Google Drive on this computer, so search is instant. File contents are never saved. Off: nothing about your Drive is saved here."
  - `IndexStatus` row with "Refresh now", when on.
  - "Search Google Drive live". Description: "After showing saved results, also asks Google Drive. Google Drive only matches the start of words."
- **Group "Storage"**:
  - "Cache size limit" (`TextField` in GB, validated as 1–500).
  - "Cached now: 1.3 GB", with the button "Clear cache". Disabled with the reason "Uploads pending" while uploads are pending.
  - Cache location shown in mono text.
- **Group "Advanced"** (collapsed by default):
  - **"Use my own client ID"** (section 12): two `TextField`s, Client ID and Client secret (masked, with a show/hide toggle), the button "Save and sign in again", and a short numbered how-to (section 12.2).
  - Status: "Using rclone's shared app" or "Using your app (…bf2a.apps.googleusercontent.com)", showing only the end of the ID.
  - The button "Use rclone's app" when a custom one is set.
- **Group "rclone"**: detected version and path, the log file location, the button "Open log".

## 12. Your own client ID

### 12.1 Behaviour

- **Empty (default)**: rclone's built-in app. Nothing to set up. rclone's own docs say that shared client ID is being retired during 2026, so Settings says so next to the field (section 18).
- **Saving an ID and secret**:
  1. Validate the format: the client ID ends with `.apps.googleusercontent.com`, and the secret isn't empty. The field error says what's wrong, following the design system's error rules.
  2. Update the remote's client ID and secret through rc `config/update`, never on a command line (section 13.2).
  3. A login belongs to the app it was made with, so run the sign-in flow again (section 7.2, steps 5–8) right away. The dialog title is "Sign in again for your app". If sign-in is cancelled, restore the previous client settings, so the account keeps working.
- **"Use rclone's app"**: clear both values through rc, sign in again, same rollback rule.
- **Several accounts**: the client ID is per account in rclone. The UI offers "Apply to all accounts" when more than one exists; each account then signs in again in turn.

### 12.2 The how-to shown in Settings (plain steps)

1. Open Google Cloud Console and create a project (a button opens the console page in the browser).
2. Enable the Google Drive API for the project.
3. Set up the OAuth consent screen: user type "External", add yourself as a test user.
4. **Publish the app** ("In production"). Otherwise Google signs you out every 7 days. For an app only you use, Google's "unverified app" warning during sign-in is expected; choose to continue.
5. Create credentials: OAuth client ID, application type "Desktop app". Copy the client ID and secret here.

The help text also states the benefit in one line: "Your own app has its own speed limits instead of sharing rclone's, and lets the index update almost instantly. rclone is retiring its shared app during 2026."

### 12.3 What the own app changes

- **Rate limits**: crawls can use a higher request rate. Raise the crawl's limit (section 9.3) to 20 when a custom client is in use.
- **Change feed**: unlocks section 12.4.

### 12.4 Change feed (phase 7, own client ID only)

- **Why only with an own app**: rclone has no command that hands its Drive change feed to us. Calling Google's Changes API directly means making Google API calls ourselves with a token. Using the token from rclone's shared app would be acting as rclone's app, which we never do. With the user's own app, it is the user's own app, so it's fine.
- **Before building**, phase 0 checks whether a newer rclone exposes the change feed in rc. If so, use that for both kinds of client and skip everything below.
- **How**:
  - A small HTTPS client lives in `ef-cloud` only (`ureq` with `rustls`, or similar; pick the smallest maintained option at the time).
  - It keeps a saved "page token" per account.
  - Every 60 s it asks Drive for changes since that token, and turns them into catalog puts and deletes (resolving paths through the catalog's ID table, and fetching unknown parents).
- **Token handling**:
  - Read the access token and its expiry from our rclone config (the remote's token field).
  - If the token is expired or about to expire, **don't refresh it ourselves** (two writers of the same config is how tokens get lost). Ask rclone to do any cheap authenticated call (rc `operations/about`), which refreshes and saves the token, then re-read the config.
- **Bonus**, possible later with the same client: Drive thumbnails for image and video files, using Drive's thumbnail links. That would lift the "no thumbnails on Drive" rule for users with their own app.

## 13. Security and privacy

### 13.1 Files and permissions

- `~/.config/echofiles/rclone/rclone.conf`: holds the refresh token, permissions 0600, directory 0700. EchoFiles never logs it, shows it or copies it.
- **Optional hardening** (later): rclone can encrypt its config with a password supplied by a command. EchoFiles could store that password in the desktop keyring (Secret Service) and hand it over through rclone's password-command option. Not in the first version, but it doesn't change anything else in this design when added.
- `~/.cache/echofiles/rclone/`: file contents, 0700.
- Catalog and delta files: 0600 in a 0700 directory.
- rc socket: in a 0700 directory under `$XDG_RUNTIME_DIR`.

### 13.2 Secrets never on a command line

- Any process's command line is readable by every process on the machine (`/proc/<pid>/cmdline`). So the client secret and any rc password **never** go into child process arguments.
- Remote creation and client changes go through rc over the socket (sections 7.2 and 12.1). Children get only the remote name and our config path.

### 13.3 Network and data flow

- EchoFiles talks to Google only through rclone, plus (phase 7, own client only) the change-feed client. No telemetry, nothing sent anywhere else.
- Browser sign-in happens in the user's own browser, on Google's page. EchoFiles never sees the Google password.

### 13.4 Logs

- No file names in EchoFiles' own logs at the default level, beyond what existing features already log. rclone's log at NOTICE level contains error messages, which can include file names. The Settings "rclone" group names the log's location, so nothing is hidden.

## 14. Testing

### 14.1 Unit tests (no network, no rclone)

- **Query translation**: every row of the table in section 10.3, plus escaping (quotes, backslashes, unicode, emoji in names), regex literal extraction, and the "skip step 2" cases.
- **Merge function** (section 10.6): confirmed / new / gone; cursor untouched → re-rank; cursor moved → insert below; selected row gone → kept and marked; generations — stale results ignored.
- **Catalog**: parse, the phone tests still pass after the generalisation, base plus delta order, delete by ID, compaction, the ID table, multiple parents, bad lines skipped (like the phone test's "incomplete line"), path traversal rejected (no `..`, no absolute paths, same as the phone index).
- **Drive URI ↔ mount path** resolution, including accounts that aren't mounted and renamed accounts.
- **RC client**: a fake rc server in the test (a Unix socket answering canned JSON), covering timeouts, errors, Google error detection from messages, and reconnect.
- **Supervisor**: backoff timing and the give-up rule, with a fake child process.
- **Settings**: old config files without a `Cloud` section load with defaults; save and reload round-trips.

### 14.2 Integration tests with real rclone, without Google

rclone can mount a **local** remote (a folder on disk), and also an in-memory remote. With one of those, test for real (and in CI, if rclone is installed in the test image): rcd start and stop, socket permissions, mount and unmount, stale-mount clean-up, ops through FUSE, upload-pending stats, the lsjson streaming crawl into a catalog, delta updates after ops, and quit with pending writes.

Only Drive-specific parts (query, Google Docs, OAuth, quotas) need a real Google account.

### 14.3 Manual checklist with a real Drive (each phase's PR includes the relevant part)

- Sign in, cancel, time out, deny in the browser, port busy.
- Browse a folder with 5,000+ files. Time to first rows and to the full list; write the numbers here.
- Open a Google Doc, "Download as" PDF, drag a Google Doc out.
- Copy 1 GB in: honest uploading state, quit during upload, restart, upload finishes.
- Delete then restore from the browser trash; permanent delete.
- Offline: cached open works, uncached gives the right message, writes queue.
- Index on: crawl a large Drive, then search. Create a file on the web and find it within a second via live search; delete one on the web and see it fade.
- Index off: confirm no Drive data under `~/.local/state/echofiles/`.
- Own client ID: switch, sign in again, cancel rolls back, the change feed updates the catalog.
- Themes: switch Omarchy themes and check the cloud world mark and icons in light and dark themes.

### 14.4 Benchmarks

- Add Drive cases to `bench/`: catalog load time and search time at 50k / 200k / 1M synthetic entries, and the cost of step 1 with Drive included versus without. The rule from the performance work holds: Drive must not make local search slower, so the catalog is skipped entirely when the query has an `in:` outside Drive.

## 15. Phases (each one is a branch and a PR; never push to main)

**Phase 0 — Spike and verification (no product code).** Answer, with a scratch test against a real Drive and write the answers into this document:

- Minimum rclone version for: rc on a Unix socket, `mount/mount`, `backend/command`, Drive `query`, `vfs/stats` / `vfs/queue`.
- Exact `query` command input and output, its fields, and whether it fetches all pages.
- Whether `config reconnect` is fully browser-driven for a new Drive remote, or asks terminal questions.
- How `lsjson` reports IDs, mime types, hashes and shortcuts, and its speed with fast-list on a large Drive.
- Whether two rclone processes sharing a config file lose tokens.
- How `link.html` exports look in the mount, and how names with `/` and other odd characters are encoded.
- Whether rc exposes the Drive change feed (decides section 12.4).
- Memory use of the rcd with one mount.

**Phase 1 — Design system.** The `world-cloud` token, the cloud sidebar item, the Connect Google Drive dialog, the sign-in waiting dialog, the uploading pill and status bar text, the Google Doc kinds and icons, the settings page mock, and the search header states ("Searching Google Drive…", "· 3 new", "didn't answer"). Done in `design/`, reviewed in the harness, with README updated.

**Phase 2 — Foundations.** The shared "remote path" helper refactor (section 7.4); the `ef-cloud` crate with the rclone locator, the version check, the RC client and the supervisor; `VolumeId::Cloud`; the config structs. Tests from section 14.1.

**Phase 3 — Connect, mount, browse.** The sign-in flow, mounting, the sidebar CLOUD section, browsing, ops through FUSE, the honest upload state, Drive-trash delete wording, Google Doc handling, preview rules, offline states, sign out. This is the first version users can try.

**Phase 4 — Settings page and own client ID.** Everything in section 11.2 and 12.1–12.3.

**Phase 5 — Index.** The catalog generalisation (phone keeps working), crawl, delta updates from ops and opened folders, step 1 search, the agents switch, `ef` output.

**Phase 6 — Live search.** The folder map, query translation, the rc query, path building, the merge and verification, header states, `ef --live`.

**Phase 7 — Polish.** Server-side copies within Drive, Undo via `untrash`, the change feed with an own client ID (or through rclone if phase 0 found it), and thumbnails for own-client users.

**Phase 8 — More of Drive.** Shared with me, shared drives, full-text search, multiple accounts polish.

## 16. Known limits (say them, don't hide them)

- **Prefix matching**: with the index off, Drive results match the start of words only.
- **Freshness with the index on and no own client ID**: changes made outside EchoFiles appear immediately through live search; deletions made outside EchoFiles disappear when shown and verified, or at the next daily crawl.
- **Speed**: the shared rclone app is slower for very large Drives. The own client ID fixes it.
- **Undo**: no Undo for Drive deletes until phase 7.
- **Thumbnails**: none for Drive until phase 7, and then only with an own client ID.
- **Google Docs**: can't be copied as files. They're exported on request.
- **One FUSE dependency**: without FUSE 3, Drive can't be browsed.

## 17. Out of scope

- Other cloud services (Dropbox, OneDrive, …). The `ef-cloud` crate is shaped so they can be added, but nothing here builds them.
- Offline "make available offline" pinning of folders (rclone's cache isn't a sync). Possible later with an explicit "Keep on this computer" action that runs a one-way rclone copy to a local folder.
- Two-way sync of a local folder with Drive.
- Sharing files or changing sharing permissions from EchoFiles. "Open in browser" covers it.

## 18. Phase 0 spike (2026-10-25)

rclone is not installed on the machine this was written from (`rclone` is not on `PATH`). `fusermount3` is at `/usr/bin/fusermount3`. Nothing below was run against a Google account. Answers come from rclone's published docs (rclone.org, fetched 2026-10-25). Items that still need a real Drive are marked **unverified**.

| Question | Answer |
|---|---|
| Minimum version | **1.64.0.** `rclone rcd` exists since v1.45. rc listens on a Unix socket with `--rc-addr unix:///path/to/socket` (or an absolute path). `mount/mount` takes `fs`, `mountPoint`, `vfsOpt`, `mountOpt`. `backend/command` takes `command`, `fs`, `arg`, `opt` and returns `result`. Older binaries were not executed, so 1.64.0 is a floor, not a measured oldest. |
| Drive `query` | `rclone backend query <remote>: "<drive query>"`. The reply is one JSON array of file objects: `id`, `name`, `mimeType`, `parents`, `size` (string), `modifiedTime`, `md5Checksum`, `webViewLink`. Quotes and backslashes in the query are escaped with a backslash. **Unverified** whether rclone fetches every page itself. The 200-result cap and the 2-character minimum stay. |
| `config reconnect` | The documented `rclone config` dialogue for a new Drive remote asks questions (client ID, secret, scope, shared drive, browser yes/no) before `http://127.0.0.1:53682/auth`. Create the remote over rc with those options already set. **Unverified** whether `config reconnect` on that remote still prompts. |
| `lsjson` | One JSON object per item when used as a stream is not what the docs show: the documented output is a JSON array, with `ID`, `MimeType`, `Path`, `Name`, `Size`, `ModTime`, `IsDir`, and `Hashes.MD5` when `--hash` is set. Shortcuts are omitted with `--drive-skip-shortcuts`. A shortcut's ID listing can be `target<TAB>shortcut`. **Unverified** on a large Drive, including fast-list speed. The crawler still parses line by line and also accepts a single JSON array, so either shape works. |
| Two processes, one config | **Unverified.** The design's assumption stands: Google refresh tokens are not rotated on each use, so a lost config write is recovered by the next refresh. If a real run loses a token, crawls go through the rcd instead. |
| `link.html` and odd names | `link.html` is a small HTML file that redirects to the document in the browser. The Drive backend does **not** encode `/`; a slash is a legal Drive name. Path building must not treat a slash inside a name as a folder. **Unverified** how the FUSE mount shows such a name. |
| Change feed in rc | The published Drive backend commands include `query`, `untrash`, `copyid` and `rescue`. There is no change-feed command. Section 12.4 stays: the feed is an own-client HTTPS call, not rclone's shared app. |
| rcd memory | **Not measured.** The 50–150 MB estimate in section 6.5 stands until someone mounts a Drive and reads the process. |

`untrash` restores everything under a directory and returns `{ "Untrashed": N, "Errors": N }`. It is not a one-file undo. Phase 7's Undo uses it only when the deleted item's parent folder is known, and says so if the command would restore more than that one item. Server-side copy of one file is rc `operations/copyfile` (unchanged). rclone also notes the shared Drive client ID is being retired during 2026, which is why Settings tells people that next to "Use my own client ID".
