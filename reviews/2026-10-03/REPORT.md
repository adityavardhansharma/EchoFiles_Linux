**EchoFiles deep architecture and correctness review — 3 October 2026**

**Verdict:** keep the foundation, but fix the security and data-safety findings before a dependable release. The checks reproduced unauthorized phone access and file loss. This is a review of the current application, not a promise that every possible bug has been found.

Full review baseline: `e1a3de75413bfdf604727284d2fc64c6bc0178ad`. During the review another session committed `9cd5e07`; its phone changes were reviewed separately, the phone/UI probes were repeated, and F56–F58 document additional findings. Subsequent uncommitted EchoConnect work in Cargo files, crates/connect and phone exports is outside this pinned review. Historical source links for changed files use preserved snapshots so their line references remain meaningful. No production source was changed by this review; temporary tests were removed.

**Inventory:** 58 findings (23 P1, 34 P2, 1 P3) and 8 additional architecture/validation risks. Related symptoms are grouped where they share the same cause. P1 means address before relying on the affected feature for important data; P2 means a meaningful correctness, privacy or reliability defect; P3 means a smaller usability defect. These are practical priorities, not CVSS scores.

**Evidence:** “Reproduced” means the stated behavior was exercised with disposable local fixtures. Where only a component was exercised, that is stated. “Code finding” means the control flow and missing safeguard were inspected, but the complete device/failure scenario was not reproduced. Risks below are explicitly not presented as demonstrated runtime failures.

**Validation:** `cargo test --workspace --locked` passed all 60 existing tests at the baseline. An isolated checkout of 9cd5e07 passed all 61 existing tests; the repeated phone probes also confirmed the four original security failures and the new Unicode-ID panic. The separate probes cover file operations, cross-filesystem moves, cache parsing, search, concurrent settings saves, phone TLS and UI action targets. Phone test connections were made only to localhost using disposable identities; test services use the app’s normal listener binding. No real phone, user document, mounted Windows volume or real pairing was intentionally modified.

`cargo clippy --workspace --all-targets --locked` completed with five distinct warnings at both reviewed revisions. The strict `-D warnings` run failed on a style warning in `fmt.rs`; these warnings are not the source of the serious bugs. Installer shell syntax and release/design Python syntax checks passed.

**Architecture in plain language:** the desktop UI coordinates nine crates. Core handles local files and operations; index handles search; config stores preferences; disks, vfs and net connect to Linux storage and network services; phone handles device communication; theme handles appearance. The boundaries are sensible, but too much responsibility for safe sequencing still lives in the UI.

**The good:** the crate split, two-phase listings, visible-row rendering, bounded normal thumbnail concurrency, indexed search layout, Linux service integration and generation checks for folder loading are useful foundations. The problem is inconsistent safety at the boundaries: job ownership, filesystem identity, peer authentication, persistence and undo.

**The bad:** too many independently started jobs update shared UI state. Some subsystems have careful generation checks or locks, while adjacent ones bypass them. A single operation coordinator, a single ordered settings writer, per-device phone state and a detailed file-operation journal would remove whole classes of these failures.

**The ugly:** the confirmed unauthorized phone access and file-loss cases below. The existing green test suite does not make these safe.

**Phone security and privacy**

**F01 · P1 · An old untrusted connection can act as the paired phone**

Evidence: Reproduced. A connection is authorized using the current global record for its claimed device ID, rather than its own certificate and connection generation. Replacing a link does not close the old one: its thread retains a sender to its own queue. In the localhost probe, an unpaired impostor connected first, the real trusted phone connected second, and the impostor then delivered an accepted clipboard event. Other paired-only packet handling uses the same check. In 9cd5e07, pairing is true if any transport for that ID is paired, while sending prefers LAN without requiring that chosen link to be paired. A trusted Bluetooth link and an untrusted LAN link sharing an ID can therefore also misroute outbound traffic; this transport combination was inspected, not reproduced.

Repair: Bind authorization to the individual authenticated connection; reject stale generations, close replaced links, and check that binding on every packet.

Source: [crates/phone/src/service.rs:582 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:582), [crates/phone/src/service.rs:693 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:693).

**F02 · P1 · An unrelated device can receive a file meant for your phone**

Evidence: Reproduced. The outgoing file listener accepts the first connection, completes TLS with the permissive certificate verifier, and sends file bytes without checking the paired phone certificate. The probe received the full dummy secret using a different certificate. This requires network reachability and winning the connection race; it does not require the paired phone key.

Repair: Compare the payload peer certificate with the intended paired device before opening or sending the source file.

Source: [crates/phone/src/service.rs:917 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:917), [crates/phone/src/service.rs:933 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:933), [crates/phone/src/tls.rs:49](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/phone/src/tls.rs:49).

**F03 · P1 · An unpaired impostor can erase a real pairing**

Evidence: Reproduced. A connection claiming an existing phone ID can send pair=false. The handler deletes the stored trust record merely because that ID is trusted, even though the connection has a different certificate. The localhost probe removed the real pairing this way.

Repair: Accept remote unpair requests only from the authenticated connection associated with that trust record.

Source: [crates/phone/src/service.rs:724 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:724).

**F04 · P1 · Pairing approval is attached to a device name/ID, not the connection being approved**

Evidence: Code finding. Pending pairing stores only ID, timestamp and which side asked. accept_pair and complete_pair trust whichever link currently occupies that ID. A link replacement between showing the code and accepting can change the certificate being trusted. Pending requests also have no expiry check; an old request can remain actionable. The update also marks every transport for an ID paired after trusting only the first link certificate, without checking that the other certificates match.

Repair: Record the exact peer certificate, connection generation, direction and expiry with the displayed pairing request, and verify them again at acceptance.

Source: [crates/phone/src/service.rs:198 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:198), [crates/phone/src/service.rs:357 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:357), [crates/phone/src/service.rs:669 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:669).

