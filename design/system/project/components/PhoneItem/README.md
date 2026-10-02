# PhoneItem

The phone in the sidebar's **Phone** section — the one button that opens everything phone.

**Provide** `state` (`none`, `pairing`, `connected`, `away`), `name` (the name set on the phone), `battery`, `charging`, `network` (Wi-Fi name), `seen` (when it was last nearby), `active`.

- **none**: a plain row, `plus` glyph, **Connect phone** — opens `PairPhone`.
- **pairing**: `Pairing…` `info` pill on the second line while the code check runs.
- **connected**: `phone` glyph in `world-phone`, the name, a `BatteryMeter` at the end; second line "Wi-Fi · home" and the battery %. Below it, indented, **Files** and **Photos** (with "32 new" when there are unimported photos) — the phone's two places you browse and drop onto.
- **away**: glyph and name `ink-muted`, "Not nearby · 2 h ago". Files and Photos fold away; the hub still opens with the last-known facts.
- Click opens `PhoneHub`. Drop files on the row → they're sent to the phone. A **Connect another phone** row ends the open section. Files also get **Send to Galaxy S24** in their right-click menu, and the command palette has a **Phone** group (Open, Photos, Files, Messages, Notifications, Ring, Send files…, Connect).
