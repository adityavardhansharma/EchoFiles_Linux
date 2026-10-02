# TransferRow

A file moving between the devices: icon, name, progress with real numbers, or size · direction · time when done.

**Provide** `name`, and `progress` + `rate` while moving, or `size`, `dir` (`to`/`from`), `when`; `failed` for a stopped transfer.

- Moving: a Stop button. Done: Open. Failed: the reason in `danger-ink` and Retry — transfers resume where they stopped.
