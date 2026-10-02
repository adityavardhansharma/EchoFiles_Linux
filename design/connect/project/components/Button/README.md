# Button

A text button whose label says exactly what happens: 40px tall, 48px (`lg`) for the main action at the bottom of a screen, 32px (`sm`) inside rows.

**Provide** `children` (a verb phrase: "Send 3 photos", "Stop ringing", never "OK"), optional `variant` (`primary`, `danger`, `ghost`; default is bordered), `icon`, `size` (`sm`, `lg`), `block` for full width.

- One `primary` per screen or sheet. Full-width `lg` buttons sit in the flow footer or at the end of a step.
- `ghost` buttons are `accent-ink` text: secondary choices like "Use Tap to send instead".
- The hit area is never under 48px, even for `sm`.
- Same colours and states as EchoFiles' Button: hover `state-hover`, press `state-press`, focus a 2px `focus-ring` outline.
