# Toast

A floating notice in the bottom-right corner for results of actions.

**Provide** `title` (what happened, past tense), `tone` (`danger`, or default accent), `children` (why), `actions`.

- Border is 2px — `accent`, `success` or `danger` — the Omarchy notification look (Hyprland's active border). Radius follows Hyprland `rounding` (0).
- Enter: rise 12px with `ease-spring` in `dur-slow`; exit: fade at 70%. Stack newest at the bottom, at most three; older collapse into "+2 more".
- Toasts are for news and problems only. A finished action — trash, delete, copy, move, rename, undo, restore, unmount — never announces itself: the files on screen already show it, and Ctrl+Z walks back through rename, move, copy, trash and new folder in order.
- Accent toasts (news: a drive opened read-only, nothing to paste, a cancelled transfer) leave after 4s, paused on hover; error toasts stay until dismissed.
