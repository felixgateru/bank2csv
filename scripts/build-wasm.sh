#!/usr/bin/env bash
# Builds the WASM module and JS bindings into pkg/web (ES module) and pkg/node (CommonJS).
# Requires: rustup target add wasm32-unknown-unknown
#           cargo install wasm-bindgen-cli --version 0.2.130 --locked
# Optional: wasm-opt (binaryen) for a smaller binary.
# Pass Cargo features to choose banks, e.g.: scripts/build-wasm.sh --no-default-features --features bank2csv-banks/im_bank_ke
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p bank2csv-wasm --target wasm32-unknown-unknown --release "$@"
WASM=target/wasm32-unknown-unknown/release/bank2csv_wasm.wasm
for target in web nodejs; do
  out=pkg/${target/nodejs/node}
  wasm-bindgen --target "$target" --out-dir "$out" "$WASM"
  if command -v wasm-opt >/dev/null; then
    wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int -o "$out/bank2csv_wasm_bg.wasm" "$out/bank2csv_wasm_bg.wasm"
  fi
done
ls -l pkg/web/bank2csv_wasm_bg.wasm
