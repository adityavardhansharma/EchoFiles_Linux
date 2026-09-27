# Search for the app: indexed and live — design, tests and benchmark results

Status: implemented in `crates/index` (library `ef_index`), 2026-09-26. The app uses both
search modes. The `ef find` command uses the index for configured roots and searches an
unindexed `--in` folder live.

Update (2026-09-26): the index file is now memory-mapped and compact (format `EFIDX003`), junk folders are skipped by default, and exclusions are editable in Settings → Search — measured on the laptop in [threading-freshness-and-next-optimizations.md](threading-freshness-and-next-optimizations.md). Numbers below are from the original cloud run.

Related: [speed-architecture-and-agent-index.md](speed-architecture-and-agent-index.md)
(why we build our own index), `build plan.md` §2.8 (index) and §2.9 (measurement).

## Summary in plain words

EchoFiles gets **two kinds of search that give the same results**:

- **Indexed search:** instant, but only as fresh as the last crawl or rescan.
- **Live search:** reads the disk every time, like `fd`, so it's always current. It works on
  any folder: USB sticks, network shares, `/tmp`.

Both use the same matching and ranking code.

1. **Indexed search is very fast at every folder size.**
   - A 100-file folder: ~1.3 µs.
   - A 100,000-file folder: 0.04–3.2 ms, even when nearly every file matches.
   - `fd` needs 51–98 ms for the same 100k search on this machine.
2. **Live search beats `fd` without any index.** At 100k files it takes 9–45 ms against
   `fd`'s 36–98 ms: **~2–4× faster**. Inside the app there's also no process to start, so
   a small folder takes ~0.1 ms against `fd`'s ~4.5 ms.
3. **Searching inside one folder costs only that folder.** With the index, a 100-file folder
   inside a 100k-file tree takes **1.3 µs**, the same as a standalone 100-file folder.
4. **Large searches use every core.** Folders with more than 50k entries are split across
   threads. That fixed the old weak spot: a query matching ~70% of 100k files went from
   ~10 ms to **3.0 ms**, and to **1.8 ms** for the top 50.
5. **Special characters, accents and similar names all work.**
   - Case, accents and fullwidth letters don't matter: "resume" finds "Résumé" in both
     Unicode forms (NFC and NFD).
   - `ß` ↔ `ss`, `İ` → `i`, and Greek final sigma all match.
   - CJK, Devanagari, emoji, shell characters, quotes, tabs, newlines and non-UTF-8 bytes
     all work.
   - Every result of both searches was checked against a brute-force walk: **2,970
     comparisons, all exact.**
6. **Both find everything `find`/`fd` find, plus more.** For "report" on 100k files: 27,400
   hits against `fd`'s 26,182. The 1,218 extra are all fullwidth "Ｒｅｐｏｒｔ" names,
   which `find` and `fd` can't match. Nothing they found is missing.
7. **The index stays fresh cheaply.** Re-crawling a 100-file folder and splicing it into a
   100k-entry index takes **0.14 ms**.
8. **The index is small.** 100k entries take 5.9 MB in memory and 3.4 MB on disk, and load
   in 7 ms.

---

## What was built

### Layout: depth-first order

The crawl writes every entry in **depth-first preorder**: a folder, then everything inside
it, then the next folder. So each folder's whole subtree is one contiguous range of ids,
`[id, end[id])`. Stored per entry (struct-of-arrays, no per-entry allocation):

| Array | Per entry | Purpose |
|---|---|---|
| `parent` | u32 | rebuild paths, walk up for the hidden check |
| `end` | u32 | subtree range; jump over a subtree to reach the next sibling |
| `flags` | u8 | file / folder / symlink / other + "name starts with a dot" |
| `off` + `names` | u32 + bytes | original name, exactly as on disk (NUL-terminated) |
| `foff` + `folded` | u32 + bytes | folded name that queries run against |

Two things this layout makes cheap:

- **Scoped search:** "search in this folder" scans only that folder's slice of the
  `folded` arena.
- **Updates:** `rescan(folder)` re-crawls one folder and replaces its range in all arrays at
  once.

### Crawl

