#!/bin/sh
# Bunker's Pion gates (macOS / Linux): the static glass gate, then pionc check + test of the UI packages.
#   PIONC=<pionc2> [PION_QBE=… PION_RT_LIB=…] sh tools/check.sh
set -e
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
PIONC="${PIONC:-pionc}"
sh "$ROOT/tools/deps.sh"
cd "$ROOT"
echo "=== static gate: no hand-rolled Glass::liquid under pion/ (invar's own lg in pion/deps excluded) ==="
if grep -rn --exclude-dir=deps "Glass::liquid" pion/; then
  echo "FAIL: Glass::liquid outside invar's packages/lg"
  exit 1
fi
echo "0 hits"
for p in pion/packages/model pion/packages/shell pion/cmd/app pion/cmd/shots; do
  echo "=== pionc check $p ==="
  "$PIONC" check "$p"
done
for p in pion/packages/model pion/packages/shell; do
  echo "=== pionc test $p ==="
  "$PIONC" test "$p"
done
