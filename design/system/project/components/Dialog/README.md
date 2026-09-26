# Dialog

A modal for decisions that can't wait: permanent delete, conflicts, unsafe writes, unlocking.

**Provide** `title` (the question, naming the thing), `icon` (colour icon), `children` (consequences + safer alternative), `footer` (buttons: Cancel first, the action last), `tone="danger"` for irreversible actions.

- `bg-raised` panel with a 2px `accent` border (Omarchy popup style; `danger` border when destructive), square corners, `shadow-float`, over `scrim`.
- Enters with scale .98 → 1 in `dur-slow`. Esc cancels. The dangerous button is never the Enter default.
