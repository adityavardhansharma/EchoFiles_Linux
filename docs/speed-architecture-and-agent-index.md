# Making EchoFiles faster (core architecture) and an agent index in Rust

Status: discussion + measured prototype, 2026-09-26. Scope: core architecture only —
animations and compositor settings are out of scope here. Supersedes the agent sections of
`performance-default-and-ai.md` where they differ: **agents get commands + a skill, no MCP.**

Machine: Ryzen 9 4900HS (8c/16t), btrfs on NVMe, Omarchy/Hyprland. Home folder: 734,414
entries in 80,133 directories. All timings warm page cache unless stated.

## All findings in plain words

1. **EchoFiles is already at the hardware limit for opening folders.** A 100,000-file folder
   lists in ~25 ms, sorts in ~7 ms and gets its sizes and dates in ~15 ms. Scrolling it holds
   60 fps. Nautilus needs 10.8 s for the same folder.
2. **The biggest remaining cost is starting the program each time.** Each launch pays
   ~70 ms for GPU/Vulkan, fonts and icons. Keep one EchoFiles engine running in the
   background and windows appear in ~10–20 ms, and every cache stays warm.
3. **Our own index in Rust works, and it's fast.** A prototype crawled all 734,414 entries
   in your home folder in 0.12 s. It finds files with exactly the same results as `find` and
   `fd`.
4. **Searching the index is almost free; loading it is the cost.** The search itself takes
   1.4 ms. As a standalone command it takes ~26 ms, because it reads a 49 MB file from disk
   each time. That's still 4.5× faster than `fd` and ~50× faster than `find`. If the running
   engine keeps the index in memory, a search drops to ~2–3 ms.
5. **The index file is too big and can shrink ~2×** (49 MB → ~20–25 MB) by not storing a
   lowercase copy, memory-mapping it, and compressing shared name prefixes.
6. **An index is only useful if it stays fresh.** The system's `plocate` index is stale:
   it knows 22 PDFs where there are 5,502. Plan: quietly re-crawl every minute (0.12 s of
   CPU), watch the folders people are looking at, and later use a small privileged helper for
   live whole-disk change tracking.
7. **For AI agents the benefit is real, but it isn't mainly milliseconds.** One AI tool call
   takes seconds, so saving 0.1 s per search is small. The big gains:
   - Agents make far fewer calls, because one `ef` command does what several `ls`/`find`
     calls did.
   - They read far less output, which means fewer tokens.
   - They stop failing on Windows drives: path translation, mount state, junk folders.
   - They get **undo** for anything they move or trash.
   - On Windows drives and cold disks, where `find` takes seconds to minutes, the index
     answers in milliseconds.
8. **Commands + a skill is the right delivery, not MCP.** Every agent can run commands, and
   the skill only loads when needed. Omarchy already does exactly this for its own tools.
9. **Windows drives are the biggest unknown and the biggest possible win.** Reading the
   NTFS Master File Table directly could index a whole Windows drive in about a second, even
   unmounted. Not measured yet, because the drives aren't mounted; this is M2 work.
10. **The rendering path has small, specific leftovers:** rare 24–25 ms frames, strings
    rebuilt every frame, and icons rasterised on first use. Fix these with a profiler first,
    and fork libraries only where the profile points.
11. **Memory (141 MB) must come down before the engine stays resident.** Main suspects:
    the full system font set, GPU driver buffers, and the index. Target < 70 MB idle.

---

## Part 1 — Where EchoFiles' time goes today

| Stage | Now | How measured |
|---|---|---|
| Process start → first frame | 67–75 ms (91 ms with a 100k folder) | `ECHOFILES_TIMING=1` |
| 100k folder: names (`getdents64`) | 24–30 ms | in-app + `cargo bench -p echofiles-core` |
| 100k folder: sort by name | 7 ms (bench), 16–19 ms in-app while startup competes for CPU | same |
| 100k folder: details (`statx` × 100k, 8 threads) | 13–16 ms | in-app |
| Scrolling 100k rows | p50 16.67 ms, p99 17.5 ms, max 17.8 ms (60 Hz) | `ECHOFILES_BENCH` |
| Memory, small folder | 141 MB RSS | `bench/compare.py` |
| vs Nautilus, 100k folder | 0.64 s vs 10.8 s to fully shown | `bench/RESULTS.md` |

Listing and rendering are already at the hardware and refresh-rate limit for warm folders.
The remaining gains are in five places: **process lifetime, cold data, work we repeat, the
render path's worst frames, and memory.**