- Parallel: one rayon task per folder.
- Names and `d_type` only, no `statx`. If a filesystem doesn't report `d_type`, one
  dirfd-relative `statat` fills it in.
- Symlinks are indexed but never followed, so link loops are safe.
- Folders that can't be opened count as empty and are reported in `Stats::unreadable`.
- A missing root is an error, not an empty index.

### Matching (`fold.rs`, `matcher.rs`)

Names and queries go through the same **fold**:

| Rule | Example |
|---|---|
| ASCII lowercase (fast path) | `REPORT.PDF` → `report.pdf` |
| NFKD + lowercase + drop Latin/Greek/Cyrillic diacritics | `Résumé` (NFC) and `Re\u{301}sume\u{301}` (NFD) → `resume` |
| Compatibility forms | `Ｒｅｐｏｒｔ` (fullwidth) → `report` |
| Full case-folding extras | `Straße` → `strasse`, `İstanbul` → `istanbul`, `ς` → `σ` |
| Meaningful marks kept | `ガ` ≠ `カ`, `कि` ≠ `क` (only diacritic blocks are dropped) |
| Non-UTF-8 bytes kept | `caf\xe9 menu` still found by `menu` |

`matcher.rs` holds everything else about a query: splitting terms, checking a name,
ranking. It's the only matching code; both indexed and live search call it, which is why
they return the same results.

### Queries (`Query`), shared by both searches

- **Terms:** text is split on whitespace, and every term must occur in the name, in any
  order. `"double quotes"` keep a phrase together.
- **`within`** (indexed only): only descendants of a folder, found with
  `Index::lookup(path)`. Live search takes the folder as a path instead.
- **`kind`:** any / files / folders.
- **`ext`:** the name must end in `.ext`. `report.pdf.bak` is not a PDF, and `.pdf` alone
  is a dotfile, not a PDF.
- **`hidden`:** off by default. Entries with a dot-named folder *below the search folder*
  are skipped, so searching inside `~/.cache` still shows its contents.
- **`limit`:** return only the best N.

**Ranking:**
1. Exact name or exact stem (`report`, `report.pdf`).
2. Name starts with the term (`reportage.txt`).
3. Term starts a word (`annual report.pdf`).
4. Anywhere (`myreport.txt`).

Within each group, shorter names come first. Ties go to tree order (indexed) or path order
(live).

### Indexed search (`Index::search`)

1. Scan the folded arena of the search folder with `memchr::memmem`, using the query's
   longest term. NUL separators stop a match from spanning two names.
2. Check each candidate against the other terms and the filters, and give it a rank key.
3. **Folders with 50,000+ entries below them** are split into `4 × cores` chunks searched in
   parallel. Each chunk keeps only its best `limit` hits, and then the chunks are merged.

### Live search (`live::search`)

- Parallel walk: one rayon task per folder, with raw `getdents` like the crawler.
- Each name is folded and matched as it's read.
- **Hidden folders are skipped without being opened** unless `hidden` is set, so the walk
  reads less than `fd -H` does.
- **Streams results:** a callback gets each folder's hits as soon as that folder has been
  read, so the UI can show results while the walk continues. The final return is ranked
  with the same keys as the index.
- **Cancellable:** setting a flag stops the walk (it returns `Interrupted`), so every
  keystroke can cancel the previous search.
- Hits are kept as raw bytes and sorted with one `memcmp` each. Sorting `PathBuf`s
  compares component by component and was measurably slower.

### Updates and storage (index)

- **`rescan(path)`** re-crawls the deepest indexed folder on the way to `path` and splices
  it in.
  - If that folder is gone, it falls back to the parent, up to the root.
  - The app calls it with the folder an inotify event names.
- **`save` / `load`:**
  - Save writes a temp file and renames it, so readers never see a half-written file.
  - The folded arena isn't stored; it's rebuilt on load, which keeps the file ~40%
    smaller.
  - Load checks the header, the total size, and the tree structure (every parent before
    its child, subtree ranges nested), so a corrupt file is rejected instead of crashing
    a later search.

---

## How it was tested (in detail)

Run: `cargo test -p echofiles-index` → **22 tests, all pass** (6 fold unit tests + 16
integration tests; ~45 s in a debug build, most of it creating 100k files and walking
them).

