# ApkToolKit

Professional APK modding toolkit: extract, patch, rebuild, and sign APKs without full extraction.
Built in Rust on top of [apkscope-core](apkscope/core), providing native Windows/Linux binaries
with optional GUI (egui) and CLI.

## Features

- **No full extraction**: Open and patch APKs in place — only the classes you touch are parsed.
- **Method-level smali patching**: View and edit individual smali methods without touching the rest of the class. Supports `--smali-file` (file or stdin) and `--smali-body` (inline).
- **Bulk smali operations**: Extract all classes as smali files (`extract-smali-all`), inject a directory of smali files back (`inject-smali-all`), or add a single new smali class (`add-smali`).
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

## Smali Editing

ApkToolKit works directly on smali (DEX bytecode in text form). You never need to fully decompile/recompile the APK — only the methods/classes you touch are rewritten.

### View smali

View a whole class or a single method. Output format defaults to smali; use `--format java` for decompiled Java.

```bash
# Whole class as smali
ApkToolKit view SystemUI.apk com.android.systemui.StatusBar --smali

# One method as smali
ApkToolKit view SystemUI.apk com.android.systemui.StatusBar --method onCreate --smali

# Dump a method to a file for editing
ApkToolKit view SystemUI.apk com.android.systemui.StatusBar --method onCreate --smali > onCreate.smali
```

### Patch a method from a smali file

```bash
# 1. Dump the method
ApkToolKit view SystemUI.apk com.android.systemui.StatusBar --method onCreate --smali > onCreate.smali

# 2. Edit onCreate.smali (change smali instructions, registers, strings, etc.)

# 3. Patch + rebuild (unsigned, META-INF stripped)
ApkToolKit patch SystemUI.apk SystemUI_modded.apk \
    com.android.systemui.StatusBar \
    onCreate \
    --smali-file onCreate.smali \
    --no-sign
```

You can also pipe a smali file from stdin:

```bash
cat onCreate.smali | ApkToolKit patch SystemUI.apk SystemUI_modded.apk \
    com.android.systemui.StatusBar \
    onCreate \
    --smali-file - \
    --no-sign
```

### Patch a method with inline smali body

Use `--smali-body` to pass the method body directly (without the `.method`/`.end method` wrapper lines):

```bash
ApkToolKit patch SystemUI.apk SystemUI_modded.apk \
    com.android.systemui.StatusBar \
    onCreate \
    --smali-body 'const/4 v0, 0x1\nreturn v0' \
    --no-sign
```

### Patch multiple methods at once (multi-patch)

`multi-patch` lets you change several methods in a single rebuild pass. Each patch spec is `Class#method=smali_file` or `Class#method=smali_body`.

```bash
ApkToolKit multi-patch SystemUI.apk SystemUI_modded.apk \
    --patch com.android.systemui.StatusBar#onCreate=onCreate.smali \
    --patch com.android.systemui.StatusBar#onResume=onResume.smali \
    --patch_body com.android.systemui.StatusBar#onPause='const/4 v0, 0x0\nreturn v0' \
    --no-sign
```

### Add / extract / inject whole smali classes

Add a brand-new smali class as a DEX entry:

```bash
ApkToolKit add-smali SystemUI.apk SystemUI_modded.apk \
    ./MyNewClass.smali \
    --no-sign
```

Extract every class in an APK to individual `.smali` files:

```bash
ApkToolKit extract-smali-all SystemUI.apk ./smali_out/
```

Inject a directory of modified smali files back into an APK:

```bash
ApkToolKit inject-smali-all SystemUI.apk SystemUI_modded.apk \
    ./smali_out/ \
    --no-sign
```

Extract or inject a single class:

```bash
# Extract one class to a smali file
ApkToolKit extract-smali-class SystemUI.apk com.android.systemui.StatusBar ./StatusBar.smali

# Inject one modified smali class back
ApkToolKit inject-smali-class SystemUI.apk SystemUI_modded.apk \
    com.android.systemui.StatusBar \
    ./StatusBar.smali \
    --no-sign
```

