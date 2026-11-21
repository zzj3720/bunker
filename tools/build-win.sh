#!/bin/sh
# Builds standalone Bunker.exe on Windows (Git Bash) using official AFK-surf/invar + LLVM clang MSVC + Rust bunker_core.lib.
set -e
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
OUT="$ROOT/out"
mkdir -p "$OUT"
LLVM_BIN="${LLVM_BIN:-/c/Users/3720/Apps/llvm/bin}"
PION_SRC="${PION_SRC:-/c/Users/3720/Apps/pionws/pion}"
PIONC="${PIONC:-$PION_SRC/out/pionc.exe}"
export PION_QBE="${PION_QBE:-$PION_SRC/out/qbe.exe}"
export PION_CLANG="$(cygpath -w "$LLVM_BIN/clang.exe")"

sh "$ROOT/tools/deps.sh"

cd "$ROOT"
echo "=== Building Rust bunker_core (x86_64-pc-windows-msvc) ==="
(cd "$ROOT/core" && cargo build --release)

echo "=== Building win-cc wrapper & pion runtime with LLVM clang MSVC ==="
"$LLVM_BIN/clang.exe" -O2 tools/win-cc.c -o "$OUT/win-cc.exe"
cp "$OUT/win-cc.exe" "$OUT/win-c++.exe"
"$LLVM_BIN/clang.exe" -std=c11 -O2 -fno-asynchronous-unwind-tables -D_CRT_SECURE_NO_WARNINGS -c "$PION_SRC/runtime/pion_rt.c" -o "$OUT/pion_rt.o"
rm -f "$OUT/libpionrt.a"
"$LLVM_BIN/llvm-ar.exe" rcs "$OUT/libpionrt.a" "$OUT/pion_rt.o"

export CC="$OUT/win-cc.exe"
export CXX="$OUT/win-c++.exe"
export PION_RT_LIB="$OUT/libpionrt.a"
export PION_TARGET=amd64_win

echo "=== Building Bunker.exe with pionc + official AFK-surf/invar ==="
"$PIONC" build --release -o "$OUT/Bunker.exe" pion/cmd/app
ls -lh "$OUT/Bunker.exe"
