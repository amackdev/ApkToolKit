//! Patch flow: modify one method in one class, rebuild, optionally restore signatures.
//!
//! Uses apkscope-core for DEX parsing, smali round-trip, and APK rebuilding.
//!
//! All operations work in patch mode: the APK/JAR is opened once, targeted
//! classes are exported to smali, edited in memory, and injected back — no
//! full extract/repack cycle, no temporary smali directory.

use crate::core::smali_patcher;
use crate::error::Result;
use std::collections::BTreeMap;
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
/// Each patch is (class_name, method_name, new_method_body). Patches targeting
/// the same class are applied sequentially to one smali export, then injected
/// once — so multiple patches on one class don't overwrite each other. Only one
/// DEX parse + rebuild cycle happens regardless of how many methods are patched.
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

    // Step 2: Group patches by class so multiple patches on the same class
    // are applied sequentially to one smali export.
    let mut by_class: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (class_name, method_name, new_body) in patches {
        by_class
            .entry(class_name.clone())
            .or_default()
            .push((method_name.clone(), new_body.clone()));
    }

    for (class_name, method_patches) in &by_class {
        let mut smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
            .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

        // Apply each method patch to the same smali text sequentially
        for (method_name, new_body) in method_patches {
            smali_text = smali_patcher::replace_method_body(&smali_text, method_name, new_body)
                .map_err(|e| {
                    crate::error::Error::Other(format!("Patch error for {class_name}#{method_name}: {e}"))
                })?;
        }

        // Inject the modified smali back into the APK's own classes/edits (once per class)
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

/// Replace `old` with `new` inside one method of one class, in place.
pub fn replace_text(
    input_apk: &str,
    output_apk: &str,
    class_name: &str,
    method_name: &str,
    old: &str,
    new: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let mut smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
        .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

    smali_text = smali_patcher::replace_text_in_method(&smali_text, method_name, old, new)
        .map_err(|e| crate::error::Error::Other(format!("Replace-text error for {class_name}#{method_name}: {e}")))?;

    {
        let classes = apk.classes().map_err(|e| {
            crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
        })?;
        classes.set_text(&smali_text).map_err(|e| {
            crate::error::Error::Other(format!("Failed to set smali text: {e}"))
        })?;
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: class_name.to_string(),
        method_patched: method_name.to_string(),
        signatures_restored,
    })
}

/// Insert `text` after the first line containing `anchor` inside a method.
pub fn insert_after(
    input_apk: &str,
    output_apk: &str,
    class_name: &str,
    method_name: &str,
    anchor: &str,
    text: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let mut smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
        .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

    smali_text = smali_patcher::insert_after_line(&smali_text, method_name, anchor, text)
        .map_err(|e| crate::error::Error::Other(format!("Insert-after error for {class_name}#{method_name}: {e}")))?;

    {
        let classes = apk.classes().map_err(|e| {
            crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
        })?;
        classes.set_text(&smali_text).map_err(|e| {
            crate::error::Error::Other(format!("Failed to set smali text: {e}"))
        })?;
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: class_name.to_string(),
        method_patched: method_name.to_string(),
        signatures_restored,
    })
}

/// Insert `text` before the first line containing `anchor` inside a method.
pub fn insert_before(
    input_apk: &str,
    output_apk: &str,
    class_name: &str,
    method_name: &str,
    anchor: &str,
    text: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let mut smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
        .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

    smali_text = smali_patcher::insert_before_line(&smali_text, method_name, anchor, text)
        .map_err(|e| crate::error::Error::Other(format!("Insert-before error for {class_name}#{method_name}: {e}")))?;

    {
        let classes = apk.classes().map_err(|e| {
            crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
        })?;
        classes.set_text(&smali_text).map_err(|e| {
            crate::error::Error::Other(format!("Failed to set smali text: {e}"))
        })?;
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: class_name.to_string(),
        method_patched: method_name.to_string(),
        signatures_restored,
    })
}

