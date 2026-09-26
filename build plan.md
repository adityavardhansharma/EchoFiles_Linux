# EchoFiles_Linux — Build Plan (v0.1 Base)
Focus: good UI + real fast performance. Native Rust, no webview. Priority: **Linux + Windows first**. Android phone comes later — only keep the design open for it now.

## 0. Vision
Native Linux file manager, world-class on dual-boot Windows disks, later extended with a Phone Link–style wireless view of an Android phone (files + photos). Single binary, Wayland-native on Omarchy/Hyprland, GPU-rendered, instant on 10k-file folders.

Machine (verified): Omarchy 4.0.2 Arch, Hyprland 0.56.2 Wayland, 1TB NVMe, AMD Vega iGPU + GTX 1660Ti, kernel 7.1.9, udisks2 2.11.2, GTK4/libadwaita/Qt6 installed, Wi-Fi `wlp2s0` 192.168.1.0/24, ufw active (only 53317 LocalSend open).
- NTFS drivers available: `ntfs3` (Komarov) **and** new in-tree `ntfs` (Namjae Jeon, merged 7.1: iomap, delayed alloc, more xfstests passing, faster multi-thread writes).
- NOT installed: Rust toolchain (`mise use rust`), `ntfs-3g`/`ntfsprogs` (no `ntfsfix`), `android-tools` (adb), `kdeconnect`.
- Installed & useful: thumbnailers (ffmpegthumbnailer, evince, glycin, gsf-office), `localsend`, existing `~/.cache/thumbnails/{x-large,fail}`.
- Omarchy theme: `~/.local/state/omarchy/current/theme/{colors.toml,icons.theme}`, change hook dir `~/.config/omarchy/hooks/theme-set.d/`.

Windows disks (probed via udisks):
- `nvme0n1p4` 376GB NTFS no-label UUID `542A98852A986630` = Windows C: (has Windows/, no label)
- `nvme0n1p6` 156GB NTFS `AVS` UUID `5672C74272C72619` = data 1
- `nvme0n1p7` 295GB NTFS `AVS` UUID `AE30B63830B6077D` = data 2 (duplicate label — key by UUID always)
- `nvme0n1p5` 843MB recovery — hide. EFI partitions — hide.
- Duplicate `AVS` label ⇒ udisks mounts at `/run/media/$USER/AVS` vs `AVS1` depending on mount order. **Never key anything by mount path.**

## 1. Stack: Rust core + Iced custom UI
- `core/fs`: listing, copy/move/delete/trash, search, thumbs, watcher. No UI dep, no `tokio::fs` (§2.1). `rustix` (getdents/statx/openat), `rayon`, `crossbeam-channel`, `mimalloc`, `notify`, `trash` (verify NTFS behaviour), `turbojpeg`+`image`+`fast_image_resize`, `resvg`, `lnk`, `nucleo`+`memchr` (v0.2). `jwalk` only until the custom walker lands. `tokio`+`zbus` only in `core/disks`.
- `core/disks`: udisks2 D-Bus enumerate/mount, NTFS detection, driver choice, alias store, drive-letter map (`nt-hive`).
- `core/phone` (later, not in v0.1 workspace): KDE Connect protocol + in-process SFTP client. See §4.
- `core/vfs`: one `Location` abstraction (v0.1 implements only local paths: Linux + NTFS mounts) so a remote backend (phone SFTP) can plug in later without touching UI, sort, thumbs, copy. Keep listing/copy/thumb APIs async and fallible (network-safe) from day one. File identity = `(VolumeId, rel_path)` where `VolumeId` = partition UUID | phone cert fingerprint | `linux-root`.
- `ui`: **Iced 0.14** (reactive rendering, concurrent image decode, `sensor`/`table`/`grid`), all components custom. `Theme` variables, `Sidebar`, `DriveItem`, `FileList` (custom widget), `Breadcrumb`, `StatusBar`, `CommandPalette`, toasts.
- Why Iced over GPUI/Slint/GTK: pure Rust, wgpu GPU, Elm `update/view` fits file state, Wayland via winit. **Risk gate:** M1 spike must prove 100k-row list at 60fps + cold start budget; else fall back to GPUI (built-in `uniform_list`). COSMIC Files (Iced) had 10k-file freezes — widget design matters more than toolkit.
- Single instance + `org.freedesktop.FileManager1` D-Bus (ShowItems/ShowFolders): second launch = new window in ms, browsers' "Show in folder" opens EchoFiles.

