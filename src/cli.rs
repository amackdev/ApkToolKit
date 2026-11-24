//! CLI definitions using clap.

use clap::Parser;

/// Global output options.
#[allow(dead_code)]
pub struct GlobalOpts {
    pub json: bool,
    pub quiet: bool,
    pub output: Option<String>,
}

#[derive(Parser, Debug)]
#[command(
    name = "apkmodder",
    about = "APK modding toolkit: extract, patch, rebuild, sign — no extraction required",
    long_about = "ApkModder modifies APKs in place. Patch individual smali methods without extracting the whole APK."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Emit JSON where supported.
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress progress lines on stderr.
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    /// Write output to file instead of stdout.
    #[arg(short = 'o', long, global = true)]
    pub output: Option<String>,

    /// Print version.
    #[arg(short = 'V', long = "version")]
    pub version: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Inspect an APK: package, classes, signatures, META-INF info.
    #[command(name = "open")]
    Open {
        /// APK file to inspect.
        apk: String,
    },

    /// View a class or method as smali or Java.
    #[command(name = "view")]
    View {
        /// APK file.
        apk: String,

        /// Class name (e.g. com.android.systemui.StatusBar).
        class: String,

        /// Optional method name (e.g. onCreate).
        #[arg(long)]
        method: Option<String>,

        /// Output format: smali (default) or java.
        #[arg(long, default_value = "smali")]
        format: String,
    },

    /// Patch a method and rebuild the APK.
    #[command(name = "patch")]
    Patch {
        /// Input APK.
        input: String,

        /// Output APK.
        output: String,

        /// Class name (e.g. com.android.systemui.StatusBar).
        class: String,

        /// Method name (e.g. onCreate).
        method: String,

        /// Path to the new smali method body file.
        #[arg(long)]
        smali_file: String,

        /// Restore signatures from this directory instead of signing.
        #[arg(long)]
        sig_dir: Option<String>,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Extract META-INF signatures from an APK to a directory.
    #[command(name = "extract-sig")]
    ExtractSig {
        /// APK file.
        apk: String,

        /// Output directory for signatures.
        output_dir: String,
    },

    /// Show META-INF signature info.
    #[command(name = "sig-info")]
    SigInfo {
        /// APK file.
        apk: String,
    },
}

pub fn print_version() {
    println!("apkmodder 0.1.0");
}

pub fn print_usage() {
    println!("apkmodder 0.1.0 - APK modding toolkit");
    println!("Usage: apkmodder <command> [options]");
    println!();
    println!("Commands:");
    println!("  open       Inspect an APK");
    println!("  view       View a class or method as smali/Java");
    println!("  patch      Patch a method and rebuild");
    println!("  extract-sig Extract META-INF signatures");
    println!("  sig-info   Show signature info");
}


