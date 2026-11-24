//! `apkmodder multi-patch` — patch multiple methods in one pass.

use crate::pipeline::patch_flow;
use crate::error::Result;
use std::io::Read;

pub fn run(
    input: &str,
    output: &str,
    patch: &[String],
    patch_body: &[String],
    sig_dir: Option<&str>,
    no_sign: bool,
) -> Result<()> {
    // Parse patch specifications: Class#method=file or Class#method=body
    let mut patches: Vec<(String, String, String)> = Vec::new();

    for spec in patch {
        let (class_method, file) = spec
            .split_once('=')
            .ok_or_else(|| crate::error::Error::Other(format!("invalid --patch spec: {spec}")))?;
        let (class, method) = class_method
            .split_once('#')
            .ok_or_else(|| crate::error::Error::Other(format!("invalid --patch spec (missing #): {spec}")))?;

        let body = if file == "-" {
            // Read from stdin
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| crate::error::Error::Io(e))?;
            buf
        } else {
            std::fs::read_to_string(file).map_err(|e| crate::error::Error::Io(e))?
        };

        patches.push((class.to_string(), method.to_string(), body));
    }

    for spec in patch_body {
        let (class_method, body) = spec
            .split_once('=')
            .ok_or_else(|| crate::error::Error::Other(format!("invalid --patch-body spec: {spec}")))?;
        let (class, method) = class_method
            .split_once('#')
            .ok_or_else(|| crate::error::Error::Other(format!("invalid --patch-body spec (missing #): {spec}")))?;

        patches.push((class.to_string(), method.to_string(), body.to_string()));
    }

    if patches.is_empty() {
        return Err(crate::error::Error::Other(
            "at least one --patch or --patch-body is required".into(),
        ));
    }

    let sig_dir_arg = if no_sign { None } else { sig_dir };

    let result = patch_flow::multi_patch(input, output, &patches, sig_dir_arg)?;

    println!(
        "Multi-patched {} method(s).",
        patches.len()
    );
    println!("Output: {}", result.output_path);

    if result.signatures_restored {
        println!("Signatures restored from backup.");
    } else {
        println!("Output is unsigned (no signatures).");
    }

    Ok(())
}