---

## Part 2 — Core-architecture changes that make EchoFiles faster

Ordered by expected impact. Each lists the evidence, and what's still a guess.

### 1. One resident engine process (`echofilesd`) shared by every window and by `ef`

- **What:** the engine is the listing cache, index, drive map, thumbnail cache, copy
  engine and undo journal. It lives in one long-running process. UI windows and the `ef`
  CLI talk to it over a Unix socket (`$XDG_RUNTIME_DIR/echofiles.sock`, user-only).
  Started by a systemd user unit or on first use. The UI can live in the same process,
  with new windows opened over D-Bus `FileManager1`.
- **Why:**
  - Every cost we pay per launch disappears after the first: GPU/Vulkan setup (~40–60 ms of
    the 67 ms), font database, icon rasterisation, directory cache.
  - The prototype shows the other half: `ef` spends **22–37 ms loading the index** and only
    **1.4 ms searching** it. A resident engine drops that load cost to zero.
- **Expected:**
  - New window ~10–20 ms instead of ~70 ms (estimate).
  - `ef find` ~2–3 ms instead of ~26 ms (1.4 ms measured query + ~0.5 ms process start +
    socket round trip).
- **Cost:** idle memory now matters. Target < 70 MB idle for engine + index; see item 7.

### 2. The index as the metadata backbone, not just search

The same index that answers agents feeds the UI:

- **Item counts** in the Size column for folders ("14 items") without an extra
  `getdents` per folder row.
- **Instant first paint of cold folders:** draw names from the index immediately, then
  reconcile with `getdents` — the same two-phase idea we use now, one step earlier.
- Recursive folder sizes, "recent", and search-as-you-type across the whole machine.
- **Evidence:** a full crawl of home takes **122–126 ms** (parallel, names only). Keeping
  it fresh (item 3) is the real work.

### 3. Keeping the index fresh — the hard part

A stale index is worse than none. `plocate` on this machine returned 22 PDFs where the
truth is 5,502. Options, combined:

| Mechanism | Covers | Cost | Needs |
|---|---|---|---|
| Periodic re-crawl in idle (`SCHED_IDLE`) | everything, eventually | ~125 ms CPU per crawl warm; seconds cold | nothing |
| inotify on the dirs the UI shows + recently changed dirs | live changes where people look | cheap | nothing |
| inotify on all 80k home dirs | all of home, live | ~40 MB kernel memory (≈500 B/watch), 80k `inotify_add_watch` calls at start | nothing (limit is 524,288) |
| fanotify `FAN_MARK_FILESYSTEM` + `FAN_REPORT_DFID_NAME` | whole filesystem, live, one mark | lowest overhead | `CAP_SYS_ADMIN` → small polkit helper |
| btrfs generation numbers (`BTRFS_IOC_TREE_SEARCH`) | catch up on what changed while EchoFiles wasn't running | very fast | root → same helper |
| NTFS USN journal | what Windows changed since last boot | fast | raw volume read → same helper |

Recommendation:
- **v1:** idle re-crawl every ~60 s, plus inotify on shown folders and any folder an
  agent touches. No privileges needed, and 125 ms/min is ~0.2% of one core.
- **v2:** a tiny privileged helper for fanotify, btrfs generations, and raw NTFS (MFT and
  USN), the same helper the plan already needs for MFT indexing.

Every agent query answers with an `indexed_at` age, and `--fresh` forces a re-crawl of the
queried subtree first.

### 4. Windows drives: read the MFT instead of walking

- Walking an NTFS mount through `ntfs3` makes one kernel lookup per entry, often cold.
- Reading the Master File Table directly gives every name and parent in one sequential
  pass. It works for **unmounted** drives too, so an agent can search `D:` without
  mounting it.
- **Not measured yet:** the Windows partitions aren't mounted. This is the biggest unknown
  and the biggest potential win: likely seconds → about a second for a full drive index.
  Benchmark in M2 alongside ntfs3 vs the new `ntfs` driver.

### 5. Visible-first details on cold folders

- Phase B (`statx`) currently covers the whole folder in order. On a cold NTFS or slow
  disk, stat the **visible rows first**, then the rest at lower priority.
- Warm btrfs doesn't need it (100k in 13–16 ms); cold NTFS will.

### 6. Render path: remove per-frame allocations and rasterisation

Frame times are fine on average but show rare 24–25 ms spikes. Known costs per frame:

- Every visible row builds 3–4 `String`s: name, size, kind, date. **Fix:** format lazily
  once per entry into a per-listing string cache.