**F05 · P1 · Receiving a file can overwrite unrelated data**

Evidence: Reproduced; concurrency consequence inspected. The receiver uses a predictable .<name>.part file opened with File::create, which follows symlinks and truncates existing files. The probe placed a .part symlink and demonstrated an unrelated file being overwritten. Concurrent downloads with the same name can also share that staging file. Final rename can overwrite a destination created after the earlier free-name check, contradicting the never-overwrite promise.

Repair: Create a unique staging file exclusively without following links, keep ownership of it, then publish with an atomic no-replace operation and retry name collisions.

Source: [crates/phone/src/service.rs:828 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:828), [crates/phone/src/service.rs:859 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:859), [crates/phone/src/service.rs:893 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:893).

**F06 · P1 · Phone file browsing automatically accepts an unverified SFTP server**

Evidence: Code finding. mount_phone answers server questions by selecting the first choice whose English text does not contain cancel. An authenticated KDE Connect connection does not authenticate the separate SSH connection. There is no comparison with an expected SSH host key. The choice-by-English-label logic is also unreliable under translated prompts.

Repair: Verify the SFTP host key through an authenticated binding where available; otherwise present a real trust decision. Do not automatically approve arbitrary questions.

Source: [crates/ui/src/phone.rs:782 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:782).

**F07 · P1 · Network peers can create unbounded work**

Evidence: Code finding; load attack not run. Every incoming TCP connection and accepted UDP discovery announcement can spawn a thread. Pending outgoing connections are not deduplicated, control queues and offers are unbounded, and an unpaired established link has no overall lifetime deadline. Per-read timeouts do not stop a peer that keeps sending slowly. These resources belong to the file manager process itself.

Repair: Limit connections, pending discoveries, queues, offers and transfer concurrency; enforce complete-handshake/identity deadlines, pairing expiry and backpressure.

Source: [crates/phone/src/service.rs:293 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:293), [crates/phone/src/service.rs:493 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:493), [crates/phone/src/service.rs:568 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:568), [crates/phone/src/service.rs:818 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:818).

**F08 · P2 · Forgetting a phone does not revoke all outstanding transfers**

Evidence: Code finding. forget marks a link unpaired but leaves offers and queued transfer work. accept_file does not recheck pairing, and download compares against the current link certificate without requiring that link to remain paired. send_files checks pairing once before starting a batch; upload later uses send_raw. Work queued before unpair can therefore proceed afterward.

Repair: Tie offers and queued jobs to a pairing/session token; cancel them and recheck authorization when trust is revoked.

Source: [crates/phone/src/service.rs:460 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:460), [crates/phone/src/service.rs:472 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:472), [crates/phone/src/service.rs:684 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:684), [crates/phone/src/service.rs:853 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:853).

**F09 · P2 · Phone identity files are not safely created or recovered**

Evidence: Code finding. write_private writes bytes before changing permissions to 0600, so a newly created key can briefly have ordinary umask-derived permissions. Identity and trust records are written in place, without an atomic publication step. A partial or mixed identity can break service startup. The hex parser also slices UTF-8 strings by byte offsets and can panic on a malformed non-ASCII certificate value.

Repair: Create private files with 0600 from the outset, publish durable records atomically, validate a complete identity before use, and decode hexadecimal as bytes with a fallible parser.

Source: [crates/phone/src/identity.rs:37](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/phone/src/identity.rs:37), [crates/phone/src/identity.rs:53](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/phone/src/identity.rs:53), [crates/phone/src/identity.rs:82](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/phone/src/identity.rs:82).

**Phone behavior and imported photos**

**F10 · P1 · Different phones share messages, notifications and storage state**

Evidence: Code finding. The UI stores one notification list, one SMS thread map, one mount and one photo collection. SMS and SFTP handlers discard the originating device ID; notification keys are compared without a device ID. Selecting another phone changes current without clearing or switching those collections. Reply and send actions use the currently selected phone. This can show one phone’s data under another and route a reply through the wrong device.

Repair: Keep a separate state object per device and carry the device ID through every view, event, reply and file operation.

Source: [crates/ui/src/phone.rs:112 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:112), [crates/ui/src/phone.rs:911 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:911), [crates/ui/src/phone.rs:1167 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:1167), [crates/ui/src/phone.rs:1422 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:1422).

**F11 · P2 · Late phone jobs can reopen or repopulate a phone that was disconnected**

Evidence: Code finding. Mounted, PhotosLoaded, Space, Screen and PhotoThumb results have no device/session generation. A mount finishing after disconnect, unpair or disabling files is accepted and installed into current state. A photo scan or thumbnail from an earlier session can also repopulate that state.

Repair: Attach device and generation to each job, cancel obsolete jobs and reject late results. Clean up mounts that finish after cancellation.

Source: [crates/ui/src/phone.rs:738 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:738), [crates/ui/src/phone.rs:817 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:817), [crates/ui/src/phone.rs:1021 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:1021).

**F12 · P1 · Photos are marked imported before the copy succeeds**

Evidence: Code finding. Import immediately inserts every photo into phone.imported and saves that list, then starts the copy jobs. Cancelling, running out of space or losing the phone still leaves those photos marked imported. Import new photos will skip them even though no successful local copy was confirmed.

Repair: Mark each photo imported only after its destination has been successfully written, and preserve unsuccessful items for retry.

Source: [crates/ui/src/phone.rs:1124 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:1124).

**F13 · P2 · Different photos can share the same imported marker**

Evidence: Code finding. The import key is filename, size and modification time. It contains neither device identity nor the source folder. Two different phones or folders can legitimately produce the same key, so one photo can be skipped because another was imported.

Repair: Include stable device identity and source-relative path; use stronger identity/content checks where deduplication is intended.

Source: [crates/ui/src/phone.rs:558 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:558).

**F14 · P2 · Turning off phone files does not block incoming SFTP setup**

