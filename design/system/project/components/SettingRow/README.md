# SettingRow

One setting: label (`ink-strong`, 13px) and a one- or two-sentence description (`ink-muted`, 12px) on the left; the control on the right, vertically centred. 12px × 16px padding.

**Provide** `label`, `description`, `disabled`, and the control as `children` (`Switch`, `SegmentedControl`, a `Button`, or a status pill).

- The description says what the setting does and its consequence ("Turning it off deletes the index"), not how it works.
- A setting that depends on another is shown `disabled` (45%) with the dependency as its description ("Needs Keep running in the background.") — never hidden.
