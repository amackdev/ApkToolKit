//! `ApkToolKit extract-libs` — extract native libraries (.so) from an APK/JAR.
//! Maps arch directories: arm64-v8a→arm64, armeabi-v7a→arm, x86_64→x64, x86→x86.

use crate::error::Result;
use std::fs;
use std::path::Path;

/// Map a DEX/native arch directory name to the short form used by the porting tool.
fn arch_dir(name: &str) -> Option<&'static str> {
    match name {
        "arm64-v8a" => Some("arm64"),
        "armeabi-v7a" => Some("arm"),
        "x86_64" => Some("x64"),
        "x86" => Some("x86"),
        _ => None,
    }
}

pub fn run(input: &str, output_dir: &str) -> Result<()> {
    let file = fs::File::open(input).map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    fs::create_dir_all(output_dir).map_err(|e| crate::error::Error::Io(e))?;

    // Collect native library entry names first (avoids borrow conflict with apk.read).
    let lib_entries: Vec<String> = apk
        .zip
        .file_names()
        .flatten()
        .map(|n| n.into_owned())
        .filter(|n| n.starts_with("lib/") && n.ends_with(".so"))
        .filter(|n| n.split('/').count() == 3)
        .collect();

    let mut count = 0u64;
    for name in lib_entries {
        let parts: Vec<&str> = name.split('/').collect();
        let arch = parts[1];
        let soname = parts[2];

        let short_arch = arch_dir(arch).unwrap_or(arch);
        let dest_dir = Path::new(output_dir).join(format!("lib/{}", short_arch));
        fs::create_dir_all(&dest_dir).map_err(|e| crate::error::Error::Io(e))?;

        let data = apk.read(&name).ok_or_else(|| {
            crate::error::Error::Other(format!("Failed to read entry {name}"))
        })?;

        let dest_path = dest_dir.join(soname);
        fs::write(&dest_path, &data).map_err(|e| crate::error::Error::Io(e))?;
        count += 1;
    }

    println!("Extracted {} native library(.so) files from {} -> {}", count, input, output_dir);
    Ok(())
}
