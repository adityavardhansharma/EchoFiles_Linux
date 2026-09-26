# Search index for the app — design, tests and benchmark results

Status: implemented in `crates/index` (library `ef_index`), 2026-09-26. Scope: the index the
EchoFiles app uses for search. Agent commands (`ef …`) are out of scope here; the old
prototype `ef` binary is only kept compiling.

Related: [speed-architecture-and-agent-index.md](speed-architecture-and-agent-index.md)
(why we build our own index), `build plan.md` §2.8 (index) and §2.9 (measurement).

## Summary in plain words

1. **Searching is very fast at every folder size.**
   - A 100-file folder searches in under 2 µs.
   - A 100,000-file folder searches in 0.1–3 ms for typical queries.
   - `find` and `fd` need 50–140 ms for the same 100k search on this machine, and a plain
     in-app walk with no index needs ~37–45 ms.
2. **Searching inside one folder costs only that folder.** A 100-file folder inside a
   100k-file tree searches in **2.3 µs**, the same as a standalone 100-file folder.
3. **Special characters, accents and similar names all work.**
   - Case, accents and fullwidth letters don't matter: "resume" finds "Résumé" in both
     Unicode forms (NFC and NFD).
   - `ß` ↔ `ss`, `İ` → `i`, and Greek final sigma all match.
   - CJK, Devanagari, emoji, shell characters, quotes, tabs, newlines and non-UTF-8 bytes
     all work.
   - Every result was checked against a brute-force walk of the real folders.
4. **It finds everything `find`/`fd` find, plus more.** On 100k files it found 27,400
   "report" hits against their 26,182. The 1,218 extra are all fullwidth "Ｒｅｐｏｒｔ"
   names, which `find` and `fd` can't match. Nothing they found was missing.
5. **Staying fresh is cheap.** Re-crawling a 100-file folder and splicing it into a
   100k-entry index takes **0.13 ms**.
6. **It's small.** 100k entries take 5.9 MB in memory and 3.4 MB on disk, and load in
   10 ms.
7. **Weak spot: queries that match almost everything.** A one-letter query at 100k takes
   ~10–11 ms, because every hit gets ranked. `fd` and `find` also slow down on that query,
   and there it's still ~5× faster than `fd` in a tree and ~10× in a flat folder (see
   "The match-everything weak spot" below). There's an improvement listed at the end.

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

### Matching (`fold.rs`)

Names and queries go through the same **fold**:

| Rule | Example |
|---|---|
| ASCII lowercase (fast path) | `REPORT.PDF` → `report.pdf` |
| NFKD + lowercase + drop Latin/Greek/Cyrillic diacritics | `Résumé` (NFC) and `Re\u{301}sume\u{301}` (NFD) → `resume` |
| Compatibility forms | `Ｒｅｐｏｒｔ` (fullwidth) → `report` |
| Full case-folding extras | `Straße` → `strasse`, `İstanbul` → `istanbul`, `ς` → `σ` |
| Meaningful marks kept | `ガ` ≠ `カ`, `कि` ≠ `क` (only diacritic blocks are dropped) |
| Non-UTF-8 bytes kept | `caf\xe9 menu` still found by `menu` |

### Queries (`Query`)

- **Terms:** text is split on whitespace, and every term must occur in the name, in any
  order. `"double quotes"` keep a phrase together.
- **`within`:** only descendants of a folder, looked up with `Index::lookup(path)`.
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

Within each group, shorter names come first, then tree order.

**How it scans:** it runs `memchr::memmem` over the folded arena using the longest term.
NUL separators stop a match from spanning two names. Each candidate is then checked
against the other terms and the filters.

### Updates and storage

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

Run: `cargo test -p echofiles-index` → **19 tests, all pass** (6 fold unit tests + 13
integration tests; ~25 s in a debug build, most of it creating 100k files).

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

### 2. The oracle: every answer checked against a brute-force walk

For each corpus the test also walks the folders with plain `std::fs::read_dir`, **not
using the index at all**. For every query it computes the expected answer by checking
every name:

- all terms occur in the folded name;
- the extension matches;
- the kind matches;
- no path component below the search folder starts with a dot (unless `hidden` is on).

The index's result set must equal that answer **exactly**: nothing missing, nothing extra.
It also asserts the index holds exactly one entry per file and folder on disk.

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

| Test | Corpus | Search folders | Checks |
|---|---|---|---|
| `folder_with_100_files` | flat 100 | root | 41 × 4 + 1 |
| `folder_with_1k_files` | flat 1,000 | root | 41 × 4 + 1 |
| `folder_with_10k_files` | flat 10,000 | root | 41 × 4 + 1 |
| `folder_with_100k_files` | flat 100,000 | root | 41 × 4 + 1 |
| `tree_with_10k_files_and_scoped_search` | tree 10,000 | root, `Documents`, one leaf folder, `.cache` (hidden folder), `Ünïcödé Ordner` | 5 × (41 × 4 + 1) |

That's **1,485 query comparisons** against the brute-force oracle, all equal.

