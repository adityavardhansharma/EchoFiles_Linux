# PairPhone

**Connect phone**: four steps in one dialog. Nothing KDE is installed on the laptop — EchoFiles speaks KDE Connect itself; the phone runs the stock KDE Connect app.

**Provide** `step` (0–3), `nearby` (`"none"` while nothing is found), `filesOk`. The stepper is clickable in the preview.

1. **Get the app** — Google Play and F-Droid entries; "It's open on my phone".
2. **Choose your phone** — phones announcing KDE Connect on this Wi-Fi, live, one click pairs (EchoFiles re-announces itself every 5 s while the dialog is open). After 10 s with none, a help box: same Wi-Fi and app open · **Allow KDE Connect…** (only when ufw is active; asks for the password once through pkexec and opens ports 1714–1764) · or type the phone's IP and **Connect** (EchoFiles connects to it directly).
3. **Check the code** — the 8-character code in four 26px tiles; the phone shows the same. When we asked: "Waiting for Galaxy S24…" until it's accepted there; **Cancel** tells the phone. When the phone asked (the dialog opens by itself, even from the background): **Pair** (Enter) or **Codes don't match**.
4. **Allow files** — the one switch on the phone (Plugin settings → Filesystem expose, then All files access). A pill re-checks on its own: `warning` "Files not shared yet" → `success` "Files available". **Open Galaxy S24** lands on the hub.

- Pairing is stored as the phone's certificate in `~/.config/echofiles/phone/trusted/`. Several phones can be paired; the hub shows the one picked in the sidebar.
