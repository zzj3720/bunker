#!/bin/sh
# Fetches pinned official AFK-surf/invar (main d952efe) into pion/deps/invar.
set -e
REV=d952efe21486eac01dbf6efb459d8e0284eb4b36
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
