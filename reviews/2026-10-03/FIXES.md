# Repair follow-up — 4 October 2026

This records the changes in the working tree following [the 58-finding review](REPORT.md). The historical report, snapshots and reproductions remain unchanged. The later [architecture review](../../design/docs/architecture-and-performance-2026-10-03.md) describes an intermediate snapshot; its overlapping correctness findings are covered below.

“Implemented” describes the code change. It does not mean every hardware, hostile-load or power-failure scenario has been reproduced. The workspace also contains concurrent EchoConnect development; this document does not attribute all working-tree changes to this repair.

## Finding map

| Finding | Implemented repair |
|---|---|
| F01 | Each packet is authorized against its connection certificate and generation. Replaced links exit; outgoing features require a paired link. |
| F02 | Outgoing payload TLS certificates must match the intended phone before bytes are sent. |
| F03 | Only an authenticated link may remotely remove pairing. |
| F04 | Pending approval contains certificate, generation, direction, creation time and displayed-code timestamp. Desktop and Android approve the displayed code; stale or changed requests cannot inherit approval. |
| F05 | Incoming files use private, exclusive temporary files and atomic no-replace publication with collision retries. |
| F06 | SSH trust questions are forwarded to the real authentication dialog. No English-label heuristic automatically accepts them. |
| F07 | Connection attempts are deduplicated and limited; control queues, offers and concurrent transfers are bounded. Identity/handshake deadlines and unpaired-session expiry limit slow peers. |
| F08 | Offers and payload jobs retain their certificate/session binding and recheck authorization during work and before publication. Forgetting removes offers and revokes links. |
| F09 | New identity ID/certificate/key are one atomic private bundle, creation is locked, legacy identities are validated, and malformed hexadecimal is rejected without UTF-8 slicing. Trust records publish atomically. |
| F10 | Messages, notifications, mounts, photos, replies and transfer state belong to a device. Events are routed to that state before handling. |
| F11 | Mount, photo, thumbnail, storage and picker results carry a session generation. Disconnect, unpair and disabling files invalidate it; obsolete mounts are unmounted. |
| F12 | Mounted-photo import markers come from successful, synced copy outcomes. Failed and skipped copies are not marked imported. |
| F13 | Import keys include device, source-relative path, size and modification time. Old ambiguous markers do not suppress new imports. |
| F14 | SFTP responses require enabled files and an outstanding request for that device. Disabling files invalidates mounted-file jobs. |
| F15 | Startup errors reach the UI; a failed attempt can be retried. Partial startup workers are stopped. |
| F16 | Merge undo uses individual committed-entry journals, preserving unrelated destination entries. Moved empty source directories are retained for metadata-preserving undo. |
| F17 | Copies are prepared before replacement; old destinations are retained in private recovery directories. Failed preparation leaves the destination intact. |
| F18 | Source/destination inode identity and resolved ancestry checks reject aliases and directory self-descendants. |
| F19 | Merge destinations are opened without following symlinks and pinned by directory descriptors during recursion. |
| F20 | Unsupported copied file types return errors; cross-filesystem moves keep their originals. |
| F21 | Cancellation reverses completed move journal entries. Failed reversal leaves the remaining journal available and reports recovery errors. |
| F22 | Planning and recursive copy errors propagate instead of silently dropping missing/unreadable entries. |
| F23 | Planned identities are rechecked, publication uses no-replace rename, and replaced entries are retained and checked before proceeding. |
| F24 | Sync failure is an explicit error. Undo is derived from actual committed actions, including cases where a source remains. |
| F25 | Cross-filesystem undo stages and syncs restoration, publishes without overwrite, syncs the restored location, then checks/removes its former source. |
| F26 | Copies preserve modes, timestamps, symlink timestamps, xattrs/ACLs and hard links. Preservation errors stop successful move cleanup. Ownership is not changed to another user. |
| F27 | Exit waits for active file work/undo to finish or cancel. Replacement backups and recovery notes survive process exit. |
| F28 | Action targets come from visible local search results or the visible listing. Special pages and virtual phone URIs do not expose hidden local targets. |
| F29 | Conflict, drive and network dialogs are queued rather than replacing the active dialog. |
| F30 | Undo jobs are serialized. Successfully reversed substeps are removed; failed steps remain retryable. |
| F31 | Settings use unique durable temporary files and one ordered UI writer with visible errors. Small state writes share a serial queue. |
| F32 | Index writers publish unique temporary files atomically; existing mappings retain their original inode. |
| F33 | Header lengths, padding, section sizes and offsets are checked before constructing mapped slices. |
| F34 | Explicit excluded paths also exclude a configured root or ancestor, using path components. |
| F35 | Live traversal uses the configured exclusion/cache-tag policy before entering subtrees. |
| F36 | GUI and CLI combine candidates using shared ranking before applying the global limit. |
| F37 | Missing or incompatible configured maps fall back to live search; failed rebuilds retain usable previous maps, and search errors clear stale results. |
| F38 | Index results carry settings generations. Publication and deletion are serialized and generation checked; old cleanup cannot remove a new build. Hidden changes and watcher overflow schedule rebuilding. |
| F39 | Folder filter/sort results carry an order revision. Metadata completion reapplies the current settings. |
| F40 | Clearing search advances its generation; failed/current searches remove previous results. |
| F41 | A wildcard query explicitly matches all eligible names. Empty queries remain empty. |
| F42 | Explicit CLI scopes search that scope live, avoiding incomplete or incorrectly selected overlapping maps. |
| F43 | Filesystem information, path classification, permission changes, launch checks and phone mount/import preparation run in workers. |
| F44 | Preview results carry generations; disabling/changing previews cancels old work and filesystem changes invalidate details. |
| F45 | Thumbnail helpers use unique private work directories and atomic cache-file publication. |
| F46 | Helpers have a deadline and cancellation; timed-out children are killed and reaped, freeing their slots. |
| F47 | Thumbnail commands parse quoting/escaping without a shell and expand field codes once. |
| F48 | A lifetime lock elects the socket owner. Runtime directories/socket ownership are checked; cleanup verifies the owned socket inode. |
| F49 | Forwarded requests use length-framed raw path bytes; URI decoding and CLI path arguments preserve non-UTF-8/newline filenames. |
| F50 | Trash recognition validates actual home/per-volume Trash locations rather than matching generic path shapes. |
| F51 | Trash directories are validated for ownership, permissions and symlinks, then pinned by descriptors for writes. |
| F52 | Desktop Exec paths quote and escape reserved characters and literal percent signs. |
| F53 | Date formatting is refreshed as time advances. |
| F54 | Installation refuses an existing unowned `ef`, including dangling symlinks, before building or overwriting it. |
| F55 | Uninstall removes owned/recognized legacy autostart launchers and preserves unrelated entries. |
| F56 | Device IDs are validated and abbreviated at character boundaries. |
| F57 | Incoming Bluetooth packets enforce the transport allowlist and reject payload offers. |
| F58 | Sensitive clipboard text is excluded from history, removed from matching old history and suppressed from reconnect replay. |