Evidence: Code finding. The files switch blocks initiating phone_files, but the incoming SFTP event handler mounts any successful offer without checking the switch or a matching outstanding request. Late Mounted results are also accepted. Files can be mounted again after the user turned this feature off.

Repair: Enforce the feature setting and request/session identity at the event boundary as well as the initiating action.

Source: [crates/ui/src/phone.rs:1239 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:1239), [crates/ui/src/phone.rs:1472 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:1472).

**F15 · P2 · Some phone startup failures disappear without an explanation**

Evidence: Code finding. The UI start wrapper assumes every Service::start error already emitted Failed. Identity loading, TLS creation and thread creation can return errors before that event is emitted. The wrapper discards those errors, leaving the feature unstarted without a useful error or retry mechanism.

Repair: Return all startup failures to the UI and support a clean retry with proper service shutdown.

Source: [crates/ui/src/phone.rs:242 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:242), [crates/phone/src/service.rs:261 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:261).

**File operations and recovery**

**F16 · P1 · Undoing a folder merge also removes or moves pre-existing files**

Evidence: Reproduced. Outcome records only a top-level source/destination pair. The UI turns a merged copy into Undo::Copy(destination), which trashes the whole existing folder. A merged move becomes Undo::Move, which moves that whole destination back. Both effects were reproduced with an existing destination file unrelated to the transfer.

Repair: Journal the individual changes and preserved replacements. Undo must affect only entries created or moved by that operation.

Source: [crates/core/src/ops.rs:492](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:492), [crates/core/src/ops.rs:754](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:754), [crates/core/src/ops.rs:776](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:776), [crates/ui/src/actions.rs:660](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:660).

**F17 · P1 · A failed replacement deletes the old destination first**

Evidence: Reproduced. Replace permanently removes the existing destination before the source is successfully copied or moved. The probe removed the source after planning; execution returned an error and the old destination had already disappeared. Read errors, disk-full errors and cancelled copies can reach the same unsafe ordering.

Repair: Stage the replacement successfully before replacing the old entry, and preserve the old version when promising undo.

Source: [crates/core/src/ops.rs:461](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:461), [crates/core/src/ops.rs:544](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:544).

**F18 · P1 · Alternate paths can defeat the checks against copying a file onto itself**

Evidence: Reproduced. Self-copy and descendant checks compare path strings. A symlinked destination can refer to the same source directory under another name. In the probe, copying x through such an alias and choosing Replace deleted the source itself. A destination alias inside the source directory also passed the planner’s copy-into-self check.

Repair: Compare actual filesystem identities and resolved ancestry before planning destructive work, with descriptor-based checks again during execution.

Source: [crates/core/src/ops.rs:361](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:361), [crates/core/src/ops.rs:452](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:452).

**F19 · P1 · A folder merge can write through destination links into another folder**

Evidence: Reproduced. The destination directory check follows symlinks, and recursive merge accepts an existing path as a directory without rejecting links. The probe merged into a destination link and overwrote a file in a separate folder outside the intended destination tree.

Repair: Handle a destination symlink as a distinct conflict rather than silently traversing it. Keep recursive writes beneath an opened destination directory.

Source: [crates/core/src/ops.rs:462](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:462), [crates/core/src/ops.rs:582](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:582).

**F20 · P1 · Moving a special file between filesystems can delete it without creating a copy**

Evidence: Reproduced. copy_tree silently skips FIFOs, sockets and devices and still returns success. A cross-filesystem move then deletes its source after sync. A named pipe moved from /tmp to /dev/shm vanished from both locations while Outcome reported a successful item. Directories containing skipped entries have the same underlying problem.

Repair: Reject unsupported types explicitly or preserve them correctly; never delete a source unless every required entry was transferred.

Source: [crates/core/src/ops.rs:479](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:479), [crates/core/src/ops.rs:521](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:521), [crates/core/src/ops.rs:607](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:607).

**F21 · P1 · Cancelling a move cannot reliably put everything back**

Evidence: Code finding. A partially merged folder can move children before returning Interrupted, but those children are not recorded in Outcome.done. Completed merged folders are rolled back as whole folders, bringing pre-existing destination data with them. Rollback errors are ignored. The UI nevertheless says everything is back where it was.

Repair: Record reversible actions per entry, report incomplete rollback honestly, and retain a recovery journal until recovery is complete.

Source: [crates/core/src/ops.rs:491](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:491), [crates/core/src/ops.rs:504](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:504), [crates/core/src/ops.rs:533](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:533), [crates/ui/src/actions.rs:666](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:666).

**F22 · P1 · Some missing or unreadable inputs are silently treated as success**

Evidence: Code finding. plan silently skips sources whose metadata cannot be read. tally drops traversal errors. More seriously, copy_tree and move_merge flatten read_dir results, discarding individual directory-entry errors. A cross-filesystem move may then remove a source tree after a supposedly successful but incomplete traversal. Cancellation of a copy also leaves completed items without registering their undo because the UI rejects all cancelled outcomes.

Repair: Propagate traversal errors, distinguish skipped from fully transferred entries, and journal successfully completed work even when the rest fails or is cancelled.

Source: [crates/core/src/ops.rs:315](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:315), [crates/core/src/ops.rs:360](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:360), [crates/core/src/ops.rs:534](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:534), [crates/core/src/ops.rs:590](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:590), [crates/ui/src/actions.rs:660](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:660).

**F23 · P1 · Conflict decisions are vulnerable to files changing between check and write**

Evidence: Code finding. Move execution checks whether a destination exists and then calls ordinary fs::rename, which can overwrite a file another operation creates in between. Keep both only searches for a free name; it does not reserve it. The fallback implementation of rename_noreplace also checks and then performs an ordinary rename. These are independent of the explicit Replace bug.

Repair: Use atomic no-replace operations or exclusive creation wherever preserving an existing destination is required, and re-prompt if the state has changed.