### 3. Focused tests

- **`ranking_puts_exact_then_prefix_then_word_then_anywhere`**
  - Folder: `report` (a folder), `report.pdf`, `reportage.txt`, `Report Final.pdf`,
    `annual report.pdf`, `myreport.txt`.
  - It asserts that exact order.
  - `limit: 3` must return exactly the first three; `limit: 0` returns nothing.
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
- **`missing_root_is_an_error`**.
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
| Hidden-entry filter switched off | `folder_with_1k_files` **fails** (oracle mismatch) |
| Name offsets off by one in `rescan`'s splice | `rescan_matches_a_fresh_build` **fails** (`check()` invariant) |

Both were restored afterwards.

---

## Benchmark results

### Machine and method

- **Machine:** cloud container, Intel Xeon @ 2.10 GHz, 4 cores, ext4, Linux 6.18. **This
  is not the Ryzen 9 4900HS / btrfs laptop** from the other docs. Expect the laptop to be
  faster, especially the parallel crawl, which has 16 threads there against 4 here.
- **Warm page cache.** Corpus files are empty (the index reads names only, so sizes don't
  matter).
- **In-process numbers:** `cargo bench -p echofiles-index` (divan, release build with fat
  LTO). **Median of 100 samples.**
- **`find` / `fd` numbers:** `python3 bench/index_compare.py` (whole process, **median of
  7**, after one warm-up). `fd` is 10.5.0.

### Search: whole folder, by folder size (flat corpus, median)

| Query | 100 files | 1,000 | 10,000 | 100,000 |
|---|---|---|---|---|
| no match (`zqxj`): pure scan cost | 0.15 µs | 0.53 µs | 6.6 µs | 87 µs |
| `[draft] (v2)` (shell characters, 2 terms) | 0.16 µs | 0.69 µs | 12.3 µs | 217 µs |
| `日本語` (CJK) | 0.59 µs | 2.5 µs | 22.8 µs | 241 µs |
| `résumé` (accent-insensitive, NFC + NFD) | 0.42 µs | 1.8 µs | 19.8 µs | 341 µs |
| extension only (`pdf`) | 1.1 µs | 10.8 µs | 113 µs | 1.57 ms |
| `report` (similar-name family, ~27% of files match) | 1.4 µs | 15.5 µs | 251 µs | 3.14 ms |
| `report final` (two terms) | 1.6 µs | 16.6 µs | 254 µs | 3.21 ms |
| `e`, top 50 only (nearly all match) | 4.3 µs | 55 µs | 829 µs | 8.9 ms |
| `e`, all hits ranked (worst case) | 4.2 µs | 61 µs | 897 µs | 9.9 ms |

- **The scan itself is under 1 ns per entry** (87 µs for 100k). Almost all remaining time
  goes into checking and ranking hits, so the cost grows with the number of *matches*,
  not files.
- Even the worst case, 100k matches, stays under one 60 Hz frame (16.7 ms).

### Search inside one folder of a 100k-file tree

| Search folder | Files in it | Time |
|---|---|---|
| a leaf folder (`Documents/reports 0/batch 0`) | ~100 | **2.3 µs** |
| `Documents` (files only) | ~12,500 | 410 µs |
| whole tree | 100,000 | 4.0 ms |

A search in a 100-file folder costs the same whether the index holds 100 or 100,000
entries.

### Index vs no index vs `find` / `fd` (query `report`, hidden files included)

| Corpus | Index search (in app) | Walk + match, no index (in app) | `find -iname` (process) | `fd -HI -i -F` (process) |
|---|---|---|---|---|
| flat 100 | **1.4 µs** | 28 µs | 1.9 ms | 4.5 ms |
| flat 1,000 | **15.5 µs** | 342 µs | 2.6 ms | 5.7 ms |
| flat 10,000 | **251 µs** | 3.7 ms | 10.3 ms | 13.5 ms |
| flat 100,000 | **3.1 ms** | 37.5 ms | 138.8 ms | 97.0 ms |
| tree 100 | — | 34 µs | 2.0 ms | 4.7 ms |
| tree 1,000 | — | 420 µs | 2.6 ms | 5.3 ms |
| tree 10,000 | — | 4.6 ms | 10.8 ms | 9.6 ms |
| tree 100,000 | **4.0 ms** | 45.0 ms | 87.9 ms | 52.4 ms |

- `find` and `fd` times include starting a process (~1–4 ms), which the app never pays;
  that's why the fair in-app comparison is the "walk, no index" column.
- **Index vs no-index walk: ~12–22× faster at every size.** Against `find`: ~45× at 100k.
  Against `fd`: ~13–31× at 100k.

**Result parity** (100k flat, "report", hidden included):

| | Hits |
|---|---|
| `find -iname` | 26,182 |
| `fd -i` | 26,182 |
| index | 27,400 |

- All 26,182 `find` hits are in the index result.
- The 1,218 extra are all `Ｒｅｐｏｒｔ N` (fullwidth letters), which only the index's
  folding matches.

