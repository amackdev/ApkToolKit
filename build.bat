@echo off
:: Build ApkModder for Windows

echo === Building ApkModder (CLI only) ===
cargo build --release

echo.
echo === Build complete ===
echo Binary: target\release\apkmodder.exe
echo.
echo To build with GUI:
echo   cargo build --release --features gui
