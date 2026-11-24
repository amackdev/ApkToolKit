//! `apkmodder discover-text` — find which classes contain a given text string.
//! Searches raw DEX bytes without full smali parse — fast for large JARs.

use crate::error::Result;
use std::fs::File;

fn contains_slice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

pub fn run(input: &str, text: &str) -> Result<()> {
    let file = File::open(input)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let dex_entries = apk.dex().map_err(|e| {
        crate::error::Error::Other(format!("Failed to read DEX: {e}"))
    })?;

    let mut found = Vec::new();
    for (_dex_name, dex_bytes) in &dex_entries {
        let dex = apkscope_core::dex::Dex::parse(dex_bytes)
            .map_err(|e| crate::error::Error::Other(format!("DEX parse error: {e}")))?;
        for c in 0..dex.class_count {
            let desc = dex.class_descriptor(c).map_err(|e| {
                crate::error::Error::Other(format!("Class descriptor error: {e}"))
            })?;
            let desc_str = apkscope_core::dex::descriptor(&desc);
            let java_name = desc_str
                .trim_start_matches('L')
                .trim_end_matches(';')
                .replace('/', ".");

            if contains_slice(dex_bytes, text.as_bytes()) {
                found.push(java_name);
            }
        }
    }

    if found.is_empty() {
        println!("No classes contain text '{}'", text);
    } else {
        for class in &found {
            println!("{}", class);
        }
    }

    Ok(())
}
