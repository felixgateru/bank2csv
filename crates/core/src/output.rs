//! CSV and JSON writers. Debit and credit columns are derived from the signed
//! amount here, so they can never disagree with it.

use crate::Statement;

#[derive(Debug, Clone)]
pub struct CsvOptions {
    pub delimiter: char,
    /// Write separate `debit` and `credit` columns (both positive) as well as `amount`.
    pub split_debit_credit: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions { delimiter: ',', split_debit_credit: true }
    }
}

pub fn to_csv(stmt: &Statement, opts: &CsvOptions) -> String {
    let d = opts.delimiter;
    let mut out = String::new();
    // `transaction_id` is always present so the column set doesn't depend on
    // options; it is empty when id generation is turned off.
    let mut header = vec!["transaction_id", "date", "value_date", "description", "reference"];
    if opts.split_debit_credit {
        header.extend(["debit", "credit"]);
    }
    header.extend(["amount", "balance"]);
    push_row(&mut out, header.iter().map(|s| s.to_string()), d);

    for t in &stmt.transactions {
        let mut row = vec![
            t.transaction_id.clone().unwrap_or_default(),
            t.date.to_string(),
            t.value_date.map(|v| v.to_string()).unwrap_or_default(),
            t.description.clone(),
            t.reference.clone().unwrap_or_default(),
        ];
        if opts.split_debit_credit {
            let (debit, credit) = if t.amount.is_negative() {
                (t.amount.abs().to_string(), String::new())
            } else {
                (String::new(), t.amount.to_string())
            };
            row.push(debit);
            row.push(credit);
        }
        row.push(t.amount.to_string());
        row.push(t.balance.map(|b| b.to_string()).unwrap_or_default());
        push_row(&mut out, row.into_iter(), d);
    }
    out
}

fn push_row(out: &mut String, fields: impl Iterator<Item = String>, delimiter: char) {
    let mut first = true;
    for f in fields {
        if !first {
            out.push(delimiter);
        }
        first = false;
        let needs_quotes = f.contains(delimiter) || f.contains('"') || f.contains('\n') || f.contains('\r');
        // Guard against CSV formula injection when the file is opened in a spreadsheet.
        let risky = f.starts_with(['=', '+', '@']) || (f.starts_with('-') && f[1..].parse::<f64>().is_err() && f.len() > 1);
        let f = if risky { format!("'{f}") } else { f };
        if needs_quotes || risky {
            out.push('"');
            out.push_str(&f.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(&f);
        }
    }
    out.push_str("\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Date, Money, Transaction};

    #[test]
    fn writes_and_escapes() {
        let s = Statement {
            transactions: vec![Transaction {
                transaction_id: Some("e670159f-3a30-5b1b-aa57-883623cac94b".into()),
                date: Date::new(2026, 11, 2).unwrap(),
                value_date: None,
                description: "Pay, \"Rent\"".into(),
                reference: Some("=SUM(A1)".into()),
                amount: Money(-150050),
                balance: Some(Money(1000)),
                source: None,
            }],
            ..Default::default()
        };
        let csv = to_csv(&s, &CsvOptions::default());
        let line = csv.lines().nth(1).unwrap();
        assert_eq!(line, "e670159f-3a30-5b1b-aa57-883623cac94b,2026-11-02,,\"Pay, \"\"Rent\"\"\",\"'=SUM(A1)\",1500.50,,-1500.50,10.00");
    }
}
