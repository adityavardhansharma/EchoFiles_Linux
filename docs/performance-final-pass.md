# Performance: final pass (September 2026)

A last look at speed and memory before calling this version done. **Nothing in this document
has been changed in the code** — it records what was measured, what the numbers mean, what
earlier numbers were wrong, and what is worth trying next, in order.

Machine: the Omarchy 4 laptop (Hyprland 0.56, Arch, btrfs home, hybrid AMD/NVIDIA). Release
build (`lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, mimalloc).

## Summary

| What | Now | Verdict |
|---|---|---|
| Start to first frame | 66–71 ms (3 runs) | Good. Most of it is Vulkan/wgpu setup. |
| Open a folder (names on screen) | < 5 ms for normal folders; 100k folder ≈ 35 ms | Good. |
| Scroll a 100k folder | p99 under one frame (earlier bench) | Good. |
| Everywhere search (in app) | index answers in a few ms; results limited to 300 rows | Good. |
| `ef find` (agents) | 5.1–7.9 ms average over 20 runs | ~20–30× faster than `fd -HI` (150 ms) on the same home folder. |
| Full index rebuild | 98 ms warm for 179,656 entries (14.1 MB file) | Cheap enough to run every minute. |
| Memory, Home folder open | 34.7 MB private (RssAnon) · 150 MB RSS total | Fine; the rest is shared GPU driver code and the mapped index. |
| Memory, 100k-entry folder | ~113 MB private, 211 MB peak RSS | **The one real weak spot.** See below. |

## 1. Startup

- 66–71 ms from process start to first frame (`ECHOFILES_TIMING=1`), measured on the installed
  binary with the index enabled. The index file is mapped on a background thread, so it adds
  nothing to the first frame.
- The vendored `iced_graphics` patch (load only JetBrains Mono up front, all other fonts in
  the background) is still the biggest single win; without it startup was ~450 ms slower.
- With **Keep running in the background** on, a second launch hands its request to the running
  process over `$XDG_RUNTIME_DIR/echofiles.sock` and exits in **6 ms**; the window appears as
  fast as Hyprland maps it. For people who open the file manager often, this is the real
  "startup time".

Nothing obvious left to cut without touching wgpu itself.

## 2. Memory — the one weak spot

Measured earlier on a synthetic 100,000-entry folder:

- ~113 MB private memory while showing it, 211 MB peak RSS.
- `MIMALLOC_SHOW_STATS=1` at exit: 143.9 MiB committed. So the memory is really held by
  mimalloc, not leaked and not in the GPU driver.

What's in there (by reading the code, not yet by profiling):

1. **Two copies of the listing during load.** Phase A sends the names-only `Listing`, then
   phase B clones it (`let mut full = names.clone()`) and fills metadata. Both stay alive until
   the UI drops the first `Arc` — for a moment two full copies plus the sort keys.
2. **Sort keys** (`NameKeys`) are a second copy of every name, normalised.
3. **Allocator retention.** mimalloc keeps freed pages per thread for reuse. The listing is
   built on rayon threads, so each worker's heap keeps its pages after the work moves on.
4. **Order vectors** for sort and filter are rebuilt per keystroke (`Vec<u32>`, 400 KB each for
   100k entries — small, but several can be alive while the UI catches up).

Worth trying, in order (each is a small, measurable change):

1. Fill metadata **in place** instead of cloning: send phase A as an `Arc<Listing>` and phase
   B as just the metadata columns (`size`, `mtime`, `flags`) to merge. Expected: −25–35 MB at
   100k.
2. Call `mi_collect(true)` (or set `MIMALLOC_PURGE_DELAY=0`) after a big folder finishes
   loading, so freed pages go back to the OS. Expected: most of the 143.9 → ~70 MiB gap.
3. Store sort keys only for names where the key differs from the name (most ASCII names
   don't need a separate key).

None of this matters for normal folders (under 40 MB private). It matters for older laptops
that open huge folders — that's why it's first on the list.

## 3. Search and the index

- The index for the whole home folder (with the recommended exclusions) is **14.1 MB on disk,
  179,656 entries**. It's memory-mapped, so it costs page cache, not private memory, and the
  kernel can drop it under pressure.
- A full re-crawl takes **98 ms warm**. The app re-crawls every 60 s on a 4-thread pool at
  nice 10 with idle I/O priority, so on a busy machine it waits its turn. At this size the
  minute timer costs well under 1% of one core.
- inotify watches the landing folders (Home, Downloads, Desktop, Documents, Pictures, Videos,
  Music) and the open folder; a new or renamed file there triggers a re-index 1.5 s later.
  Dotfile churn and plain writes (`CLOSE_WRITE`) no longer mark the index dirty — only name
  changes do, because the index only stores names.
- `ef find` is 5–8 ms because it maps the same file and searches the stored folded names; no
  crawl, no daemon.

Next steps, only if needed:

- **Incremental update** for watched folders (patch one folder's entries instead of
  re-crawling everything). Only worth it once someone indexes millions of files; at 180k a
  full rebuild is cheaper than the bookkeeping.
- **Cold-cache numbers** (see §5): the 98 ms rebuild will be seconds on a cold disk. The low
  I/O priority keeps that from hurting, but it should be measured.

## 4. Numbers that were wrong earlier

Stated plainly so nobody builds on them:

1. **"Hover costs X% CPU per mouse move" — invalid.** The test script used the old
   `hyprctl dispatch setfloating pid:X` syntax. Hyprland 0.56 moved to Lua dispatch
   (`hl.dsp.window.float({...})`), so the command silently did nothing, the window was never
   placed, and the pointer wasn't over EchoFiles. Needs a re-run with the fixed script.
2. **"81% of home is junk" — really 72%.** 335k of the files were EchoFiles' own benchmark
   data in `~/.cache/echofiles-bench`. Without it: 178k of ~625k real entries are excluded by
   the recommended rules (72%).
3. **Benchmark files in `.cache` are not "cache hits".** `.cache` is an ordinary folder; the
   files were real files on disk. What *is* true: every run after the first read the
   directory data from the kernel's page cache (warm cache). That's the normal case for a
   file manager, but it isn't the first-open-after-boot case — see §5.

## 5. What hasn't been measured yet

- **Cold cache.** Dropping caches needs root (`echo 3 > /proc/sys/vm/drop_caches`), so all
  numbers here are warm. To measure without root: `vmtouch -e <folder>` evicts one folder's
  pages. Expect folder opens on a cold NVMe to be 2–5× slower and the full re-crawl to take
  a second or more.
- **NTFS drives.** All numbers are for btrfs `/home`. The Windows partitions weren't mounted
  during this pass; `ntfs3` directory reads are typically slower than btrfs, and the index
  doesn't cover Windows drives yet.
- **Hover/redraw cost** (see §4.1), with the fixed test script.
- **Memory over a long session** in background mode (days of uptime, many folders opened).
  The periodic re-crawl allocates and frees ~15 MB a minute; mimalloc should reuse it, but it
  needs a 24-hour RSS log to be sure.

## 6. Recommended order

1. Metadata in place instead of `clone()` (§2.1) — biggest memory win, small change.
2. `mi_collect` after big loads (§2.2).
3. Re-run the hover measurement and a 24-hour background-mode RSS log.
4. Cold-cache and NTFS numbers.
5. Only then: incremental index updates.
