#!/bin/bash
# Builds the Windows desktop app and the Windows command-line program (cloud workspace recipe).
set -e
export PATH=/usr/lib/rust-1.97/bin:$PATH
export __CARGO_TESTS_ONLY_SRC_ROOT=/home/claude/rust-src/library RUSTC_BOOTSTRAP=1
ROOT="$(cd "$(dirname "$0")" && pwd)"
cd "$ROOT/app"
[ -d node_modules ] || npm install --no-audit --no-fund
npm run build
cd src-tauri
cargo build --release -Zbuild-std=std,panic_abort --target x86_64-pc-windows-gnu
cd "$ROOT/cli"
cargo build --release -Zbuild-std=std,panic_abort --target x86_64-pc-windows-gnu --config 'target.x86_64-pc-windows-gnu.linker="x86_64-w64-mingw32-gcc"'
ls -la "$ROOT"/app/src-tauri/target/x86_64-pc-windows-gnu/release/*.exe "$ROOT"/cli/target/x86_64-pc-windows-gnu/release/*.exe
