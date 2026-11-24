//! `apkmodder sig-info` — show META-INF signature info.

use crate::core::meta_inf_handler;
use crate::error::Result;

pub fn run(apk: &str) -> Result<()> {
    let info = meta_inf_handler::inspect_meta_inf(apk)?;
    println!("{}", meta_inf_handler::format_meta_inf_info(&info));
    Ok(())
}
