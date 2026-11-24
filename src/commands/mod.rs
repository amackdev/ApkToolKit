//! Command dispatch for ApkModder.

mod gui;
mod multi_patch;
mod open;
mod view;
mod patch;
mod extract_sig;
mod sig_info;

use crate::cli::Commands;
use crate::cli::GlobalOpts;
use crate::error::Result;

pub fn run(cmd: Commands, _global_opts: &GlobalOpts) -> Result<()> {
    match cmd {
        Commands::Open { apk } => open::run(&apk),
        Commands::View {
            apk,
            class,
            method,
            format,
        } => view::run(&apk, &class, method.as_deref(), &format),
        Commands::Patch {
            input,
            output,
            class,
            method,
            smali_file,
            smali_body,
            sig_dir,
            no_sign,
        } => patch::run(
            &input,
            &output,
            &class,
            &method,
            smali_file.as_deref(),
            smali_body.as_deref(),
            sig_dir.as_deref(),
            no_sign,
        ),
        Commands::MultiPatch {
            input,
            output,
            patch,
            patch_body,
            sig_dir,
            no_sign,
        } => multi_patch::run(
            &input,
            &output,
            &patch,
            &patch_body,
            sig_dir.as_deref(),
            no_sign,
        ),
        Commands::ExtractSig { apk, output_dir } => {
            extract_sig::run(&apk, &output_dir)
        }
        Commands::SigInfo { apk } => sig_info::run(&apk),
        Commands::Gui { apk } => gui::run(apk),
    }
}