/// Remove all lines containing `pattern` inside a method.
pub fn remove_lines(
    input_apk: &str,
    output_apk: &str,
    class_name: &str,
    method_name: &str,
    pattern: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let mut smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
        .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

    smali_text = smali_patcher::remove_lines_matching(&smali_text, method_name, pattern)
        .map_err(|e| crate::error::Error::Other(format!("Remove-lines error for {class_name}#{method_name}: {e}")))?;

    {
        let classes = apk.classes().map_err(|e| {
            crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
        })?;
        classes.set_text(&smali_text).map_err(|e| {
            crate::error::Error::Other(format!("Failed to set smali text: {e}"))
        })?;
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: class_name.to_string(),
        method_patched: method_name.to_string(),
        signatures_restored,
    })
}

/// Append a new method to a class.
pub fn append_method(
    input_apk: &str,
    output_apk: &str,
    class_name: &str,
    method_decl: &str,
    body: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let mut smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
        .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

    smali_text = smali_patcher::append_method(&smali_text, method_decl, body)
        .map_err(|e| crate::error::Error::Other(format!("Append-method error for {class_name}: {e}")))?;

    {
        let classes = apk.classes().map_err(|e| {
            crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
        })?;
        classes.set_text(&smali_text).map_err(|e| {
            crate::error::Error::Other(format!("Failed to set smali text: {e}"))
        })?;
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: class_name.to_string(),
        method_patched: String::new(),
        signatures_restored,
    })
}

/// Replace `old` with `new` in all method bodies of a specific class.
pub fn replace_text_class(
    input_apk: &str,
    output_apk: &str,
    class_name: &str,
    old: &str,
    new: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let mut smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
        .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

    smali_text = replace_text_in_all_methods(&smali_text, old, new)
        .map_err(|e| crate::error::Error::Other(format!("Replace-text-class error for {class_name}: {e}")))?;

    {
        let classes = apk.classes().map_err(|e| {
            crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
        })?;
        classes.set_text(&smali_text).map_err(|e| {
            crate::error::Error::Other(format!("Failed to set smali text: {e}"))
        })?;
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: class_name.to_string(),
        method_patched: String::new(),
        signatures_restored,
    })
}

