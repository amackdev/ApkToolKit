//! `ApkToolKit gui` — launch the GUI viewer.

use crate::error::Result;

pub fn run(apk: Option<String>) -> Result<()> {
    crate::gui::run_gui(apk).map_err(|e| crate::error::Error::Other(e.to_string()))
}
