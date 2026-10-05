# Architecture and performance review evidence

The report is [architecture-and-performance-2026-10-03.md](../../../design/docs/architecture-and-performance-2026-10-03.md).

This directory contains a frozen source snapshot, file hashes, the existing workspace test log, and a separate audit probe. It does not replace the earlier review in `reviews/2026-10-03/REPORT.md`.

The snapshot was captured on 3 October 2026 at 22:39:33 UTC, including uncommitted work. Supporting icons and vendored fonts were copied afterward without changing captured source files. `snapshot.json` records hashes of every included file. The snapshot contains the Rust workspace and reviewed Android source/configuration; it is not an Android build distribution with Gradle caches or compiled libraries.

## Reproduce the existing tests

Run from the repository root:

```bash
CARGO_TARGET_DIR="$PWD/target" cargo test \
  --manifest-path reviews/2026-10-03/architecture-performance/snapshot/Cargo.toml \
  --workspace --locked
```

The recorded run passed 69 tests. Phone tests use disposable identities and localhost test connections, although the service uses its normal listener/discovery implementation. Android runtime and end-to-end device behavior are not covered by this command.

## Reproduce the additional probe

```bash
python3 reviews/2026-10-03/architecture-performance/run_probe.py
```

The runner builds the snapshot's core, index, and config libraries with `--locked`. It uses Cargo's JSON artifact output to select those exact libraries, compiles `probe.rs`, and executes it in a temporary directory. `probe-artifacts.json` records the selected libraries. Set `ECHOFILES_REVIEW_TARGET` to use a different Cargo output directory.

All probe data writes, including the trash directory, are temporary. The probe does not start a phone service or access real documents. It checks replacement undo, failed replacement, source aliases, FIFO copying, progress counters, malformed index parsing, concurrent configuration saves, and phone JSONL search.

The UI-undo comparison executes the same `Undo::Copy` construction found in the captured UI handler. It exercises the core behavior; it does not automate a GUI click. The alternate case exercises `Undo::Journal`.

The configuration error count is scheduling-dependent. The exact recorded count is evidence of the recorded run, not an expected error rate. The parser panic is caught in the dev build; the runner does not assert that the exact bytes follow the same path in release.

The phone search fixture contains 10,000 cached JSONL entries. One warm-up precedes nine timed searches returning 50 results. The workspace libraries use the dev profile; no release GUI comparison is implied. The host was under active development with an Android emulator. See `environment.json` for the recorded environment and `probe-results.txt` for the final recorded probe run.

## Historical benchmark sources

GUI comparison numbers in the report come from `bench/RESULTS.md`, `bench/results.json`, and `bench/FLEA.md`. They were not rerun against this snapshot. Methods, thumbnail workload differences, cache state, and memory metric differences must remain attached to those values.
