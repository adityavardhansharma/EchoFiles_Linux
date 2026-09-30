# SignInDialog

The server wants a user name and password — asked in EchoFiles' own dialog (EchoFiles serves GVfs' `MountOperation`), never a terminal prompt.

**Provide** `title` ("Sign in to Media on nas.local"), `detail` (GVfs' sentence), `user`, `domain` (SMB only), `anonymous` (the server allows guests), `guest`, `retry`.

- **Registered user / Guest** `SegmentedControl` only when the server allows guests; Guest hides the fields and the button reads **Connect as guest**.
- Fields as the server needs them: User name (pre-filled from the address, else the login name) and Domain (`WORKGROUP`) side by side for SMB; Password below. Focus starts in the first empty field; Tab moves between them, Enter signs in.
- **Remember password in the keyring** (off by default): on saves it permanently through GVfs and the Secret Service; off keeps it until logout, so reconnecting in the same session doesn't ask again.
- A wrong password reopens the dialog with the field in `danger` and "That didn't work. Check the user name and password and try again."
- Questions go through the same channel: an unknown SSH host key shows GVfs' message in a `bg-deep` well with its choices as buttons (**Log In Anyway** primary, **Cancel Login** ghost). Nothing is ever accepted on Enter.
