# TabStrip

Tabs across the top of the window on the `bg-deep` strip.

**Provide** `tabs` (`{label, icon}`), `active`.

- The active tab joins the toolbar (`bg`) and carries a 2px `accent` top bar — the same signal as Hyprland's active border. Close buttons appear on hover and on the active tab.
- Ctrl+T new, Ctrl+W close, Ctrl+Tab / Ctrl+Shift+Tab cycle, middle-click a folder (or a sidebar place) to open it in a tab; middle-click a tab to close it; right-click a tab for New tab / Close tab. Tabs never animate position.
- A tab split into two panes carries a small `columns` glyph after its name. Switching to a tab refreshes its folders.
