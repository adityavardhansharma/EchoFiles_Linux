# NotificationsPage

The phone's current notifications, grouped by app.

**Provide** `mode` (`app` | `desktop`).

- Each app is one bordered group: app initial on its colour, name, count; rows with title, text, time and a × that dismisses on the phone too. Apps that allow quick replies (WhatsApp, Telegram, Messages) get a reply field on the newest one.
- **Settings → Phone → Phone notifications**: **Off** (nothing comes over), **In app** (only this page — the default; the subtitle says so), **Desktop too** (also pops up as a normal desktop notification while EchoFiles runs; with Keep running in the background on, even with the window closed). Apps in *Never show notifications from* are dropped on arrival.
- **Dismiss all** clears them here and on the phone.
