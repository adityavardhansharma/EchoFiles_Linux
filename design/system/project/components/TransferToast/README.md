# TransferToast

Progress for copy, move, delete and extraction: title, current file, bar, throughput and time left.

**Provide** `title`, `detail` (source and current file), `value` (0–100) or `indeterminate` (during planning), `meta` (bytes done · speed), `eta`.

- The bar is linear and fed from the engine's atomic counters at frame rate — never a message per chunk. At 100% it turns `success` and the tick draws.
- "Flushed to disk" appears only after the batch `syncfs` completes, so people know when it's safe to reboot into Windows.
- Pause and Cancel (Esc) are always there; cancel undoes a partial move.
