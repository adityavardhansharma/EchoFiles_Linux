86 single-ink UI glyphs: 24-unit grid, 2px stroke, round caps and joins.

- These uploads are drawn in ink `#4B5565` so they show in `<img>`; the repo copies (`assets/icons/glyph/`) use `currentColor`, and the `Icon` component inherits colour from its parent (`ink-muted` at rest, `ink-strong` on hover, `accent-ink` when active).
- For actions and UI only — never for file types (use the colour icons).
