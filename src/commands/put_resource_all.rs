//! `ApkToolKit put-resource-all` — put all resources from a directory back into an APK/JAR.
//!
//! Does a single APK rebuild for all resources (efficient for bulk operations).
//!
//! Example: `ApkToolKit put-resource-all SystemUI.apk ./extracted/`
//! puts all files under ./extracted/ back into SystemUI.apk preserving paths.

use crate::error::Result;
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn run(
    input: &str,
    output: &str,
    input_dir: &str,
    sig_dir: Option<&str>,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { sig_dir };
    let input_dir_path = Path::new(input_dir);
    if !input_dir_path.is_dir() {
        return Err(crate::error::Error::Other(format!(
            "Input directory not found: {input_dir}"
        )));
    }

    let mut file = std::fs::File::open(input)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(&mut file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    // Collect all files from input_dir recursively
    let mut sources: Vec<(String, Vec<u8>)> = Vec::new();
    collect_files(input_dir_path, input_dir_path, &mut sources)?;

    if sources.is_empty() {
        return Err(crate::error::Error::Other(format!(
            "No files found in directory: {input_dir}"
        )));
    }

    // Sort by path for deterministic order
    sources.sort_by(|a, b| a.0.cmp(&b.0));

    let mut put_count = 0usize;
    for (entry_path, data) in &sources {
        apk.put(entry_path, data.clone())
            .map_err(|e| crate::error::Error::Other(format!("Failed to put {entry_path}: {e}")))?;
        put_count += 1;
        eprintln!("put-resource-all: put {entry_path}");
    }

    let _edits = apk.edits().map_err(|e| {
        crate::error::Error::Other(format!("Failed to write APK edits: {e}"))
    })?;

    let temp_output = format!("{output}.tmp");
    {
        let mut out_file = std::fs::File::create(&temp_output)
            .map_err(|e| crate::error::Error::Io(e))?;
        let _bytes_written = apk.rebuild(&mut out_file).map_err(|e| {
            crate::error::Error::Rebuild(e.to_string())
        })?;
        out_file.flush().map_err(|e| crate::error::Error::Io(e))?;
    }

    if let Some(dir) = sig_dir {
        crate::core::signature_handler::restore_signatures(&temp_output, dir, output)?;
        let _ = std::fs::remove_file(&temp_output);
    } else {
        std::fs::rename(&temp_output, output).map_err(|e| crate::error::Error::Io(e))?;
    }

    eprintln!(
        "put-resource-all: put {put_count} entr{} into {output}",
        if put_count == 1 { "y" } else { "ies" }
    );

    Ok(())
}

/// Recursively collect all files from `dir`, recording their path relative to `root`.
fn collect_files(
    dir: &Path,
    root: &Path,
    sources: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|e| crate::error::Error::Io(e))? {
        let entry = entry.map_err(|e| crate::error::Error::Io(e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, root, sources)?;
        } else if path.is_file() {
            let rel_path = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let data = fs::read(&path).map_err(|e| crate::error::Error::Io(e))?;
            sources.push((rel_path, data));
        }
    }
    Ok(())
}