Primary implementation: [phone service](../../crates/phone/src/service.rs), [identity](../../crates/phone/src/identity.rs), [phone UI](../../crates/ui/src/phone.rs), [operations](../../crates/core/src/ops.rs), [UI actions](../../crates/ui/src/actions.rs), [index coordination](../../crates/ui/src/indexer.rs), [desktop integration](../../crates/ui/src/system.rs).

## Additional architecture findings

The newer report's U1–U5 correspond to the journal, settings, parser, device-state and import repairs above. U6 is addressed by resource budgets, revocation and service shutdown. Android service destruction now stops the Rust phone/SFTP services and ignores events from an old lifecycle.

Additional concrete fixes from that report:

- Transfer progress counts each leaf once for copies and merged moves (B9).
- Thumbnail completions carry the exact job token/mtime; stale completions cannot overwrite replacements. Duplicate eviction entries are removed, cached dimensions are bounded, and the cache budget is 32 MiB of raw RGBA pixels, excluding renderer overhead (B7).
- Phone search uses the shared filename matcher, keeps its full match count separate from its display limit, and participates in global ranking. Complete phone snapshots are published atomically by the concurrent EchoConnect work (B5).
- Selected-folder measurements enumerate set bits rather than scanning the entire display order on every update (B2).
- SFTP directory pages use a deque rather than shifting the remaining vector on each page (part of B10).

R04 now probes unfamiliar volumes read-only before deciding whether to mount writable. R05 moves GPU environment initialization ahead of thread startup. R06 adds desktop PR/push tests and a release test gate with Python explicitly installed. R07 limits phone image input/decode/cache resources.

Remaining architecture/performance work is not represented as a demonstrated fixed bug: a common cancellable task coordinator (B1/R01), shared immutable listing columns (B3), bounded top-k/count query APIs (B4), parsed phone-cache reuse (B5), incremental indexing and benchmarking (B6/R02), backend capability modeling (B8), durable sync batching measurements (B9), and stable Android directory sessions plus fully offloaded blocking SFTP I/O (B10). Desktop phone networking still has no master enable/disable setting (remaining part of R03).

## Recovery and validation limits

Replacement and merged-directory recovery folders are intentionally retained; their `RECOVERY.txt` describes the original raw pathname and preserved entry. Live Undo consumes the in-memory journal. After a process crash, recovery is manual from those retained entries; there is no automatic restart-recovery UI or automatic deletion policy. Do not delete a recovery folder until its previous data is no longer needed. This prioritizes retaining original bytes over reclaiming disk space.

Automated checks cover disposable file fixtures, localhost TLS peers, settings/index publication, search and UI helper boundaries. They do not establish power-loss recovery, full ENOSPC behavior, every ACL/filesystem combination, real multi-phone/Bluetooth/SFTP prompts, Windows hibernation, sustained hostile load, or Android battery/device behavior (R08). Android compilation is not device validation.

Regression sources: [file operations](../../crates/core/tests/review_regressions.rs), [phone security/lifecycle](../../crates/phone/tests/security.rs), [index and CLI](../../crates/index/tests/review_regressions.rs), plus unit regressions in config, pane, search, phone UI, desktop integration and thumbnails.

Validation commands:

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked
bash -n scripts/install.sh
cd android
./gradlew :app:compileDebugKotlin -x cargoNdk
```

Final checks passed: 94 Rust tests, zero failures; workspace Clippy completed successfully with warnings; installer shell syntax and disposable ownership checks passed; Android debug Kotlin compilation passed; `git diff --check` was clean. Logs: [Rust tests](repair-workspace-tests.txt), [Clippy](repair-clippy.txt), [Android compilation](repair-android-check.txt).

The Kotlin check regenerates the host UniFFI bindings; it skips the Android native cross-build and does not install an APK. Installer ownership behavior was separately checked with disposable XDG bin/data/config directories: a foreign `ef` and foreign autostart survived, while owned and recognized legacy autostart entries were removed.
