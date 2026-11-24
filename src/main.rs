//! ApkModder — Professional APK modding toolkit.
//!
//! Open, patch, rebuild, and sign APKs without full extraction.
//! Built on apkscope-core for DEX parsing, smali round-trip,
//! resource editing, rebuild, and signing.

#![recursion_limit = "256"]
#![warn(clippy::all)]

mod cli;
mod commands;
mod core;
mod error;
mod pipeline;

#[cfg(feature = "gui")]
mod gui;

use clap::Parser;
use std::process::ExitCode;

/// Global allocator: mimalloc returns freed memory to the OS.
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    if cli.version {
        cli::print_version();
        return ExitCode::SUCCESS;
    }

    match cli.command {
        Some(cmd) => {
            let global_opts = crate::cli::GlobalOpts {
                json: cli.json,
                quiet: cli.quiet,
                output: cli.output.clone(),
            };
            match commands::run(cmd, &global_opts) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        None => {
            cli::print_usage();
            ExitCode::from(2)
        }
    }
}
