//! `apkmodder replace-text-class` — scoped sed replacement in all methods of one class.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(
    input: &str,
    output: &str,
    class: &str,
    old: &str,
    new: &str,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { None };

    let result = patch_flow::replace_text_class(input, output, class, old, new, sig_dir)?;

    println!(
        "Replaced text in all methods of {} (unsigned, no signatures).",
        result.class_patched
    );
    println!("Output: {}", result.output_path);

    Ok(())
}
