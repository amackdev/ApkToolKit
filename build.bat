@echo off
:: Build ApkToolKit for Windows

echo === Building ApkToolKit (CLI only) ===
cargo build --release

echo.
echo === Build complete ===
echo Binary: target\release\ApkToolKit.exe
echo.
echo To build with GUI:
echo   cargo build --release --features gui
