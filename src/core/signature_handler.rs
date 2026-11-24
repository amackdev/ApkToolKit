//! Handle APK signatures: extract and restore META-INF.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use crate::error::Result;

/// Extract all META-INF entries from an APK to a directory.
pub fn extract_signatures<P: AsRef<Path>, Q: AsRef<Path>>(
    apk_path: P,
    output_dir: Q,
) -> Result<Vec<String>> {
    let file = File::open(apk_path.as_ref()).map_err(|e| crate::error::Error::Io(e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| crate::error::Error::Zip(e))?;

    fs::create_dir_all(output_dir.as_ref()).map_err(|e| crate::error::Error::Io(e))?;

    let mut extracted = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| crate::error::Error::Zip(e))?;
        let name = entry.name().to_owned();

        if name.starts_with("META-INF/") {
            let out_path = output_dir.as_ref().join(name.trim_start_matches("META-INF/"));
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| crate::error::Error::Io(e))?;
            }

            let mut out_file = File::create(&out_path).map_err(|e| crate::error::Error::Io(e))?;
            std::io::copy(&mut entry, &mut out_file).map_err(|e| crate::error::Error::Io(e))?;

            extracted.push(name);
        }
    }

    Ok(extracted)
}

/// Restore signature files from a directory into an APK file at a specific path.
pub fn restore_signatures<P: AsRef<Path>, Q: AsRef<Path>, R: AsRef<Path>>(
    source_apk: P,
    sig_dir: Q,
    output_path: R,
) -> Result<()> {
    let file = File::open(source_apk.as_ref()).map_err(|e| crate::error::Error::Io(e))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| crate::error::Error::Zip(e))?;

    let sig_dir = sig_dir.as_ref();

    let mut sig_files: Vec<(String, Vec<u8>)> = Vec::new();
    collect_sig_files(sig_dir, sig_dir, &mut sig_files)?;

    let out_file = File::create(output_path.as_ref()).map_err(|e| crate::error::Error::Io(e))?;
    let mut out_zip = zip::ZipWriter::new(out_file);

    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| crate::error::Error::Zip(e))?;
        let name = entry.name().to_owned();

        if name.starts_with("META-INF/") {
            continue;
        }

        let mut buf = Vec::new();
        Read::read_to_end(&mut entry, &mut buf).map_err(|e| crate::error::Error::Io(e))?;

        out_zip
            .start_file(name, options)
            .map_err(|e| crate::error::Error::Zip(e))?;
        out_zip.write_all(&buf).map_err(|e| crate::error::Error::Io(e))?;
    }

    for (rel_name, data) in &sig_files {
        let entry_name = format!("META-INF/{rel_name}");
        out_zip
            .start_file(entry_name, options)
            .map_err(|e| crate::error::Error::Zip(e))?;
        out_zip.write_all(data).map_err(|e| crate::error::Error::Io(e))?;
    }

    out_zip.finish().map_err(|e| crate::error::Error::Zip(e))?;

    Ok(())
}

/// Recursively collect signature files from a directory.
fn collect_sig_files(
    base_dir: &Path,
    dir: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|e| crate::error::Error::Io(e))? {
        let entry = entry.map_err(|e| crate::error::Error::Io(e))?;
        let path = entry.path();

        if path.is_dir() {
            collect_sig_files(base_dir, &path, files)?;
        } else {
            let rel = path
                .strip_prefix(base_dir)
                .map_err(|e| crate::error::Error::Path(e))?
                .to_string_lossy()
                .replace('\\', "/");
            let data = fs::read(&path).map_err(|e| crate::error::Error::Io(e))?;
            files.push((rel.to_string(), data));
        }
    }

    Ok(())
}


