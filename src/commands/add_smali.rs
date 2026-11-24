//! `apkmodder add-smali` — add a smali class as a new DEX entry.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(
    input: &str,
    output: &str,
    smali_file: &str,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { None };

    let result = patch_flow::add_smali(input, output, smali_file, sig_dir)?;

    println!("Added smali class from {} (unsigned, no signatures).", smali_file);
    println!("Output: {}", result.output_path);

    Ok(())
}
