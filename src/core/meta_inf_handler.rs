//! META-INF specific handling: check, list, and manage signature blocks.

use crate::error::Result;
use std::path::Path;

/// Check if META-INF exists in an APK and list its entries.
pub struct MetaInfInfo {
    pub exists: bool,
    pub entries: Vec<String>,
    pub has_v1: bool,
    pub has_v2: bool,
    #[allow(dead_code)]
    pub has_v3: bool,
}

impl MetaInfInfo {
    pub fn new() -> Self {
        Self {
            exists: false,
            entries: Vec::new(),
            has_v1: false,
            has_v2: false,
            has_v3: false,
        }
    }
}

/// Inspect META-INF of an APK.
pub fn inspect_meta_inf<P: AsRef<Path>>(apk_path: P) -> Result<MetaInfInfo> {
    let entries = crate::core::apk_reader::list_meta_inf_entries(&apk_path)?;
    let mut info = MetaInfInfo::new();
    info.exists = !entries.is_empty();
    info.entries = entries.clone();

    for entry in &entries {
        let upper = entry.to_uppercase();
        if upper.starts_with("META-INF/") {
            let rest = &upper[9..];
            if rest == "MANIFEST.MF" || rest.ends_with(".SF") || rest.ends_with(".RSA") {
                info.has_v1 = true;
            }
            if rest.ends_with(".RSA") || rest.ends_with(".DSA") || rest.ends_with(".EC") {
                info.has_v1 = true;
            }
        }
    }

    // v2/v3 signatures live in the APK signing block, not in META-INF.
    if crate::core::signature_handler::signing_block_present(&apk_path) {
        info.has_v2 = true;
        info.has_v3 = true;
    }

    Ok(info)
}

/// Print human-readable META-INF info.
pub fn format_meta_inf_info(info: &MetaInfInfo) -> String {
    let mut lines = Vec::new();
    lines.push(format!("META-INF present: {}", info.exists));
    lines.push(format!("Entries: {}", info.entries.len()));
    for entry in &info.entries {
        lines.push(format!("  - {entry}"));
    }
    lines.push(format!("v1 (JAR) signature: {}", info.has_v1));
    lines.push(format!("v2/v3 signature: {}", info.has_v2));
    lines.join("\n")
}
