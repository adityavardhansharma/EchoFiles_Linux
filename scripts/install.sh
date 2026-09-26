#!/usr/bin/env bash
# Build EchoFiles and install it for the current user (no root):
#   ~/.local/bin/echofiles, ~/.local/bin/ef, a launcher entry and the app icon.
# Uninstall: scripts/install.sh --uninstall
set -euo pipefail

cd "$(dirname "$0")/.."
BIN="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
APPS="$DATA/applications"
ICON="$DATA/icons/hicolor/scalable/apps"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$BIN/echofiles" "$BIN/ef" "$APPS/echofiles.desktop" "$ICON/echofiles.svg"
  echo "Removed EchoFiles. Settings (~/.config/echofiles) and the index (~/.cache/echofiles) were kept."
  exit 0
fi

cargo build --release -p echofiles -p echofiles-index --bins
mkdir -p "$BIN" "$APPS" "$ICON"
install -m 755 target/release/echofiles "$BIN/echofiles"
install -m 755 target/release/ef "$BIN/ef"
install -m 644 assets/brand/echofiles-logo.svg "$ICON/echofiles.svg"

cat > "$APPS/echofiles.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=EchoFiles
GenericName=File Manager
Comment=Fast file manager for Linux and Windows drives
Exec=$BIN/echofiles %F
Icon=echofiles
Terminal=false
Categories=System;FileTools;FileManager;Utility;
MimeType=inode/directory;
StartupWMClass=echofiles
Keywords=files;folders;search;explorer;
Actions=Settings;

[Desktop Action Settings]
Name=Settings
Exec=$BIN/echofiles --settings
EOF

command -v update-desktop-database >/dev/null && update-desktop-database -q "$APPS" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true

echo "Installed EchoFiles to $BIN (echofiles, ef)."
case ":$PATH:" in *":$BIN:"*) ;; *) echo "Note: $BIN isn't on your PATH." ;; esac
