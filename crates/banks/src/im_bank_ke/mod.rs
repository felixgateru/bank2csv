//! I&M Bank (Kenya) — emailed PDF account statements.
//!
//! Layout `email_v1` (produced by "OmniOMS" / iText, observed Nov 2026):
//!
//! - Delivered as an AES-128 encrypted PDF; the password is required.
//! - Each statement page repeats an account block and a table header:
//!   `Tran Date | Value Date | Ref No | Withdrawals | Deposits | Balance | Transaction Narrative`.
//! - Dates are `DD-MM-YY` in rows and `DD-MM-YYYY` in the header block.
//! - Amounts are right-aligned under their headers. Balances carry a `Cr`/`Dr` suffix.
//! - The narrative wraps onto lines *above and below* the row that holds the
//!   amounts (about ±5pt, sometimes three lines). Narrative-only lines are
//!   attached to the nearest amount row on the same page.
//! - The first row is `B/F` (brought forward) and carries the opening balance.
//! - The table ends with `Total <withdrawals> <deposits>`, followed by
//!   `Balance as of <date> <closing>`. Both go into the statement and are
//!   checked by the shared validator.
//! - Page 1 contains invisible text above the letterhead (y < 40) and the last
//!   page holds promotions. Only lines under a table header are read.

use bank2csv_core::layout::{self, Align, ColumnSpec, Columns, Line};
use bank2csv_core::{
    BankParser, Date, DateOrder, Error, Money, RawDocument, Result, SourceRef, SpanDocument, Statement, Tier,
    Transaction,
};

pub struct ImBankKe;

pub static PARSER: ImBankKe = ImBankKe;

const LAYOUT: &str = "email_v1";
const LINE_TOLERANCE: f32 = 2.0;
/// Narrative fragments further than this from any amount row are an error.
const MAX_NARRATIVE_DISTANCE: f32 = 14.0;

const COLUMNS: &[ColumnSpec] = &[
    ColumnSpec { key: "date", labels: &["Tran Date"], align: Align::Left },
    ColumnSpec { key: "value_date", labels: &["Value Date"], align: Align::Left },
    ColumnSpec { key: "reference", labels: &["Ref No"], align: Align::Left },
    ColumnSpec { key: "withdrawal", labels: &["Withdrawals"], align: Align::Right },
    ColumnSpec { key: "deposit", labels: &["Deposits"], align: Align::Right },
    ColumnSpec { key: "balance", labels: &["Balance"], align: Align::Right },
    ColumnSpec { key: "narrative", labels: &["Transaction Narrative"], align: Align::Left },
];

impl BankParser for ImBankKe {
    fn id(&self) -> &'static str {
        "im_bank_ke"
    }

    fn name(&self) -> &'static str {
        "I&M Bank (Kenya)"
    }

    fn layouts(&self) -> &'static [&'static str] {
        &[LAYOUT]
    }

    fn detect(&self, doc: &RawDocument) -> Option<Tier> {
        let RawDocument::Spans(d) = doc else { return None };
        let text = d.full_text();
        let has_header = text.contains("Transaction Narrative") && text.contains("Tran Date");
        let has_bank = text.contains("I&M Bank");
        match (has_bank, has_header) {
            (true, true) => Some(Tier::Definitive),
            (false, true) => Some(Tier::Probable),
            _ => None,
        }
    }

    fn parse(&self, doc: &RawDocument) -> Result<Statement> {
        let RawDocument::Spans(d) = doc else {
            return Err(Error::UnsupportedFormat("I&M parser expects a PDF".into()));
        };
        parse_spans(d)
    }
}

/// An amount row being built, plus the narrative fragments attached to it.
struct Pending {
    page: u32,
    line: u32,
    y: f32,
    date: Date,
    value_date: Option<Date>,
    reference: String,
    amount: Option<Money>,
    balance: Money,
    narrative: Vec<(f32, String)>,
}

