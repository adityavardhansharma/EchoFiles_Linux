# IndexStatus

The search index's health in one row: a `StatePill`, what it means in numbers, and **Rebuild now**.

**Provide** `state` (`off` | `opening` | `building` | `updating` | `ready` | `problem`), optional `detail`.

- Ready: "179,581 items · 13 MB on disk · updated 21 s ago" — counts, size and freshness, live.
- Building/Updating: `info` pill, the button reads "Indexing…" and is disabled. Problem: `warning` pill and the reason with the folder named.
- Sits directly under the Search index switch in Settings → Search & index.