/// Replace `old` with `new` in all method bodies across all classes.
/// This is the patch-mode equivalent of `grep -lr PATTERN | xargs sed -i "s/OLD/NEW/g"`.
///
/// Parallelism: class export + smali modification run across threads (each thread opens
/// its own file handle). Injection back into the APK is sequential because `Classes`
/// holds shared mutable DEX-edit state.
/// Replace text in all methods of all classes (global scoped sed).
/// Optimized: only exports/re-injects classes whose DEX bytes contain the target text,
/// avoiding full-class smali parse for large JARs like framework.jar.
pub fn replace_text_global(
    input_apk: &str,
    output_apk: &str,
    old: &str,
    new: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    // Phase 1: Collect all class names (single open, DEX parsed once).
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;
    let dex_entries = apk.dex().map_err(|e| {
        crate::error::Error::Other(format!("Failed to read DEX: {e}"))
    })?;
    let class_names: Vec<String> = {
        let mut names = Vec::new();
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
                names.push(java_name);
            }
        }
        names.sort();
        names.dedup();
        names
    };

    if class_names.is_empty() {
        return Err(crate::error::Error::Other("no classes found in APK".into()));
    }

    // Phase 2: Collect candidate classes via per-class DEX byte pre-filter.
    // For each class, scan its method bytecode for `old_bytes` (hex constants
    // as little-endian bytes, or plain UTF-8 text). Only classes whose code
    // sections contain the target bytes get exported to smali — this avoids
    // parsing 70k+ classes when only a handful contain the target text.
    let old_bytes = decode_search_bytes(old);
    let mut candidate_classes: Vec<String> = Vec::new();
    for (_dex_name, dex_bytes) in &dex_entries {
        let dex = apkscope_core::dex::Dex::parse(dex_bytes)
            .map_err(|e| crate::error::Error::Other(format!("DEX parse error: {e}")))?;
        for c in 0..dex.class_count {
            let class_data = dex.class_data(c).map_err(|e| {
                crate::error::Error::Other(format!("Class data error: {e}"))
            })?;
            let desc = dex.class_descriptor(c).map_err(|e| {
                crate::error::Error::Other(format!("Class descriptor error: {e}"))
            })?;
            let desc_str = apkscope_core::dex::descriptor(&desc);
            let java_name = desc_str
                .trim_start_matches('L')
                .trim_end_matches(';')
                .replace('/', ".");
            // Scan method code sections for target bytes
            let mut found = false;
            for method in &class_data.methods {
                if let Some(code) = &method.code {
                    if contains_slice_le(code.insns, &old_bytes) {
                        found = true;
                        break;
                    }
                }
            }
            if found {
                candidate_classes.push(java_name);
            }
        }
    }

    if candidate_classes.is_empty() {
        eprintln!(
            "replace-text-global: text '{}' not found in any class — nothing patched",
            old
        );
        if input_apk != output_apk {
            std::fs::copy(input_apk, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        }
        return Ok(PatchResult {
            output_path: output_apk.to_string(),
            class_patched: String::new(),
            method_patched: String::new(),
            signatures_restored: false,
        });
    }

    eprintln!(
        "replace-text-global: {} class(es) contain target bytes — exporting smali",
        candidate_classes.len()
    );

    // Phase 3: Export only candidate classes to smali in parallel, then grep-style
    // search the smali TEXT for the target string (authoritative check).
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, candidate_classes.len());
    let chunk_size = (candidate_classes.len() + num_threads - 1) / num_threads;
    let chunks: Vec<Vec<String>> = candidate_classes
        .chunks(chunk_size)
        .map(|c| c.to_vec())
        .collect();

    let modified: Vec<(String, String)> = {
        let handles: Vec<_> = chunks
            .into_iter()
            .map(|chunk| {
                let input_apk = input_apk.to_string();
                let old = old.to_string();
                let new = new.to_string();
                std::thread::spawn(move || -> Result<Vec<(String, String)>> {
                    let file = File::open(&input_apk)
                        .map_err(|e| crate::error::Error::Io(e))?;
                    let mut apk = apkscope_core::apk::Apk::new(file)
                        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;
                    let mut out = Vec::with_capacity(chunk.len());
                    let mut skip_count = 0usize;
                    for class_name in &chunk {
                        let smali_text = match apkscope_core::smali::export_class_from(&mut apk, class_name) {
                            Ok(s) => s,
                            Err(e) => {
                                skip_count += 1;
                                continue;
                            }
                        };

                        // Grep-style: does this class's smali text contain `old`?
                        if !smali_text.contains(&old) {
                            continue;
                        }

                        let replaced = match replace_text_in_all_methods(&smali_text, &old, &new) {
                            Ok(s) => s,
                            Err(e) => {
                                skip_count += 1;
                                continue;
                            }
                        };
                        out.push((class_name.clone(), replaced));
                    }
                    if skip_count > 0 {
                        eprintln!("replace-text-global: warning — skipped {skip_count} class(es) in this chunk (export errors)");
                    }
                    Ok(out)
                })
            })
            .collect();

        let mut results = Vec::new();
        let mut hit_count = 0usize;
        for h in handles {
            let join_result = h.join().map_err(|_| {
                crate::error::Error::Other("thread panic during parallel export".into())
            })?;
            let chunk_results = join_result?;
            hit_count += chunk_results.len();
            for (class_name, smali) in chunk_results {
                results.push((class_name, smali));
            }
        }
        eprintln!(
            "replace-text-global: {} class(es) contain target text — modifying",
            hit_count
        );
        results
    };

    if modified.is_empty() {
        // No classes contained the target text — nothing to patch.
        eprintln!(
            "replace-text-global: text '{}' not found in any class — nothing patched",
            old
        );
        if input_apk != output_apk {
            std::fs::copy(input_apk, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        }
        return Ok(PatchResult {
            output_path: output_apk.to_string(),
            class_patched: String::new(),
            method_patched: String::new(),
            signatures_restored: false,
        });
    }

    // Phase 3: Sequential inject (single open, shared `Classes` state).
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;
    let classes = apk.classes().map_err(|e| {
        crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
    })?;
    for (class_name, smali_text) in &modified {
        classes.set_text(smali_text).map_err(|e| {
            crate::error::Error::Other(format!("Failed to set smali text for {class_name}: {e}"))
        })?;
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: String::new(),
        method_patched: String::new(),
        signatures_restored,
    })
}
pub fn replace_bytes_global(
    input_apk: &str,
    output_apk: &str,
    old_hex: &str,
    new_hex: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    /// Decode a `0x`-prefixed hex constant into little-endian DEX bytes.
    /// Returns None if the input is not a valid `0x` hex constant.
    fn decode_0x_hex(s: &str) -> Option<Vec<u8>> {
        let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
        if s.is_empty() {
            return None;
        }
        // Pad odd-length hex with a leading zero.
        let padded = if s.len() % 2 != 0 {
            format!("0{}", s)
        } else {
            s.to_string()
        };
        let bytes: Vec<u8> = (0..padded.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&padded[i..i + 2], 16).ok())
            .collect::<Option<Vec<u8>>>()?;
        // Reverse for little-endian DEX byte order.
        let mut le = bytes;
        le.reverse();
        Some(le)
    }

    /// Try to decode input as bytes. Priority:
    /// 1. `0x`-prefixed hex → little-endian bytes
    /// 2. Raw even-length hex → raw bytes
    /// 3. Otherwise → UTF-8 text bytes
    fn try_decode_bytes(s: &str) -> Vec<u8> {
        if s.starts_with("0x") || s.starts_with("0X") {
            return decode_0x_hex(s).unwrap_or_else(|| s.as_bytes().to_vec());
        }
        if s.len() % 2 == 0 && !s.is_empty() {
            if let Some(bytes) = (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
                .collect::<Option<Vec<u8>>>()
            {
                return bytes;
            }
        }
        s.as_bytes().to_vec()
    }

    let old = try_decode_bytes(old_hex);
    let new = try_decode_bytes(new_hex);
    if old.len() != new.len() {
        return Err(crate::error::Error::Other(
            "old and new must have the same length (in bytes)".into(),
        ));
    }

    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let dex_entries = apk.dex().map_err(|e| {
        crate::error::Error::Other(format!("Failed to read DEX: {e}"))
    })?;

    for (dex_name, dex_bytes) in &dex_entries {
        let mut modified = dex_bytes.clone();
        let mut pos = 0;
        while pos <= modified.len().saturating_sub(old.len()) {
            if modified[pos..pos + old.len()] == old[..] {
                modified[pos..pos + old.len()].copy_from_slice(&new[..]);
                pos += old.len();
            } else {
                pos += 1;
            }
        }

        if modified != *dex_bytes {
            apk.put_entry(dex_name, modified).map_err(|e| {
                crate::error::Error::Other(format!("Failed to put DEX entry {dex_name}: {e}"))
            })?;
        }
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: String::new(),
        method_patched: String::new(),
        signatures_restored,
    })
}

