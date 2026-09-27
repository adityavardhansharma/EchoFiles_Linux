# FileRow

One row of `FileList`, shown here in every state it can take.

**Provide** `file` (see FileList), `renaming`.

- While renaming, a problem with the name (empty, `/`, already exists, or a character Windows can't store on NTFS) shows in a `danger-soft` strip at the bottom of the pane, naming the fix: "Windows doesn't allow ? in names. Press Enter to use “Q3_final.xlsx”." Enter then applies the fixed name.
- Clicking anywhere else in the list commits the rename; Esc cancels it.

- Drop target: `accent-soft` + 1px `accent` inset, the icon pops (`ease-spring`), and a 2px bar fills over 700ms — when full, the folder opens (spring-loading).
- Rename: inline field on `bg-deep` with the stem pre-selected and the extension left out; Enter commits, Esc cancels, Tab moves to the next row.
- Removing: collapses in `dur-base` with `ease-exit`, then an Undo toast appears.
