# Toast

A floating notice in the bottom-right corner for results of actions.

**Provide** `title` (what happened, past tense), `tone` (`success`, `danger`, or default accent), `children` (why, for errors), `actions`.

- Border is 2px — `accent`, `success` or `danger` — the Omarchy notification look (Hyprland's active border). Radius follows Hyprland `rounding` (0).
- Enter: rise 12px with `ease-spring` in `dur-slow`; exit: fade at 70%. Stack newest at the bottom, at most three; older collapse into "+2 more".
- Success toasts auto-dismiss after 4s (paused on hover); error toasts stay until dismissed. Every destructive action gets an Undo toast.
