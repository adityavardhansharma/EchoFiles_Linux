# SegmentedControl

Two to four mutually exclusive options in one well: view mode, density, sort scope.

**Provide** `options` (`{value, icon?, text?, label}`), `value` or uncontrolled default, `onChange`, `label` for the group.

- The selected segment lifts to `bg-raised` with `accent-ink` content; others stay `ink-muted`. The switch is instant (`dur-fast` colour only, no sliding pill).
- Icon-only segments must carry `label`. More than four options → a menu.
