//! Handle APK signatures: extract and restore META-INF and the APK signing block.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use crate::error::Result;

const SIGNING_BLOCK_FILE: &str = "signing-block.bin";

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

    // Also extract the APK signing block (v2/v3) if present, so it can be
    // restored after a rebuild — matching APKEditor's -sig workflow. This is
    // required for apps (such as SystemUI) that carry no v1 (JAR) signature.
    if let Some(block) = read_signing_block_bytes(apk_path.as_ref()) {
        let out_path = output_dir.as_ref().join(SIGNING_BLOCK_FILE);
        fs::write(&out_path, &block).map_err(|e| crate::error::Error::Io(e))?;
        extracted.push(format!("APK signing block ({SIGNING_BLOCK_FILE})"));
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
    let output_path = output_path.as_ref();

    let mut sig_files: Vec<(String, Vec<u8>)> = Vec::new();
    collect_sig_files(sig_dir, sig_dir, &mut sig_files)?;

    // Write the (unsigned) zip to a temporary file first; the signing block,
    // if any, is inserted afterwards.
    let temp_zip = format!("{}.unsigned", output_path.display());
    // Remove stale temp file if present.
    let _ = fs::remove_file(&temp_zip);

    let out_file = File::create(&temp_zip).map_err(|e| crate::error::Error::Io(e))?;
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
        // Skip the extracted signing-block blob: it is not a zip entry.
        if rel_name == SIGNING_BLOCK_FILE {
            continue;
        }
        let entry_name = format!("META-INF/{rel_name}");
        out_zip
            .start_file(entry_name, options)
            .map_err(|e| crate::error::Error::Zip(e))?;
        out_zip.write_all(data).map_err(|e| crate::error::Error::Io(e))?;
    }

    out_zip.finish().map_err(|e| crate::error::Error::Zip(e))?;

    // Restore the APK signing block (v2/v3) if it was extracted.
    let block_path = sig_dir.join(SIGNING_BLOCK_FILE);
    if block_path.exists() {
        let block = fs::read(&block_path).map_err(|e| crate::error::Error::Io(e))?;
        insert_signing_block(Path::new(&temp_zip), &block, output_path)?;
        let _ = fs::remove_file(&temp_zip);
    } else {
        fs::rename(&temp_zip, output_path).map_err(|e| crate::error::Error::Io(e))?;
    }

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

/// Whether the APK carries an APK signing block (v2/v3).
pub fn signing_block_present<P: AsRef<Path>>(apk_path: P) -> bool {
    read_signing_block_bytes(apk_path.as_ref()).is_some()
}

/// Read the raw APK signing block bytes (including size prefix and magic) if present.
fn read_signing_block_bytes(apk_path: &Path) -> Option<Vec<u8>> {
    let data = fs::read(apk_path).ok()?;
    let idx = data.windows(4).rposition(|w| w == b"PK\x05\x06")?;
    if idx + 22 > data.len() {
        return None;
    }
    let cd_offset = u32::from_le_bytes(data[idx + 16..idx + 20].try_into().ok()?) as usize;
    if cd_offset < 24 || cd_offset > data.len() {
        return None;
    }
    if &data[cd_offset - 16..cd_offset] != b"APK Sig Block 42" {
        return None;
    }
    let size = u64::from_le_bytes(data[cd_offset - 24..cd_offset - 16].try_into().ok()?) as usize;
    if size < 24 || size as u64 > cd_offset as u64 {
        return None;
    }
    let start = cd_offset - size - 8;
    if start > cd_offset {
        return None;
    }
    Some(data[start..cd_offset].to_vec())
}

/// Insert a previously extracted APK signing block into an unsigned APK file.
fn insert_signing_block(source_apk: &Path, block: &[u8], output_path: &Path) -> Result<()> {
    let data = fs::read(source_apk).map_err(|e| crate::error::Error::Io(e))?;
    let idx = data
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .ok_or_else(|| crate::error::Error::Other("no end of central directory".into()))?;
    if idx + 20 > data.len() {
        return Err(crate::error::Error::Other("truncated APK".into()));
    }
    let cd_offset = u32::from_le_bytes(data[idx + 16..idx + 20].try_into().unwrap()) as usize;

    let mut out = Vec::with_capacity(data.len() + block.len());
    out.extend_from_slice(&data[..cd_offset]);
    out.extend_from_slice(block);
    out.extend_from_slice(&data[cd_offset..]);

    // Fix up the central directory offset in the (shifted) EOCD record.
    let new_idx = idx + block.len();
    let new_cd_offset = cd_offset + block.len();
    if new_idx + 20 > out.len() {
        return Err(crate::error::Error::Other("truncated APK after insert".into()));
    }
    out[new_idx + 16..new_idx + 20].copy_from_slice(&(new_cd_offset as u32).to_le_bytes());

    fs::write(output_path, &out).map_err(|e| crate::error::Error::Io(e))?;
    Ok(())
}