/// Add a smali source file as a new DEX class entry in the APK/JAR.
/// Determines the next classesN.dex number, assembles the smali into DEX,
/// and puts it as a new entry.
pub fn add_smali(
    input_apk: &str,
    output_apk: &str,
    smali_file: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    let smali_text = std::fs::read_to_string(smali_file)
        .map_err(|e| crate::error::Error::Io(e))?;

    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    // Prefer reusing an existing DEX that has free capacity, to avoid creating
    // a new classesN.dex for every added class.
    let dex_entries = apk.dex().map_err(|e| {
        crate::error::Error::Other(format!("Failed to read DEX: {e}"))
    })?;

    let mut reused = false;
    for (idx, (_name, bytes)) in dex_entries.iter().enumerate() {
        if let Ok(dex) = apkscope_core::dex::Dex::parse(bytes) {
            let counts = dex.counts();
            // Conservative thresholds: leave headroom for the new class's tables.
            if dex.class_count < 64000
                && counts[3] < 64000
                && counts[0] < 64000
                && counts[1] < 64000
            {
                let classes = apk.classes().map_err(|e| {
                    crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
                })?;
                classes.set_text_in(idx, &smali_text).map_err(|e| {
                    crate::error::Error::Other(format!("Failed to add smali to DEX {idx}: {e}"))
                })?;
                reused = true;
                break;
            }
        }
    }

    if !reused {
        // Fall back to creating a new classesN.dex entry.
        let mut max_dex = 0;
        for (name, _) in &dex_entries {
            let num: u32 = name
                .trim_start_matches("classes")
                .trim_end_matches(".dex")
                .parse()
                .unwrap_or(1);
            if num > max_dex {
                max_dex = num;
            }
        }
        let next_dex = format!("classes{}.dex", max_dex + 1);

        // Determine DEX version from existing DEX
        let dex_version = dex_entries
            .first()
            .and_then(|(_, bytes)| bytes.get(4..7))
            .map(|v| [v[0], v[1], v[2]])
            .unwrap_or(*b"035");

        // Assemble smali into DEX bytes
        let dex_bytes = apkscope_core::smali::assemble(&[&smali_text], dex_version)
            .map_err(|e| crate::error::Error::Other(format!("Smali assemble error: {e}")))?;

        // Put the new DEX as an entry
        apk.put_entry(&next_dex, dex_bytes).map_err(|e| {
            crate::error::Error::Other(format!("Failed to put entry {next_dex}: {e}"))
        })?;
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: String::new(),
        method_patched: String::new(),
        signatures_restored,
    })
}

