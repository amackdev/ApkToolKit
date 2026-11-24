//! Command dispatch for ApkModder.

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
            sig_dir,
            no_sign,
        } => patch::run(&input, &output, &class, &method, &smali_file, sig_dir.as_deref(), no_sign),
        Commands::ExtractSig { apk, output_dir } => {
            extract_sig::run(&apk, &output_dir)
        }
        Commands::SigInfo { apk } => sig_info::run(&apk),
    }
}
