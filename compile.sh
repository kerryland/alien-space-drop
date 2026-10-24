#!/bin/sh
# Build ALIEN SPACE DROP as a standalone Windows executable.
#   ./compile.sh           -> release build (default)
#   ./compile.sh debug     -> debug build
# The exe is copied to dist/alien-space-drop.exe
set -e

if [ "$1" = "debug" ]; then
    cargo build
    BIN=target/debug/alien-space-drop.exe
else
    cargo build --release
    BIN=target/release/alien-space-drop.exe
fi

mkdir -p dist
cp -f "$BIN" dist/alien-space-drop.exe
echo "Built dist/alien-space-drop.exe"