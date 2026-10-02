# PhoneDevice

The phone, drawn flat like the colour icons — a flat-sided Android phone in the `icon-slate` slots with antenna bands, volume rocker and side key both on the right, thin even bezels and a pin-hole camera — showing a One UI–style lock screen. No gradients, glare or shadow: it recolours with the Omarchy theme like every other icon. It is the hero of `PhoneHub` and the thumbnail in Settings → Phone.

**Provide** `state` (`connected`, `ringing`, `away`), `wallpaper` (`photo` = the phone's newest camera photo, `aurora` = the EchoFiles wallpaper), `photo` (thumbnail handle), `battery`, `time`, `date`, `notice`, `width` (196 on the hub, 34 in Settings).

- The protocol tells EchoFiles the phone's name and type, not its model, so this is one generic modern Android phone — never a picture of a specific brand, and never iPhone-like (no notch or island, no thick rounded chrome band, no centred thin clock).
- Lock screen, Android-style: status bar with notification icons, signal, Wi-Fi and battery; date and weather above a bold **stacked clock** (hours over minutes, left-aligned); an Android notification card ("EchoFiles · now", title, text); in-display fingerprint; phone and camera shortcuts in the corners; a short gesture handle.
- **Latest photo** is the default and makes the drawing feel like *your* phone: the newest picture in `DCIM/Camera`, read from its EXIF thumbnail (no full download), behind a top and bottom vignette so the clock always reads. **EchoFiles** is a flat wallpaper: three flat circles in `world-network`, `accent` and `world-linux` over a fixed near-black screen, so it follows the theme and the white clock always reads.
- Clock, date and battery are live from the phone. The screen text uses the phone's sans (Inter / Roboto), never the app's mono — it is a picture of a phone, not app UI.
- **Ringing** (Ring phone): the device buzzes (a short wiggle every 1.2s) and three `accent` rings pulse out; the notification becomes "Ringing from EchoFiles · Swipe to stop". **Away**: screen off, "Not nearby", 72% opacity, desaturated. All motion stops under reduced motion.
- The app draws it once per state as a vector and caches the bitmap per size; the photo is the only raster in it.
