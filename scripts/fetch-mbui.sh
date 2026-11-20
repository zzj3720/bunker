#!/bin/sh
# Fetch mbui and its component library (libs/mbui and libs/sf-ui of AFK-surf/monoid at the commit in
# mbui.pin) into vendor/mbui and vendor/sf-ui: one shallow, blob-less fetch of that commit, those two folders
# unpacked from it. ui/moon.work names both.
set -e
R=$(cd "$(dirname "$0")/.." && pwd)
SHA=$(tr -d ' \r\n' < "$R/mbui.pin")
V=$R/vendor
if [ "$(cat "$V/.pin" 2>/dev/null)" = "$SHA" ] && [ -f "$V/mbui/moon.mod" ] && [ -f "$V/sf-ui/moon.mod" ]; then
  echo "fetch-mbui: mbui and sf-ui $SHA already there"
  exit 0
fi
G=$V/.monoid.git
mkdir -p "$V"
[ -d "$G" ] || { git init -q --bare "$G" && git -C "$G" remote add origin https://github.com/AFK-surf/monoid.git; }
git -C "$G" fetch -q --depth 1 --filter=blob:none origin "$SHA"
for lib in mbui sf-ui; do
  # (kept: what a build made in it, out/ and _build/, is the build's cache; the sources replaced)
  rm -rf "$V/$lib/src" "$V/$lib/scripts" "$V/$lib/tools"
  mkdir -p "$V/$lib"
  git -C "$G" archive FETCH_HEAD "libs/$lib" | tar -x -C "$V/$lib" --strip-components=2
done
echo "$SHA" > "$V/.pin"
echo "fetch-mbui: mbui and sf-ui $SHA in vendor/"
