# DriveItem

A drive in the sidebar: name, state, usage bar and free space in two lines.

**Provide** `name` (display name: alias, else drive letter + label — never the mount path or a raw UUID), `state` (`mounted`, `readonly`, `dirty`, `locked`, `unmounted`, `mounting`), `used` (percent), `world`, `meta`, `free`.

- Mounted drives show no pill (it is the normal state). Read-only and Needs check use `warning`, Locked `danger`, Not mounted is neutral and dims the name.
- Clicking an unmounted drive mounts it and opens it; a spinner replaces the pill while udisks works. Usage bars grow once on mount (400ms).
- Usage above 90% turns the bar `warning`, above 97% `danger`.
