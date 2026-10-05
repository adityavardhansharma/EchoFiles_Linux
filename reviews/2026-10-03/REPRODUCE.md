**Reproducing the review**

The original probes demonstrate bugs in commit `e1a3de75413bfdf604727284d2fc64c6bc0178ad`. The phone and UI probes were repeated after `9cd5e07`; the additional Unicode identity case requires that newer revision. The final phone tests and workspace checks used an isolated archive of `9cd5e07` because another session continued changing the main workspace. They assert the observed bad outcomes. A probe passing means its reproduction worked, not that the application is safe. Change the assertions to require safe behavior before adopting them as regression tests.

The preserved `*_results.txt` files are the actual outputs from the review. Files ending in `_baseline.txt` preserve checks made before the phone update. The phone panic in `phone_results.txt` is the expected observed defect, not a crash of the runner. The reusable runner below was assembled afterward; its individual commands and probe sources were exercised during the review, but the combined runner was not run again.

For repeatable results, use a disposable checkout of `9cd5e07` and copy this review folder into it. From that checkout, run:

```bash
python reviews/2026-10-03/run_probes.py
```

Requirements: the repository's Rust toolchain/dependencies, `python`, `mkfifo`, extended-attribute support in the temporary filesystem, and `/dev/shm` on a different filesystem from `/tmp`. The runner assumes Cargo's normal `target/debug/deps` location. It runs the original tests first so the standalone probes can link to current libraries.

Only generated disposable data is used for filesystem cases. The runner gives core cases a private `XDG_DATA_HOME`, keeping them away from the user's Trash. The phone probes make their active connections to localhost with newly generated identities; the normal service implementation still binds network listeners and broadcasts discovery. The phone and UI probe sources are temporarily placed in Cargo test directories, created exclusively, then removed in `finally`. Phone/UI fixtures identify themselves in stdout and are left under `/tmp` for inspection. No production source patch is needed.

The concurrent-settings reproduction is a stress check. Its precise error count depends on scheduling; the recorded run saw 184 failed saves out of 400. A later run with fewer or no errors does not establish that the shared temporary-file race has been fixed.

**CLI cases**

Use a private configuration/cache and a disposable tree with two roots: a parent containing `other.txt`, and its `skip` child containing `report.txt`. Configure `exclude_names = ["skip"]` and roots containing both the parent and child. Then run the current debug `ef` binary with that private `XDG_CONFIG_HOME` and `XDG_CACHE_HOME`:

```text
ef index
ef find '*' --count
ef find report --in <absolute child path> --count
ef find report --count
```

The recorded outputs were 0, 0 and 1 for the three searches. The wildcard should not give zero for a nonempty matching scope; the scoped search should find the file in the separately indexed child. Full fixture paths and output are in `cli_results.txt`.

No full disk, power-loss, real Android, real SFTP interception, hostile removable-volume or Windows-mount tests were performed. The report marks these conditions as code findings or remaining validation work.
