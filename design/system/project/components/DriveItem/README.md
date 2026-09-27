# DriveItem

A drive in the sidebar: name, state, usage bar and free space in two lines.

**Provide** `name` (display name: alias, else drive letter + label — never the mount path or a raw UUID), `state` (`mounted`, `readonly`, `dirty`, `locked`, `unmounted`, `mounting`), `used` (percent), `world`, `meta`, `free`.

- The state pill leads the second line, before the size, so the two never collide in the 236px sidebar.
- Mounted drives show no pill (it is the normal state). Read-only and Needs check use `warning`, Locked `danger`, Not mounted is neutral and dims the name.
- Clicking an unmounted drive mounts it and opens it; a spinner replaces the pill while udisks works. Usage bars grow once on mount (400ms).
- Usage above 90% turns the bar `warning`, above 97% `danger`.
- With a pill, the second line shows only the free space ("38 GB free"); without one, "61 GB free of 295 GB". The total is always in the tooltip.
- Pills: **Mounting…** (`info`) while udisks works — the password dialog appears if Linux asks for one; **Read-only** (`warning`) for C: and anything mounted read-only; **Needs check** (`warning`) when Windows didn't shut down fully and the drive fell back to read-only; **Locked** (`danger`) for BitLocker; **Not mounted** (neutral).
- Tooltip: the mount path and driver (`/run/media/…/AVS · ntfs3`), or the device when unmounted. Right-click: Open, Open in new tab, **Allow writing…** (C: only, `danger`), **Unmount**, All drives.