fn parse_spans(doc: &SpanDocument) -> Result<Statement> {
    let mut stmt = Statement { layout: LAYOUT.to_string(), ..Default::default() };
    let mut rows: Vec<Pending> = Vec::new();
    let mut totals: Option<(Money, Money)> = None;

    for page in &doc.pages {
        let lines = layout::lines(page, LINE_TOLERANCE);
        read_account_block(&lines, &mut stmt);

        let Some(header_pos) = lines.iter().position(|l| l.text().contains("Transaction Narrative")) else {
            continue;
        };
        let columns = Columns::from_header(&lines[header_pos], COLUMNS)
            .ok_or_else(|| Error::parse_failed(page.number, lines[header_pos].index, "table header columns not found"))?;

        let page_start = rows.len();
        let mut fragments: Vec<(f32, String, u32)> = Vec::new();
        let mut table_ended = false;

        for line in &lines[header_pos + 1..] {
            let text = line.text();
            if is_footer(&text) {
                break;
            }
            if table_ended {
                if let Some(rest) = text.strip_prefix("Balance as of") {
                    stmt.closing_balance = last_amount(rest);
                }
                continue;
            }
            if text.starts_with("Total") {
                let row = columns.assign(line);
                let w = Money::parse(row.get("withdrawal"));
                let dep = Money::parse(row.get("deposit"));
                match (w, dep) {
                    (Some(w), Some(dep)) => totals = Some((w, dep)),
                    _ => return Err(Error::parse_failed(page.number, line.index, "unreadable Total row")),
                }
                table_ended = true;
                continue;
            }

            let row = columns.assign(line);
            let narrative = row.get("narrative").to_string();
            let only_narrative = row.cells.iter().all(|(k, v)| *k == "narrative" || v.is_empty());
            if only_narrative {
                fragments.push((line.y, narrative, line.index));
                continue;
            }

            let pending = amount_row(page.number, line, &row)?;
            if !narrative.is_empty() {
                fragments.push((line.y, narrative, line.index));
            }
            rows.push(pending);
        }

        // Attach each narrative fragment to the nearest amount row on this page.
        let page_rows = &mut rows[page_start..];
        for (y, text, index) in fragments {
            let nearest = page_rows
                .iter_mut()
                .min_by(|a, b| (a.y - y).abs().total_cmp(&(b.y - y).abs()))
                .filter(|r| (r.y - y).abs() <= MAX_NARRATIVE_DISTANCE)
                .ok_or_else(|| Error::parse_failed(page.number, index, format!("narrative `{text}` has no amount row")))?;
            nearest.narrative.push((y, text));
        }
    }

    for mut row in rows {
        row.narrative.sort_by(|a, b| a.0.total_cmp(&b.0));
        let description = row.narrative.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(" ");
        match row.amount {
            None if description == "B/F" => {
                if stmt.opening_balance.is_some() {
                    return Err(Error::parse_failed(row.page, row.line, "second B/F row"));
                }
                stmt.opening_balance = Some(row.balance);
            }
            None => return Err(Error::parse_failed(row.page, row.line, "row has no withdrawal or deposit")),
            Some(amount) => stmt.transactions.push(Transaction {
                transaction_id: None,
                date: row.date,
                value_date: row.value_date,
                description,
                reference: Some(row.reference).filter(|r| !r.is_empty()),
                amount,
                balance: Some(row.balance),
                source: Some(SourceRef { page: row.page, line: row.line }),
            }),
        }
    }

    if let Some((w, dep)) = totals {
        stmt.total_debits = Some(w);
        stmt.total_credits = Some(dep);
    }
    Ok(stmt)
}

fn amount_row(page: u32, line: &Line, row: &layout::Row) -> Result<Pending> {
    let fail = |reason: String| Error::parse_failed(page, line.index, reason);
    let date = Date::parse(row.get("date"), DateOrder::DayFirst)
        .ok_or_else(|| fail(format!("unexpected line `{}`", line.text())))?;
    let value_date = match row.get("value_date") {
        "" => None,
        s => Some(Date::parse(s, DateOrder::DayFirst).ok_or_else(|| fail(format!("bad value date `{s}`")))?),
    };
    let money = |key: &str| -> Result<Option<Money>> {
        match row.get(key) {
            "" => Ok(None),
            s => Money::parse(s).map(Some).ok_or_else(|| fail(format!("bad {key} amount `{s}`"))),
        }
    };
    let amount = match (money("withdrawal")?, money("deposit")?) {
        (Some(w), None) => Some(-w.abs()),
        (None, Some(d)) => Some(d.abs()),
        (None, None) => None,
        (Some(_), Some(_)) => return Err(fail("row has both a withdrawal and a deposit".into())),
    };
    let balance = money("balance")?.ok_or_else(|| fail("row has no balance".into()))?;
    Ok(Pending {
        page,
        line: line.index,
        y: line.y,
        date,
        value_date,
        reference: row.get("reference").to_string(),
        amount,
        balance,
        narrative: Vec::new(),
    })
}

/// Reads account number, currency and period from the block above the table.
fn read_account_block(lines: &[Line], stmt: &mut Statement) {
    for line in lines {
        let spans: Vec<&str> = line.spans.iter().map(|s| s.text.as_str()).collect();
        for (i, s) in spans.iter().enumerate() {
            let next = spans.get(i + 1).copied();
            match *s {
                "Account Number" if stmt.account.is_none() => stmt.account = next.map(String::from),
                "Account Currency" if stmt.currency.is_none() => stmt.currency = next.and_then(currency_code),
                "Statement Period" if stmt.period_start.is_none() => {
                    stmt.period_start = next.and_then(|d| Date::parse(d, DateOrder::DayFirst));
                    if spans.get(i + 2) == Some(&"To") {
                        stmt.period_end = spans.get(i + 3).and_then(|d| Date::parse(d, DateOrder::DayFirst));
                    }
                }
                _ => {}
            }
        }
    }
}

fn currency_code(name: &str) -> Option<String> {
    let code = match name.trim().to_ascii_uppercase().as_str() {
        "KENYAN SHILLINGS" | "KENYAN SHILLING" | "KES" => "KES",
        "US DOLLAR" | "US DOLLARS" | "USD" => "USD",
        "EURO" | "EUR" => "EUR",
        "POUND STERLING" | "GBP" => "GBP",
        _ => return None,
    };
    Some(code.to_string())
}

fn last_amount(text: &str) -> Option<Money> {
    text.split_whitespace().rev().find_map(Money::parse)
}

fn is_footer(text: &str) -> bool {
    text.contains("System Generated Email Attachment")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currency_names() {
        assert_eq!(currency_code("KENYAN SHILLINGS").as_deref(), Some("KES"));
        assert_eq!(currency_code("Martian Credits"), None);
    }
}
