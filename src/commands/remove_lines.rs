//! `ApkToolKit remove-lines` — remove lines matching a pattern in a method.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(
    input: &str,
    output: &str,
    class: &str,
    method: &str,
    pattern: &str,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { None };

    let result = patch_flow::remove_lines(input, output, class, method, pattern, sig_dir)?;

    println!(
        "Removed lines matching pattern in {}#{} (unsigned, no signatures).",
        result.class_patched, result.method_patched
    );
    println!("Output: {}", result.output_path);

    Ok(())
}
