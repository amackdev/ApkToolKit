//! `ApkToolKit replace-text-global` — global scoped sed replacement in all methods.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(
    input: &str,
    output: &str,
    old: &str,
    new: &str,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { None };

    let result = patch_flow::replace_text_global(input, output, old, new, sig_dir)?;

    println!(
        "Replaced text globally in all methods (unsigned, no signatures)."
    );
    println!("Output: {}", result.output_path);

    Ok(())
}
