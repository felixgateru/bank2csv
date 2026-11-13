//! Deterministic transaction ids.
//!
//! Many statements (I&M Bank Kenya among them) print no per-transaction
//! reference. Downstream systems still need a key to deduplicate re-imports and
//! overlapping statements, so we derive one from the transaction itself:
//!
//! ```text
//! key = JSON ["v1", bank, account, date, amount, basis, value, occurrence]
//! id  = UUIDv5(NAMESPACE, key)
//! ```
//!
//! - `bank`: parser id (`im_bank_ke`).
//! - `account`: account number as printed, upper-cased, whitespace removed.
//!   Masked numbers (`001XXXXXXX0001`) are fine: the mask is stable per account.
//! - `date`: posting date, ISO 8601. `amount`: signed minor units, e.g. `-1000000`.
//! - `basis`/`value`: `"bal"` and the balance after the transaction in minor
//!   units when the statement shows one; otherwise `"desc"` and the description
//!   upper-cased with whitespace collapsed. The balance is preferred because it
//!   tells apart identical-looking transactions on the same day and does not
//!   depend on how wrapped narrative text was joined.
//! - `occurrence`: 1, 2, ... for repeats of the same key within the statement,
//!   counted in chronological order. Covers e.g. +100, -100, +100 on one day.
//!
//! The bank's own `reference` is deliberately not part of the key and is never
//! overwritten: it stays exactly what the bank printed.
//!
//! **Changing anything above changes every id and breaks consumers.** Bump
//! `SCHEME`, keep the old scheme available, and add a decision record.

use crate::validate::{date_order, Order};
use crate::Statement;
use std::collections::HashMap;
use uuid::Uuid;

/// Version of the id scheme, reported in `meta.id_scheme`.
pub const SCHEME: &str = "v1";

/// Fixed namespace for bank2csv ids. Never change it.
pub const NAMESPACE: Uuid = Uuid::from_u128(0x206120a8_825d_4e8c_8080_bf08a9940efc);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// Every id was keyed on the running balance.
    Balance,
    /// Every id was keyed on the description (no balances on the statement).
    Description,
    /// Some rows had balances and some did not.
    Mixed,
}

/// Fills `transaction_id` on every transaction. Returns which basis was used and
/// any warnings about how unique the ids are.
pub fn assign(stmt: &mut Statement) -> (Basis, Vec<String>) {
    let mut warnings = Vec::new();
    let account = normalise_account(stmt.account.as_deref().unwrap_or(""));
    if account.is_empty() {
        warnings.push("no account number on the statement; transaction ids are only unique within this file".into());
    }

    let order: Vec<usize> = match date_order(&stmt.transactions) {
        Order::NewestFirst => (0..stmt.transactions.len()).rev().collect(),
        _ => (0..stmt.transactions.len()).collect(),
    };

    let mut seen: HashMap<String, u32> = HashMap::new();
    let (mut by_balance, mut by_desc) = (0usize, 0usize);
    for i in order {
        let t = &stmt.transactions[i];
        let (basis, value) = match t.balance {
            Some(b) => {
                by_balance += 1;
                ("bal", b.minor().to_string())
            }
            None => {
                by_desc += 1;
                ("desc", normalise_text(&t.description))
            }
        };
        let base = serde_json::to_string(&[
            SCHEME,
            stmt.bank.as_str(),
            account.as_str(),
            &t.date.to_string(),
            &t.amount.minor().to_string(),
            basis,
            value.as_str(),
        ])
        .expect("strings serialize");
        let n = seen.entry(base.clone()).or_insert(0);
        *n += 1;
        // Append the occurrence as the final array element.
        let key = format!("{},\"{}\"]", &base[..base.len() - 1], n);
        stmt.transactions[i].transaction_id = Some(Uuid::new_v5(&NAMESPACE, key.as_bytes()).to_string());
    }

    let basis = match (by_balance, by_desc) {
        (_, 0) => Basis::Balance,
        (0, _) => Basis::Description,
        _ => Basis::Mixed,
    };
    if basis != Basis::Balance && !stmt.transactions.is_empty() {
        warnings.push(
            "some transactions have no running balance; their ids are keyed on the description, \
             which may differ between export formats"
                .into(),
        );
    }
    (basis, warnings)
}

fn normalise_account(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase()
}

