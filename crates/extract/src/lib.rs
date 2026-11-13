//! File bytes → `RawDocument`. The format is detected from magic bytes.

pub mod pdf;

use bank2csv_core::{Error, RawDocument, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Unknown,
}

pub fn sniff(bytes: &[u8]) -> Format {
    // The PDF header may be preceded by junk; the spec allows it within the first 1024 bytes.
    let head = &bytes[..bytes.len().min(1024)];
    if head.windows(5).any(|w| w == b"%PDF-") {
        return Format::Pdf;
    }
    Format::Unknown
}

pub fn extract(bytes: &[u8], password: Option<&str>) -> Result<RawDocument> {
    match sniff(bytes) {
        Format::Pdf => pdf::extract(bytes, password).map(RawDocument::Spans),
        Format::Unknown => Err(Error::UnsupportedFormat("only PDF is supported in v0.1".into())),
    }
}