### 1. The test corpus (`corpus.rs`)

A generator creates real files on disk. Each name is chosen by a fixed pseudo-random mix
of its number, so every run creates the same names. Every name includes its number, so
`n` files always means exactly `n` entries.

| Share | Group | Examples |
|---|---|---|
| 4/16 | **similar "report" family** | `report 17.pdf`, `Report 3.PDF`, `REPORT 9`, `report_final 5.md`, `report-final-v2 8.txt`, `Report Final FINAL 2.docx`, `reports 4.rs`, `reportage 1.jpg`, `2026-report 6.tar.gz`, `rep ort 7.pdf`, `annual report 12.pdf`, `Q3 Report (copy) 40.txt` |
| 8/16 | everyday names | `IMG_20260105_000042.jpg`, `Screenshot 2026-05-01 at 10.3.png`, `DSC_00012.JPG`, `invoice-7.pdf`, `notes 8.md`, `budget_2026_9.xlsx`, `main_10.rs`, `index-11.ts` |
| 2/16 | **Unicode** | `Résumé` (NFC), `Résumé` (NFD), `Straße`, `日本語メモ`, `🎉 party`, `Ｒｅｐｏｒｔ` (fullwidth), `ΣΊΣΥΦΟΣ`, `naïve café`, `हिंदी नोट्स`, `emoji 👨‍👩‍👧` (ZWJ family), `İstanbul` |
| 1/16 | **special characters** | `[draft] (v2) {braces}`, `it's "quoted"`, `$HOME & more`, `back\slash`, `star*q?`, `-dash`, `#hash %`, `semi;colon:`, a tab in the name, a **newline** in the name, leading/trailing spaces, and `bad… \xff\xfe latin1 caf\xe9.txt` (**not valid UTF-8**) |
| 1/16 | **hidden** | `.config-N`, `.report-N.pdf` (a hidden file that *does* match "report") |
| 1 in 1000 | **near-max length** | `report long 999 xxxx…x.txt` (~245 bytes; Linux allows 255) |

Two layouts:

- **Flat:** all files in one folder. This is the "a folder with 100 / 1k / 10k / 100k
  files" case.
- **Tree:** three folder levels, about 100 files per leaf folder.
  - Top folders: `Documents`, `Photos 2026`, `Projects`, `Archive (old)`, `Ünïcödé Ordner`,
    `.cache` (hidden), `reports`, `Downloads`.
  - Middle folders: `reports N` / `part N`, so folder names match "report" too.
  - Leaf folders: `batch N`.

A built-in guard fails the tests if the corpus stops containing any of the special groups,
so the tests can't pass on an accidentally boring corpus.

### 2. The oracle: every answer of both searches checked against a brute-force walk

For each corpus the test also walks the folders with plain `std::fs::read_dir`, using
**neither the index nor the live search**. For every query it computes the expected answer
by checking every name:

- all terms occur in the folded name;
- the extension matches;
- the kind matches;
- no path component below the search folder starts with a dot (unless `hidden` is on).

For every query, **both** of these must equal the oracle's answer exactly (nothing missing,
nothing extra):
- the index's result;
- live search's result, starting from the same folder.

The test also asserts:
- the index holds exactly one entry per file and folder on disk;
- every hit live search streamed through its callback is also in its final result, and it
  streamed nothing else.

**41 query texts**, chosen to hit every hard case:

- **Similar names:** `report`, `REPORT`, `Report Final`, `final report` (terms in reverse
  order), `rep ort` (two terms), `reportage`, `reports`.
- **Accents and case:** `résumé`, `RESUME`, NFD `résumé`, `cafe`, `istanbul`.
- **Special folding:** `strasse`, `STRAßE`, `σίσυφος`, `ΣΊΣΥΦΟΣ`, `Ｒｅｐｏｒｔ`.
- **Scripts and emoji:** `日本語`, `メモ`, `🎉`, `👨‍👩‍👧`, `हिंदी`.
- **Shell and punctuation:** `[draft]`, `(v2) {braces}`, `it's`, `$home &`,
  `back\slash`, `star*q?`, `-dash`, `#hash %`, `semi;colon:`.
