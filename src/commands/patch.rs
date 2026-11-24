//! `apkmodder patch` — patch a method and rebuild.

use crate::pipeline::patch_flow;
use crate::error::Result;
use std::io::Read;

pub fn run(
    input: &str,
    output: &str,
    class: &str,
    method: &str,
    smali_file: Option<&str>,
    smali_body: Option<&str>,
    sig_dir: Option<&str>,
    no_sign: bool,
) -> Result<()> {
    // Read the new method body from one of: --smali-body, --smali-file (- = stdin)
    let new_body = match smali_body {
        Some(body) => body.to_string(),
        None => {
            let smali_file = smali_file.ok_or_else(|| {
                crate::error::Error::Other(
                    "either --smali-file or --smali-body is required".into(),
                )
            })?;

            if smali_file == "-" {
                // Read from stdin
                let mut buf = String::new();
                std::io::stdin()
                    .read_to_string(&mut buf)
                    .map_err(|e| crate::error::Error::Io(e))?;
                buf
            } else {
                std::fs::read_to_string(smali_file).map_err(|e| crate::error::Error::Io(e))?
            }
        }
    };

    // Determine if we should restore signatures
    let sig_dir_arg = if no_sign { None } else { sig_dir };

    let result = patch_flow::run(input, output, class, method, &new_body, sig_dir_arg)?;

    if result.signatures_restored {
        println!(
            "Patched {}#{} and restored signatures.",
            result.class_patched, result.method_patched
        );
    } else {
        println!(
            "Patched {}#{} (unsigned, no signatures).",
            result.class_patched, result.method_patched
        );
    }
    println!("Output: {}", result.output_path);

    Ok(())
}