Source: [crates/core/src/ops.rs:135](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:135), [crates/core/src/ops.rs:452](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:452), [crates/core/src/ops.rs:474](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:474).

**F24 · P1 · A failed disk sync can be presented as a successful move**

Evidence: Code finding. execute records sync failure only as synced=false and retains cross-filesystem sources. It adds no error. The UI considers an error-free, non-cancelled outcome successful and adds Move undo. That undo can later fail against the still-existing source, while the transfer has already disappeared as successful.

Repair: Make durability failure an explicit outcome/error, report that originals remain, and build undo from actual completed actions.

Source: [crates/core/src/ops.rs:516](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:516), [crates/ui/src/actions.rs:658](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:658).

**F25 · P1 · Undoing a cross-filesystem move deletes its current copy before syncing the restored copy**

Evidence: Code finding. The normal move path tries to sync the destination before deleting originals. Undo::Move’s EXDEV path copies back and immediately removes the current copy without any equivalent sync. A crash or power loss during that gap can lose the only durable copy.

Repair: Run undo through the same durable transfer engine and only delete the current copy once the restoration is safely committed.

Source: [crates/core/src/ops.rs:759](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:759).

**F26 · P2 · Copies and cross-filesystem moves lose extra metadata**

Evidence: Reproduced for extended attributes; other metadata inspected. The copy implementation preserves ordinary mode bits and timestamps, but not extended attributes, ACLs or hard-link relationships. The probe copied a file with user.audit and found no extended attributes at the destination. On a move, deletion of the original makes such metadata loss permanent. Metadata-setting failures are also silently ignored.

Repair: Define and implement a metadata-preservation policy, especially for moves, and report unsupported or failed preservation.

Source: [crates/core/src/ops.rs:598](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:598), [crates/core/src/ops.rs:621](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:621), [crates/core/src/ops.rs:670](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:670).

**F27 · P1 · Closing the app can terminate active file operations immediately**

Evidence: Code finding. With the default background=false, closing the window exits without consulting active transfers. Ctrl+Q exits directly as well. Workers have no persistent recovery journal. A transfer can be interrupted during a merge or after the old destination has been removed, bypassing even the existing cancellation cleanup.

Repair: Keep the app alive until operations finish or cancel safely; present active work clearly and make interrupted work recoverable.

Source: [crates/ui/src/app.rs:1145](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:1145), [crates/ui/src/app.rs:1356](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:1356).

**F28 · P1 · Delete and other shortcuts can act on files hidden behind the current screen**

Evidence: Reproduced at the target-selection layer; keyboard route inspected. Pane::targets always reads the underlying folder listing. Phone/drive/share pages keep that listing and its selection; Everywhere search uses a separate result cursor that targets ignores. The probe showed important.txt still being targeted on a phone page and even with empty Everywhere results. The global Delete/Copy/Cut/Properties routes use this same target selection without a special-page or search-result guard.

Repair: Resolve action targets from the actual visible screen and selection, and disable file actions on screens without file targets.

Source: [crates/ui/src/pane.rs:342](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/pane.rs:342), [crates/ui/src/phone.rs:699 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:699), [crates/ui/src/actions.rs:349](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:349), [crates/ui/src/app.rs:1435](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:1435).

**F29 · P2 · Concurrent operations can overwrite the only conflict or authentication dialog**

Evidence: Code finding. Each planning result can assign self.dialog without queuing or cancelling the previous dialog’s operation. Two transfers that both need a decision can leave one transfer waiting forever after its dialog disappears. Network and drive prompts use the same slot and can displace transfer decisions too.

Repair: Queue decision requests with operation IDs and explicit ownership; replacing a dialog must either preserve or cancel its pending operation.

Source: [crates/ui/src/actions.rs:333](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:333), [crates/ui/src/network.rs:435](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/network.rs:435), [crates/ui/src/drives.rs:225](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/drives.rs:225).

**F30 · P2 · Failed undo entries are lost, and several undo jobs can run out of order**

Evidence: Code finding. Undo pops its entry before attempting recovery. A failure or partial failure does not put the remaining work back. Repeated Ctrl+Z starts independent background jobs, so dependent operations can undo in the wrong order: for example, undoing a rename and then undoing the copy it renamed.

Repair: Serialize undo operations and retain the uncompleted journal entry after errors.

Source: [crates/ui/src/actions.rs:587](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:587), [crates/core/src/ops.rs:747](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:747).

**Settings, indexing and search**

**F31 · P2 · Settings and small state files have competing writers**

Evidence: Reproduced for concurrent settings writes. The Settings screen has a save lock, but persist_settings bypasses it and spawns uncoordinated writes to the same settings.tmp. Phone switches and sidebar changes use that path. A stress probe produced 184 save errors in 400 writes; callers on this path discard errors. Last-folder, phone-seen, phone-imported and recent-server snapshots also use independent in-place writes, allowing older snapshots to land last.

Repair: Use one ordered persistence service for all settings saves, unique temporary files and atomic publication for state snapshots, and visible save errors.

Source: [crates/ui/src/settings.rs:158](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/settings.rs:158), [crates/ui/src/app.rs:1168](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:1168), [crates/config/src/lib.rs:355](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/config/src/lib.rs:355), [crates/ui/src/phone.rs:324 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:324), [crates/ui/src/network.rs:294](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/network.rs:294).

**F32 · P2 · The app and CLI can write the same index temporary file at once**

Evidence: Code finding. Index::save always uses path.with_extension("tmp"). Both the background indexer and ef index can write that file without a shared lock. One writer can publish the inode while another still writes to it, violating the read-only mmap assumption, or one can remove the other’s temporary pathname. Index::save also does not fsync the file or parent directory.

Repair: Give each writer its own exclusively created temporary file and coordinate publication. Never modify a published mapped inode; validate and reopen a completed snapshot.

