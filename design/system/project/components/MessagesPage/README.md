# MessagesPage

Texts from the phone: conversations left, the open chat right, a reply box that sends through the phone.

**Provide** nothing; the list is clickable.

- Conversation rows: initial avatar (contact colour from the icon slots), name, last message, time, unread count in `accent`. Search filters names and text.
- Bubbles: theirs `bg-raised` with a hairline, yours `accent` with `on-accent` text; 14px radius with the tail corner squared. A day chip separates days.
- **Send** (Enter) sends a normal SMS from the phone; the line under the box says so. Loaded when the page opens (the phone sends the threads on request), so it works without a background service.
- Off in Settings → Phone → Messages (the default): no texts reach the laptop at all.