- **Whitespace inside names:** `tab\there`, `new\nline`.
- **Non-UTF-8 name:** `latin1`.
- **Everyday and extensions:** `IMG_2026`, `.pdf`, `.tar.gz`.
- **Stress and edge cases:** `e` (matches nearly everything), `7` (digits), `long` (the
  245-byte names), `zqxj` (matches nothing).

Each text runs with **4 option sets**:

1. defaults;
2. hidden files on;
3. files only, extension `pdf`;
4. folders only, hidden on.

There's also an extension-only query written as `TAR.GZ` (case-insensitive extension).

**Where it runs:**

| Test | Corpus | Search folders | Queries |
|---|---|---|---|
| `folder_with_100_files` | flat 100 | root | 41 × 4 + 1 |
| `folder_with_1k_files` | flat 1,000 | root | 41 × 4 + 1 |
| `folder_with_10k_files` | flat 10,000 | root | 41 × 4 + 1 |
| `folder_with_100k_files` | flat 100,000 (**above the parallel threshold**) | root | 41 × 4 + 1 |
| `tree_with_10k_files_and_scoped_search` | tree 10,000 | root, `Documents`, one leaf folder, `.cache` (hidden folder), `Ünïcödé Ordner` | 5 × (41 × 4 + 1) |

That's 1,485 queries, each checked for indexed **and** live search: **2,970 comparisons**
against the brute-force oracle, all equal.

### 3. Focused tests

- **`ranking_puts_exact_then_prefix_then_word_then_anywhere`**
  - Folder: `report` (a folder), `report.pdf`, `reportage.txt`, `Report Final.pdf`,
    `annual report.pdf`, `myreport.txt`.
  - It asserts that exact order.
  - `limit: 3` must return exactly the first three; `limit: 0` returns nothing.
- **`live_search_ranks_like_the_index`:** the same folder, the same exact order from live
  search, and `limit: 2` returns 2.
- **`parallel_search_with_limit_equals_the_top_of_the_full_ranking`**
  - On 100k files (parallel path), for `e`, `report` and `résumé` with limits 1, 50 and
    1000, the result must equal the first N of the full ranking.
  - This proves the per-chunk shortcut never drops a hit that belongs in the top N.
- **`quoted_phrase_keeps_word_order`**
  - `annual report` finds both `annual report.pdf` and `report annual.pdf`.
  - `"annual report"` finds only the first.
  - Empty quotes and a blank query find nothing.
- **`similar_names_are_all_found_and_distinct`**
  - Files: `report.pdf`, `Report.pdf`, `REPORT.PDF`, `report (1).pdf`, `report (2).pdf`,
    `report copy.pdf`, `report.pdf.bak`.
  - All 7 are found.
  - With `ext: pdf`, exactly 6 are found: `.pdf.bak` is excluded.
- **`non_utf8_names_round_trip_to_real_paths`**
  - A file named `caf\xe9 \xff menu.txt` is found by `MENU`.
  - Its stored name is byte-identical, and the path the index builds for it really exists
    on disk.
- **`symlinks_are_indexed_not_followed`**
  - A symlink pointing back at the root: the crawl ends, and the link is indexed as a
    `Symlink` with nothing below it.
- **`live_search_can_be_cancelled_and_needs_a_real_root`**
  - A set cancel flag returns `Interrupted`.
  - A missing root is an error.
  - A symlink loop doesn't hang the walk.
- **`missing_root_is_an_error`** (index).
- **`rescan_matches_a_fresh_build`** (3,000-file tree):
  1. **Add** a file and a new nested folder with a file inside, then rescan the leaf →
     results equal a fresh full build, 2 hits.
  2. **Delete** the new folder and rescan *the deleted path*. It falls back to the parent
     → 1 hit.
  3. **Rename** `Documents` → `Papers` and rescan the root → same size and same results as
     a fresh build for `report`, `résumé`, `papers`, `documents`.
  4. **Empty** a big folder (shrinking splice) → its subtree is 0, and the total equals a
     fresh build.
  5. A path outside the root is an error.

  After every step, `check()` verifies every structural invariant:
  - array lengths;
  - every parent comes before its child;
  - subtree ranges are nested;
  - every name is NUL-terminated;
  - every folded name equals `fold(name)`.
