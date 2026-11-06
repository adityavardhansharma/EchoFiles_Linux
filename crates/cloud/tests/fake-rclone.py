#!/usr/bin/env python3
"""A stand-in for rclone in tests: `version`, and `rcd` answering rc calls on a Unix
socket. Every call is appended to $FAKE_RCLONE_LOG as one JSON line."""
import json, os, socket, sys

args = sys.argv[1:]
log = os.environ.get("FAKE_RCLONE_LOG")

def record(entry):
    if log:
        with open(log, "a") as f:
            f.write(json.dumps(entry) + "\n")

record({"argv": args})
if "version" in args:
    print("rclone v1.68.2\n- os/version: test")
    sys.exit(0)
if "rcd" not in args:
    sys.exit(0)
addr = args[args.index("--rc-addr") + 1]
assert addr.startswith("unix://"), addr
path = addr[len("unix://"):]
try:
    os.unlink(path)
except FileNotFoundError:
    pass
s = socket.socket(socket.AF_UNIX)
s.bind(path)
s.listen(8)
while True:
    c, _ = s.accept()
    data = b""
    while b"\r\n\r\n" not in data:
        data += c.recv(65536)
    head, body = data.split(b"\r\n\r\n", 1)
    length = int([l for l in head.split(b"\r\n") if l.lower().startswith(b"content-length:")][0].split(b":")[1])
    while len(body) < length:
        body += c.recv(65536)
    method = head.split(b" ")[1].decode().lstrip("/")
    params = json.loads(body or b"{}")
    record({"method": method, "params": params})
    reply = {}
    status = "200 OK"
    if method == "core/version":
        reply = {"version": "v1.68.2"}
    elif method == "operations/about":
        reply = {"total": 16106127360, "used": 4509715660, "free": 11596411700}
    elif method == "vfs/stats":
        reply = {"diskCache": {"uploadsInProgress": 1, "uploadsQueued": 1}}
    elif method == "operations/list":
        reply = {"list": [{"Path": "Budget.link.html", "Name": "Budget.link.html", "Size": -1, "MimeType": "application/vnd.google-apps.spreadsheet", "ModTime": "2026-09-14T18:22:33Z", "IsDir": False, "ID": "1a"}]}
    elif method == "operations/deletefile" and params.get("remote") == "missing.txt":
        status, reply = "500 Internal Server Error", {"error": "object not found"}
    out = json.dumps(reply).encode()
    c.sendall(b"HTTP/1.1 " + status.encode() + b"\r\nContent-Type: application/json\r\nContent-Length: " + str(len(out)).encode() + b"\r\nConnection: close\r\n\r\n" + out)
    c.close()
    if method == "core/quit":
        sys.exit(0)
