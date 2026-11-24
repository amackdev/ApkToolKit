//! `apkmodder open` — inspect an APK.

use crate::core::apk_reader;
use crate::core::meta_inf_handler;
use crate::pipeline::patch_flow;
use crate::error::Result;

pub fn run(apk: &str) -> Result<()> {
    let package = apk_reader::get_package_name(apk)
        .unwrap_or_else(|_| "unknown".to_string());

    let classes = patch_flow::list_classes(apk)?;

    let meta_info = meta_inf_handler::inspect_meta_inf(apk)?;

    println!("=== APK Inspection ===");
    println!("File: {apk}");
    println!("Package: {package}");
    println!("Classes: {}", classes.len());
    for class in &classes {
        println!("  - {class}");
    }
    println!();
    println!("{}", meta_inf_handler::format_meta_inf_info(&meta_info));

    Ok(())
}
