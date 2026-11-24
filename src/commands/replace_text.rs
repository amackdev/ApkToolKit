//! `apkmodder replace-text` — scoped sed replacement inside a method body.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(
    input: &str,
    output: &str,
    class: &str,
    method: &str,
    old: &str,
    new: &str,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { None };

    let result = patch_flow::replace_text(input, output, class, method, old, new, sig_dir)?;

    println!(
        "Replaced text in {}#{} (unsigned, no signatures).",
        result.class_patched, result.method_patched
    );
    println!("Output: {}", result.output_path);

    Ok(())
}
