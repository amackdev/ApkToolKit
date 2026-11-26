//! `ApkToolKit put-entry` — put/replace a single entry in an APK/JAR.

use crate::error::Result;
use std::io::Write;

pub fn run(
    input: &str,
    output: &str,
    entry: &str,
    input_file: &str,
    sig_dir: Option<&str>,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { sig_dir };

    let data = std::fs::read(input_file).map_err(|e| crate::error::Error::Io(e))?;

    let mut file = std::fs::File::open(input).map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(&mut file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    apk.put_entry(entry, data)
        .map_err(|e| crate::error::Error::Other(format!("Failed to put entry {entry}: {e}")))?;

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

    println!("Put entry {entry} into {}.", input);
    println!("Output: {}", output);

    Ok(())
}
