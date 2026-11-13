//! Core of bank2csv: canonical model, parser trait, registry, layout helpers,
//! validation and output. Has no PDF dependencies, so parsers can be tested
//! from serialized `RawDocument` fixtures.

pub mod date;
pub mod error;
pub mod ids;
pub mod layout;
pub mod model;
pub mod money;
pub mod output;
pub mod parser;
pub mod raw;
pub mod registry;
pub mod validate;

pub use date::{Date, DateOrder};
pub use error::{Error, Result};
pub use model::{SourceRef, Statement, Transaction};
pub use money::Money;
pub use parser::{BankParser, Tier};
pub use raw::{GridDocument, Page, RawDocument, Sheet, Span, SpanDocument};
pub use registry::{ConvertOptions, Meta, Parsed};
