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
SKILL_SOURCE="$DATA/echofiles/agents/skills/echofiles"
# Files this script installed, so --uninstall never removes someone else's `ef`.
MANIFEST="$DATA/echofiles/installed-files"

# Quote a path for a desktop entry's Exec key (Desktop Entry spec: quoted argument, then
# the general string escaping of `\`).
exec_arg() {
  printf '"%s"' "$(printf '%s' "$1" | sed -e 's/\\/\\\\\\\\/g' -e 's/["`$]/\\\\&/g' -e 's/%/%%/g')"
}

if [[ "${1:-}" == "--uninstall" ]]; then
  for skills_dir in "$HOME/.agents/skills" "$HOME/.claude/skills" "$HOME/.codex/skills" "$HOME/.pi/agent/skills"; do
    link="$skills_dir/echofiles"
    if [[ -L "$link" && "$(readlink "$link")" == "$SKILL_SOURCE" ]]; then
      rm "$link"
    fi
  done
  if [[ -f "$MANIFEST" ]]; then
    while IFS= read -r file; do
      if [[ -n "$file" ]]; then rm -f "$file"; fi
    done < "$MANIFEST"
    rm -f "$MANIFEST"
  else
    # Installed before the record existed: `ef` is a common name, so leave it to the user.
    rm -f "$BIN/echofiles" "$APPS/echofiles.desktop" "$ICON/echofiles.svg"
    if [[ -e "$BIN/ef" ]]; then
      echo "Left $BIN/ef in place; remove it yourself if it's EchoFiles' ef." >&2
    fi
  fi
  echo "Removed EchoFiles. Settings (~/.config/echofiles) and the index (~/.cache/echofiles) were kept."
  exit 0
fi

cargo build --release -p echofiles -p echofiles-index --bins
# Honour CARGO_TARGET_DIR / build.target-dir.
TARGET="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
TARGET="${TARGET:-target}"
mkdir -p "$BIN" "$APPS" "$ICON" "$(dirname "$MANIFEST")"
install -m 755 "$TARGET/release/echofiles" "$BIN/echofiles"
install -m 755 "$TARGET/release/ef" "$BIN/ef"
install -m 644 assets/brand/echofiles-logo.svg "$ICON/echofiles.svg"
"$BIN/echofiles" --sync-agent-skill

cat > "$APPS/echofiles.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=EchoFiles
GenericName=File Manager
Comment=Fast file manager for Linux and Windows drives
Exec=$(exec_arg "$BIN/echofiles") %F
Icon=echofiles
Terminal=false
Categories=System;FileTools;FileManager;Utility;
MimeType=inode/directory;
StartupWMClass=echofiles
Keywords=files;folders;search;explorer;
Actions=Settings;

[Desktop Action Settings]
Name=Settings
Exec=$(exec_arg "$BIN/echofiles") --settings
EOF
printf '%s\n' "$BIN/echofiles" "$BIN/ef" "$APPS/echofiles.desktop" "$ICON/echofiles.svg" > "$MANIFEST"

command -v update-desktop-database >/dev/null && update-desktop-database -q "$APPS" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true

echo "Installed EchoFiles to $BIN (echofiles, ef)."
case ":$PATH:" in *":$BIN:"*) ;; *) echo "Note: $BIN isn't on your PATH." ;; esac
