//! `apkmodder insert-before` — insert text before an anchor line in a method.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(
    input: &str,
    output: &str,
    class: &str,
    method: &str,
    anchor: &str,
    text: &str,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { None };

    let result = patch_flow::insert_before(input, output, class, method, anchor, text, sig_dir)?;

    println!(
        "Inserted text before anchor in {}#{} (unsigned, no signatures).",
        result.class_patched, result.method_patched
    );
    println!("Output: {}", result.output_path);

    Ok(())
}
