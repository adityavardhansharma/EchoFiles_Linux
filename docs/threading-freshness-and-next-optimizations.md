# Threads, fresh search results, and what to speed up next

Status: 2026-09-26. Follows [search-index.md](search-index.md) and
[speed-architecture-and-agent-index.md](speed-architecture-and-agent-index.md). Measured on the
laptop (Ryzen 9 4900HS, 16 threads, btrfs, warm cache) unless marked *estimate*.

## Summary in plain words

1. **The index now barely uses RAM.** It's searched straight from its file on disk, through
   the page cache. On your whole home folder (960k entries), a search keeps only **0.5–9 MB**
   of EchoFiles' own memory. The index file sits in page cache Linux can drop whenever it
   needs RAM. On an old laptop it simply makes way.
2. **Skipping junk folders shrank your index by 81%:** 960k → 178k entries, 48 MB → 14 MB,
   crawl 190 ms → 45 ms. Nothing you'd search for was lost. All 47,420 "extra" PDFs were
   benchmark test files, plus one test PDF inside a Rust package.
3. **`ef` now beats `fd` clearly,** even as a standalone command: 5–11 ms against `fd`'s
   134–146 ms on your home folder. With no exclusions it's still 11–53 ms.
4. **A file added 2 minutes ago is always found.** Three layers make that true (see
   Part 2): watch the folders where new files land, re-crawl quietly every minute (45 ms), and
   check the folder you're in live on every search.
5. **Threads are used in the right places.** One design gap remains: background indexing
   shares the same thread pool as what you're waiting for. The fix is two pools, with the
   background one at idle priority.
6. **Best next speed-ups:** keep the index open in a running engine; group path building in
   `ef` output (output is now most of its time); pre-rasterise icons; stop the per-frame
   string allocations in the list; update folders shallowly instead of re-crawling whole
   subtrees.

---

## Part 1 — What changed today, measured

### Memory-mapped, compact index (`MappedIndex`, file format `EFIDX003`)

- The file stores the search-ready (folded) names, so opening it needs **no folding**. The
  old format rebuilt them on every load, which cost ~100 ms.
- A name that's already in folded form (lowercase ASCII, the most common case) is **stored
  once**, not twice.
- Opening it maps the file and runs one validation pass: **1.6 ms** for 178k entries,
  **5.7 ms** for 960k.
- Search runs on the mapped bytes. The in-memory index and the mapped index share the same
  search code (`view.rs`), and a test checks they give identical answers for every test
  query, with hidden files on and off, inside a folder, and with extension + limit.

### Junk-folder exclusions (`Options`, Settings → Search)

| Rule | What's skipped | Default |
|---|---|---|
| Folder names | contents of `node_modules`, `.git`, `.cache`, `.cargo`, `.rustup`, `.venv`, `__pycache__`, `.gradle`, `.npm`, `.pnpm-store`, `.yarn`, `.m2`, `.tox`, `.mypy_cache`, `.pytest_cache`, `.next`, `.nuxt`, `.hg`, `.svn` | on |
| `CACHEDIR.TAG` | contents of any folder carrying this standard marker: Rust `target/`, Cargo registry, Gradle JDKs, Hugging Face cache, fontconfig cache | on |
| Paths | a folder the user picks, **entirely**, including the folder itself | none |

- Folders excluded by name or tag are still found by name; only their contents are left out.
- Rescans never pull excluded contents back in.
- The rules are saved in the index file, so later rescans keep them.
- Settings live in `~/.config/echofiles/settings.toml` (section `[search]`), edited in-app under **Settings →
  Search** (Ctrl+, or the toolbar gear). The design is in the design system: `SettingsPage`,
  `PathListEditor`, `NameChips`, `SettingRow`.

### Results on your home folder (best of 7, `ef` as a standalone command)

