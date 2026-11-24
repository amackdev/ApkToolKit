//! `apkmodder patch` — patch a method and rebuild.

use crate::pipeline::patch_flow;
use crate::error::Result;
use std::fs;

pub fn run(
    input: &str,
    output: &str,
    class: &str,
    method: &str,
    smali_file: &str,
    sig_dir: Option<&str>,
    no_sign: bool,
) -> Result<()> {
    // Read the new method body from the smali file
    let new_body = fs::read_to_string(smali_file)
        .map_err(|e| crate::error::Error::Io(e))?;



    // Determine if we should restore signatures
    let sig_dir_arg = if no_sign {
        None
    } else {
        sig_dir
    };

    let result = patch_flow::run(input, output, class, method, &new_body, sig_dir_arg)?;

    if result.signatures_restored {
        println!("Patched {}#{} and restored signatures.", result.class_patched, result.method_patched);
    } else {
        println!("Patched {}#{} (unsigned, no signatures).", result.class_patched, result.method_patched);
    }
    println!("Output: {}", result.output_path);

    Ok(())
}