Source: [crates/index/src/store.rs:45](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/store.rs:45), [crates/index/src/store.rs:60](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/store.rs:60), [crates/ui/src/indexer.rs:80](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/indexer.rs:80), [crates/index/src/bin/ef.rs:171](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/bin/ef.rs:171).

**F33 · P2 · Malformed index headers can panic instead of being rejected**

Evidence: Reproduced in the debug build. Header sizes are partly checked, but padding and additions such as HEADER + pad4(...) happen before complete overflow validation. A saved file with an oversized root length caused a panic in the probe. Release uses panic=abort; exact optimized malformed-file behavior was not separately exercised. Corrupt-cache recovery must not depend on arithmetic wrapping or panics.

Repair: Use checked arithmetic for every offset and padding calculation and validate all ranges before slicing; add malformed-file/property tests.

Source: [crates/index/src/store.rs:36](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/store.rs:36), [crates/index/src/store.rs:130](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/store.rs:130), [Cargo.toml:34 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/Cargo.toml:34).

**F34 · P2 · A folder listed under Never show in search can still be indexed**

Evidence: Reproduced. exclude_paths_in explicitly drops an exclusion equal to the current root and ignores ancestors of that root. If an excluded folder is also a configured root, or a configured root lies below an excluded ancestor, it can be crawled normally. The probe configured the same folder as root and exclusion and still found its secret filename.

Repair: Apply exclusions to configured roots themselves and their ancestors, with a clear rule that the exclusion wins.

Source: [crates/config/src/lib.rs:151](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/config/src/lib.rs:151), [crates/index/src/lib.rs:176](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/lib.rs:176).

**F35 · P2 · Turning the index off changes which cache files are excluded**

Evidence: Reproduced. The live UI search walks everything and filters names/paths afterward, but never applies skip_cache_folders/CACHEDIR.TAG. A valid tagged cache produced zero indexed matches and one live match in the probe. Excluded trees are also still read before the post-filter, wasting I/O.

Repair: Pass the same exclusion options into the live walker and prune excluded subtrees before traversal.

Source: [crates/ui/src/search.rs:97](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/search.rs:97), [crates/ui/src/search.rs:120](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/search.rs:120), [crates/index/src/live.rs:61](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/live.rs:61).

**F36 · P2 · Search can hide an exact match behind weaker matches from another location**

Evidence: Reproduced. UI search fills the 300-result limit from each configured root in order; it does not rank the combined results. With 300 weak matches in the first root and an exact match in the second, the exact match was omitted although total=301. The CLI similarly spends --limit on earlier roots first.

Repair: Merge ranked candidates across all roots before applying the overall limit.

Source: [crates/ui/src/search.rs:61](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/search.rs:61), [crates/index/src/bin/ef.rs:91](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/bin/ef.rs:91).

**F37 · P2 · A partly available index silently leaves search locations out**

Evidence: Code finding. If any root has a mapped index, the UI uses indexed search and ignores roots without one instead of searching them live. A failed rebuild replaces the previous RootIndex map with None, dropping an otherwise usable previous snapshot. A problem is shown in Settings, but the search count itself does not explain that it covers only part of the requested locations.

Repair: Keep the last good snapshot, combine indexed and live results when needed, and label incomplete results visibly.

Source: [crates/ui/src/app.rs:529](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:529), [crates/ui/src/search.rs:56](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/search.rs:56), [crates/ui/src/indexer.rs:87](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/indexer.rs:87), [crates/ui/src/app.rs:1089](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:1089).

**F38 · P2 · Old index jobs can publish results for settings that are no longer current**

Evidence: Code finding. IndexOpened and IndexBuilt contain roots but no settings generation. Changing roots/exclusions or toggling indexing during a build lets an old job replace the current root list while the current index flag is true. The disable cleanup handles one off-state case but not off/on or arbitrary configuration changes.

Repair: Version index configuration, reject obsolete completion messages and schedule the latest requested build once the old one stops.

Source: [crates/ui/src/app.rs:553](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:553), [crates/ui/src/app.rs:572](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:572), [crates/ui/src/app.rs:1076](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:1076).

**F39 · P2 · Older folder filters or sort jobs can overwrite newer ones**

Evidence: Code finding. reorder reuses the folder-load generation for every query and sort request. Two background reorders for the same folder are therefore indistinguishable. A slower old query can finish last and replace the results of the newer query; late metadata loading also brings the sort captured when loading started.

Repair: Track a separate ordering/query revision and recompute against the current sort/filter when metadata arrives.

Source: [crates/ui/src/pane.rs:168](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/pane.rs:168), [crates/ui/src/pane.rs:289](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/pane.rs:289), [crates/ui/src/app.rs:691](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:691).

**F40 · P2 · Clearing or failing a search can leave stale results attached to the pane**

Evidence: Reproduced for unchanged generation; resulting stale handling inspected. clear_search cancels the live token but does not invalidate search_generation. Indexed search ignores that token, so a late indexed result remains acceptable after navigation or clearing. SearchDone(None) also leaves the previous results intact, so a failed live query can continue showing results for an older query.

Repair: Invalidate the generation on every scope/navigation/clear change and represent failures explicitly rather than retaining unrelated results.

Source: [crates/ui/src/pane.rs:392](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/pane.rs:392), [crates/ui/src/app.rs:916](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:916).

**F41 · P2 · The CLI wildcard query returns no files unless another filter supplies a search term**

Evidence: Reproduced. ef find "*" converts the query to an empty string, while Matcher::new rejects a query with no terms or extension. The probe returned count=0 for an indexed folder containing files. This also affects a broad --dirs or --files query using "*".

Repair: Represent match-all explicitly and test it with --count, --dirs, --files and --limit.

Source: [crates/index/src/bin/ef.rs:102](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/bin/ef.rs:102), [crates/index/src/matcher.rs:46](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/matcher.rs:46).

**F42 · P2 · A scoped CLI search can choose the wrong overlapping index**

