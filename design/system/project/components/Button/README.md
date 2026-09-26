# Button

A 28px text button with an optional leading glyph and trailing key hint; the label says exactly what will happen.

**Provide** `children` (a verb phrase: "Copy here", "Delete permanently", never "OK"), optional `variant` (`primary`, `danger`, `ghost`; default is the bordered secondary), `icon`, `kbd` (key hint shown inside), `size="sm"` (24px, for toasts, banners and panes).

- One `primary` per surface: the action Enter triggers. `danger` only for irreversible actions (permanent delete, clear dirty flag, format), and it is never the Enter default.
- Buttons live in dialog footers, banners, toasts, empty states and panes. The toolbar uses `IconButton`.
- Hover adds `state-hover`, press `state-press` (Omarchy's 8% / 22% foreground layers); primary brightens instead. Focus draws a 2px `focus-ring` outline, 1px off the edge.
- Disabled uses `opacity-disabled` and keeps its label, so people can still read what is unavailable.