- **`save_and_load_give_the_same_answers`** (5,000-file tree)
  - Save, load, run `check()`: all 41 queries return identical ids.
  - A truncated file and the old prototype format (`EFIDX001`) are both rejected.
- **Fold unit tests** (`fold.rs`): ASCII, NFC/NFD accents, `ß`/`İ`/sigma/fullwidth,
  CJK/emoji unchanged, kana and Devanagari marks kept, invalid UTF-8 bytes kept.

### 4. Do the tests actually catch bugs?

Checked by breaking the code on purpose and re-running:

| Deliberate bug | Result |
|---|---|
| Index: hidden-entry filter switched off | `folder_with_1k_files` **fails** (oracle mismatch) |
| Index: name offsets off by one in `rescan`'s splice | `rescan_matches_a_fresh_build` **fails** (`check()` invariant) |
| Live: hidden folders not skipped | `folder_with_1k_files` **fails** (live vs oracle) |
| Parallel: each chunk drops its last entry | `folder_with_100k_files` **fails** (oracle mismatch) |

All four were restored afterwards.

---

## Benchmark results

### Machine and method

- **Machine:** cloud container, Intel Xeon @ 2.10 GHz, 4 cores, ext4, Linux 6.18. **This
  is not the Ryzen 9 4900HS / btrfs laptop** from the other docs. Expect the laptop to be
  faster, especially everything parallel, which has 16 threads there against 4 here.
- **Warm page cache.** Corpus files are empty (search reads names only, so sizes don't
  matter).
- **In-process numbers:** `cargo bench -p echofiles-index` (divan, release build with fat
  LTO). **Median of 100 samples.**
- **Allocator:** the benchmarks use **mimalloc, the same allocator as the app**. With
  glibc's default allocator, live search in the tree was ~2.5–3× slower when there were many
  hits, because many threads were allocating paths at once.
- **`find` / `fd` numbers:** `python3 bench/index_compare.py` (whole process, **median of
  7**, after one warm-up). `fd` is 10.5.0.
- All numbers below come from **one run** on the same machine, so they compare fairly.
  Repeated runs vary by about ±10–20%.

### Indexed search: whole folder, by folder size (flat corpus)

| Query | 100 files | 1,000 | 10,000 | 100,000 |
|---|---|---|---|---|
| no match (`zqxj`): pure scan cost | 0.18 µs | 0.51 µs | 5.9 µs | 68 µs |
| `[draft] (v2)` (shell characters, 2 terms) | 0.28 µs | 0.84 µs | 11.9 µs | 126 µs |
| `日本語` (CJK) | 0.48 µs | 1.5 µs | 15.3 µs | 147 µs |
| `résumé` (accent-insensitive, NFC + NFD) | 0.44 µs | 1.8 µs | 27.9 µs | 207 µs |
| extension only (`pdf`) | 1.3 µs | 11.3 µs | 172 µs | 903 µs |
| `report final` (two terms) | 1.3 µs | 14.3 µs | 224 µs | 846 µs |
| `report` (similar-name family, ~27% of files match) | 1.3 µs | 14.4 µs | 262 µs | 1.33 ms |
| `e`, top 50 only (~70% match) | 3.4 µs | 39 µs | 608 µs | 1.81 ms |
| `e`, all hits ranked (worst case) | 3.9 µs | 49 µs | 686 µs | 3.20 ms |

- **The scan itself is under 1 ns per entry** (68 µs for 100k). The rest of the time goes
  into checking and ranking hits, so the cost grows with the number of *matches*, not
  files.
- **The 100k column is parallel** (above 50k entries); the others use one thread. That's
  why `report` costs ~18× more going 1k → 10k but only ~5× more going 10k → 100k.

### Indexed search inside one folder of a 100k-file tree

| Search folder | Files in it | Time |
|---|---|---|
| a leaf folder (`Documents/reports 0/batch 0`) | ~100 | **1.3 µs** |
| `Documents` (files only) | ~12,500 | 320 µs |
| whole tree | 100,000 | 1.34 ms |

A search in a 100-file folder costs the same whether the index holds 100 or 100,000
entries.

### Indexed vs live vs `find` / `fd` (query `report`, hidden files included)

