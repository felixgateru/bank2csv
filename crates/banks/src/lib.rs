//! Bank parsers. Each bank is a module behind a Cargo feature.
//! To add a bank: add a module, a feature, and one line in `parsers()`.

use bank2csv_core::BankParser;

#[cfg(feature = "im_bank_ke")]
pub mod im_bank_ke;

/// Every compiled-in parser, in registry order (ties within a tier keep this order).
#[allow(clippy::vec_init_then_push)] // one cfg-gated push per bank
pub fn parsers() -> Vec<&'static dyn BankParser> {
    #[allow(unused_mut)]
    let mut list: Vec<&'static dyn BankParser> = Vec::new();
    #[cfg(feature = "im_bank_ke")]
    list.push(&im_bank_ke::PARSER);
    list
}