fn normalise_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Date, Money, Transaction};

    fn tx(day: u8, amount: i64, balance: Option<i64>, desc: &str) -> Transaction {
        Transaction {
            transaction_id: None,
            date: Date::new(2026, 10, day).unwrap(),
            value_date: None,
            description: desc.into(),
            reference: None,
            amount: Money(amount),
            balance: balance.map(Money),
            source: None,
        }
    }

    fn stmt(txns: Vec<Transaction>) -> Statement {
        Statement { bank: "test_bank".into(), account: Some("001 xxx 0001".into()), transactions: txns, ..Default::default() }
    }

    fn ids(s: &Statement) -> Vec<String> {
        s.transactions.iter().map(|t| t.transaction_id.clone().unwrap()).collect()
    }

    fn all_distinct(v: &[String]) -> bool {
        let mut sorted = v.to_vec();
        sorted.sort();
        sorted.dedup();
        sorted.len() == v.len()
    }

    /// Pins the scheme. If this fails, every consumer's ids changed: bump SCHEME
    /// and write a decision record instead of updating the expected value.
    #[test]
    fn scheme_v1_is_stable() {
        let mut s = stmt(vec![tx(1, -1_000_000, Some(8_558_718), "MPESA Payment")]);
        assign(&mut s);
        let key = r#"["v1","test_bank","001XXX0001","2026-10-01","-1000000","bal","8558718","1"]"#;
        assert_eq!(ids(&s)[0], Uuid::new_v5(&NAMESPACE, key.as_bytes()).to_string());
        assert_eq!(ids(&s)[0], "e670159f-3a30-5b1b-aa57-883623cac94b");
    }

    #[test]
    fn identical_transfers_same_day_get_distinct_ids() {
        let mut s = stmt(vec![
            tx(1, -1_000_000, Some(9_000_000), "MPESA Payment"),
            tx(1, -1_000_000, Some(8_000_000), "MPESA Payment"),
        ]);
        assign(&mut s);
        assert!(all_distinct(&ids(&s)));
    }

    #[test]
    fn balance_returning_to_same_value_uses_occurrence() {
        // +100, -100, +100: rows 0 and 2 have the same date, amount and balance.
        let mut s = stmt(vec![tx(1, 10_000, Some(110_000), "IN"), tx(1, -10_000, Some(100_000), "OUT"), tx(1, 10_000, Some(110_000), "IN")]);
        assign(&mut s);
        assert!(all_distinct(&ids(&s)));
    }

    #[test]
    fn overlapping_statements_share_ids() {
        let all = [
            tx(1, -500, Some(9_500), "A"),
            tx(2, -500, Some(9_000), "B"),
            tx(3, 2_000, Some(11_000), "C"),
            tx(4, -1_000, Some(10_000), "D"),
        ];
        let mut first = stmt(all[..3].to_vec());
        let mut second = stmt(all[1..].to_vec());
        assign(&mut first);
        assign(&mut second);
        assert_eq!(ids(&first)[1..], ids(&second)[..2]);
    }

    #[test]
    fn newest_first_gives_same_ids() {
        let rows = [tx(1, 10_000, Some(110_000), "IN"), tx(1, -10_000, Some(100_000), "OUT"), tx(2, 10_000, Some(110_000), "IN")];
        let mut chrono = stmt(rows.to_vec());
        let mut reversed = stmt(rows.iter().rev().cloned().collect());
        assign(&mut chrono);
        assign(&mut reversed);
        let mut r = ids(&reversed);
        r.reverse();
        assert_eq!(ids(&chrono), r);
    }

    #[test]
    fn description_text_does_not_affect_balance_keyed_ids() {
        let mut a = stmt(vec![tx(1, -500, Some(9_500), "PesaPal*Kuku Foods Ken")]);
        let mut b = stmt(vec![tx(1, -500, Some(9_500), "PesaPal*Kuku Foods Kenya Ltd")]);
        assign(&mut a);
        assign(&mut b);
        assert_eq!(ids(&a), ids(&b));
    }

    #[test]
    fn falls_back_to_description_and_warns() {
        let mut s = stmt(vec![tx(1, -500, None, "Coffee  shop"), tx(1, -500, None, "COFFEE SHOP"), tx(1, -500, None, "Bus")]);
        let (basis, warnings) = assign(&mut s);
        assert_eq!(basis, Basis::Description);
        assert!(!warnings.is_empty());
        let v = ids(&s);
        // Same normalised description: distinguished only by occurrence.
        assert!(all_distinct(&v));
    }

    #[test]
    fn missing_account_warns() {
        let mut s = stmt(vec![tx(1, -500, Some(9_500), "A")]);
        s.account = None;
        let (_, warnings) = assign(&mut s);
        assert!(warnings.iter().any(|w| w.contains("account")));
    }

    #[test]
    fn other_banks_and_accounts_get_other_ids() {
        let mut a = stmt(vec![tx(1, -500, Some(9_500), "A")]);
        let mut b = a.clone();
        b.bank = "other_bank".into();
        let mut c = a.clone();
        c.account = Some("002".into());
        assign(&mut a);
        assign(&mut b);
        assign(&mut c);
        assert!(all_distinct(&[ids(&a)[0].clone(), ids(&b)[0].clone(), ids(&c)[0].clone()]));
    }
}
