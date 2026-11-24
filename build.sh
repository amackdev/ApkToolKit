#!/bin/bash
# Build ApkModder for Linux

set -e

echo "=== Building ApkModder (CLI only) ==="
cargo build --release

echo ""
echo "=== Build complete ==="
echo "Binary: target/release/apkmodder"
echo ""
echo "To build with GUI:"
echo "  cargo build --release --features gui"
