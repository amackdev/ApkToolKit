//! `ApkToolKit append-method` — append a new method to a class.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(
    input: &str,
    output: &str,
    class: &str,
    method_decl: &str,
    smali_body: &str,
    sig_dir: Option<&str>,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { sig_dir };

    let result = patch_flow::append_method(input, output, class, method_decl, smali_body, sig_dir)?;

    println!(
        "Appended method to {} (unsigned, no signatures).",
        result.class_patched
    );
    println!("Output: {}", result.output_path);

    Ok(())
}
