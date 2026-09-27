# PathBar

The breadcrumb: clickable segments that turn into a typed path on Ctrl+L or double-click.

**Provide** `segments` (`{label, icon?}`; the first carries the volume's display name and icon), or `editing` with `path`.

- Windows volumes show their drive-letter name ("AVS (D:)"), never `/run/media/…`; the typed mode shows the real path and accepts Windows paths (`D:\Work`) too.
- Only the newest segment animates (slides 6px in `dur-base`); navigation itself is instant.
- Last segment is `ink-strong` bold; the rest `ink-muted` with hover layers. Overflow collapses middle segments into a `…` menu.
- Ctrl+L or a double-click on the empty part of the bar switches to the typed path (text selected); Enter goes there — a file's path opens its folder with the file selected — and Esc goes back. `D:\Work` works once EchoFiles knows the letters (they're read from Windows the first time C: is mounted).
- During a drag, segments are drop targets like sidebar places.
