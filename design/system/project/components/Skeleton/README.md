# Skeleton

Placeholder rows for a folder that takes longer than 150ms to list (cold NTFS, huge directories).

**Provide** `rows` (fill the viewport).

- Never shown before 150ms — fast folders paint straight away with no flash. Names from the first batch replace skeleton rows in place.
- Shimmer loops once per `dur-ambient` (1.4s) and stops under reduced motion.
