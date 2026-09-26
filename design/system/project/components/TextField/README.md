# TextField

A single-line field on the `bg-deep` well with an optional label and inline error.

**Provide** `id`, `label`, value props, optional `icon`, `trailing` element, `error` (a sentence that says what is wrong and the fix).

- Focus swaps the border to `focus-ring` plus a 1px ring; errors use `danger` border and `danger-ink` message.
- Validate Windows names live when the destination is NTFS (reserved names, `: ? * " < > |`, trailing dot or space) and offer the fixed name in the message.