Evidence: Reproduced. With a parent root that skips a folder and a second root that indexes that folder, --in can find the folder entry in the parent index and stop there, even though the parent contains none of its children. The probe found report with an unscoped query but returned zero when explicitly scoped to its folder.

Repair: Choose the most specific suitable root and fall back to another complete index or a live walk when the selected subtree is excluded/incomplete.

Source: [crates/index/src/bin/ef.rs:91](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/bin/ef.rs:91), [crates/index/src/bin/ef.rs:139](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/index/src/bin/ef.rs:139).

**UI responsiveness, previews and desktop integration**

**F43 · P2 · Slow filesystem calls still run on the UI thread**

Evidence: Code finding; stalled mounts not simulated. Navigation calls statfs synchronously. Several actions call metadata/is_dir, file attribute writes or chmod synchronously, and drive results collect statfs for every mount on the UI thread. A stalled FUSE/network mount can freeze the entire interface despite background directory listing.

Repair: Keep filesystem and subprocess waits out of update/view paths, including small-looking metadata queries; return results with cancellation and generation checks.

Source: [crates/ui/src/pane.rs:130](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/pane.rs:130), [crates/ui/src/pane.rs:173](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/pane.rs:173), [crates/ui/src/actions.rs:292](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:292), [crates/ui/src/actions.rs:735](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:735), [crates/ui/src/drives.rs:131](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/drives.rs:131).

**F44 · P2 · Preview details can stay stale and obsolete folder measurements keep running**

Evidence: Code finding. Preview freshness is keyed only by path, so editing or replacing the selected file in place does not refresh its displayed facts. gather starts a recursive directory-size worker before delivering its result. If that result is discarded because selection changed, Message::Preview does not cancel that worker. Disabling the preview also returns without cancelling an existing measurement.

Repair: Version preview requests with file freshness and cancel both accepted and discarded work whenever selection, file contents or visibility changes.

Source: [crates/ui/src/app.rs:1020](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:1020), [crates/ui/src/app.rs:1587](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:1587), [crates/ui/src/preview.rs:141](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/preview.rs:141).

**F45 · P2 · Thumbnail jobs can overwrite each other’s temporary images**

Evidence: Code finding. External thumbnail output uses process ID plus only the first byte of an MD5 digest. There are just 256 output names per process, with two workers allowed concurrently. Different files sharing that byte can write/read/remove the same temporary file, producing a wrong or missing thumbnail. The public /tmp pathname is also predictable.

Repair: Use securely created unique temporary files/directories for every job.

Source: [crates/ui/src/thumbs.rs:254](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/thumbs.rs:254).

**F46 · P2 · A hung thumbnail helper can permanently consume the thumbnail workers**

Evidence: Code finding. run_thumbnailer waits for the external program with status() and no deadline or cancellation. Two hung helpers occupy the entire two-job thumbnail allowance. Navigating away does not stop them, so thumbnails can remain stuck for the rest of the session.

Repair: Give helper processes a deadline and cancellation path, kill/reap them when obsolete, and always release their worker slot.

Source: [crates/ui/src/thumbs.rs:18](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/thumbs.rs:18), [crates/ui/src/thumbs.rs:263](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/thumbs.rs:263).

**F47 · P2 · Some valid thumbnail helper commands are parsed incorrectly**

Evidence: Code finding. Exec lines from .thumbnailer files are split on whitespace rather than parsed with desktop-entry quoting rules. Quoted executable paths, quoted arguments and placeholders inside quotes are passed incorrectly, causing supported previews to fail and potentially be recorded as failed in the cache.

Repair: Use a proper desktop Exec parser and substitute field codes after tokenization.

Source: [crates/ui/src/thumbs.rs:257](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/thumbs.rs:257).

**F48 · P2 · Two launches can both believe they own the application socket**

Evidence: Code finding. Each process first tries connecting, then listen unconditionally removes the socket pathname and binds. Two simultaneous launches can both miss the first check and one can unlink the other’s live socket. The exiting instance can also unlink a socket now owned by another instance. Without XDG_RUNTIME_DIR, all users fall back to the same /tmp/echofiles.sock pathname.

Repair: Use an atomic per-user instance lock, validate runtime-directory ownership, and unlink only the socket inode owned by this process.

Source: [crates/ui/src/system.rs:49](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/system.rs:49), [crates/ui/src/system.rs:67](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/system.rs:67), [crates/ui/src/main.rs:57](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/main.rs:57), [crates/config/src/lib.rs:317](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/config/src/lib.rs:317).

**F49 · P2 · Some legal Linux filenames are corrupted when passed between app launches**

Evidence: Code finding. The instance protocol sends paths through display() in a newline-delimited UTF-8 string. Non-UTF-8 filenames are lossy, and a filename containing a newline is truncated by read_line. The file:// launch path also uses a lossy percent decoder; last_folder writes raw bytes but reads only valid UTF-8. Normal core listing/indexing already supports raw filenames, so these boundaries break that support.

Repair: Use a length-delimited protocol carrying raw path bytes, and keep URI decoding separate from text display.

Source: [crates/ui/src/system.rs:32](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/system.rs:32), [crates/ui/src/system.rs:49](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/system.rs:49), [crates/ui/src/system.rs:77](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/system.rs:77), [crates/ui/src/system.rs:254](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/system.rs:254), [crates/net/src/address.rs:252](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/net/src/address.rs:252).

**F50 · P2 · The Trash detector can mistake an ordinary folder for the Trash**

Evidence: Code finding. is_trash_files accepts any directory called files beneath Trash, .Trash-* or any numeric parent. An ordinary project folder such as /work/123/files is therefore classified as Trash. Pressing Delete there offers permanent deletion instead of ordinary trashing. The existing confirmation still applies; this is not silent permanent deletion.

Repair: Identify actual trash locations using validated mount and user trash roots, not just the final two path components.

Source: [crates/core/src/trash.rs:204](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/trash.rs:204), [crates/ui/src/actions.rs:520](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/actions.rs:520).