/// Replace `old` with `new` in all method bodies of a smali class text.
fn replace_text_in_all_methods(smali: &str, old: &str, new: &str) -> Result<String> {
    // Replace old with new everywhere in the smali text (like sed -i 's/old/new/g'),
    // not just inside method bodies. This ensures .field declarations, annotations,
    // and other non-method locations are also patched, so a second run won't find
    // the old text again.
    let mut result = String::with_capacity(smali.len());
    for line in smali.lines() {
        result.push_str(&line.replace(old, new));
        result.push('\n');
    }
    Ok(result)
}

/// Convert a search string to DEX byte candidates for pre-filtering.
///
/// For hex constants like `0x1fa267d`, DEX stores them as 32-bit integers
/// (little-endian bytes `7d 26 1a 0f`), not ASCII text. We convert the hex
/// digits to raw bytes and return both big-endian and little-endian forms.
/// For plain text, returns UTF-8 bytes.
///
/// Returns all candidate byte sequences; the pre-filter matches if ANY
/// candidate appears in DEX bytes (superset approach — may have false
/// positives but no false negatives for common encodings).
fn decode_search_bytes(s: &str) -> Vec<u8> {
    let s = s.trim();
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));

    if let Some(hex) = hex {
        // Parse hex digits into bytes
        let padded = if hex.len() % 2 != 0 {
            format!("0{}", hex)
        } else {
            hex.to_string()
        };
        let bytes: Vec<u8> = (0..padded.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&padded[i..i + 2], 16).ok())
            .collect();
        if !bytes.is_empty() {
            // Return big-endian bytes (little-endian check done by caller
            // via reversed window scan)
            return bytes;
        }
    }

    // Fallback: UTF-8 bytes
    s.as_bytes().to_vec()
}

/// Check if `needle` appears in `haystack`, trying both directions
/// (for little-endian DEX constant encoding).
fn contains_slice_le(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    // Big-endian
    if haystack.windows(needle.len()).any(|w| w == needle) {
        return true;
    }
    // Little-endian (reversed)
    let rev: Vec<u8> = needle.iter().rev().cloned().collect();
    haystack.windows(rev.len()).any(|w| w == rev.as_slice())
}

pub fn extract_smali_class(input_apk: &str, class_name: &str, output: &str) -> Result<()> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
        .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

    if let Some(parent) = std::path::Path::new(output).parent() {
        std::fs::create_dir_all(parent).map_err(|e| crate::error::Error::Io(e))?;
    }

    std::fs::write(output, &smali_text).map_err(|e| crate::error::Error::Io(e))?;

    Ok(())
}

