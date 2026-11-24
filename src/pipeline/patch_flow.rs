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
    let class_names: Vec<String> = {
        let file = File::open(input_apk)
            .map_err(|e| crate::error::Error::Io(e))?;
        let mut apk = apkscope_core::apk::Apk::new(file)
            .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;
        let dex_entries = apk.dex().map_err(|e| {
            crate::error::Error::Other(format!("Failed to read DEX: {e}"))
        })?;
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

    // Phase 2: Parallel export + modify ONLY classes containing the target text.
    // Each thread opens its own file handle. Skips classes that don't contain `old`.
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, class_names.len());
    let chunk_size = (class_names.len() + num_threads - 1) / num_threads;
    let chunks: Vec<Vec<String>> = class_names
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
                    for class_name in &chunk {
                        // Quick check: does this class's DEX bytes contain the target text?
                        // Skip export entirely if not - avoids smali parse for most classes.
                        let dex_entries = apk.dex().map_err(|e| {
                            crate::error::Error::Other(format!("Failed to read DEX: {e}"))
                        })?;
                        let mut contains_text = false;
                        for (_dex_name, dex_bytes) in &dex_entries {
                            if !contains_slice(dex_bytes, old.as_bytes()) {
                                contains_text = true;
                                break;
                            }
                        }
                        if !contains_text {
                            continue;
                        }

                        let mut smali_text = apkscope_core::smali::export_class_from(&mut apk, class_name)
                            .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class_name}: {e}")))?;
                        smali_text = replace_text_in_all_methods(&smali_text, &old, &new)
                            .map_err(|e| crate::error::Error::Other(format!("Replace-text-global error for {class_name}: {e}")))?;
                        out.push((class_name.clone(), smali_text));
                    }
                    Ok(out)
                })
            })
            .collect();

        let mut results = Vec::new();
        for h in handles {
            let join_result = h.join().map_err(|_| {
                crate::error::Error::Other("thread panic during parallel export".into())
            })?;
            let chunk_results = join_result?;
            for (class_name, smali) in chunk_results {
                results.push((class_name, smali));
            }
        }
        results
    };

    if modified.is_empty() {
        // No classes contained the target text — nothing to patch.
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

    // Determine the next classesN.dex number
    let dex_entries = apk.dex().map_err(|e| {
        crate::error::Error::Other(format!("Failed to read DEX: {e}"))
    })?;
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
    apk.put_entry(&next_dex, dex_bytes)
        .map_err(|e| crate::error::Error::Other(format!("Failed to put entry {next_dex}: {e}")))?;

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
fn contains_slice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn replace_text_in_all_methods(smali: &str, old: &str, new: &str) -> Result<String> {
    let lines: Vec<&str> = smali.lines().collect();
    let mut result = String::new();
    let mut in_method = false;
    let mut depth: i32 = 0;

    for line in &lines {
        let trimmed = line.trim();
        if trimmed.starts_with(".method") {
            depth += 1;
            in_method = true;
            result.push_str(line);
            result.push('\n');
        } else if trimmed == ".end method" {
            depth -= 1;
            if depth == 0 {
                in_method = false;
            }
            result.push_str(line);
            result.push('\n');
        } else if in_method {
            // Inside a method body: replace old with new
            result.push_str(&line.replace(old, new));
            result.push('\n');
        } else {
            // Outside methods: leave unchanged
            result.push_str(line);
            result.push('\n');
        }
    }

    Ok(result)
}

/// Extract one class as smali from an APK/JAR to a file.
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
