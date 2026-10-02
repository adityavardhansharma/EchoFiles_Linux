# ReceivedList

Files the phone sent to this laptop, newest first: in progress with a bar and speed, or landed with when and a **Show** button.

**Provide** `items` (`{name, size, progress?, done?, when?}`), `title`.

- Files share from any phone app (Share → KDE Connect → this laptop). With **Accept files automatically** on (the default) they save straight to `~/Downloads/Phone`; name clashes keep both as "name (2).ext", never overwrite. Off, each send asks first with `ReceiveToast`.
- **Open folder** opens the save folder in a tab. Receiving needs EchoFiles running; with the window closed only if Keep running in the background is on.