| | Entries | Crawl | File | `ef find pdf --ext` | `ef find report` | `ef find "build plan"` | EchoFiles' own memory |
|---|---|---|---|---|---|---|---|
| Everything | 959,755 | 190 ms | 47.9 MB | 40 ms | 53 ms | 11 ms | 0.5–8.6 MB |
| Recommended exclusions | 177,682 | **45 ms** | **14.0 MB** | **4.8 ms** | **5.1 ms** | **4.6 ms** | **0.5–3.2 MB** |
| `fd -H -I` (reads disk) | — | — | — | 146 ms | 139 ms | 134 ms | — |

- **Where `ef`'s time goes now (everything indexed):** open 5.7 ms, query 2–5 ms,
  **printing 15–29 ms** when 47–67k paths are output. For agents, `--limit` keeps output
  small.
- **Tests:** 25 in `echofiles-index` (22 before, plus mapped equality, name exclusions and
  path exclusions + settings), all pass; the whole workspace builds with no warnings.

---

## Part 2 — "If search uses the index, how do I find a file I added 2 minutes ago?"

Four layers, from cheapest to most thorough:

| Layer | What it covers | Delay | Cost |
|---|---|---|---|
| **1. Watch the folders new files land in**: Downloads, Desktop, Documents, Pictures, the folders open in EchoFiles, and the ~500 most recently changed folders (inotify) | nearly every new file | **< 1 s** | a few hundred watches, < 1 MB kernel memory |
| **2. Quiet re-crawl of all indexed folders** every ~60 s at idle priority; longer on battery | everything else, including changes EchoFiles missed | **≤ 60 s** | **45 ms** CPU per crawl on your home folder |
| **3. Live check at search time**: every search also reads the folder you're in (and its subfolders, if small) and merges in anything new, dropping anything gone | the place you're looking | **0 s** | ~0.1–10 ms |
| **4. "Search everywhere, live"** when there are no results or you press Enter: a full live walk of the indexed folders | anything at all | 0 s | ~50 ms warm on your home folder with exclusions (*estimate* from the 45 ms crawl) |

So a file saved 2 minutes ago is in the index already, through layer 1 or 2. A file saved 2
seconds ago is found by layer 1 if it's in a watched folder, or by layer 3 if you're looking
at its folder. Layer 4 covers everything else.

**Why not watch every folder?** Your home folder has 24,660 folders even with exclusions.
Watching all of them with inotify pins each folder's inode in kernel memory, roughly
**15–25 MB of RAM that can't be reclaimed** (*estimate*, ~0.6–1 KB per watched inode). That
works against the low-memory goal. Layers 1 + 2 give nearly the same freshness for far less.

**How updates reach a mapped (read-only) index:** new and changed entries go into a small
in-memory "recent changes" list that every search also scans. It's only a few thousand
names, so microseconds. Each layer-2 crawl then writes a new index file (45 ms crawl + ~36 ms
save) and swaps it in, and the list empties. This is how search engines handle updates (an
LSM-style design), and it keeps memory flat.

**Later, with a small privileged helper:** fanotify on the whole filesystem reports every
change instantly with **one** watch and no per-folder memory. That would make layers 1 and 2
nearly redundant. It needs root rights, so it waits for the helper the MFT index also needs.

---

## Part 3 — Threads: where one, where many

| Work | Threads today | Right? | Why |
|---|---|---|---|
| Drawing, input, `update`/`view` | the UI thread only | yes | nothing blocks it; frames stay ≤ 17.8 ms at 100k rows |
| Reading a folder's names (`getdents`) | 1 | yes | the kernel reads one folder sequentially; nothing to split |
| Sizes and dates (`statx`) | rayon, chunks of 1,024 | yes | 8 threads were 5–7× faster than 1 on 100k; folders under ~1k stay on one thread, avoiding hand-off overhead |
| Sorting | 1 below 20k, rayon above | yes | 100k sorts in 7 ms; small folders sort in µs on one thread |
| Index crawl | one rayon task per folder | yes | trees crawl in parallel; one huge flat folder is kernel-bound |
| Indexed search | 1 below 50k entries, all cores above | yes | small scopes finish in µs; 100k on 16 threads: 0.8 ms |
| Live search | one rayon task per folder | mostly | tiny folders are slower than one thread (80 µs vs 30 µs), which is still invisible |
| Settings save, drive probe | a short-lived thread each | fine | rare |
| "Slow folder" timer | **a new OS thread per navigation** | no | spawning a thread just to sleep 150 ms; use one shared timer |
| Status-bar hidden count | ran on **every frame** | fixed | now counted once per folder, on the loader thread |

