# Icon

A 24-unit, 2px-stroke, round-capped glyph that inherits `currentColor`; used for every UI action, never for file types.

**Provide** `name` (one of the 86 glyphs in `assets/icons/glyph/`), optional `size` (default 16 = `icon-glyph`), `label` when the glyph stands alone with meaning.

- Colour comes from the parent: `ink-muted` at rest, `ink-strong` on hover, `accent-ink` when active or pressed, a semantic `*-ink` inside banners and toasts.
- Sizes: 16 in toolbars, menus and sidebar; 14 inside buttons and fields; 12 in chevrons, sort arrows and badges.
- Never fill a glyph, never mix it with a colour icon in the same slot, never use it for a file type (that is `FileIcon`).
- The Rust app rasterises the same SVGs with `resvg`, substituting `currentColor` with the resolved token, and caches one bitmap per (glyph, size, scale, colour).
