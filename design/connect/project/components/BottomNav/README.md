# BottomNav

Four destinations — Home, Clipboard, Transfers, Settings — on a 64px `bg-deep` bar.

**Provide** `active` index, optional `badges` (`{index: count}`), `onChange`.

- The current item gets the `bg` ground, a 2px `accent` bar on its top edge and `accent-ink` — the same mark as EchoFiles' active tab.
- Labels always show. A badge counts things waiting (a transfer in progress).
