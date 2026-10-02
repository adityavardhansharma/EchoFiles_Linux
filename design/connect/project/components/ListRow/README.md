# ListRow

One row: a lead (glyph tile, file icon, avatar or any node), title, up to two lines of subtitle, and a trailing control or chevron. 56px, 72px with a subtitle.

**Provide** `title`, optional `sub`, one lead (`icon` + `tint`, `file`, `avatar` + `avatarColor`, `lead`), `trailing`, `chevron`, `onClick`, `below` (extra content under the text), `disabled`.

- A row with `onClick` or `chevron` is a button: `state-hover` and `state-press` layers.
- Dependent rows show disabled with the dependency as their subtitle, never hidden.