| Corpus | Indexed (in app) | **Live** (in app) | Simple walk, 1 thread (in app) | `find -iname` (process) | `fd -HI -i -F` (process) |
|---|---|---|---|---|---|
| flat 100 | **1.3 µs** | 80 µs | 30 µs | 1.8 ms | 4.8 ms |
| flat 1,000 | **14 µs** | 424 µs | 273 µs | 2.4 ms | 5.4 ms |
| flat 10,000 | **262 µs** | 3.7 ms | 2.8 ms | 9.7 ms | 13.6 ms |
| flat 100,000 | **1.33 ms** | 35.1 ms | 35.7 ms | 150.4 ms | 97.8 ms |
| tree 100 | — | 114 µs | 32 µs | 1.8 ms | 4.3 ms |
| tree 1,000 | — | 230 µs | 387 µs | 2.7 ms | 5.7 ms |
| tree 10,000 | — | 1.7 ms | 4.3 ms | 10.9 ms | 9.1 ms |
| tree 100,000 | **1.34 ms** | 15.3 ms | 42.5 ms | 87.8 ms | 50.8 ms |

**Notes on this table:**
- The "simple walk" column is the old one-thread `std::fs` baseline, kept for comparison.
- The `find` and `fd` times include starting a process (~1–4 ms), which the app never pays.
- The index was only benchmarked on the 100k tree, so smaller tree rows have no indexed
  number.

**What it shows:**
- **Indexed:** ~11–27× faster than walking in-app, and ~38–74× faster than `fd` at 100k.
- **Live vs `fd`:** ~2.8× faster on the 100k flat folder, ~3.3× on the 100k tree, ~5× on
  the 10k tree. On small folders it's ~40–60× faster, but that's mostly `fd`'s process
  start, not a smarter walk.
- **Live vs the simple walk:**
  - **Trees:** ~2.8× faster, because folders are read in parallel.
  - **One huge flat folder:** the same speed. A single folder can only be read by one
    thread, so both are limited by the kernel's folder read (`getdents`).
  - **Tiny folders:** slower (80–114 µs against ~30 µs), from handing work to threads.
    That's still invisible to a person.

**Result parity** (100k flat, "report", hidden included):

| | Hits |
|---|---|
| `find -iname` | 26,182 |
| `fd -i` | 26,182 |
| indexed | 27,400 |
| live | 27,400 (asserted equal to the oracle and the index in the tests) |

- All 26,182 `find` hits are in the index result.
- The 1,218 extra are all `Ｒｅｐｏｒｔ N` (fullwidth letters), which only our folding
  matches.

### Queries that match almost everything: indexed vs live vs `fd` / `find`

The same three queries on both 100k corpora, hidden files included.

**Flat folder, 100,000 files**

| Query | Hits (ours / `fd`) | Indexed | Indexed, top 50 | **Live** | `fd` | `find` |
|---|---|---|---|---|---|---|
| `zqxj` (no match) | 0 / 0 | **0.04 ms** | — | 30.4 ms | 92.0 ms | 123.8 ms |
| `report` (~27%) | 27,400 / 26,182 | **1.49 ms** | — | 35.8 ms | 95.4 ms | 152.5 ms |
| `e` (~70%) | 69,759 / 67,374 | **3.16 ms** | 1.81 ms | 44.7 ms | 86.8 ms | 155.9 ms |

**Tree, 100,000 files**

| Query | Hits (ours / `fd`) | Indexed | Indexed, top 50 | **Live** | `fd` | `find` |
|---|---|---|---|---|---|---|
| `zqxj` (no match) | 0 / 0 | **0.04 ms** | 0.04 ms | 9.3 ms | 36.0 ms | 73.5 ms |
| `report` (~27%) | 27,433 / 26,215 | **1.34 ms** | 0.86 ms | 15.1 ms | 53.3 ms | 88.7 ms |
| `e` (~70%) | 69,797 / 67,412 | **3.02 ms** | 1.86 ms | 22.9 ms | 57.3 ms | 99.6 ms |

**What this shows:**