## 2. Performance architecture (most important)
Goal: the fastest file manager on Linux. Every rule below is backed by a measurement or a known failure elsewhere.

### 2.0 Measured on this machine (Ryzen 9 4900HS 8c/16t AVX2, 15GB RAM, btrfs zstd:3 NVMe, warm cache)
| Operation | 10k files | 100k files |
|---|---|---|
| `getdents64` names only | 2.5 ms | 24 ms (37 ms with 32KB buf → 26 ms with 1MB buf) |
| `lstat` full path, 1 thread | 15 ms | 142 ms |
| `statx` dirfd-relative, 1 thread | 24 ms | 118 ms |
| `statx` dirfd-relative, 4 threads | 5.5 ms | 37 ms |
| `statx` dirfd-relative, 8 threads | **3.2 ms** | **23 ms** |

| Write 2000 × 16KB files | Time |
|---|---|
| `fsync` per file | 4610 ms |
| one `syncfs` at end | **163 ms (28× faster)** |

Fonts: 714 font files / 464 MB installed. Iced loads all system fonts at startup (iced#2455) → can eat the whole 300 ms budget.
Not yet measured (M2): cold cache, ntfs3 vs new `ntfs` driver, parallel statx scaling on NTFS.

Implications: metadata must be fetched in parallel; names paint first; durability is batched; startup must not scan fonts.

### 2.1 Threading model
- UI thread: `update/view/draw` only. Never fs syscalls, decoding, sorting, D-Bus waits.
- **No `tokio::fs`** (one `spawn_blocking` hop per syscall). Filesystem work runs on dedicated pools; tokio only for D-Bus (`zbus`) and later network.
- Pools: `listing` (8 threads, highest priority), `thumbs` (4, niced), `ops` (copy/move/delete), `index` (1-2, `SCHED_IDLE` + ioprio idle, paused on battery saver).
- Every job carries a generation / cancel token, checked between batches. Navigation drops pending prefetch + off-screen thumbnail jobs.
- Results cross to UI as `Arc` snapshots over `crossbeam`/`flume`; UI wake-ups coalesced to max one per frame.

### 2.2 Listing pipeline
- `rustix` raw `getdents64`, reusable 1MB buffer per thread. Open dir once, keep the dirfd.
- **Phase A** (names + `d_type`) → first paint. **Phase B**: `statx(dirfd, name, AT_SYMLINK_NOFOLLOW|AT_STATX_DONT_SYNC, SIZE|MTIME|MODE)` in 1-2k-name chunks across the listing pool. Never full-path stat in loops.
- Thread count auto-tuned per fs type (ntfs3 may serialize on inode locks — benchmark in M2). `DT_UNKNOWN` → resolved in phase B.
- Phase B updates columns in place; re-sort only if sorted by size/date, keeping selection + scroll anchor stable.
- Symlink targets stat'd lazily for visible rows only (icon / broken state).
- Dir cache: `(VolumeId, rel_path)` → `Arc<Listing>` + dir mtime/ino; revalidate with one `statx`. Dir mtime doesn't change when a child file's content changes → re-`statx` visible rows lazily on revisit. LRU bounded by bytes (~64MB).
- Prefetch: folder hovered >80ms or selected → list at low priority. Parent prefetched on open.
- Recursive walker (folder size, copy planning, search crawl): parallel, `openat`-relative, uses `d_type` to skip stat when only counting; `STATX_BLOCKS` for on-disk size (btrfs compression: show apparent vs disk).
- Targets (warm): 10k complete <8ms; 100k first paint <30ms, complete <60ms.

### 2.3 Data layout
- `Listing` as struct-of-arrays: `names` arena (`Box<[u8]>`), `name_off: [u32]`, `kind: [u8]`, `size: [u64]`, `mtime: [i64]`, `flags: [u16]`, `ext_id: [u16]`. ~40 B/entry → 100k ≈ 4 MB. No per-entry `PathBuf`/`String`; full paths built on demand.
- Views are `Arc<[u32]>` permutations (sort) + filtered index lists; selection is a bitset. Changing sort/filter never copies entries.
- Sort keys computed once per column on demand: name key = ASCII fast-path casefold (full Unicode casefold only for non-ASCII) + natural digit runs packed. `par_sort_unstable_by` (rayon) above ~20k entries. Target: 100k sort <10ms.
- Extensions interned to `u16` → mime/icon lookup table. Global allocator: `mimalloc`.

### 2.4 Change tracking
- inotify only on dirs shown in open tabs/panes (limit 524288 watches — fine).
- Coalesce 50-100ms, then **apply deltas** (create/delete/rename by name) to the listing; full rescan only on `IN_Q_OVERFLOW` or event storms.
- Own operations applied optimistically; their inotify echoes suppressed.
- NTFS: while mounted only Linux writes it → inotify is complete. Changes made by Windows between boots are handled by revalidation (v0.1) and the USN journal (v0.2 index).

### 2.5 Copy / move / delete engine
- **Plan phase first** (parallel walk): total bytes/files, conflicts, Windows-name pre-flight, free space (`statvfs`). Nothing is written until the plan is clean.
- Strategy chain: `rename()` → EXDEV → `FICLONE` reflink (btrfs copies of any size are instant) → `copy_file_range` → buffered copy.
- Large files (>64MB) across filesystems: single stream, 4-8MB buffers with read/write overlapped (2-4 buffers in flight; io_uring linked read→write as an optimization to benchmark), `fallocate` destination, `posix_fadvise(SEQUENTIAL)` + `DONTNEED` behind the cursor so a 50GB copy doesn't evict the page cache. Max ~2 big streams: Linux and Windows partitions share one NVMe.
- Small files (<1MB): 8 workers, one read + one write from pooled buffers, grouped by destination dir with cached dest dirfds (`openat`), preserve mtime (`futimens`) + mode.
- **Durability: no per-file fsync** (measured 28×). `syncfs(dest)` at the end, plus checkpoints every ~1GB during moves so sources can be unlinked progressively. Journal records each checkpoint.
- Progress: `AtomicU64` counters read by the UI at frame rate (no message per chunk); smoothed ETA.
- Permanent delete: parallel bottom-up `unlinkat` with dirfds; indeterminate progress unless counts are already known.
- Trash: same-fs rename = instant. Verify the `trash` crate uses `$topdir/.Trash-$uid` on NTFS and never copies across filesystems; otherwise implement the spec directly.
- Folder sizes: parallel walker, cached per `(VolumeId, rel, dir mtime)`, shown progressively.

### 2.6 Thumbnails & icons
- Lookup order: memory LRU → freedesktop disk cache (shared with other apps) → embedded EXIF thumbnail (read first ~64KB of camera JPEGs) → scaled decode.
- JPEG via libjpeg-turbo (`turbojpeg`) with DCT scaling (1/2…1/8): decode directly at thumbnail size, several times faster than full decode + resize. PNG/WebP/AVIF via Rust crates, HEIC via libheif. Resize with `fast_image_resize` (AVX2).
- Video/PDF/Office via system `.thumbnailer` subprocesses, max 2 concurrent, `nice`/`ionice`.
- Save thumbnail PNGs with fast compression (level 1). Freedesktop sizes/`fail/echofiles/`/`Thumb::URI`+`Thumb::MTime` as in the spec.
- GPU: upload thumbnails into a texture atlas / reuse handles; use Iced 0.14 concurrent image decoding.
- Icons: one icon theme (from Omarchy `icons.theme`), SVG rasterized with `resvg` once per (icon, size, scale), cached in `~/.cache/echofiles/icons/`, looked up by interned extension. Embedded fallback set.

### 2.7 Startup (cold <300ms, warm <100ms, 2nd window <30ms)
- Fonts: embed the UI font; load system fonts lazily in the background (only needed for CJK/emoji fallback in filenames) or use cosmic-text's cached font index (`FontSystem::new_cached`, check availability in Iced's pinned version). Patch/fork Iced if needed — this is the biggest startup risk.
- GPU: low-power adapter (Vega), Vulkan only, persisted wgpu pipeline cache; never initialize the NVIDIA ICD.
- First frame drawn from cached state (last folder snapshot + last drive list from disk), then refreshed async. udisks/D-Bus never blocks the first frame.
- Single instance + `org.freedesktop.FileManager1` → later windows open inside the running process. Optional Hyprland `exec-once` preload.
- Release profile: `lto="fat"`, `codegen-units=1`, `panic="abort"`, `-C target-cpu=x86-64-v3`, PGO via `cargo-pgo` on a recorded workload (BOLT optional), stripped binary.