**F51 · P1 · Trash directories on another volume are trusted without validating their ownership or links**

Evidence: Code finding; hostile removable volume not tested. The code checks that the shared .Trash is a sticky real directory, but does not similarly validate the user subdirectory or fallback .Trash-UID, files and info directories. create_dir_all accepts existing paths and can traverse directory symlinks. On a shared or adversarial volume, pre-created directories can redirect where trashed files are stored or expose them to another user.

Repair: Validate ownership, type and permissions for every trash directory and use directory-relative operations that refuse symlink traversal.

Source: [crates/core/src/trash.rs:64](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/trash.rs:64), [crates/core/src/trash.rs:134](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/trash.rs:134).

**F52 · P2 · Starting at login breaks when the executable path contains spaces or desktop field characters**

Evidence: Code finding. set_start_at_login writes Exec=<raw current_exe> --background. It does not quote or escape the path for a desktop entry. The installer already has a dedicated escaping helper for its normal launcher, but the autostart path does not use equivalent handling.

Repair: Use the same correct desktop-entry argument encoding for both launchers.

Source: [crates/ui/src/system.rs:119](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/system.rs:119), [scripts/install.sh:18](/home/adityavardhansharma/Projects/EchoFiles_Linux/scripts/install.sh:18).

**F53 · P3 · Today and Yesterday labels grow stale while the app stays open**

Evidence: Code finding. DateFormatter captures the current date at creation, and App keeps that instance for the whole session. There is no refresh of self.dates at midnight or after a clock/timezone change. A background-running app can therefore keep yesterday’s Today labels.

Repair: Refresh the formatter when the local day or timezone changes.

Source: [crates/core/src/fmt.rs:74](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/fmt.rs:74), [crates/ui/src/app.rs:290](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:290).

**Installation and removal**

**F54 · P2 · The source installer overwrites an unrelated ef executable**

Evidence: Code finding. The installer writes ~/.local/bin/ef unconditionally and then records it as its own installed file. Its uninstall fallback acknowledges that ef is a common name, but there is no corresponding ownership check before installation overwrites an existing command.

Repair: Detect pre-existing commands that are not owned by EchoFiles and preserve them or require an explicit replacement choice.

Source: [scripts/install.sh:56](/home/adityavardhansharma/Projects/EchoFiles_Linux/scripts/install.sh:56).

**F55 · P2 · Uninstall leaves the login launcher pointing at a removed executable**

Evidence: Code finding. Start at login creates an autostart/echofiles.desktop entry. The source uninstaller removes the binaries and normal launcher but neither tracks nor removes that autostart entry. A user who enabled startup is left with a broken login entry after uninstall.

Repair: Remove only the EchoFiles-owned autostart entry during uninstall and include it in installation ownership bookkeeping.

Source: [crates/ui/src/system.rs:111](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/system.rs:111), [scripts/install.sh:22](/home/adityavardhansharma/Projects/EchoFiles_Linux/scripts/install.sh:22).

**Additional findings in the phone update (9cd5e07)**

**F56 · P1 · An unpaired device can crash the phone worker with its device ID**

Evidence: Reproduced in debug; release consequence inspected. The new logging helper slices a device ID at byte eight without checking a UTF-8 character boundary. Device IDs from the network are not restricted to ASCII before this helper runs. An unpaired localhost peer named aaaaaaaé caused the link worker to panic, even with logging disabled, because formatting happens before the logging check. Release builds set panic=abort, so this panic would terminate the file manager process; the optimized release binary was not separately attacked.

Repair: Validate device IDs before registration and shorten text at character boundaries. Keep malformed peer input on an ordinary error path.

Source: [crates/phone/src/service.rs:138 (9cd5e07)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/snapshot-9cd5e07/crates/phone/src/service.rs:138), [crates/phone/src/service.rs:794 (9cd5e07)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/snapshot-9cd5e07/crates/phone/src/service.rs:794), [Cargo.toml:33 (9cd5e07)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/snapshot-9cd5e07/Cargo.toml:33).

**F57 · P2 · Bluetooth packet restrictions apply only to outgoing traffic**

Evidence: Code finding; real Bluetooth not tested. The new Bluetooth transport is described as carrying clipboard, calls and small control packets. send_raw applies bluetooth_ok to outgoing packets, but run_link sends every incoming non-identity packet to the common handler without the transport or an allowlist check. A paired Bluetooth peer can therefore trigger handlers for features the transport policy excludes. This is a policy-enforcement gap, not a demonstrated pairing bypass or payload transfer over Bluetooth.

Repair: Carry the transport into dispatch and enforce the same explicit packet policy on both receive and send.

Source: [crates/phone/src/service.rs:117 (9cd5e07)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/snapshot-9cd5e07/crates/phone/src/service.rs:117), [crates/phone/src/service.rs:341 (9cd5e07)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/snapshot-9cd5e07/crates/phone/src/service.rs:341), [crates/phone/src/service.rs:845 (9cd5e07)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/snapshot-9cd5e07/crates/phone/src/service.rs:845).

**F58 · P2 · Clipboard text marked sensitive still goes into history**

Evidence: Code finding. The phone update adds a sensitive flag specifically to ask the receiver not to keep password-manager copies in history. The desktop event handler ignores that flag and inserts the text into its 20-item clipboard history. The flag is correctly decoded by the service, so it is lost at the UI boundary.

Repair: Use the sensitive flag when receiving clipboard text: make it available for the intended paste without retaining it in history or later reconnect replay.

Source: [crates/phone/src/service.rs:574 (9cd5e07)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/snapshot-9cd5e07/crates/phone/src/service.rs:574), [crates/phone/src/service.rs:967 (9cd5e07)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/snapshot-9cd5e07/crates/phone/src/service.rs:967), [crates/ui/src/phone.rs:1431 (9cd5e07)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/snapshot-9cd5e07/crates/ui/src/phone.rs:1431).