### The one real design gap: priorities

Everything uses **one global rayon pool**. When background indexing runs (the layer-2
crawl), a folder you just opened queues behind it.

**Fix:** two pools. A foreground pool (all cores, normal priority) for listing, sorting and
search. A background pool (2–4 threads, `SCHED_IDLE` + idle I/O priority) for crawls,
thumbnails and folder sizes. Cost: a few idle threads. Their stacks are virtual memory, so
there's no real RAM increase. Background work then never delays what you're waiting for,
and on a busy machine it pauses by itself.

---

## Part 4 — What else can be faster (no extra RAM)

| # | Change | Where | Expected gain | Notes |
|---|---|---|---|---|
| 1 | **Resident engine keeps the index open** | architecture | `ef` 5–11 ms → ~2–3 ms; no open/validate per call | also where the "recent changes" list lives |
| 2 | **Group path building in output** (hit ids are in tree order, so neighbours share parent paths) | `ef`, UI results | printing 15–29 ms → a few ms on huge result sets (*estimate*) | pure CPU, no memory |
| 3 | **Shallow folder updates** instead of re-crawling a whole subtree | index `rescan` | a change directly in `~` re-reads one folder, not the whole home folder | keeps unchanged subfolders |
| 4 | **Don't copy the listing between the two loading phases** | UI loader | removes a temporary second copy (~4 MB for 100k) | send only the size/date columns |
| 5 | **Write `statx` results in place** | core listing | removes a 16 B/entry temporary buffer | small speed + memory win |
| 6 | **Cache formatted row text** (size, kind, date) per folder | file list | fewer allocations per frame; smoother worst-case frames | measure the rare 24 ms frames first |
| 7 | **One shared timer** for the 150 ms "slow folder" skeleton | UI | no thread spawn per navigation | tiny |
| 8 | **Validate the index file once per file, not per open** | index | 1.6–5.7 ms per open → ~0 | matters only without the engine |
| 9 | **Stream the crawl into the final arrays** instead of batches + copy | index build | lower peak memory while indexing (68 MB → ~40 MB for 960k, *estimate*) | speed about the same |

### Library-level changes worth making

Rule, as before: fork only when a benchmark or profile proves the win, keep the patch small,
and document it in the vendored folder.

| Library | Change | Expected gain | Confidence |
|---|---|---|---|
| **iced / resvg** | pre-rasterise all icons into one GPU texture atlas per theme, instead of resvg on first use per size | removes first-scroll hitches; fewer texture binds | high — no fork needed, our own code on top of Iced |
| **cosmic-text / fontdb** (via our `vendor/iced_graphics` patch) | load fallback fonts on demand per script instead of all 714 in the background | ~20–450 ms less background CPU at startup; tens of MB less mapped memory | medium — small extension of the existing patch |
| **iced_wgpu** | save the GPU pipeline cache to disk | ~10–20 ms off a cold start (*estimate*) | medium; irrelevant once the engine is resident |
| **iced text** | keep shaped filenames across frames in our own cache | lower worst-case frame time | unknown — profile first; frames are already ≤ 17.8 ms |
| **kernel NTFS driver** (`ntfs3` vs the new `ntfs`) | pick the faster driver for your Windows drives | the biggest unknown for Windows drives | measure in M2 |
| `memchr`, `rayon`, `mimalloc` | — | none | already the fastest options |

---

## Decisions needed

1. **Freshness:** layers 1–4 as described, and the privileged fanotify helper later — OK?
2. **Two thread pools** (foreground / idle background) as part of the engine work — OK?
3. **Order of the next work:** resident engine → search in the UI, with the settings you
   just saw → shallow updates → output path grouping → icon atlas?
