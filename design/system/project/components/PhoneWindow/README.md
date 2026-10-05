# PhoneWindow

The whole app with the phone open — the reference composition for every phone screen.

**Provide** `view` (`hub`, `photos`, `messages`, `notifications`), `state`, `wallpaper`.

- The **Phone** section sits between Windows and Network, marked `world-phone`, collapsible like the others (open by default; collapsed it still shows the phone row).
- Phone pages open in the pane like folders: back/forward work, the breadcrumb reads "Galaxy S24 › Photos", middle-click opens one in a new tab. Files is the normal file list rooted at the phone's important folders, with a *Show all storage* switch.
