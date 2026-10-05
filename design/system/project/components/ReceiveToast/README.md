# ReceiveToast

What appears when the phone sends files.

**Provide** `from`, `what`.

- **Accept files automatically on** (default): no question — a `TransferToast` "Receiving 3 files from Galaxy S24" for sends longer than ~0.4s, and the files appear in `ReceivedList`.
- **Off**: this toast asks first — "Galaxy S24 wants to send 3 files · 12 MB", the names and where they'll go, **Accept** (`primary`) and **Decline**. It stays until answered; declining tells the phone.
