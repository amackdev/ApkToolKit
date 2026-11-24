//! Read APKs in place without full extraction.
//! Uses apkscope-core's Apk abstraction.

use crate::error::Result;
use std::fs::File;
use std::path::Path;

/// Read a specific entry from the APK as raw bytes.
pub fn read_entry<P: AsRef<Path>>(apk_path: P, entry_name: &str) -> Result<Vec<u8>> {
    let file = File::open(apk_path.as_ref()).map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;
    apk.read(entry_name)
        .ok_or_else(|| crate::error::Error::Other(format!("Entry not found: {entry_name}")))
}

/// Get the original META-INF signature entries from an APK.
pub fn list_meta_inf_entries<P: AsRef<Path>>(apk_path: P) -> Result<Vec<String>> {
    let file = File::open(apk_path.as_ref()).map_err(|e| crate::error::Error::Io(e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| crate::error::Error::Zip(e))?;

    let mut meta_inf = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| crate::error::Error::Zip(e))?;
        let name = entry.name().to_owned();
        if name.starts_with("META-INF/") {
            meta_inf.push(name);
        }
    }

    Ok(meta_inf)
}

/// Read the package name from the APK's manifest using apkscope-core's XML reader.
pub fn get_package_name<P: AsRef<Path>>(apk_path: P) -> Result<String> {
    let manifest_bytes = read_entry(apk_path.as_ref(), "AndroidManifest.xml")?;
    let doc = apkscope_core::xml::read(&manifest_bytes)
        .map_err(|e| crate::error::Error::Other(format!("Failed to decode manifest: {e}")))?;

    // Find the root manifest node and its package attribute
    for node in &doc.nodes {
        if let apkscope_core::xml::Node::Start(start) = node {
            for attr in &start.attrs {
                if (attr.name as usize) < doc.strings.strings.len() {
                    let attr_name = &doc.strings.strings[attr.name as usize];
                    if attr_name == "package" {
                        // attr.value is table::Value { ty, data }; STRING ty=3 means data is string index
                        if attr.value.ty == apkscope_core::table::Value::STRING {
                            let str_idx = attr.value.data as usize;
                            if str_idx < doc.strings.strings.len() {
                                return Ok(doc.strings.strings[str_idx].clone());
                            }
                        }
                        // Fallback: raw data as string index
                        let str_idx = attr.value.data as usize;
                        if str_idx < doc.strings.strings.len() {
                            return Ok(doc.strings.strings[str_idx].clone());
                        }
                    }
                }
            }
        }
    }

    Ok("unknown".to_string())
}

/// Check if the APK has traditional signing files (CERT.RSA, MANIFEST.MF, etc.)
#[allow(dead_code)]
pub fn has_signing_files<P: AsRef<Path>>(apk_path: P) -> Result<bool> {
    let entries = list_meta_inf_entries(apk_path)?;
    Ok(entries.iter().any(|e| {
        let upper = e.to_uppercase();
        upper.ends_with(".RSA")
            || upper.ends_with(".SF")
            || upper.ends_with(".DSA")
            || upper.ends_with(".EC")
            || upper == "META-INF/MANIFEST.MF"
            || upper.contains("SIG_BLOCK")
    }))
}
