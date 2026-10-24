#!/bin/sh
# Build the WebAssembly build and assemble the static site in web/.
#   cargo build --release --target wasm32-unknown-unknown
# The compiled wasm is copied next to index.html + gl.js, ready to serve:
#   npx http-server web -p 8080     (or: basic-http-server web)
set -e

cargo build --release --target wasm32-unknown-unknown
mkdir -p web
cp target/wasm32-unknown-unknown/release/alien-space-drop.wasm web/alien_space_drop.wasm
ls -la web/
echo "Web build ready in web/ - serve it over http and open index.html"