# Faster still, default file manager, and AI agents

Status: discussion draft, 2026-09-26. Decisions needed are marked **Decide**.

This document covers three things you asked about:

1. How much faster EchoFiles can get, including forking the libraries it depends on.
2. Whether EchoFiles can become this PC's default file manager, and what that involves.
3. Whether EchoFiles can give AI agents faster, safer file access through an Omarchy-style
   system-wide skill.

Short answer: all three are achievable. (1) has a few measured wins left, and most of the
remaining gains are perceived speed rather than raw speed. (2) is a handful of standard
Linux mechanisms. (3) is the most interesting, and the one that could set EchoFiles apart.

---

## 1. Performance: where we are and what's left

### Measured today (M1 build, warm cache)

| | EchoFiles | Nautilus 50.2 |
|---|---|---|
| Window appears | 87–117 ms | ~505 ms |
| 100k-file folder fully shown | 0.64 s | 10.8 s |
| CPU spent on the 100k folder | 0.43 s | 15.5 s |
| Memory with the 100k folder | 221 MB | 416 MB |
| Scrolling 100k rows | 60 fps, worst frame 17.8 ms | not measurable from outside |

Source: `bench/RESULTS.md` (rerun with `python3 bench/compare.py`).

### Changes already made to other people's code

Forking is allowed and already in use:

- **`vendor/iced_graphics`**: Iced parsed all 714 system fonts before the first frame
  could draw text (457 ms on a cold cache). The patch loads only the UI font at startup and
  swaps in the full font database from a background thread.
- **Runtime settings Iced doesn't expose** (`crates/ui/src/gpu.rs`):
  - Iced defaults to the high-performance GPU, which wakes the GTX 1660Ti; using the Vega
    iGPU saves ~140 ms.
  - On hybrid laptops, keeping the Vulkan loader from loading NVIDIA's driver saves a
    further ~80 ms.

  First frame went from 490 ms to 67–75 ms.

Rules for any further fork:

1. A benchmark shows the win before the patch lands.
2. The patch stays small and is documented in the vendored folder (`ECHOFILES_PATCH.md`).
3. We track upstream and drop the fork when upstream fixes the issue.

### Candidates, ranked by expected value

