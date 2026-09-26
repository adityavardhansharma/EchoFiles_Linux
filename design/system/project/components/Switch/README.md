# Switch

An on/off setting that applies immediately, with its label on the left.

**Provide** `children` (the setting, phrased as the on state), `checked`/`defaultChecked`, `onChange`.

- Track `bg-deep` → `accent` when on; thumb slides 14px in `dur-base`. Use in Settings and the More menu, never inside dialogs that have a confirm button (use `Checkbox` there).