### 2.8 Search & index (v0.2, layout decided now)
- Whole-volume index in the same SoA layout: parent id + name arena. 1M entries ≈ 40-60 MB.
- Persisted zero-copy (`rkyv` or custom mmap format) → loads in milliseconds.
- Matching: `nucleo` (parallel fuzzy), `memchr::memmem` fast path for plain substrings. Target: 1M names <30ms.
- NTFS: raw `$MFT` read via small polkit-approved helper + USN journal for incremental updates after Windows has run.
- btrfs: one crawl → persisted → inotify on hot dirs + dir-mtime revalidation; optional helper using btrfs generation numbers (`find-new`) to catch up quickly after the app was closed.
- Indexer: `SCHED_IDLE`, ioprio idle, pauses on battery saver.

### 2.9 Measurement discipline
- `core` benches (`divan` or `criterion`): list 10k/100k, sort 100k, filter, natural key, small/large copy, thumbnail decode, index query.
- Corpus generator: 10k / 100k / 1M files, deep trees, unicode + Windows-illegal names; same corpus copied to an NTFS partition.
- Cold-cache runs: user runs `drop_caches` manually; NTFS cold = fresh mount.
- `tracing` spans + Tracy (`tracing-tracy`) behind a feature flag; frame-time HUD in debug builds (F12); `samply`/`perf` flamegraphs.
- Benchmarks tracked over time; >10% regression blocks merge.

