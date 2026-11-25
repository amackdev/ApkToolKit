//! `ApkToolKit extract-entry` — extract a single entry from an APK/JAR.

use crate::error::Result;

pub fn run(input: &str, entry: &str, output: &str) -> Result<()> {
    let mut file = std::fs::File::open(input).map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(&mut file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let data = apk.read(entry).ok_or_else(|| {
        crate::error::Error::Other(format!("Entry not found: {entry}"))
    })?;

    std::fs::write(output, &data).map_err(|e| crate::error::Error::Io(e))?;

    println!("Extracted {entry} from {} -> {}", input, output);

    Ok(())
}
