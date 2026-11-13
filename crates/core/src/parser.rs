use crate::{RawDocument, Result, Statement};

/// How sure a parser is that a document belongs to it.
/// Tiers are compared, not scores: independently written parsers can't
/// produce calibrated floats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Generic parsers that can handle anything of the right shape.
    Fallback = 1,
    /// Layout looks right but no unique identifier was found.
    Probable = 2,
    /// A unique signal was found (bank name plus SWIFT/BIC, a fixed header string, ...).
    Definitive = 3,
}

pub trait BankParser: Sync {
    /// Stable id used in options and output, e.g. `im_bank_ke`.
    fn id(&self) -> &'static str;

    /// Human-readable name, e.g. `I&M Bank (Kenya)`.
    fn name(&self) -> &'static str;

    /// Layout variants this parser understands, for `list_banks()`.
    fn layouts(&self) -> &'static [&'static str];

    /// `None` means "not mine". Must be cheap: it runs for every parser.
    fn detect(&self, doc: &RawDocument) -> Option<Tier>;

    /// Parse into the canonical model. Set `Statement::layout` to the variant used.
    fn parse(&self, doc: &RawDocument) -> Result<Statement>;
}
