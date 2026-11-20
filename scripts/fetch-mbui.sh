#!/bin/sh
# Fetch mbui (libs/mbui of AFK-surf/monoid at the commit in mbui.pin) into vendor/monoid, sparse and shallow:
# only libs/mbui is checked out. ui/moon.work names vendor/monoid/libs/mbui.
set -e
R=$(cd "$(dirname "$0")/.." && pwd)
SHA=$(tr -d ' \r\n' < "$R/mbui.pin")
D=$R/vendor/monoid
if [ -f "$D/libs/mbui/moon.mod" ] && [ "$(git -C "$D" rev-parse HEAD 2>/dev/null)" = "$SHA" ]; then
  echo "fetch-mbui: mbui $SHA already there"
  exit 0
fi
mkdir -p "$D"
[ -d "$D/.git" ] || { git -C "$D" init -q && git -C "$D" remote add origin https://github.com/AFK-surf/monoid.git; }
git -C "$D" config core.autocrlf false
git -C "$D" sparse-checkout set --cone libs/mbui
git -C "$D" fetch -q --depth 1 --filter=blob:none origin "$SHA"
git -C "$D" checkout -q --detach FETCH_HEAD
echo "fetch-mbui: mbui $SHA in vendor/monoid/libs/mbui"
