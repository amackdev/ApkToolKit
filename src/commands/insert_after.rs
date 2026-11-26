//! `ApkToolKit insert-after` — insert text after an anchor line in a method.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(
    input: &str,
    output: &str,
    class: &str,
    method: &str,
    anchor: &str,
    text: &str,
    sig_dir: Option<&str>,
    no_sign: bool,
) -> Result<()> {
    let sig_dir = if no_sign { None } else { sig_dir };

    let result = patch_flow::insert_after(input, output, class, method, anchor, text, sig_dir)?;

    println!(
        "Inserted text after anchor in {}#{} (unsigned, no signatures).",
        result.class_patched, result.method_patched
    );
    println!("Output: {}", result.output_path);

    Ok(())
}
