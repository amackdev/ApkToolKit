//! `ApkToolKit view` — view a class or method as smali/Java.

use crate::core::smali_patcher;
use crate::error::Result;

pub fn run(
    apk: &str,
    class: &str,
    method: Option<&str>,
    format: &str,
) -> Result<()> {
    let file = std::fs::File::open(apk)
        .map_err(|e| crate::error::Error::Io(e))?;
    let mut apk = apkscope_core::apk::Apk::new(file)
        .map_err(|e| crate::error::Error::Other(format!("Failed to open APK: {e}")))?;

    let smali = apkscope_core::smali::export_class_from(&mut apk, class)
        .map_err(|e| crate::error::Error::Other(format!("Smali export error for {class}: {e}")))?;

    match method {
        Some(m) => {
            if smali_patcher::has_method(&smali, m) {
                // Extract just the method body from the full class smali
                let method_text = extract_method(&smali, m)
                    .ok_or_else(|| crate::error::Error::MethodNotFound(m.to_string()))?;
                if format == "java" {
                    println!("// Java view not yet implemented. Showing smali instead:");
                    println!("{method_text}");
                } else {
                    println!("{method_text}");
                }
            } else {
                return Err(crate::error::Error::MethodNotFound(m.to_string()));
            }
        }
        None => {
            if format == "java" {
                println!("// Java view not yet implemented. Showing smali instead:");
                println!("{}", smali);
            } else {
                println!("{}", smali);
            }
        }
    }

    Ok(())
}

/// Extract just one method's smali text (from `.method` to `.end method` inclusive).
fn extract_method(smali: &str, method_name: &str) -> Option<String> {
    let start = smali_patcher::find_method_line(smali, method_name)?;
    let end = smali_patcher::find_method_end(smali, start)?;
    let lines: Vec<&str> = smali.lines().collect();
    Some(lines[start..=end].join("\n"))
}
