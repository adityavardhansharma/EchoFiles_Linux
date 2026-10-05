# EchoFiles and Flea field benchmark

Run from this repository with a built EchoFiles release binary and a built Flea checkout:

```sh
python3 bench/flea_field.py --flea /path/to/flea/target/release/flea
```

The script builds Flea's three published GUI fixture shapes on btrfs: 100,000 empty
text files, 2,000 mixed media files, and 1,700 matched media files with HEIC and MKV
removed. It checks fixture counts and reuses completed fixtures. Three fresh launches
per app and fixture are the default. It saves raw runs to
`bench/flea-field-results.csv` and a separate, untimed eight-format thumbnail
capability check to `bench/flea-capability-results.json`. It refuses to run while
either app already has a window open. It never deletes an unmarked fixture.

`mapped_ms` is launch to Hyprland window registration. `settled_ms` is launch to
500 ms with no change in process-tree CPU ticks, after mapping. `pss_mib` is peak
window-process PSS sampled during the run. `cpu_s` is process-tree CPU tick delta;
this is less complete than Flea's delegated-cgroup accounting because reparented
helpers can escape the process tree. `thumbs` counts files in a private
`XDG_CACHE_HOME/thumbnails` directory; this includes normal and large sizes.
The cache is cleared before each run. The capability pass offers one sample of each
format for 45 seconds and is not timed or ranked.

The default is **warm page cache**, because this run had no sudo authentication for
`/proc/sys/vm/drop_caches`. `--cold` enables Flea's per-run page-cache drop and
asks for sudo authentication in a terminal. Memory and timing depend on the desktop,
graphics stack, load, and installed thumbnailers; compare the paired rows from one
run, not these rows with Flea's published numbers from another machine.
This run occurred with one-minute system load near 2; Flea's original harness waits
for load below 0.50, so treat these timings as exploratory. Re-run on an idle machine
for a publishable comparison.

EchoFiles has no TUI entrant, so Flea's terminal-only bracket has no EchoFiles
counterpart. Flea's backend `--prewarm` experiment also has no EchoFiles equivalent.

## Run on 27 September 2026

Ryzen 9 4900HS, btrfs, Linux 7.1.9-arch1-2, warm page cache; EchoFiles
`0a5b134`, Flea `f304c38`. Three-run medians:

| Fixture | App | Mapped | CPU-idle settled | Window PSS | Process-tree CPU | Thumbnails |
|---|---|---:|---:|---:|---:|---:|
| 100,000 text | EchoFiles | 168 ms | 0.87 s | 207.9 MiB | 0.84 s | 0 |
| 100,000 text | Flea | 1,043 ms | 2.05 s | 157.5 MiB | 0.98 s | 0 |
| 2,000 media | EchoFiles | 125 ms | 1.58 s | 167.3 MiB | 2.22 s | 18 |
| 2,000 media | Flea | 1,029 ms | 2.29 s | 159.4 MiB | 3.35 s | 31 |
| 1,700 matched | EchoFiles | 130 ms | 1.62 s | 172.0 MiB | 2.34 s | 18 |
| 1,700 matched | Flea | 1,014 ms | 2.31 s | 159.1 MiB | 3.32 s | 31 |

EchoFiles maps and reaches CPU idle sooner on all three fixtures. Flea uses less
window PSS on the 100,000-file fixture. **Media settle time and CPU are not a ranked
speed comparison**, because Flea produced 31 thumbnails and EchoFiles produced 18.
The separate capability pass verifies whether an app can produce each format at all.
With one file per format and the full 45-second allowance, **both apps produced JPG,
PNG, WebP, HEIC, MP4, WebM, and MKV; neither produced a TXT thumbnail**. See the JSON
for the raw capability result.

## Backend listing on the scale fixture

EchoFiles' existing Divan listing benchmark accepts `ECHOFILES_BENCH_100K` so it can
run on the exact scale folder instead of EchoFiles' older mixed-file corpus:

```sh
ECHOFILES_BENCH_100K=$HOME/.cache/echofiles-bench/flea-field/scale \
  cargo bench -p echofiles-core --bench listing -- d100k --sample-count 10
```

In this run, EchoFiles' median was 23.89 ms for names, 38.23 ms for names and all
metadata, and 3.17 ms for name sorting. Three direct Flea `--backend` requests on
the same folder reported a median 29.34 ms read, 3.57 ms sort, and 0.57 ms stat
for its first 350 rows. Flea's stat phase covers 350 rows; EchoFiles' full-listing
phase covers all rows. The timings describe different amounts of work and should
be compared phase by phase only where their definitions match.
