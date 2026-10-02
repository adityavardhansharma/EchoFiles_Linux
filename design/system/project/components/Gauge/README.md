# Gauge

A hub fact as a 56px ring gauge with label, value and one line — used for the phone's battery and storage.

**Provide** `icon`, `label`, `value`, `unit`, `pct` (ring fill), `color`, `sub`, `bolt` (charging).

- 48px ring, 5px stroke on `line`, round cap; it fills once in 400ms like `UsageBar` (none under reduced motion). The glyph sits in the middle, `ink-muted`, or a `success-ink` bolt while charging.
- Card: flat `bg` with a 1px `line` border and `radius-2` — no tint, highlight or shadow. Label in `label` style, value 22px/800, unit 12px muted.
