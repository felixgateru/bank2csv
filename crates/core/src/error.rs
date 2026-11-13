use crate::Money;
use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Typed errors. Each has a stable `code()` that the JS host can match on.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum Error {
    #[error("the document is password-protected; pass a password")]
    Encrypted,
    #[error("the password is incorrect")]
    WrongPassword,
    #[error("the document has no text layer (scanned image); run OCR first")]
    NeedsOcr,
    #[error("unsupported file format: {0}")]
    UnsupportedFormat(String),
    #[error("the file could not be read: {0}")]
    Corrupt(String),
    #[error("no parser recognised this statement")]
    NoParserMatched,
    #[error("several parsers matched equally well: {0:?}; pass `bank` to choose")]
    AmbiguousBank(Vec<String>),
    #[error("unknown bank id `{0}`")]
    UnknownBank(String),
    #[error("parse failed at page {page}, line {line}: {reason}")]
    ParseFailed { page: u32, line: u32, reason: String },
    #[error("statement does not reconcile: expected closing balance {expected}, computed {actual}")]
    Unreconciled { expected: Money, actual: Money },
    #[error("printed {kind} total is {expected}, transactions add up to {actual}")]
    TotalsMismatch { kind: &'static str, expected: Money, actual: Money },
    #[error("running balance mismatch at transaction {index}: statement shows {expected}, computed {actual}")]
    BalanceMismatch { index: usize, expected: Money, actual: Money },
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::Encrypted => "ENCRYPTED",
            Error::WrongPassword => "WRONG_PASSWORD",
            Error::NeedsOcr => "NEEDS_OCR",
            Error::UnsupportedFormat(_) => "UNSUPPORTED_FORMAT",
            Error::Corrupt(_) => "CORRUPT",
            Error::NoParserMatched => "NO_PARSER_MATCHED",
            Error::AmbiguousBank(_) => "AMBIGUOUS_BANK",
            Error::UnknownBank(_) => "UNKNOWN_BANK",
            Error::ParseFailed { .. } => "PARSE_FAILED",
            Error::Unreconciled { .. } => "UNRECONCILED",
            Error::BalanceMismatch { .. } => "BALANCE_MISMATCH",
            Error::TotalsMismatch { .. } => "TOTALS_MISMATCH",
        }
    }

    pub fn parse_failed(page: u32, line: u32, reason: impl Into<String>) -> Error {
        Error::ParseFailed { page, line, reason: reason.into() }
    }

    /// Validation failures may be rescued by trying another candidate parser.
    pub fn is_validation(&self) -> bool {
        matches!(self, Error::Unreconciled { .. } | Error::BalanceMismatch { .. } | Error::TotalsMismatch { .. })
    }
}
