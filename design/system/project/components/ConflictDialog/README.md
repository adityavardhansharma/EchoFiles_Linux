# ConflictDialog

Resolves name collisions during copy and move, side by side, once for all or file by file.

**Provide** `title`, `count` (conflicts in the batch).

- The two files are compared by modified date and size; the newer one is marked in `success-ink`.
- The pre-flight check finds every conflict before any byte is written, so this appears once at the start, never mid-copy.
- Windows-name problems (`:`, `?`, reserved names, trailing dots on NTFS) get their own dialog right after, with the fixed names listed.
- **Keep both** is the Enter default (it loses nothing): the copy becomes "Report-Q3 (2).xlsx". Replace becomes **Merge** when both are folders. Cancel drops the whole transfer. "Do this for all N conflicts" applies the choice to the rest.
