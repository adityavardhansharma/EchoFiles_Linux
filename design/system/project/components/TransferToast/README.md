# TransferToast

Progress for copy, move, delete and extraction: title, current file, bar, throughput and time left.

**Provide** `title`, `detail` (source and current file), `value` (0–100) or `indeterminate` (during planning), `meta` (bytes done · speed), `eta`.

- The bar is linear and fed from the engine's atomic counters at frame rate — never a message per chunk. It appears only for work that takes longer than ~0.4s.
- It leaves the moment the batch `syncfs` completes — the drive is then safe to unplug or reboot from. Only a transfer with failures stays, turned `danger`, until dismissed.
- Pause and Cancel (Esc) are always there; cancel undoes a partial move.
