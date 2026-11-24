#!/bin/sh
# Fetches pinned official AFK-surf/invar (main a60d978) into pion/deps/invar.
set -e
REV=a60d978ac4464aeec1c36a3e9d0bb095a24cd37a
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
