#!/bin/bash
# Build ApkToolKit for Linux

set -e

echo "=== Building ApkToolKit (CLI only) ==="
cargo build --release

echo ""
echo "=== Build complete ==="
echo "Binary: target/release/ApkToolKit"
echo ""
echo "To build with GUI:"
echo "  cargo build --release --features gui"