/// Inject one modified smali class back into an APK/JAR.
pub fn inject_smali_class(
    input_apk: &str,
    output_apk: &str,
    class_name: &str,
    smali_file: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    let smali_text = std::fs::read_to_string(smali_file)
        .map_err(|e| crate::error::Error::Io(e))?;

    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    {
        let classes = apk.classes().map_err(|e| {
            crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
        })?;
        classes.set_text(&smali_text).map_err(|e| {
            crate::error::Error::Other(format!("Failed to set smali text for {class_name}: {e}"))
        })?;
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{{output_apk}}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: class_name.to_string(),
        method_patched: String::new(),
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

/// Discover which class(es) contain a given method name in an APK/JAR.
/// Searches all DEX files' method_ids for methods whose name contains `method_name`.
pub fn discover_method(input_apk: &str, method_name: &str) -> Result<Vec<String>> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let dex_entries = apk
        .dex()
        .map_err(|e| crate::error::Error::Other(format!("Failed to read DEX: {e}")))?;

    let mut found: Vec<String> = Vec::new();
    for (_dex_name, dex_bytes) in &dex_entries {
        let dex = apkscope_core::dex::Dex::parse(dex_bytes)
            .map_err(|e| crate::error::Error::Other(format!("DEX parse error: {e}")))?;

        for c in 0..dex.class_count {
            let class_data = dex.class_data(c).map_err(|e| {
                crate::error::Error::Other(format!("Class data error: {e}"))
            })?;
            for method in &class_data.methods {
                let method_ref = dex.method_ref(method.method_idx).map_err(|e| {
                    crate::error::Error::Other(format!("Method ref error: {e}"))
                })?;
                // method_ref format: Lcls;->name(params)Ret
                if let Some((_, rest)) = method_ref.split_once("->") {
                    if let Some(name) = rest.split('(').next() {
                        if name.contains(method_name) {
                            let desc = dex.class_descriptor(c).map_err(|e| {
                                crate::error::Error::Other(format!("Class descriptor error: {e}"))
                            })?;
                            let java_name = desc
                                .trim_start_matches('L')
                                .trim_end_matches(';')
                                .replace('/', ".");
                            if !found.contains(&java_name) {
                                found.push(java_name.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    found.sort();
    Ok(found)
}

/// Extract all classes from an APK/JAR as individual smali files into `output_dir`.
/// Each class is written as `<output_dir>/<package>/<Class>.smali`.
///
/// Parallelism: class export + disk write run across threads (each thread opens its own
/// file handle; each writes to a distinct path). Safe for concurrent execution.
pub fn extract_smali_all(input_apk: &str, output_dir: &str) -> Result<usize> {
    let class_names = list_classes(input_apk)?;

    if class_names.is_empty() {
        return Ok(0);
    }

    let input_apk = input_apk.to_string();
    let output_dir = output_dir.to_string();

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, class_names.len());
    let chunk_size = (class_names.len() + num_threads - 1) / num_threads;
    let chunks: Vec<Vec<String>> = class_names
        .chunks(chunk_size)
        .map(|c| c.to_vec())
        .collect();

    let handles: Vec<_> = chunks
        .into_iter()
        .map(|chunk| {
            let input_apk = input_apk.clone();
            let output_dir = output_dir.clone();
            std::thread::spawn(move || -> Result<usize> {
                let file = File::open(&input_apk)
                    .map_err(|e| crate::error::Error::Io(e))?;
                let mut apk = apkscope_core::apk::Apk::new(file)
                    .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;
                let mut count = 0usize;
                for class_name in &chunk {
                    let smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
                        .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;

                    let rel_path = class_name.replace('.', "/");
                    let smali_path = format!("{output_dir}/{rel_path}.smali");

                    if let Some(parent) = std::path::Path::new(&smali_path).parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| crate::error::Error::Io(e))?;
                    }

                    std::fs::write(&smali_path, &smali_text)
                        .map_err(|e| crate::error::Error::Io(e))?;
                    count += 1;
                }
                Ok(count)
            })
        })
        .collect();

    let mut total = 0usize;
    for h in handles {
        let join_result = h.join().map_err(|_| {
            crate::error::Error::Other("thread panic during smali extract".into())
        })?;
        total += join_result?;
    }

    Ok(total)
}

/// Inject all smali files from `smali_dir` back into an APK/JAR.
/// Each .smali file is parsed as one class and injected into the appropriate DEX.
pub fn inject_smali_all(
    input_apk: &str,
    output_apk: &str,
    smali_dir: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    let file = File::open(input_apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    // Collect all .smali files from the directory
    let mut sources = Vec::new();
    for entry in std::fs::read_dir(smali_dir)
        .map_err(|e| crate::error::Error::Io(e))?
    {
        let entry = entry.map_err(|e| crate::error::Error::Io(e))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("smali") {
            let smali_text = std::fs::read_to_string(&path)
                .map_err(|e| crate::error::Error::Io(e))?;
            sources.push(smali_text);
        }
    }

    if sources.is_empty() {
        return Err(crate::error::Error::Other(
            "no smali files found in directory".into(),
        ));
    }

    // Inject all smali sources into the APK
    {
        let classes = apk.classes().map_err(|e| {
            crate::error::Error::Other(format!("Failed to access APK classes: {e}"))
        })?;
        for source in &sources {
            classes.set_text(source).map_err(|e| {
                crate::error::Error::Other(format!("Failed to set smali text: {e}"))
            })?;
        }
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: String::new(),
        method_patched: String::new(),
        signatures_restored,
    })
}

/// Add all smali files from a directory or zip archive into an APK/JAR.
/// New classes are placed into existing DEX files that have free capacity when
/// possible, and any that do not fit are assembled into new classesN.dex files.
pub fn add_smali_all(
    input_apk: &str,
    output_apk: &str,
    smali_path: &str,
    sig_dir: Option<&str>,
) -> Result<PatchResult> {
    // Collect smali sources from a directory or zip file.
    let mut sources: Vec<(String, String)> = Vec::new();
    let temp_dir;
    let dir = if std::path::Path::new(smali_path).is_dir() {
        std::path::PathBuf::from(smali_path)
    } else if smali_path.ends_with(".zip") {
        temp_dir = std::env::temp_dir().join(format!("add_smali_all_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).map_err(|e| crate::error::Error::Io(e))?;
        let file = File::open(smali_path).map_err(|e| crate::error::Error::Io(e))?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| crate::error::Error::Zip(e))?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| crate::error::Error::Zip(e))?;
            let name = entry.name().to_owned();
            if name.ends_with(".smali") && !entry.is_dir() {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut entry, &mut buf).map_err(|e| crate::error::Error::Io(e))?;
                let out_path = temp_dir.join(&name);
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| crate::error::Error::Io(e))?;
                }
                std::fs::write(&out_path, &buf).map_err(|e| crate::error::Error::Io(e))?;
            }
        }
        temp_dir
    } else {
        return Err(crate::error::Error::Other("smali path must be a directory or .zip file".into()));
    };

    fn collect(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<(String, String)>) -> Result<()> {
        for entry in std::fs::read_dir(dir).map_err(|e| crate::error::Error::Io(e))? {
            let entry = entry.map_err(|e| crate::error::Error::Io(e))?;
            let path = entry.path();
            if path.is_dir() {
                collect(&path, base, out)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("smali") {
                let text = std::fs::read_to_string(&path).map_err(|e| crate::error::Error::Io(e))?;
                out.push((path.strip_prefix(base).unwrap_or(&path).to_string_lossy().replace('\\', "/"), text));
            }
        }
        Ok(())
    }
    collect(&dir, &dir, &mut sources)?;

    if sources.is_empty() {
        if smali_path.ends_with(".zip") {
            let _ = std::fs::remove_dir_all(&dir);
        }
        return Err(crate::error::Error::Other("no smali files found".into()));
    }

    let file = File::open(input_apk).map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let dex_entries = apk.dex().map_err(|e| crate::error::Error::Other(format!("Failed to read DEX: {e}")))?;

    let mut base_class_counts: Vec<usize> = Vec::with_capacity(dex_entries.len());
    let mut base_method_counts: Vec<usize> = Vec::with_capacity(dex_entries.len());
    let mut base_string_counts: Vec<usize> = Vec::with_capacity(dex_entries.len());
    for (_, bytes) in &dex_entries {
        if let Ok(dex) = apkscope_core::dex::Dex::parse(bytes) {
            let counts = dex.counts();
            base_class_counts.push(dex.class_count);
            base_method_counts.push(counts[3]);
            base_string_counts.push(counts[0]);
        } else {
            base_class_counts.push(usize::MAX);
            base_method_counts.push(usize::MAX);
            base_string_counts.push(usize::MAX);
        }
    }
    let mut added_class_counts = vec![0usize; dex_entries.len()];
    let mut added_method_counts = vec![0usize; dex_entries.len()];
    let mut added_string_counts = vec![0usize; dex_entries.len()];

    let mut leftover: Vec<String> = Vec::new();

    {
        let classes = apk.classes().map_err(|e| crate::error::Error::Other(format!("Failed to access APK classes: {e}")))?;
        for (_rel, text) in &sources {
            let method_count = text.matches(".method").count().max(1);
            let mut target: Option<usize> = None;
            for idx in 0..dex_entries.len() {
                if base_class_counts[idx] == usize::MAX { continue; }
                if base_class_counts[idx] + added_class_counts[idx] + 1 >= 64000 { continue; }
                if base_method_counts[idx] + added_method_counts[idx] + method_count >= 64000 { continue; }
                if base_string_counts[idx] + added_string_counts[idx] + 64 >= 65536 { continue; }
                target = Some(idx);
                break;
            }
            if let Some(idx) = target {
                classes.set_text_in(idx, text).map_err(|e| crate::error::Error::Other(format!("Failed to add smali: {e}")))?;
                added_class_counts[idx] += 1;
                added_method_counts[idx] += method_count;
                added_string_counts[idx] += 64;
            } else {
                leftover.push(text.clone());
            }
        }
    }

    if !leftover.is_empty() {
        let mut max_dex = 0;
        for (name, _) in &dex_entries {
            let num: u32 = name.trim_start_matches("classes").trim_end_matches(".dex").parse().unwrap_or(1);
            if num > max_dex { max_dex = num; }
        }
        let dex_version = dex_entries.first().and_then(|(_, bytes)| bytes.get(4..7)).map(|v| [v[0], v[1], v[2]]).unwrap_or(*b"035");
        let refs: Vec<&str> = leftover.iter().map(|s| s.as_str()).collect();
        match apkscope_core::smali::assemble(&refs, dex_version) {
            Ok(dex_bytes) => {
                let next_dex = format!("classes{}.dex", max_dex + 1);
                apk.put_entry(&next_dex, dex_bytes).map_err(|e| crate::error::Error::Other(format!("Failed to put entry {next_dex}: {e}")))?;
            }
            Err(_) => {
                let mut n = max_dex + 1;
                for src in &leftover {
                    let dex_bytes = apkscope_core::smali::assemble(&[src.as_str()], dex_version)
                        .map_err(|e| crate::error::Error::Other(format!("Smali assemble error: {e}")))?;
                    let name = format!("classes{}.dex", n);
                    apk.put_entry(&name, dex_bytes).map_err(|e| crate::error::Error::Other(format!("Failed to put entry {name}: {e}")))?;
                    n += 1;
                }
            }
        }
    }

    let _edits = apk.edits().map_err(|e| crate::error::Error::Other(format!("Failed to write APK edits: {e}")))?;

    let temp_output = format!("{output_apk}.tmp");
    {
        let mut out_file = File::create(&temp_output).map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| crate::error::Error::Rebuild(e.to_string()))?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    let signatures_restored = if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output_apk)?;
        let _ = std::fs::remove_file(&temp_output);
        true
    } else {
        std::fs::rename(&temp_output, output_apk).map_err(|e| crate::error::Error::Io(e))?;
        false
    };

    if smali_path.ends_with(".zip") {
        let _ = std::fs::remove_dir_all(&dir);
    }

    Ok(PatchResult {
        output_path: output_apk.to_string(),
        class_patched: String::new(),
        method_patched: String::new(),
        signatures_restored,
    })
}