- Colour icons are SVGs rasterised by resvg on first use at each size. **Fix:**
  pre-rasterise the icon set into a GPU texture atlas per theme at build or theme-change
  time.
- Text shaping of names: Iced's text cache is keyed per frame. **Possible fork:** keep
  shaped names across frames in our own cache. Instrument first (add `tracing` spans around
  update/view/draw; Iced's `debug` feature) — **don't fork without a profile.**

### 7. Memory: 141 MB is too much for a resident engine

- Likely sources to measure:
  - Full system font database: 714 fonts; mapped pages count toward RSS.
  - Vulkan driver allocations and wgpu staging buffers.
  - The index itself: prototype 49 MB on disk.
- **Index diet:**
  - Drop the stored lowercase copy (lowercase at load: ~3 ms for 16 MB) → ~33 MB.
  - Memory-map instead of reading.
  - Front-code names within a directory.
  - Keep `parent` and `off` as `u32`.
  - Estimate: ~20–25 MB for 734k entries.
- **Font diet:** only load fallback fonts on demand (cosmic-text fallback query per
  script), instead of the whole system set — a further patch to our vendored
  `iced_graphics`.

### 8. Startup that remains (only matters without the resident process)

Of the ~67 ms remaining, most is Vulkan instance/device creation and first pipeline
creation. A **wgpu pipeline cache on disk** (fork `iced_wgpu` if it doesn't expose one) is
worth ~10–20 ms (estimate). Low priority once item 1 exists.

### 9. Already decided and measured (for completeness)

- Batch `syncfs` instead of per-file fsync in the copy engine (28× on 2,000 small files).
- Rename → reflink → `copy_file_range` chain.
- Dirfd-relative parallel `statx`.

---

## Part 3 — Does routing agents through EchoFiles actually make them faster?

### Measured: the prototype index vs the tools agents use today

Prototype: `crates/index` (`ef index ~`, `ef find …`). Names-only parallel crawl, one file
on disk, `memchr::memmem` over a NUL-separated lowercase name arena, parent chains
rebuilt to paths on output. Best of 7 runs, output to `/dev/null`, whole home folder:

| Query | `find` | `fd -H -I` | `ef` CLI (loads index each call) | `ef` in resident engine (projected) |
|---|---|---|---|---|
| all `*.pdf` (5,502 hits) | 1,253 ms | 125 ms | **27 ms** | ~2–3 ms |
| name contains "report" (6,068) | 1,327 ms | 122 ms | **27 ms** | ~2–3 ms |
| rare name "build plan" | 1,356 ms | 118 ms | **26 ms** | ~2–3 ms |

- Results are identical: 5,502 / 5,502 / 5,502 and 6,068 / 6,068 / 6,068 across `find`,
  `fd` and `ef`.
- `ef` breakdown: load 22–37 ms, query 1.4–1.5 ms, output 1.2–1.7 ms. Process start is
  0.5 ms.
- Building the index: 122–126 ms for 734k entries; the file is 49 MB.
- An earlier `find` run in this session took 330 ms. `find`'s time varies a lot with dentry
  cache state; `fd` and `ef` are stable.

### What that means for an agent — honestly

- **Per call, it's a real speedup:** 4.5× faster than `fd` today, ~40× with the resident
  engine, and 50–500× faster than `find`, which is what agents most often reach for.
- **Per task, model latency dominates.** An LLM tool call takes 1–10 s end to end, so
  saving 120 ms on one search is invisible. The speed benefit becomes real when:
  1. **Searching many times.** Agents chain five to twenty `find`/`ls` calls exploring an
     unfamiliar tree. At 1.3 s each with `find`, that's 7–26 s of pure waiting.
  2. **Windows drives and cold caches.** A cold walk of a 400 GB NTFS drive takes many
     seconds to minutes; an MFT-backed index answers in milliseconds, even unmounted.
  3. **Fewer round trips.** `ef tree --budget 4kb` or `ef find --in D: --ext pdf --newer 7d`
     answers in one call what takes an agent three or four shell calls. Each round trip saved is
     seconds, far more than any millisecond win.
  4. **Fewer tokens.** Structured, junk-filtered, budgeted output means less for the model
     to read, and reading is billed and slow.
  5. **Fewer wrong turns.** Windows path translation (`D:\Work` → the right mount), mount
     state, and junk filtering (`$RECYCLE.BIN`, `desktop.ini`) prevent failed commands and
     retries.
- **No gain:** reading one known file (`cat` is already optimal), and content search
  (ripgrep is excellent). `ef` can hand ripgrep a pre-filtered file list, which helps only
  on huge trees.

**Verdict:** yes, it benefits agents. Mostly through fewer, better calls and the Windows
drives; raw milliseconds are the smaller part. It's worth building because the index also
makes the UI faster (Part 2, item 2), so agents get it almost for free.

### Commands + skill, no MCP — why that's the right shape

- **Works with every agent Omarchy supports.** They all run shell commands; not all speak
  MCP. Omarchy's own `omarchy` and `diagnose-crash` skills work exactly this way: a
  `SKILL.md` in `/usr/share/omarchy/default/agents/skills/`, symlinked into
  `~/.claude/skills/`.
- **No always-on tool schema in the agent's context.** An MCP server's tool definitions
  sit in every conversation. A skill loads only when the task is about files.
- **Nothing extra to run.** The CLI talks to the resident engine if it's up, and falls back
  to loading the index file itself (~26 ms — still faster than `fd`).
- **Composable.** `ef find … | xargs rg …` works; MCP tools don't pipe.
- Easy to audit: every agent action is a command line in the transcript.

Proposed skill (`/usr/share/echofiles/agents/skills/echofiles/SKILL.md`, symlinked into
`~/.claude/skills/echofiles`). Its description tells agents to use it for finding, listing,
sizing, moving or trashing user files, and always for Windows drives:

| Command | Output | Replaces |
|---|---|---|
| `ef find <text> [--ext E] [--in PATH\|D:] [--newer 7d] [--dirs] [--limit N]` | paths, one per line; `--json` for fields | `find`, `fd`, `locate` |
| `ef ls <path> [--json] [--all]` | kind, size, date, Windows attributes; junk hidden | `ls -la` |
| `ef tree <path> [--depth N] [--budget 4kb]` | shape of a tree within a byte budget | repeated `ls`, `tree` |
| `ef du <path>` | cached sizes | `du -sh` |
| `ef drives` | volumes, drive letters, mount state, free space | `lsblk`, `findmnt` |
| `ef path <windows-or-linux-path>` | the other form; mounts on request | guesswork |
| `ef mv / cp / trash …` | journal id for undo | `mv`, `cp`, `rm` (never `rm -rf`) |
| `ef undo <id>` | reverts an agent's change | nothing today |
| `ef reveal <path>` | opens EchoFiles on it for the user | nothing today |

Every read command prints `# index age: 12s` on stderr and accepts `--fresh`.

### Building the index in Rust — yes, our own

- **Why not reuse plocate or Baloo:**
  - plocate is root-updated daily, so stale (22 vs 5,502 here), and knows nothing about
    NTFS, drive letters or junk.
  - Baloo and Tracker are heavyweight desktop search daemons that do content indexing we
    don't need.
- **Why our own:**
  - The prototype already beats `fd` by 4.5× with ~350 lines of code.
  - It shares data structures with the UI listing (SoA, name arena, `u32` ids).
  - It can take NTFS MFT input, where no existing Linux indexer goes.
- **Next steps for the real one (`crates/index`):**
  1. Memory-mapped file format; drop the stored lowercase copy (49 MB → ~33 MB, then
     front-coding → ~20–25 MB).
  2. Incremental updates (inotify + idle re-crawl), with `indexed_at` per subtree.
  3. Optional `statx` columns (size, mtime) filled lazily for `--newer` / `du`.
  4. Fuzzy ranking with `nucleo` for the UI's search-as-you-type; plain substring for
     agents (predictable, fast).
  5. MFT reader for NTFS via the privileged helper (v2).
  6. Serve it from `echofilesd` over the socket; `ef` falls back to the file.

---

## Decisions needed

1. **Resident engine:** build `echofilesd` now (next after the UI pass), since it's the
   biggest single speed lever for both the UI and agents?
2. **Freshness v1 without privileges** (idle re-crawl + targeted inotify), privileged
   helper later — OK?
3. **Agent writes:** read-only `ef` first, then `mv/cp/trash` through the undo journal?
4. **Index scope:** home + mounted Windows drives by default; other paths only when asked?

## How to reproduce

```sh
cargo build --release -p echofiles-index
target/release/ef index ~                 # crawl + save (~/.cache/echofiles/home.efidx)
EF_TIMING=1 target/release/ef find pdf --ext > /dev/null
python3 bench/compare.py                  # EchoFiles vs Nautilus UI benchmark
```