- **Every approach slows down as hits grow, including `fd`.** `fd` reads the disk on every
  search, so it costs 36–92 ms even when nothing matches. Many matches add cost on top,
  mostly for writing the results: `fd` in the tree goes 36 → 53 → 57 ms.
- **Indexed has almost no fixed cost** (0.04 ms with no match), so its time is almost all
  per-hit work.
  - Before the parallel search, the `e` query took ~10 ms.
  - Split across 4 cores it's **3.0–3.2 ms**, and **1.8 ms** with `limit: 50`, because each
    chunk throws away everything outside its own top 50 before the merge.
- **Live is 1.9–3.9× faster than `fd` on every query here.**
  - It skips process start and output printing.
  - It doesn't read hidden folders when they're excluded (these runs include hidden files,
    so this didn't help here).
  - It reads folders with raw `getdents` in parallel.
- **Both of ours find more hits than `fd`.** The extra `e` hits are names where `e` only
  appears after folding: `é` in `Résumé` and `café`, and fullwidth `ｅ` in `Ｒｅｐｏｒｔ`.
  The extra `report` hits are the fullwidth `Ｒｅｐｏｒｔ` names.

### Build, load, update, size (index)

| | 100 | 1,000 | 10,000 | 100,000 |
|---|---|---|---|---|
| Build (crawl + layout), flat | 95 µs | 521 µs | 3.4 ms | 32.5 ms |
| Build, tree | 100 µs | 269 µs | 2.0 ms | 16.9 ms |
| Load from disk (tree) | 6.2 µs | 53 µs | 665 µs | 7.3 ms |
| Memory (heap) | 0.01 MB | 0.06 MB | 0.57 MB | 5.9 MB |
| File on disk | <0.01 MB | 0.03 MB | 0.33 MB | 3.4 MB |

- **Rescan of one ~100-file folder inside the 100k index:** **136 µs** median (crawl +
  splice).
- A tree builds faster than a flat folder of the same size: many small folders crawl in
  parallel, while one huge folder is read by one thread.
- A full index build of the 100k tree (16.9 ms) takes about as long as **one** live search
  of it (9–23 ms). So the index pays for itself from the second search onwards.
- Scaled to the home folder from the other doc (734k entries), expect ~43 MB of memory
  (~59 bytes per entry here, with longer real names adding a little).

---

## When the app should use which

| Situation | Use |
|---|---|
| Search box in an indexed place (home, mounted Windows drives) | **Indexed**: 1–3 ms at 100k, fine on every keystroke |
| Folder the index doesn't cover (USB, network share, `/tmp`) | **Live**, streaming results as it goes |
| Index still being built on first run | **Live** |
| Confirming indexed results (Auto mode) | Indexed first, then **live** on the current folder to add anything new and drop anything gone |
| User chose "always live" | **Live** |

---

## Known limits and next steps

1. **Per-hit cost still dominates big result sets.** Parallel search brought ~70k hits
   down to ~3 ms. A bounded heap inside each chunk, instead of collecting and then
   selecting, would cut `limit` searches further.
2. **Live search on one huge flat folder can't use more cores:** it's bound by the
   kernel's folder read (`getdents`). Folding and matching could move to other threads
   while the next batch of names is read, but the gain would be small.
3. **Memory:** names are stored twice (original + folded). For ASCII-only names the folded
   copy could be skipped with a per-entry flag, roughly halving the arena.
4. **Load time:** memory-map the file instead of reading it; only the folded arena then
   needs to be built at load.
5. **Terms containing `/`** (path fragments like `docs/report`) match nothing today;
   matching against full paths needs a separate path pass.
6. **Not wired into the UI yet**, and no scheduler: nothing indexes automatically yet.
   What's missing:
   - the indexer service (inotify + debounce, a 60 s idle re-crawl with an adaptive
     interval, saving to disk);
   - Auto mode's merge of indexed and live results;
   - the search box.
7. **Re-run these benchmarks on the laptop** (btrfs, 16 threads) and on an NTFS
   partition, per build plan §2.9.

## Reproduce

```sh
cargo test  -p echofiles-index                 # 22 tests
cargo bench -p echofiles-index                 # generates corpora in ~/.cache/echofiles-bench/index/
python3 bench/index_compare.py                 # find / fd on the same corpora (two tables)
```
