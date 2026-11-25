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

        /// Path to the new smali method body file. Use "-" to read from stdin.
        #[arg(long)]
        smali_file: Option<String>,

        /// Inline smali method body (alternative to --smali-file).
        #[arg(long)]
        smali_body: Option<String>,

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

    /// Launch the GUI viewer.
    #[cfg(feature = "gui")]
    #[command(name = "gui")]
    Gui {
        /// APK file to open at start.
        apk: Option<String>,
    },

    /// Patch multiple methods at once (multi-smali editing).
    #[command(name = "multi-patch")]
    MultiPatch {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Patch specification: Class#method=smali_file (repeatable).
        #[arg(long)]
        patch: Vec<String>,

        /// Inline patch: Class#method=smali_body (repeatable).
        #[arg(long)]
        patch_body: Vec<String>,

        /// Restore signatures from this directory.
        #[arg(long)]
        sig_dir: Option<String>,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Discover which class contains a given method name in an APK/JAR.
    #[command(name = "discover-method")]
    DiscoverMethod {
        /// Input APK or JAR file.
        input: String,

        /// Method name to search for (e.g. isBuildConsistent).
        method: String,
    },

    /// Discover which classes contain a given text string in an APK/JAR.
    /// Searches raw DEX bytes without full smali parse — fast for large JARs.
    #[command(name = "discover-text")]
    DiscoverText {
        /// Input APK or JAR file.
        input: String,

        /// Text to search for (e.g. 0x1fa265a).
        text: String,
    },

    /// Replace text inside a method body (scoped sed replacement).
    #[command(name = "replace-text")]
    ReplaceText {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Class name (e.g. com.android.systemui.StatusBar).
        class: String,

        /// Method name (e.g. onCreate).
        method: String,

        /// Text to replace.
        old: String,

        /// Replacement text.
        new: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Insert text after a line matching `anchor` inside a method.
    #[command(name = "insert-after")]
    InsertAfter {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Class name (e.g. com.android.systemui.StatusBar).
        class: String,

        /// Method name (e.g. onCreate).
        method: String,

        /// Anchor line (text to search for; text is inserted after it).
        anchor: String,

        /// Text to insert.
        text: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Insert text before a line matching `anchor` inside a method.
    #[command(name = "insert-before")]
    InsertBefore {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Class name (e.g. com.android.systemui.StatusBar).
        class: String,

        /// Method name (e.g. onCreate).
        method: String,

        /// Anchor line (text to search for; text is inserted before it).
        anchor: String,

        /// Text to insert.
        text: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Remove all lines matching `pattern` inside a method.
    #[command(name = "remove-lines")]
    RemoveLines {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Class name (e.g. com.android.systemui.StatusBar).
        class: String,

        /// Method name (e.g. onCreate).
        method: String,

        /// Pattern to match (lines containing this are removed).
        pattern: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Append a new method to a class.
    #[command(name = "append-method")]
    AppendMethod {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Class name (e.g. com.android.systemui.StatusBar).
        class: String,

        /// Full `.method ...` declaration line for the new method.
        #[arg(long)]
        method_decl: String,

        /// Inline smali method body (without .method/.end method).
        #[arg(long)]
        smali_body: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Replace text in all methods of a specific class.
    #[command(name = "replace-text-class")]
    ReplaceTextClass {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Class name (e.g. com.android.systemui.StatusBar).
        class: String,

        /// Text to replace.
        old: String,

        /// Replacement text.
        new: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Replace text in all methods of all classes (global scoped sed).
    #[command(name = "replace-text-global")]
    ReplaceTextGlobal {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Text to replace.
        old: String,

        /// Replacement text.
        new: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Raw byte find/replace across all DEX entries (fast, no smali parsing).
    /// Accepts `0x`-prefixed hex constants (e.g. "0x1fa265a"), raw hex, or UTF-8 text.
    /// Old and new must decode to the same byte length.
    #[command(name = "replace-bytes-global")]
    ReplaceBytesGlobal {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Old byte sequence (0x-hex, raw hex, or UTF-8 text, e.g. "0x1fa265a").
        old_hex: String,

        /// New byte sequence (same byte length as old_hex).
        new_hex: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Extract a single entry from an APK/JAR to a file.
    #[command(name = "extract-entry")]
    ExtractEntry {
        /// Input APK or JAR file.
        input: String,

        /// Entry name inside the archive (e.g. resources.arsc).
        entry: String,

        /// Output file path.
        output: String,
    },

    /// Put/replace a single entry in an APK/JAR from a file.
    #[command(name = "put-entry")]
    PutEntry {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Entry name inside the archive (e.g. resources.arsc).
        entry: String,

        /// Input file path.
        input_file: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Add a smali class source file as a new DEX class in the APK/JAR.
    #[command(name = "add-smali")]
    AddSmali {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Path to the smali source file.
        smali_file: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Extract all classes from an APK/JAR as smali files into a directory.
    #[command(name = "extract-smali-all")]
    ExtractSmaliAll {
        /// Input APK or JAR file.
        input: String,

        /// Output directory for smali files.
        output_dir: String,
    },

    /// Inject all smali files from a directory back into an APK/JAR.
    #[command(name = "inject-smali-all")]
    InjectSmaliAll {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Directory containing smali files to inject.
        smali_dir: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Extract one class as smali from an APK/JAR.
    #[command(name = "extract-smali-class")]
    ExtractSmaliClass {
        /// Input APK or JAR file.
        input: String,

        /// Class name (e.g. android.security.KeyStore2).
        class: String,

        /// Output smali file path.
        output: String,
    },

    /// Inject one modified smali class back into an APK/JAR.
    #[command(name = "inject-smali-class")]
    InjectSmaliClass {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Class name (e.g. android.security.KeyStore2).
        class: String,

        /// Input smali file path.
        smali_file: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Extract a resource file (e.g. res/values/bools.xml) from an APK/JAR.
    #[command(name = "extract-resource")]
    ExtractResource {
        /// Input APK or JAR file.
        input: String,

        /// Resource path inside the APK (e.g. res/values/bools.xml).
        resource: String,

        /// Output file path.
        output: String,
    },

    /// Put/replace a resource file (e.g. res/values/bools.xml) in an APK/JAR.
    #[command(name = "put-resource")]
    PutResource {
        /// Input APK or JAR file.
        input: String,

        /// Output APK or JAR file.
        output: String,

        /// Resource path inside the APK (e.g. res/values/bools.xml).
        resource: String,

        /// Input file path.
        input_file: String,

        /// Do not sign (output will be unsigned).
        #[arg(long)]
        no_sign: bool,
    },

    /// Extract native libraries (.so) from an APK/JAR to a directory.
    /// Maps arch dirs: arm64-v8a→arm64, armeabi-v7a→arm, x86_64→x64, x86→x86.
    #[command(name = "extract-libs")]
    ExtractLibs {
        /// Input APK or JAR file.
        input: String,

        /// Output directory for extracted libraries.
        output_dir: String,
    },

    /// Extract all resources matching a pattern into a directory.
    /// Example: `extract-resource-all SystemUI.apk bools ./out/`
    /// extracts res/values/bools.xml, res/values-night/bools.xml, etc.
    #[command(name = "extract-resource-all")]
    ExtractResourceAll {
        /// Input APK or JAR file.
        input: String,

        /// Pattern to match in resource paths (e.g. "bools" matches
        /// res/values/bools.xml, res/values-night/bools.xml, etc.).
        pattern: String,

        /// Output directory for extracted resources.
        output_dir: String,
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
    println!("  open           Inspect an APK");
    println!("  view           View a class or method as smali/Java");
    println!("  patch          Patch a method body (--smali-file or --smali-body)");
    println!("  multi-patch    Patch multiple methods at once (--patch / --patch-body)");
    println!("  replace-text   Replace text inside a method body (scoped sed)");
    println!("  insert-after   Insert text after an anchor line in a method");
    println!("  insert-before  Insert text before an anchor line in a method");
    println!("  remove-lines   Remove lines matching a pattern in a method");
    println!("  append-method  Append a new method to a class");
    println!("  replace-text-class  Replace text in all methods of a specific class");
    println!("  replace-text-global  Replace text in all methods (global scoped sed)");
    println!("  replace-bytes-global  Raw byte find/replace across all DEX entries");
    println!("  extract-entry  Extract a single entry from an APK/JAR");
    println!("  put-entry      Put/replace a single entry in an APK/JAR");
    println!("  add-smali      Add a smali class source as a new DEX class");
    println!("  extract-smali-all  Extract all classes as smali files to a directory");
    println!("  inject-smali-all  Inject smali files from a directory back into APK/JAR");
    println!("  extract-smali-class  Extract one class as smali from APK/JAR");
    println!("  inject-smali-class  Inject one modified smali class back into APK/JAR");
    println!("  extract-resource     Extract a resource file (e.g. res/values/bools.xml)");
    println!("  extract-resource-all  Extract all resources matching a pattern to a directory");
    println!("  put-resource         Put/replace a resource file (e.g. res/values/bools.xml)");
    println!("  discover-method  Find which class contains a method");
    println!("  extract-sig    Extract META-INF signatures");
    println!("  sig-info       Show signature info");
    println!("  gui            Launch the GUI viewer");
}
