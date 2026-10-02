# BatteryMeter

The phone's charge as a 20×10 battery with its percentage.

**Provide** `level` (0–100), `charging`, `label={false}` to hide the number (the sidebar shows it on the second line).

- Fill is `ink-muted`; `success` with a `bolt` while charging; `warning` at 20% and below; `danger` at 10% and below. The tooltip says "Battery 18%".
- **Low battery warning** (Settings → Phone) shows one accent `Toast` at 15%: "Galaxy S24 is at 15%".
