//! Error types returned by the public astchunk pipeline APIs.

use bytestring::ByteString;

/// Errors produced by the astchunk library.
#[derive(Debug, Clone)]
pub enum AstchunkError {
    /// The requested language is not supported.
    UnsupportedLanguage {
        /// Language name that was not recognized.
        language: ByteString,
    },
    /// Tree-sitter parsing failed.
    ParseFailed {
        /// Language being parsed.
        language: ByteString,
        /// Description of the failure.
        message: ByteString,
    },
    /// A builder configuration value is invalid.
    InvalidConfiguration {
        /// Configuration field name.
        field: &'static str,
        /// Description of why the value is invalid.
        message: &'static str,
    },
    /// A required origin field is missing.
    InvalidOrigin {
        /// The missing field name.
        field: &'static str,
    },
    /// An exporter requires a field that was not provided.
    ExportRequirementMissing {
        /// The exporter that needs the field.
        exporter: &'static str,
        /// The missing field name.
        field: &'static str,
    },
}

impl std::fmt::Display for AstchunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedLanguage { language } => {
                write!(f, "unsupported language: {language}")
            }
            Self::ParseFailed { language, message } => {
                write!(f, "parse failed for {language}: {message}")
            }
            Self::InvalidConfiguration { field, message } => {
                write!(f, "invalid configuration for `{field}`: {message}")
            }
            Self::InvalidOrigin { field } => {
                write!(f, "invalid origin: missing `{field}`")
            }
            Self::ExportRequirementMissing { exporter, field } => {
                write!(f, "export requirement missing for {exporter}: `{field}`")
            }
        }
    }
}

impl std::error::Error for AstchunkError {}