**Additional architecture and validation risks**

**R01 · The UI remains the coordinator for almost everything**

App owns file transfers, undo, previews, indexing, networking, phone data and dialogs. This makes cross-feature state mistakes easier. Move operation ownership into services with explicit inputs, results and cancellation.

Source: [crates/ui/src/app.rs:135](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:135).

**R02 · Index freshness repeatedly scans the whole configured collection**

The 60-second timer and watched name changes call build_all rather than incremental rescan. Low-priority workers help but do not remove repeated I/O, memory allocation or flash/battery cost. Benchmark large and remote roots before calling this scalable.

Source: [crates/ui/src/app.rs:1562](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:1562), [crates/ui/src/indexer.rs:80](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/indexer.rs:80).

**R03 · Phone networking is always started and cannot be cleanly stopped**

App::boot starts the service and clipboard watcher even when the sidebar is hidden. There is no master off switch or shutdown path, and listener threads retain the service. Hiding the UI is not advertised as disabling networking, so this is a control/lifecycle gap rather than claiming that switch is broken.

Source: [crates/ui/src/app.rs:334](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/app.rs:334), [crates/ui/src/phone.rs:242 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:242), [crates/phone/src/service.rs:293 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/src/service.rs:293).

**R04 · The first Windows system-volume mount can briefly be read-write**

When the cached system flag is absent, mount tries read-write, looks for the Windows hive, then unmounts/remounts read-only. That does not meet a strict read-only-until-authorized policy. No Windows volume was mounted or tested during this review. Probe unfamiliar volumes read-only first.

Source: [crates/disks/src/lib.rs:250](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/disks/src/lib.rs:250).

**R05 · Unsafe environment setup happens after a thread has been started**

main calls system::listen, which spawns the socket thread, before gpu::configure mutates process environment. Its safety comment says the program is still single-threaded. Move environment setup ahead of all thread creation. No runtime memory fault was demonstrated.

Source: [crates/ui/src/main.rs:70](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/main.rs:70), [crates/ui/src/gpu.rs:13](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/gpu.rs:13).

**R06 · Release automation has no test gate and relies on an undeclared Python prerequisite**

The only checked-in workflow is a manual release build. Neither it nor the package recipe runs tests; there is no pull-request test workflow. It invokes python without explicitly installing Python. I did not build a clean Arch container to establish whether the current image supplies it indirectly.

Source: [.github/workflows/release.yml:28](/home/adityavardhansharma/Projects/EchoFiles_Linux/.github/workflows/release.yml:28), [packaging/arch/PKGBUILD.in:20](/home/adityavardhansharma/Projects/EchoFiles_Linux/packaging/arch/PKGBUILD.in:20).

**R07 · Phone image decoding and caches need resource limits**

The screen image reads the whole photo and decodes it without the explicit allocation cap used by normal thumbnails; the phone thumbnail cache has no eviction policy. A large or hostile image can consume substantial memory, and cache storage grows over time. No decompression-bomb or exhaustion test was run.

Source: [crates/ui/src/phone.rs:538 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:538), [crates/ui/src/phone.rs:416 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/ui/src/phone.rs:416), [crates/ui/src/thumbs.rs:243](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/ui/src/thumbs.rs:243).

**R08 · Hardware and failure coverage is far below the feature surface**

The baseline existing tests exercise 60 cases, including useful search and pairing paths. They do not establish recovery under power loss, ENOSPC, inotify overflow, hostile peers, multiple phones, blocked mounts, real Android variants, Windows hibernation or all supported desktop sessions. The tests can pass while the reproduced failures remain.

Source: [crates/phone/tests/loopback.rs:43 (e1a3de7)](/home/adityavardhansharma/Projects/EchoFiles_Linux/reviews/2026-10-03/baseline/crates/phone/tests/loopback.rs:43), [crates/core/src/ops.rs:820](/home/adityavardhansharma/Projects/EchoFiles_Linux/crates/core/src/ops.rs:820).

**Repair order**

1. Close the phone trust gaps and unsafe incoming-file handling. Add negative tests with impostor certificates and replaced sessions.
2. Make copy/move/replace/undo operate on a per-entry journal with safe staging, identity checks and explicit durability results. Fix hidden action targets and graceful exit at the same time.
3. Separate phone state by device, version asynchronous jobs, and commit photo-import records only after successful copies.
4. Consolidate settings/index publication and correct search exclusions, ranking and scopes.
5. Finish desktop, preview, thumbnail and installer fixes; put the original suite and regression cases into automatic checks.

**What remains unverified**

This was a deep source review with targeted local execution, not a complete formal verification, fuzzing campaign, dependency-advisory audit or live desktop usability test. Full disk, failing storage, power loss, hostile removable-volume trash directories, Windows/NTFS behavior, actual SFTP host-key interception, real Android interoperability, denial-of-service load and clean release-container installation still need dedicated fixtures. The report does not claim zero remaining bugs or label untested attack scenarios as demonstrated exploits.

**Evidence files**

- [Core/index probe](core_index_probe.rs) and [results](core_index_results.txt).
- [Phone probe](phone_probe.rs) and [results](phone_results.txt).
- [Search/settings probe](search_state_probe.rs) and [results](search_state_results.txt).
- [UI target-selection probe](ui_probe.rs) and [results](ui_results.txt).
- [CLI results](cli_results.txt), [baseline tests](workspace_tests_baseline.txt), [baseline Clippy output](clippy_baseline.txt), [9cd5e07 tests](workspace_tests.txt), and [9cd5e07 Clippy output](clippy.txt).
- [Machine-readable findings](findings.json).

The audit probes intentionally assert the currently observed bad behavior so that their observations are reproducible. They are not passing safety tests: before adding them to CI, reverse the expectations to assert the required safe behavior and add deterministic failure injection. See [reproduction notes](REPRODUCE.md).
