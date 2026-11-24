//! Error types for ApkModder.

use thiserror::Error;

/// Result type alias used throughout ApkModder.
pub type Result<T> = std::result::Result<T, Error>;

/// All possible errors from ApkModder operations.
#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum Error {
    /// APK file not found or unreadable.
    #[error("APK not found or unreadable: {0}")]
    ApkNotFound(String),

    /// APK is not a valid ZIP / APK file.
    #[error("Invalid APK format: {0}")]
    InvalidApk(String),

    /// Class not found in DEX files.
    #[error("Class not found: {0}")]
    ClassNotFound(String),

    /// Method not found in class.
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Smali parse error.
    #[error("Smali error at line {line}: {msg}")]
    Smali { line: usize, msg: String },

    /// Signature directory error.
    #[error("Signature directory error: {0}")]
    SignatureDir(String),

    /// META-INF handling error.
    #[error("META-INF error: {0}")]
    MetaInf(String),

    /// Rebuild error.
    #[error("Rebuild error: {0}")]
    Rebuild(String),

    /// Signing error.
    #[error("Signing error: {0}")]
    Signing(String),

    /// IO error wrapper.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// ZIP error wrapper.
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Strip prefix error wrapper.
    #[error("Path error: {0}")]
    Path(#[from] std::path::StripPrefixError),

    /// Generic error.
    #[error("{0}")]
    Other(String),
}

impl From<apkscope_core::Error> for Error {
    fn from(e: apkscope_core::Error) -> Self {
        Error::Other(format!("apkscope-core: {e}"))
    }
}
