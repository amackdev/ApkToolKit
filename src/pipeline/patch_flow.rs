//! Patch flow: modify one method in one class, rebuild, optionally restore signatures.
//!
//! Uses apkscope-core for DEX parsing, smali round-trip, and APK rebuilding.

use crate::core::smali_patcher;
use crate::error::Result;
use std::fs::File;
use std::io::Write;

/// Result of a patch operation.
pub struct PatchResult {
    pub output_path: String,
    pub class_patched: String,
    pub method_patched: String,
    pub signatures_restored: bool,
}

/// Run the patch pipeline on an APK.
///
/// 1. Open the APK in place
/// 2. Export the target class to smali
/// 3. Patch the method body in smali
/// 4. Inject the modified smali back into the APK edits
/// 5. Rebuild the APK (unsigned)
/// 6. Optionally restore original META-INF signatures
pub fn run(
    input_apk: &str,
    output_apk: &str,
    class_name: &str,
    method_name: &str,
    new_method_body: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    multi_patch(
        input_apk,
        output_apk,
        &[(class_name.to_string(), method_name.to_string(), new_method_body.to_string())],
        sig_dir,
    )
}

/// Run multiple patches on the same APK in a single pass.
///
/// Each patch is (class_name, method_name, new_method_body). All patches are
/// applied to the same APK instance, so only one DEX parse + rebuild cycle
/// happens regardless of how many methods are patched.
pub fn multi_patch(
    input_apk: &str,
    output_apk: &str,
    patches: &[(String, String, String)],
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    if patches.is_empty() {
        return Err(crate::error::Error::Other("no patches specified".into()));
    }

    // Step 1: Open the APK for reading and editing
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    // Step 2: Export each class to smali, patch the method, inject back

    for (class_name, method_name, new_body) in patches {
        let mut smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
            .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

        // Patch the method body in the smali
        smali_text = smali_patcher::replace_method_body(&smali_text, method_name, new_body)?;

        // Inject the modified smali back into the APK's own classes/edits
        {
            let classes = apk.classes().map_err(|e| {
                crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
            })?;
            classes.set_text(&smali_text).map_err(|e| {
                crate::error::Error::Other(format!("Failed to set smali text: {e}"))
            })?;
        }
    }

    // Write all edits (modified DEXs) into the APK's entries map
    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    // Step 4: Rebuild the APK to a temporary file (unsigned)
    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    // Step 5: Handle signatures
    let signatures_restored = if let Some(dir) = sig_dir {
        // Restore original META-INF signatures from the backup directory
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        // Clean up temp file
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        // Just move the unsigned output to the final location
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    let last = &patches[patches.len() - 1];
    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: last.0.clone(),
        method_patched: last.1.clone(),
        signatures_restored,
    })
}

/// List all classes in an APK.
pub fn list_classes(input_apk: &str) -> Result<Vec<String>> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let dex_entries = apk.dex()
        .map_err(|e| crate::error::Error::Other(format!("Failed to read DEX: {e}")))?;

    let mut classes = Vec::new();
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
            classes.push(java_name);
        }
    }

    classes.sort();
    classes.dedup();
    Ok(classes)
}
