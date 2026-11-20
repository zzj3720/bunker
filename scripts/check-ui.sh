#!/bin/bash
# Type-check the UI (and run its tests with T=1) against the pinned mbui, on a machine with MoonBit.
#   bash scripts/check-ui.sh            moon check (native)
#   T=1 bash scripts/check-ui.sh        and moon test (wasm-gc) of ui/src/shell
set -e
R=$(cd "$(dirname "$0")/.." && pwd)
sh "$R/scripts/fetch-mbui.sh"
if [ -z "${MBUI_TOOLS:-}" ] && [ -d "$HOME/stillfail-ci/mbui/tools" ]; then export MBUI_TOOLS="$HOME/stillfail-ci/mbui/tools"; fi
[ -d "$HOME/.moon/bin" ] && export PATH="$HOME/.moon/bin:$PATH"
. "$R/vendor/mbui/scripts/env.sh"
cd "$R/ui"
moon version
moon check --target native
if [ "${T:-0}" = 1 ]; then
  moon test --target wasm-gc --release -p bunker/shell
fi