| # | Change | Kind | Expected win | Cost |
|---|---|---|---|---|
| 1 | **Resident process**: one running EchoFiles serves new windows over D-Bus (`FileManager1`) | architecture | new window ~70 ms → ~10–20 ms; the directory cache survives between windows | medium |
| 2 | **Hyprland open animation off for EchoFiles only** (`noanim` window rule) | config (your consent) | the window looks finished ~0.4 s sooner — the biggest visible delay left, and it isn't ours | tiny |
| 3 | **Search index** (v0.2): NTFS MFT read + btrfs crawl, memory-mapped | new subsystem | "find all PDFs in home": 330 ms (`find`) / 146 ms (`fd`) → a few ms | large |
| 4 | **Fonts: cached index on disk** instead of parsing 714 files each launch (patch cosmic-text/fontdb) | fork | background CPU ~20–450 ms → ~5 ms; lower memory | small |
| 5 | **Pre-rasterised icons**: bake the SVG icon set into a GPU texture atlas at build time, per theme | own code | removes resvg work on first show of each icon; smoother first scroll | small |
| 6 | **Idle memory diet**: 141 MB today; target < 70 MB idle (font mmaps, wgpu staging buffers, mimalloc tuning) | profiling, maybe fork | memory, not speed; matters once resident | medium |
| 7 | **wgpu pipeline cache on disk** (fork iced_wgpu if it doesn't expose it) | fork | ~10–20 ms off cold start | small |
| 8 | **io_uring for bulk `statx`** on cold NTFS/HDD | own code | only helps cold or slow disks; warm btrfs is already 13–16 ms for 100k | medium |

What forking can't fix:
- **Disk physics:** a cold NTFS read is bounded by the NVMe.
- **Compositor animations:** Hyprland owns those, so row 2 is a config change, not code.
- **Nautilus-style thumbnails of every file:** doing less work beats doing work faster.

**Decide:** can EchoFiles add a Hyprland window rule that turns off only its own open
animation? It would be one line in `~/.config/hypr/`, written with your OK and easy to remove.

---

## 2. Making EchoFiles the default file manager

Nothing here needs root, and every step is reversible. Nautilus stays installed.

| What people do | Who decides today | How EchoFiles takes over |
|---|---|---|
| Open a folder from anywhere (`xdg-open ~/Downloads`, apps' "Open folder") | `xdg-mime` → `org.gnome.Nautilus.desktop` | Ship `echofiles.desktop` with `MimeType=inode/directory;`, then `xdg-mime default echofiles.desktop inode/directory` |
| "Show in folder" from browsers, download bars, IDEs | D-Bus `org.freedesktop.FileManager1` → Nautilus (`/usr/share/dbus-1/services/…`) | Implement `ShowFolders` / `ShowItems` / `ShowItemProperties`; install a user service file in `~/.local/share/dbus-1/services/`, which takes precedence over the system one |
| Super+Shift+F / Super+Alt+Shift+F in Omarchy | `/usr/share/omarchy/default/hypr/bindings/applications.lua` → `nautilus` | Override the two bindings in `~/.config/hypr/bindings.lua` (user file, survives Omarchy updates) |
| Open/save dialogs in every app | xdg-desktop-portal FileChooser (GTK/GNOME backend) | Implement `org.freedesktop.impl.portal.FileChooser` so every app's file picker is EchoFiles. Most ambitious; v0.3+ |

**When:** after M3, once copy, move, trash, rename and undo exist. Becoming the default
before it can move files would be a worse experience than Nautilus. I'd do it in this
order: `.desktop` + `xdg-mime` → FileManager1 → keybindings → file chooser portal. An
`echofiles --make-default` / `--restore-default` pair would do and undo all of it.

**Decide:** default right after M3, or run side by side for a while first?

---

## 3. AI agents: faster, safer file access

### Why agents are slow at files today

An agent working on this PC does file work by spawning shell commands:
- `ls`, `find`, `grep`, `cat`: each is a new process, re-walks the tree from scratch, and
  prints human-formatted text that the agent then has to read back as tokens.
- Measured on your home folder (733k entries, warm cache): `find ~ -name '*.pdf'` takes
  **330 ms**, `fd` 146 ms. `plocate` answers in 8 ms, but its index is stale (22 results
  instead of 5,502).
- On a Windows drive it's worse. The drive may not be mounted, NTFS is slower cold, and the
  agent doesn't know that "D:\Work" is `/run/media/…/AVS`. Nor does it know that
  `$RECYCLE.BIN`, `System Volume Information` and `desktop.ini` are noise.
- Deleting is dangerous: an agent's `rm` has no undo.

### What EchoFiles can offer: one engine, three front doors

```
                    ┌──────────────── EchoFiles engine (resident) ────────────────┐
  EchoFiles UI ───► │ listing cache · search index (MFT + btrfs) · drive map      │
  `ef` CLI ───────► │ Windows path translation · junk filter · copy/move/trash    │
  MCP server ─────► │ with journal + undo · change notifications                  │
                    └──────────────────────────────────────────────────────────────┘
                          unix socket, ~1 ms per request, JSON lines
```

The agent-facing `ef` command. Every command supports `--json`, `--limit` and `--fields` to
keep output token-cheap:

| Command | Does | Why it beats shell tools |
|---|---|---|
| `ef find <query> [--in D:] [--ext pdf] [--newer 7d]` | fuzzy or exact name search from the index | milliseconds instead of a full tree walk; covers unmounted Windows drives through the MFT index |
| `ef ls <path>` | listing with kind, size, date and Windows attributes; junk hidden | one call, structured output, natural sort, same view as the UI |
| `ef tree <path> --depth 2 --budget 4kb` | folder overview that fits a token budget | agents get the shape of a project without dumping 10k lines |
| `ef du <path>` | folder sizes, cached | no re-walk |
| `ef drives` | volumes, drive letters, mount state, free space | the agent knows the Windows world exists |
| `ef path 'D:\Work\a.xlsx'` | Windows ↔ Linux path translation (mounts on request) | no guessing `/run/media/…/AVS1` |
| `ef copy/move/trash <src…> <dst>` | same engine as the UI: pre-flight, progress, journal | **undoable**; never `rm -rf`; Windows-name pre-flight |
| `ef reveal <path>` | opens EchoFiles with the file selected | the agent can *show* you what it found |
| `ef watch <path>` | streams change events | agents react to new downloads and saves |
| `ef recent` | recently opened and modified files | "the thing I was working on" |

Content search (`grep`) stays with ripgrep, which is already excellent. `ef` can feed it a
pre-filtered file list from the index (for example "only PDFs changed this week") so it
reads less.

### How agents learn it exists: the Omarchy way

Omarchy already does this for its own tools. It ships skills in
`/usr/share/omarchy/default/agents/skills/` and symlinks them into `~/.claude/skills/`; the
`omarchy` and `diagnose-crash` skills are available to agents on this PC right now.
EchoFiles would:

1. Ship `echofiles/SKILL.md`, a short, trigger-rich description plus the command
   reference, in `/usr/share/echofiles/agents/skills/echofiles/`.
2. Symlink it into `~/.claude/skills/echofiles` (and the equivalent for other agents Omarchy
   supports) on install, with an `echofiles --agents install|uninstall` switch.
3. Offer the same commands as an MCP server (`ef mcp`) for agents that prefer tools over
   shell commands.

The skill tells agents, in effect: *"For finding, listing, moving or deleting user files,
especially on Windows drives, use `ef`. It's instant, structured, and every change can be
undone."*

### Safety model

- Agent requests are **read-only by default**. Writes go through the undo journal, and
  permanent delete isn't offered to agents at all (trash only).
- Windows (C:) stays read-only unless you've enabled writing in the app. Agents can't
  change that setting.
- Every agent write shows a toast in EchoFiles: "Claude moved 3 files to AVS (D:) —
  **Undo**". A log is kept in `~/.local/state/echofiles/agent.log`.
- The socket is only reachable by your user (`$XDG_RUNTIME_DIR`, mode 0600).

### Realistic gains

- Name search across large trees: hundreds of ms → single-digit ms. Across Windows drives:
  seconds → ms, once the MFT index exists.
- Fewer agent round-trips: one `ef tree --budget` call replaces several `ls`/`find` calls.
- Fewer tokens: structured, budgeted output instead of long human-formatted listings.
- Reading a single file: **no gain**. `cat` is already optimal, and `ef` won't pretend
  otherwise.

**Decide:**
1. CLI + skill first (works with every agent via the shell), then MCP? That's my
   recommendation.
2. Should the engine stay resident (fast for both UI and agents, ~70 MB idle target), or
   start on demand (slower first call)?
3. May agents write at all (move, copy, trash with undo), or should v1 be read-only?

---

## Proposed order

1. **Now → M2:** finish the UI pass. Add the resident process + FileManager1 (needed for
   both "default" and agents). Mount drives and read drive letters.
2. **M3:** file operations with journal + undo. After this: the make-default switch.
3. **v0.2:** search index. Then `ef` CLI + EchoFiles skill (read-only), then agent writes
   through the journal, then MCP.
4. **v0.3+:** file chooser portal, phone.