### Smali quick reference

- **Registers**: local variables are `v0`, `v1`, … (wide types like `double`/`long` take two registers).
- **Method signature**: `Lcom/android/systemui/StatusBar;->onCreate(Landroid/os/Bundle;)V`
  - `L<class>;-><method>(<args>)<return>`
  - `V` = void, `Z` = boolean, `B` = byte, `S` = short, `C` = char, `I` = int, `J` = long, `F` = float, `D` = double, `[` = array, `L<class>;` = object.
- **Body wrapper**: a smali method body passed to `--smali-body`/`--smali-file` should contain the instructions between `.method` and `.end method` (those wrapper lines are added automatically for inline bodies).
- **Labels**: branch targets use `:`-prefixed labels (e.g. `:cond_0`).
- **String constants**: `const-string v0, "hello world"`.
- **Field access**: `sget`/`sput` (static), `iget`/`iput` (instance), `sget-object`/`iput-object`, etc.
- **Method invocation**: `invoke-virtual`, `invoke-static`, `invoke-direct`, `invoke-super`, `invoke-interface`, `invoke-virtual/range`, etc.

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
├── Cargo.toml            # Package manifest (name = "ApkToolKit")
├── Cargo.lock
├── build.sh              # Linux build script
├── build.bat             # Windows build script
├── .gitignore
├── src/
│   ├── main.rs           # Binary entry point (CLI dispatch)
│   ├── cli.rs            # Clap CLI definition + print_version / print_usage
│   ├── error.rs          # Error types
│   ├── commands/         # Command handlers
│   │   ├── mod.rs
│   │   ├── open.rs           # ApkToolKit open — inspect an APK
│   │   ├── view.rs           # ApkToolKit view — view class/method as smali/Java
│   │   ├── patch.rs          # ApkToolKit patch — patch a method and rebuild
│   │   ├── multi_patch.rs    # ApkToolKit multi-patch — patch multiple methods
│   │   ├── append_method.rs  # ApkToolKit append-method — append a new method
│   │   ├── add_smali.rs      # ApkToolKit add-smali — add a smali class as DEX entry
│   │   ├── extract_smali_class.rs   # extract one class as smali
│   │   ├── inject_smali_class.rs    # inject one modified smali class
│   │   ├── extract_resource_all.rs  # extract all resources matching a pattern
│   │   ├── put_resource_all.rs      # put all resources back into APK/JAR
│   │   ├── extract_entry.rs / put_entry.rs  # single entry extract/put
│   │   ├── extract_libs.rs          # extract native libraries (.so)
│   │   ├── extract_sig.rs / sig_info.rs  # META-INF signature extract/info
│   │   ├── replace_text.rs / replace_text_class.rs / replace_text_global.rs
│   │   ├── replace_bytes_global.rs  # raw byte find/replace across DEX entries
│   │   ├── discover_method.rs / discover_text.rs
│   │   ├── insert_before.rs / insert_after.rs / remove_lines.rs
│   │   └── gui.rs            # ApkToolKit gui — launch the GUI viewer
│   ├── core/             # Core modding logic
│   │   ├── mod.rs
│   │   ├── apk_reader.rs       # APK/JAR reading (DEX, classes, entries)
│   │   ├── smali_patcher.rs    # smali method patching logic
│   │   ├── signature_handler.rs # signing / signature restore
│   │   └── meta_inf_handler.rs  # META-INF block handling
│   ├── pipeline/         # Patch pipeline orchestration
│   │   ├── mod.rs
│   │   └── patch_flow.rs
│   ├── gui/              # egui GUI (feature-gated)
│   │   ├── mod.rs
│   │   └── app.rs
└── apkscope-core/        # Sibling crate: DEX, smali, resources, rebuild, signing
```

## License

Apache-2.0
