//! Balance reconciliation. A statement that doesn't add up produced a wrong CSV,
//! so failures are errors unless the caller opts into lenient mode.

use crate::{Error, Money, Statement, Transaction};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reconciliation {
    /// Every check that the statement's data allows passed.
    Reconciled,
    /// No opening/closing or running balances to check against.
    Unverifiable,
    /// Checks failed but the caller asked for lenient mode.
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    Chronological,
    NewestFirst,
    /// Dates go both up and down. Allowed, but reported as a warning.
    Mixed,
}

pub fn date_order(txns: &[Transaction]) -> Order {
    let up = txns.windows(2).any(|w| w[1].date > w[0].date);
    let down = txns.windows(2).any(|w| w[1].date < w[0].date);
    match (up, down) {
        (_, false) => Order::Chronological,
        (false, true) => Order::NewestFirst,
        (true, true) => Order::Mixed,
    }
}

/// Checks running balances and opening + Σ = closing. Returns the first failure.
pub fn check(stmt: &Statement) -> Result<Reconciliation, Error> {
    let order = date_order(&stmt.transactions);
    // Validate in chronological order regardless of how the statement is printed.
    let chrono: Vec<(usize, &Transaction)> = match order {
        Order::NewestFirst => stmt.transactions.iter().enumerate().rev().collect(),
        _ => stmt.transactions.iter().enumerate().collect(),
    };

    let mut checked_anything = false;
    let mut running: Option<Money> = stmt.opening_balance;
    for (index, t) in &chrono {
        if let (Some(prev), Some(shown)) = (running, t.balance) {
            let computed = prev + t.amount;
            if computed != shown {
                return Err(Error::BalanceMismatch { index: *index, expected: shown, actual: computed });
            }
            checked_anything = true;
        }
        running = t.balance.or(running.map(|r| r + t.amount));
    }

    if let (Some(open), Some(close)) = (stmt.opening_balance, stmt.closing_balance) {
        let total: Money = stmt.transactions.iter().fold(Money::ZERO, |acc, t| acc + t.amount);
        let computed = open + total;
        if computed != close {
            return Err(Error::Unreconciled { expected: close, actual: computed });
        }
        checked_anything = true;
    } else if let (Some(close), Some(last)) = (stmt.closing_balance, chrono.last().and_then(|(_, t)| t.balance)) {
        if close != last {
            return Err(Error::Unreconciled { expected: close, actual: last });
        }
        checked_anything = true;
    }

    let (debits, credits) = stmt.transactions.iter().fold((Money::ZERO, Money::ZERO), |(d, c), t| {
        if t.amount.is_negative() { (d + t.amount.abs(), c) } else { (d, c + t.amount) }
    });
    for (kind, printed, actual) in [("debit", stmt.total_debits, debits), ("credit", stmt.total_credits, credits)] {
        if let Some(expected) = printed {
            if expected != actual {
                return Err(Error::TotalsMismatch { kind, expected, actual });
            }
            checked_anything = true;
        }
    }

    Ok(if checked_anything { Reconciliation::Reconciled } else { Reconciliation::Unverifiable })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Date;

    fn tx(day: u8, amount: i64, balance: Option<i64>) -> Transaction {
        Transaction {
                transaction_id: None,
            date: Date::new(2026, 10, day).unwrap(),
            value_date: None,
            description: String::new(),
            reference: None,
            amount: Money(amount),
            balance: balance.map(Money),
            source: None,
        }
    }

    fn stmt(open: Option<i64>, close: Option<i64>, txns: Vec<Transaction>) -> Statement {
        Statement {
            opening_balance: open.map(Money),
            closing_balance: close.map(Money),
            transactions: txns,
            ..Default::default()
        }
    }

    #[test]
    fn reconciles_chronological() {
        let s = stmt(Some(1000), Some(700), vec![tx(1, -500, Some(500)), tx(2, 200, Some(700))]);
        assert_eq!(check(&s), Ok(Reconciliation::Reconciled));
    }

    #[test]
    fn reconciles_newest_first() {
        let s = stmt(Some(1000), Some(700), vec![tx(2, 200, Some(700)), tx(1, -500, Some(500))]);
        assert_eq!(check(&s), Ok(Reconciliation::Reconciled));
    }

    #[test]
    fn detects_running_mismatch() {
        let s = stmt(Some(1000), None, vec![tx(1, -500, Some(500)), tx(2, 200, Some(800))]);
        assert!(matches!(check(&s), Err(Error::BalanceMismatch { index: 1, .. })));
    }

    #[test]
    fn detects_closing_mismatch() {
        let s = stmt(Some(1000), Some(999), vec![tx(1, -500, None)]);
        assert!(matches!(check(&s), Err(Error::Unreconciled { .. })));
    }

    #[test]
    fn unverifiable_without_balances() {
        let s = stmt(None, None, vec![tx(1, -500, None)]);
        assert_eq!(check(&s), Ok(Reconciliation::Unverifiable));
    }
}
