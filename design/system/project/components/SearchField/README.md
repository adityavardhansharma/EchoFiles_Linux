# SearchField

Filter-as-you-type for the current folder, with a scope chip once search widens.

**Provide** `id`, `placeholder`, optional `scope` ("in AVS (D:)", "everywhere").

- Typing filters the visible listing instantly (no debounce under 10k items). Press `/` or Ctrl+F to focus; Esc clears then blurs.
- The key hint shows until there is a scope; the scope chip is `accent-soft`.
