//! `ApkToolKit discover-method` — find which class contains a method.

use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(input: &str, method: &str) -> Result<()> {
    let classes = patch_flow::discover_method(input, method)?;

    if classes.is_empty() {
        println!("No class found containing method: {method}");
        return Ok(());
    }

    println!("Method '{method}' found in {} class(es):", classes.len());
    for class in &classes {
        println!("  {class}");
    }

    Ok(())
}
