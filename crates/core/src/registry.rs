//! Chooses a parser for a document, runs it and validates the result.
//! The list of parsers is passed in explicitly (see `bank2csv-banks::parsers()`).

use crate::ids::{self, Basis};
use crate::validate::{self, Order, Reconciliation};
use crate::{BankParser, Error, RawDocument, Result, Statement, Tier};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// Force a parser by id instead of auto-detecting.
    pub bank: Option<String>,
    /// Return output even if validation fails (`reconciled = failed`).
    pub lenient: bool,
    /// Fill `transaction_id` with deterministic ids (see `ids`). Default: true.
    pub generate_ids: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        ConvertOptions { bank: None, lenient: false, generate_ids: true }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Meta {
    pub bank: String,
    pub layout: String,
    pub reconciled: Reconciliation,
    pub order: Order,
    /// Parser ids tried, in order. More than one means a fallback happened.
    pub candidates_tried: Vec<String>,
    pub transaction_count: usize,
    /// Id scheme version (`v1`), or `None` if ids were not generated.
    pub id_scheme: Option<&'static str>,
    /// What the ids were keyed on, or `None` if ids were not generated.
    pub id_basis: Option<Basis>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Parsed {
    pub statement: Statement,
    pub meta: Meta,
    pub warnings: Vec<String>,
}

/// Candidate parsers for `doc`, best tier first. Errors on a tie at the top tier.
pub fn candidates<'a>(parsers: &[&'a dyn BankParser], doc: &RawDocument) -> Result<Vec<(&'a dyn BankParser, Tier)>> {
    let mut found: Vec<(&dyn BankParser, Tier)> =
        parsers.iter().filter_map(|p| p.detect(doc).map(|t| (*p, t))).collect();
    if found.is_empty() {
        return Err(Error::NoParserMatched);
    }
    // Stable sort keeps registry order within a tier.
    found.sort_by(|a, b| b.1.cmp(&a.1));
    let top = found[0].1;
    let tied: Vec<String> = found.iter().filter(|(_, t)| *t == top).map(|(p, _)| p.id().to_string()).collect();
    if tied.len() > 1 && top != Tier::Fallback {
        return Err(Error::AmbiguousBank(tied));
    }
    Ok(found)
}

pub fn parse(parsers: &[&dyn BankParser], doc: &RawDocument, opts: &ConvertOptions) -> Result<Parsed> {
    if let RawDocument::Structured(stmt) = doc {
        return finish(stmt.clone(), vec![stmt.bank.clone()], opts);
    }

    let chosen: Vec<&dyn BankParser> = match &opts.bank {
        Some(id) => vec![*parsers.iter().find(|p| p.id() == id).ok_or_else(|| Error::UnknownBank(id.clone()))?],
        None => candidates(parsers, doc)?.into_iter().map(|(p, _)| p).collect(),
    };

    let mut tried = Vec::new();
    let mut first_err: Option<Error> = None;
    for parser in chosen {
        tried.push(parser.id().to_string());
        let result = parser.parse(doc).and_then(|mut stmt| {
            stmt.bank = parser.id().to_string();
            finish(stmt, tried.clone(), &ConvertOptions { lenient: false, ..opts.clone() })
        });
        match result {
            Ok(parsed) => return Ok(parsed),
            Err(e) => {
                // Only validation failures, or a parser that couldn't make sense of
                // the document, justify trying the next candidate.
                let retry = e.is_validation() || matches!(e, Error::ParseFailed { .. });
                if first_err.is_none() {
                    first_err = Some(e);
                }
                if !retry {
                    break;
                }
            }
        }
    }

    let err = first_err.unwrap_or(Error::NoParserMatched);
    if opts.lenient && err.is_validation() {
        // Re-run the first candidate and return its output marked as failed.
        let id = &tried[0];
        let parser = parsers.iter().find(|p| p.id() == id).expect("tried parser exists");
        let mut stmt = parser.parse(doc)?;
        stmt.bank = id.clone();
        let mut parsed = finish(stmt, tried.clone(), opts)?;
        parsed.warnings.insert(0, err.to_string());
        return Ok(parsed);
    }
    Err(err)
}

fn finish(mut stmt: Statement, tried: Vec<String>, opts: &ConvertOptions) -> Result<Parsed> {
    let mut warnings = Vec::new();
    let order = validate::date_order(&stmt.transactions);
    if order == Order::Mixed {
        warnings.push("transaction dates are not in a consistent order".to_string());
    }
    let reconciled = match validate::check(&stmt) {
        Ok(r) => r,
        Err(e) if opts.lenient && e.is_validation() => Reconciliation::Failed,
        Err(e) => return Err(e),
    };
    if reconciled == Reconciliation::Unverifiable {
        warnings.push("statement has no balances to reconcile against; totals were not verified".to_string());
    }
    if stmt.transactions.is_empty() {
        warnings.push("no transactions found".to_string());
    }
    let id_basis = if opts.generate_ids {
        let (basis, id_warnings) = ids::assign(&mut stmt);
        warnings.extend(id_warnings);
        Some(basis)
    } else {
        None
    };
    Ok(Parsed {
        meta: Meta {
            bank: stmt.bank.clone(),
            layout: stmt.layout.clone(),
            reconciled,
            order,
            candidates_tried: tried,
            transaction_count: stmt.transactions.len(),
            id_scheme: opts.generate_ids.then_some(ids::SCHEME),
            id_basis,
        },
        statement: stmt,
        warnings,
    })
}
