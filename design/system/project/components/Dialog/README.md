# Dialog

A modal for decisions that can't wait: permanent delete, conflicts, unsafe writes, unlocking.

**Provide** `title` (the question, naming the thing), `icon` (colour icon), `children` (consequences + safer alternative), `footer` (buttons: Cancel first, the action last), `tone="danger"` for irreversible actions.

- `bg-raised` panel with a 2px `accent` border (Omarchy popup style; `danger` border when destructive), square corners, `shadow-float`, over `scrim`.
- Enters with scale .98 → 1 in `dur-slow`. Esc cancels. The dangerous button is never the Enter default.
- The dialogs EchoFiles has: **Delete permanently** (Shift+Del; offers Move to Trash; if a drive has no trash it says why and offers only permanent delete), **Empty the Trash?**, **Password needed** (EchoFiles' own polkit prompt for mounting internal drives; a wrong password turns the field `danger` with "That password didn't work"), **Allow writing to Windows (C:)?**, **Properties** (Alt+Enter or the menu: the item's facts only — size, dates, location, permissions or Windows attributes — with a × in the corner, Close and Esc; for several items, how many, their total size counted live and where), `ConflictDialog`, and **Some names won't work on Windows** (lists "a:b.txt → a_b.txt", Enter renames and continues).
