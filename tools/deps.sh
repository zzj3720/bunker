#!/bin/sh
# Fetches pinned official AFK-surf/invar (main 368ff1b) into pion/deps/invar.
set -e
REV=368ff1b5652ed692786d34afffdfc9f1c0a4cdce
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
DIR="$ROOT/pion/deps/invar"
REPO="${INVAR_REPO:-https://github.com/AFK-surf/invar.git}"
mkdir -p "$ROOT/pion/deps"
if [ -L "$DIR" ] || [ ! -d "$DIR/.git" ]; then
  rm -rf "$DIR"
  git clone -q "$REPO" "$DIR"
else
  git -C "$DIR" fetch -q origin "$REV" || git -C "$DIR" fetch -q origin
fi
git -C "$DIR" checkout -q "$REV"
case "$(uname -s)" in MINGW*|MSYS*) sh "$DIR/packages/host/tools/fetch-webview2.sh" ;; esac
echo "deps: pion/deps/invar at $(git -C "$DIR" rev-parse --short HEAD)"
