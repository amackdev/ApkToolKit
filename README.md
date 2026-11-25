# ApkToolKit

Professional APK modding toolkit: extract, patch, rebuild, and sign APKs without full extraction.
Built in Rust on top of [apkscope-core](apkscope/core), providing native Windows/Linux binaries
with optional GUI (egui) and CLI.

## Features

- **No full extraction**: Open and patch APKs in place — only the classes you touch are parsed.
- **Method-level smali patching**: View and edit individual smali methods without touching the rest of the class.
- **Signature preserve mode**: Extract original `META-INF/` signatures, patch the APK, then restore the original signature block (same workflow as APKEditor's `d -t sig` / `b -t sig`).
- **Optional signing**: Sign with your own keystore (JKS/PKCS#12) or leave unsigned.
- **CLI + GUI**: Native CLI for scripting, optional egui GUI for interactive work.
- **Cross-platform**: Windows (x86_64/aarch64), Linux (glibc/musl), macOS.

## Install

```bash
cd ApkToolKit
cargo build --release
# Binary: target/release/ApkToolKit
```

With GUI:

```bash
cargo build --release --features gui
```

## CLI Usage

### Inspect an APK

```bash
ApkToolKit open SystemUI.apk
```

Shows package name, class count, and META-INF signature info.

### View a class or method as smali

```bash
# View whole class
ApkToolKit view SystemUI.apk com.android.systemui.StatusBar --smali

# View one method
ApkToolKit view SystemUI.apk com.android.systemui.StatusBar --method onCreate --smali
```

### Patch a method (no signing)

```bash
# 1. Dump the method to edit
ApkToolKit view SystemUI.apk com.android.systemui.StatusBar --method onCreate --smali > onCreate.smali

# 2. Edit onCreate.smali in your editor

# 3. Patch + rebuild (unsigned, META-INF stripped)
ApkToolKit patch SystemUI.apk SystemUI_modded.apk \
    com.android.systemui.StatusBar \
    onCreate \
    --smali-file onCreate.smali \
    --no-sign
```

### Patch and restore original signatures (APKEditor-compatible workflow)

```bash
# 1. Extract original signatures
ApkToolKit extract-sig SystemUI.apk ./sig-backup/

# 2. Dump and edit the method
ApkToolKit view SystemUI.apk com.android.systemui.StatusBar --method onCreate --smali > onCreate.smali
# (edit onCreate.smali)

# 3. Patch and restore original META-INF
ApkToolKit patch SystemUI.apk SystemUI_modded.apk \
    com.android.systemui.StatusBar \
    onCreate \
    --smali-file onCreate.smali \
    --sig-dir ./sig-backup/
```

This produces `SystemUI_modded.apk` with your patched method and the original signature block restored — matching the APKEditor `d -t sig` / `b -t sig` workflow.

### Sign with your own key

```bash
ApkToolKit patch SystemUI.apk SystemUI_signed.apk \
    com.android.systemui.StatusBar \
    onCreate \
    --smali-file onCreate.smali \
    --sign --ks platform.jks --ks-alias platform
```

## GUI

```bash
ApkToolKit gui SystemUI.apk
```

(Requires `--features gui` build.)

## Testing

Test APKs can be placed in the parent directory. `SystemUI.apk` is present at
`/home/amackpro/testing/ANDROIDAPK/SystemUI.apk`.

## Architecture

```
ApkToolKit/
├── src/
│   ├── main.rs           # Entry point
│   ├── cli.rs            # Clap CLI definition
│   ├── commands/         # Command handlers (open, view, patch, extract-sig, sig-info)
│   ├── core/             # Core modding logic (apk_reader, smali_patcher, signature_handler, meta_inf_handler)
│   ├── pipeline/         # Patch pipeline orchestration
│   ├── gui/              # egui GUI (feature-gated)
│   └── error.rs          # Error types
├── apkscope-core/        # Sibling crate: DEX, smali, resources, rebuild, signing
└── Cargo.toml
```

## License

Apache-2.0
