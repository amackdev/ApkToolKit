//! `ApkToolKit extract-resource-all` — extract all resources matching a pattern
//! from an APK/JAR into a directory.
//!
//! Uses apkscope-core's resource decoding, so binary XML under `res/`
//! and values from `resources.arsc` are decoded to human-readable text XML.
//!
//! Example: `ApkToolKit extract-resource-all SystemUI.apk bools ./out/`
//! extracts `res/values/bools.xml`, `res/values-night/bools.xml`, etc.
//! as decoded text XML (not raw binary XML bytes).

use crate::error::Result;
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn run(input: &str, pattern: &str, output_dir: &str) -> Result<()> {
    let mut file = std::fs::File::open(input)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(&mut file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let paths = apk
        .files()
        .map_err(|e| crate::error::Error::Other(format!("Failed to list APK files: {e}")))?;

    let matches: Vec<String> = paths
        .into_iter()
        .filter(|path| path.contains(pattern))
        .collect();

    if matches.is_empty() {
        eprintln!(
            "extract-resource-all: no entries matching '{pattern}' found in {input}"
        );
        return Ok(());
    }

    fs::create_dir_all(output_dir).map_err(|e| crate::error::Error::Io(e))?;

    let mut extracted = 0usize;
    for path in &matches {
        let data = apk
            .file(path)
            .map_err(|e| crate::error::Error::Other(format!("Failed to read {path}: {e}")))?
            .ok_or_else(|| crate::error::Error::Other(format!("Entry not found: {path}")))?;

        // Preserve the directory structure relative to output_dir.
        let out_path = Path::new(output_dir).join(path);

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| crate::error::Error::Io(e))?;
        }

        let mut out_file = std::fs::File::create(&out_path)
            .map_err(|e| crate::error::Error::Io(e))?;
        out_file.write_all(&data).map_err(|e| crate::error::Error::Io(e))?;

        extracted += 1;
        eprintln!("extract-resource-all: extracted {path} -> {out_path:?}");
    }

    eprintln!(
        "extract-resource-all: extracted {extracted} entr{} matching '{pattern}' into {output_dir}",
        if extracted == 1 { "y" } else { "ies" }
    );

    Ok(())
}