### The match-everything weak spot: index vs `fd` / `find`

Same three queries on both 100k corpora, hidden files included.
- Index: 101 runs, median, in-process.
- `fd` / `find`: 7 runs after a warm-up, median, whole process.

**Flat folder, 100,000 files**

| Query | Index hits | Index, all ranked | Index, top 50 | `fd` hits | `fd` | `find` |
|---|---|---|---|---|---|---|
| `zqxj` (no match) | 0 | **0.08 ms** | 0.08 ms | 0 | 110 ms | 142 ms |
| `report` | 27,400 | **3.4 ms** | 3.0 ms | 26,182 | 112 ms | 149 ms |
| `e` (nearly everything) | 69,759 | **10.0 ms** | 9.2 ms | 67,374 | 105 ms | 160 ms |

**Tree, 100,000 files**

| Query | Index hits | Index, all ranked | Index, top 50 | `fd` hits | `fd` | `find` |
|---|---|---|---|---|---|---|
| `zqxj` (no match) | 0 | **0.08 ms** | 0.12 ms | 0 | 34 ms | 80 ms |
| `report` | 27,433 | **4.0 ms** | 3.2 ms | 26,215 | 41 ms | 86 ms |
| `e` (nearly everything) | 69,797 | **10.8 ms** | 9.2 ms | 67,412 | 52 ms | 91 ms |

**What this shows:**

- **`fd` has the weak spot too, but it hides behind its walk.** `fd` and `find` read the
  disk on every search, so even a query that matches nothing costs 34–142 ms. Many matches
  add some cost on top, mostly writing the results: `fd` in the tree goes 34 → 41 → 52 ms.
- **The index has almost no fixed cost** (0.08 ms for no match). So the per-hit work,
  checking each hit and ranking it, is nearly all of its time. That's why its worst case
  looks large next to its own best case, not next to `fd`.
- **Even the index's worst case beats `fd`'s best case.** `e` at 10–11 ms is still ~3× faster
  than `fd` matching *nothing* in the tree, and ~10× faster in the flat folder.
- **`fd` does less work per hit.** It prints hits in the order it finds them and doesn't
  rank. The index sorts ~70k hits best-first, and that sorting is most of its 10 ms.
- **`limit: 50` barely helps today** (10.0 → 9.2 ms). The index still computes a rank key
  for every hit before selecting the best 50. That's the fix in "Known limits" item 1.
- **The index finds more.** Its extra `e` hits are names where `e` only appears after
  folding: `é` in `Résumé` and `café`, and fullwidth `ｅ` in `Ｒｅｐｏｒｔ`. The extra
  `report` hits are the fullwidth `Ｒｅｐｏｒｔ` names.

### Build, load, update, size

| | 100 | 1,000 | 10,000 | 100,000 |
|---|---|---|---|---|
| Build (crawl + layout), flat | 84 µs | 438 µs | 3.3 ms | 36.9 ms |
| Build, tree | 132 µs | 307 µs | 2.3 ms | 19.6 ms |
| Load from disk (tree) | 9.6 µs | 42 µs | 676 µs | 10.2 ms |
| Memory (heap) | 0.01 MB | 0.06 MB | 0.57 MB | 5.9 MB |
| File on disk | <0.01 MB | 0.03 MB | 0.33 MB | 3.4 MB |

- **Rescan of one ~100-file folder inside the 100k index:** **133 µs** median (crawl +
  splice).
- A tree builds faster than a flat folder of the same size: many small folders crawl in
  parallel, while one huge folder is read by one thread.
- Scaled to the home folder from the other doc (734k entries), expect ~43 MB of memory
  (~59 bytes per entry here, with longer real names adding a little). A search would be
  about 0.6 ms of scan plus the per-hit cost.

---

## Known limits and next steps

1. **Ranking many hits costs more than scanning.** ~70k hits take ~10 ms, and `limit: 50`
   only saves ~1 ms. Fix: score while scanning, and keep a bounded heap of the best N when
   `limit` is set, instead of scoring every hit and then selecting. Target: under 3 ms for
   a one-letter query at 100k, with the search box always using a limit.
2. **Memory:** names are stored twice (original + folded). For ASCII-only names the folded
   copy could be skipped with a per-entry flag, roughly halving the arena.
3. **Load time:** memory-map the file instead of reading it; only the folded arena then
   needs to be built at load.
4. **Terms containing `/`** (path fragments like `docs/report`) match nothing today;
   matching against full paths needs a separate path pass.
5. **Not wired into the UI yet**, and no inotify glue. The API for both is ready: `build`
   on a background thread, `search` from the search box, `rescan` on events.
6. **Re-run these benchmarks on the laptop** (btrfs, 16 threads) and on an NTFS
   partition, per build plan §2.9.

## Reproduce

```sh
cargo test  -p echofiles-index                 # 19 tests
cargo bench -p echofiles-index                 # generates corpora in ~/.cache/echofiles-bench/index/
python3 bench/index_compare.py                 # find / fd on the same corpora
```
