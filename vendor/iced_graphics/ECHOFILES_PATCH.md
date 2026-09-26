# EchoFiles patch to iced_graphics 0.14.0

Upstream `font_system()` parses every installed font (714 files / 464 MB on the dev machine,
~450 ms) before the first frame can draw text. This patch adds two functions to `src/text.rs`:

- `set_startup_fonts(paths)` — create the font system with only these files plus the
  embedded icon font (a few ms).
- `load_system_fonts_deferred()` — build the full database on the calling (background)
  thread without holding the lock, then swap it in and bump `Version` so cached text is
  re-shaped with fallback fonts available.

Everything else is the published 0.14.0 source. Drop this vendor copy once upstream offers
lazy or cached system-font loading.
