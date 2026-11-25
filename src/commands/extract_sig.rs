//! `ApkToolKit extract-sig` — extract META-INF signatures.

use crate::core::signature_handler;
use crate::error::Result;

pub fn run(apk: &str, output_dir: &str) -> Result<()> {
    let extracted = signature_handler::extract_signatures(apk, output_dir)?;

    println!("Extracted {} signature file(s) to {}", extracted.len(), output_dir);
    for name in &extracted {
        println!("  - {name}");
    }

    Ok(())
}
