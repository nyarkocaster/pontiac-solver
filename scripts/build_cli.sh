#!/bin/bash
# Rebuilds the command-line program and refreshes the bundled Linux copy.
set -e
cd "$(dirname "$0")/../cli"
cargo build --release
cp target/release/pontiac ../pontiac/bin/pontiac-linux-x86_64
strip ../pontiac/bin/pontiac-linux-x86_64 2>/dev/null || true
../pontiac/bin/pontiac-linux-x86_64 version
# the package ships copies of the sample spots and the guide; keep them in step
cp ../examples/*.json ../pontiac/examples/
cp ../examples/expected/river_quick.result.json ../pontiac/examples/river_quick.expected.json
cp ../docs/HEADLESS.md ../pontiac/HEADLESS.md
