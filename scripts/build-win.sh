#!/bin/bash
# Build Bunker for Windows in Git Bash: MSVC's environment, mbui fetched (mbui.pin), the core (Rust, a static library
# linked into the app), then the app through mbui's own scripts/win.sh (MBUI_APP_MOD: an app of a module outside mbui).
#   MODE=release|debug scripts/build-win.sh            -> out/win/Bunker.exe
set -e
R=$(cd "$(dirname "$0")/.." && pwd)
cd "$R"
MODE=${MODE:-release}
. scripts/msvc-env.sh
# mbui's C/C++ is UTF-8: on a machine whose code page is not (936 on a Chinese Windows) cl misreads it (C4819) and a
# comment's last character can swallow the line after it. (-, not /: Git Bash would turn /utf-8 into a path.)
export CL="-utf-8 ${CL:-}"
[ -d "$HOME/.moon/bin" ] && export PATH="$HOME/.moon/bin:$PATH"
[ -d "$HOME/.cargo/bin" ] && export PATH="$HOME/.cargo/bin:$PATH"
sh scripts/fetch-mbui.sh
moon version
if [ -f core/Cargo.toml ]; then
  (cd core && cargo build --release --lib)
  export LIB="$(cygpath -w "$R/core/target/release");$LIB"
fi
MBUI=vendor/mbui
# WebView2's SDK where mbui's win.sh looks for it (its own unpacking needs a Python, which a Windows machine may lack)
WV2_VERSION=$(sed -n 's/^WV2_VERSION=//p' "$MBUI/scripts/win.sh")
WV2="$MBUI/out/webview2/$WV2_VERSION"
if [ ! -f "$WV2/build/native/include/WebView2.h" ]; then
  mkdir -p "$WV2"
  curl -sSfL --retry 3 -o "$WV2.nupkg" "https://www.nuget.org/api/v2/package/Microsoft.Web.WebView2/$WV2_VERSION"
  unzip -q -o "$WV2.nupkg" -d "$WV2"
  rm -f "$WV2.nupkg"
fi
MBUI_APP_MOD="$R/ui" APP=app MODE=$MODE bash "$MBUI/scripts/win.sh" build
mkdir -p out/win
cp -p "$MBUI/out/win/app/_build/native/$MODE/build/bunker/app/app.exe" out/win/Bunker.exe
ls -la out/win/Bunker.exe
# MSVC's telemetry uploader (vctip.exe), started by cl and link, outlives the build and holds a task open: ended here.
taskkill //F //IM vctip.exe >/dev/null 2>&1 || true
