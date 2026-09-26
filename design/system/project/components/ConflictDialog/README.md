# ConflictDialog

Resolves name collisions during copy and move, side by side, once for all or file by file.

**Provide** `title`, `count` (conflicts in the batch).

- The two files are compared by modified date and size; the newer one is marked in `success-ink`.
- The pre-flight check finds every conflict before any byte is written, so this appears once at the start, never mid-copy.
- Windows-name problems (`:`, `?`, case-only duplicates on NTFS) get the same dialog with a rename rule instead of Replace.