### 2.10 Performance rules (non-negotiable)
1. UI thread never does syscalls, decoding, sorting, or network waits.
2. No full-path syscalls in loops — dirfd-relative only.
3. No per-file fsync. Batch durability with `syncfs`.
4. No `tokio::fs`. No FUSE, ever.
5. Every job is cancellable by generation; visible rows first.
6. No per-entry heap allocations in listings (arena + SoA).
7. Caches keyed by `VolumeId` + rel path, with explicit invalidation.
8. Every performance claim has a benchmark.

Paths as `PathBuf`/`OsString` at API edges (Windows unicode). Errors as `FsError{kind,path}` → toast, never panic.

Targets: cold open <300ms, warm open <100ms, 100k-row scroll 60fps (frame <8ms on the UI thread), back/forward <16ms, 10k listing <8ms warm, 100k sort <10ms, copy progress at 1MB granularity, 1M-name search <30ms (v0.2).

## 3. Windows support (world-class read first)
- Driver: setting `ntfs3 | ntfs(new)`, default `ntfs3` until the new driver proves itself on these disks; benchmark both in M2. Always pass fstype explicitly (`ntfs` name is ambiguous now).
- Mount: `zbus` system bus → `org.freedesktop.UDisks2` `GetManagedObjects` → filter `IdType=ntfs*`, exclude <1GB → `Filesystem.Mount({fstype:"ntfs3",options:"uid=$UID,gid=$GID,umask=022,windows_names,prealloc,iocharset=utf8"})` (all in udisks default ntfs3 allow-list). No sudo (polkit). Read `MountPoints` to skip mounted.
- **C: mounts read-only by default**; data partitions rw. Writing to C: = explicit toggle with warning.
- Optional "stable mount points" setup: writes fstab entries (`/mnt/win-c`, …, `x-udisks-auth`) only on user confirm.
- Drive letters: read `C:\Windows\System32\config\SYSTEM` hive `MountedDevices` (`nt-hive`) → "Windows (C:)", "AVS (D:)", "AVS (E:)". Display = `alias[UUID] ?? letter+label ?? heuristic`. Aliases in `~/.config/echofiles/aliases.json` keyed by UUID.
- Dirty/Fast Startup: on `volume is dirty` → mount `ro` + banner "Restart Windows (not Shutdown) / `powercfg /h off`, disable Fast Startup". Repair needs `ntfsprogs-plus` (pairs with new driver) or `ntfs-3g` — not installed; banner says so. Repair only on explicit confirm w/ data-loss warning. Never auto-force.
- Hidden: honor DOS Hidden/System attributes like Explorer (verify ntfs3 exposure: `hidden` mount opt vs `system.dos_attrib` xattr) + junk list (toggle Show system): `$RECYCLE.BIN`, `System Volume Information`, `$MFT/$LogFile`, `pagefile.sys,hiberfil.sys,swapfile.sys`, `found.nnn`, `Thumbs.db,Desktop.ini`.
- Semantics: `windows_names` enforced. ADS → `+ADS` badge (**verify** how ntfs3/ntfs expose streams before building). Junctions/reparse → resolve intra-volume, link overlay; OneDrive/WOF placeholders flagged cloud-only. Case-insensitive: casefold search/dedup, block case-only `Foo`/`foo` creates. Compressed/sparse transparent, EFS-no-cert shows lock. Warn >260 chars / trailing dot-space.
- Trash on NTFS: v0.1 freedesktop `.Trash-$UID` (hidden from view). v0.2 signature feature: write into `$RECYCLE.BIN\<SID>\` with `$I`/`$R` pairs so files are restorable from Windows.
- `.lnk` via `lnk` crate: overlay, resolve `C:\...` to mount root, broken state, never execute directly.
- Known folders: probe `Users/<profile>/{Desktop,Documents,Downloads,Pictures,Music,Videos}` skip `Default,Public,All Users,Default User`, plus `Users/Public`. Localized via `Desktop.ini`, OneDrive-redirect noted.
- Extras: "Copy as Windows path" (`C:\Users\...`), Windows attributes in Properties.
- BitLocker v0.1: detect → lock icon + prompt placeholder. Unlock via `cryptsetup bitlk` is v0.2. TPM-only stays Windows-only.
- WSL `\\wsl$` is 9P not NTFS → hint only.

## 4. Android phone (LATER — design reference only, do not build in v0.1)
v0.1 constraints so this plugs in cleanly: `Location`/`VolumeId` abstraction, async fallible fs APIs, no code assuming every path is a local `PathBuf`, sidebar built from a list of sections (so a Phone section can be added).

Goal: phone appears in sidebar next to Linux/Windows; browse its important folders, a dedicated **Photos** view, send/receive, over Wi-Fi only. No USB, no root.

Phone side: stock **KDE Connect** Android app (Play/F-Droid) — no app of our own to write in v0.2. Desktop side: our own Rust implementation inside `core/phone` (no `kdeconnectd`/Qt dependency).
- Discovery: UDP broadcast/listen port 1716 (+ mDNS `_kdeconnect._udp`), TCP 1714-1764. Identity packet → TLS (self-signed certs) → pairing request with verification code shown on both screens. Trusted devices = cert fingerprint in `~/.config/echofiles/devices/`. Protocol v7/v8 — verify against current Android app. Reuse/borrow from `kdeconnect-proto` crate (tokio backend) or `rust-connect`.
- Firewall: ufw blocks it today. First-run screen tells the user to run `sudo ufw allow 1714:1764/tcp` and `/udp` (app never edits firewall itself).
- Files: send `kdeconnect.sftp.request {startBrowsing:true}` → phone starts SFTP server and replies `{ip,port,user,password,multiPaths,pathNames}`. Connect in-process with `russh-sftp`. **No sshfs/FUSE** (hangs the whole file manager when phone sleeps / leaves Wi-Fi).
- SFTP speed: `READDIR` returns attrs with names (no per-file stat round trips); pipeline 32-64 outstanding reads/writes per file; keep one session alive while phone view is open, reconnect transparently. Listings cached, revalidated on visit (no inotify over SFTP), manual refresh.
- "Important only" view (default): Camera (`DCIM/Camera`), Screenshots, Pictures, Download, Documents, WhatsApp/Telegram media, Recordings. Hide `Android/`, dot-dirs, app caches. "Show all storage" toggle.
- **Photos** gets its own sidebar entry under the phone: timeline grid grouped by day/month, aggregated from Camera + Screenshots + messaging media folders.
  - Fast thumbs over Wi-Fi: range-read first ~64KB of JPEG and use embedded EXIF thumbnail (no full download); HEIC → embedded thumbnail item; video → placeholder + lazy frame grab after download. Cache under phone fingerprint.
  - Actions: open (downloads to cache), save to Linux / Windows folder, multi-select → copy/move, "import new photos" (one-way, dated folders, e.g. `~/Pictures/Phone/2026-09/`, skip already imported by name+size+mtime).
- Send to phone: KDE Connect share plugin (drag file onto phone in sidebar). LocalSend protocol (port 53317 already open) as alternate target.
- Optional turbo mode (later): wireless ADB (Developer options → Wireless debugging, pairing code) for multi-GB transfers; needs `android-tools`; off by default because it resets on reboot/network change.
- Later (v0.3+): clipboard sync, notifications, SMS via same connection. If SFTP/EXIF limits hurt: own companion Android app serving MediaStore thumbnails + change feed over HTTP/QUIC.
- States: not paired, pairing code, phone asleep/unreachable (show cached listing greyed + "reconnecting"), permission denied on phone (grant "All files access" in KDE Connect).

## 5. UI (good UI, custom-built)
**Source of truth: the EchoFiles design system** — https://claude.ai/artifact/3SkYbhr8K9fudGDyLYJfQ7 (repo: `design/`).
- Tokens: `design/tokens.json`, derived from Omarchy `colors.toml` by `design/build_tokens.py` (port these rules to Rust; run at startup + on `theme-set` hook). Contrast guaranteed per theme.
- Icons: `assets/icons/glyph/*.svg` (86, `currentColor`) and `assets/icons/color/*.svg` (71, palette slots recoloured per theme before `resvg`). Generator: `design/build_icons.py`. Logo: `assets/brand/`.
- Components + composition reference (AppWindow, DualPane, MotionSpec): `design/system/`; local preview `python3 design/build_harness.py` → `design/harness.html`.
- Omarchy rules: font = omarchy font, radius follows Hyprland `rounding`, dialogs/toasts carry 2px accent border (Hyprland active border), no CSD, animations off when Hyprland animations off.

Theme from Omarchy `colors.toml` + `icons.theme`, live reload via theme-set hook. `Theme{bg,surface,border,accent,radius,font}` single source.
- Sidebar, three sections as first-class items (no hidden expander):
  - **Linux**: Home, Documents, Downloads, Music, Pictures, Videos, bookmarks.
  - **Windows**: "Windows (C:)", "AVS (D:)", "AVS (E:)" with bar, free space, pill (Read-only / Dirty / Locked). Click unmounted drive = mount + open with inline spinner. Known Windows user folders nested under C:.
  - **Phone** (later): device name, battery, connection dot, Files (important folders), **Photos**.
- Main: breadcrumb (Ctrl+L → editable path with fuzzy completion), tabs, optional dual-pane split (e.g. Linux left / Windows right, F5 copy / F6 move), dense list view default, auto grid in Pictures/Video/Photos, view mode remembered per folder, sort header, status bar (count, selected size, mount/connection state).
- Keyboard-first: type-to-jump, Ctrl+K command palette (every action), optional hjkl, Space = quick-look preview pane.
- Ops: open via xdg-open, new file/folder, rename inline, trash default, Shift+Delete permanent w/ confirm, properties, progress toast + cancel, error toast, **Ctrl+Z undo** for rename/move/trash (journal).
- States: empty, loading skeleton, dirty-ro banner, BitLocker locked, broken-link. (Later: phone offline/pairing.)

## 6. Milestones
- M1 scaffold + risk spike: install Rust, workspace `core,disks,vfs,ui`, release profile (§2.7), Iced 0.14 on Hyprland, fmt/clippy, udisks probe lists 3 UUIDs. Bench harness + corpus generator + tracing/Tracy (§2.9). **Spike:** custom `FileList` widget with 100k rows + lazy font loading — measure frame time, time-to-first-frame, cold start on Vega (go/no-go for Iced vs GPUI).
- M2 disks + perf core: mount/unmount via zbus, driver setting + ntfs3 vs ntfs benchmark, UUID identity, drive letters, alias.json, threading model + pools, two-phase parallel listing + SoA listing + dir cache + gen-cancel, parallel sort, 60fps on 100k; NTFS cold/warm + parallel statx scaling benchmarks.
- M3 ops: create/rename/trash/copy-move with strategy chain + batch syncfs + pre-flight + progress/cancel, undo journal, watcher, thumbs (spec-compliant + system thumbnailers), `.lnk`/junction/ADS badges, hidden/junk filter, known-folders sidebar.
- M4 polish + ship v0.1: Omarchy theme sync, icons, banners, command palette, tabs, dual-pane, FileManager1 D-Bus, <300ms open.
- v0.2 (Linux + Windows depth): search (nucleo + MFT/crawl index), BitLocker unlock, Recycle Bin integration, on-disk relabel.
- v0.3+ Android: KDE Connect discovery/pairing, SFTP browse (important folders), Photos timeline with EXIF thumbs, send/receive, import; later clipboard/notifications, wireless ADB turbo, optional companion app.
