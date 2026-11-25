//! `ApkToolKit replace-bytes-global` — fast raw byte find/replace across all DEX entries.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(
    input: &str,
    output: &str,
    old_hex: &str,
    new_hex: &str,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { None };

    let result = patch_flow::replace_bytes_global(input, output, old_hex, new_hex, sig_dir)?;

    println!(
        "Replaced bytes globally across all DEX entries (unsigned, no signatures)."
    );
    println!("Output: {}", result.output_path);

    Ok(())
}
