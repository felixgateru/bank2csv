//! Fixed-point money. Amounts are stored as `i64` minor units (cents) so that
//! sums are exact. Never use `f64` for money.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Neg, Sub};

/// A signed amount in minor units (two decimal places).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(pub i64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub fn minor(self) -> i64 {
        self.0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub fn abs(self) -> Money {
        Money(self.0.abs())
    }

    /// Parses amounts as they appear on statements:
    /// `1,234.56`, `-1,234.56`, `(1,234.56)`, `1,234.56-`, `1,234.56 DR`, `1,234.56CR`, `1234`.
    /// A trailing `DR` or a minus sign makes the value negative.
    /// Returns `None` if the text is not an amount.
    pub fn parse(text: &str) -> Option<Money> {
        let mut s = text.trim().to_ascii_uppercase();
        if s.is_empty() {
            return None;
        }
        let mut negative = false;
        for suffix in ["DR", "CR"] {
            if let Some(rest) = s.strip_suffix(suffix) {
                negative = suffix == "DR";
                s = rest.trim_end().to_string();
                break;
            }
        }
        if s.starts_with('(') && s.ends_with(')') {
            negative = true;
            s = s[1..s.len() - 1].trim().to_string();
        }
        if let Some(rest) = s.strip_prefix('-') {
            negative = !negative;
            s = rest.trim_start().to_string();
        } else if let Some(rest) = s.strip_suffix('-') {
            negative = !negative;
            s = rest.trim_end().to_string();
        } else if let Some(rest) = s.strip_prefix('+') {
            s = rest.trim_start().to_string();
        }
        let s: String = s.chars().filter(|c| *c != ',' && *c != ' ').collect();
        if s.is_empty() {
            return None;
        }
        let (int_part, frac_part) = match s.split_once('.') {
            Some((i, f)) => (i, f),
            None => (s.as_str(), ""),
        };
        if int_part.is_empty() && frac_part.is_empty() {
            return None;
        }
        if !int_part.chars().all(|c| c.is_ascii_digit()) || !frac_part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if frac_part.len() > 2 {
            return None;
        }
        let int_val: i64 = if int_part.is_empty() { 0 } else { int_part.parse().ok()? };
        let frac_val: i64 = match frac_part.len() {
            0 => 0,
            1 => frac_part.parse::<i64>().ok()? * 10,
            _ => frac_part.parse().ok()?,
        };
        let minor = int_val.checked_mul(100)?.checked_add(frac_val)?;
        Some(Money(if negative { -minor } else { minor }))
    }
}

impl fmt::Display for Money {
    /// Plain decimal with two places and no thousands separators, e.g. `-1234.50`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        write!(f, "{}{}.{:02}", sign, abs / 100, abs % 100)
    }
}

impl Add for Money {
    type Output = Money;
    fn add(self, rhs: Money) -> Money {
        Money(self.0 + rhs.0)
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Money) {
        self.0 += rhs.0;
    }
}

impl Sub for Money {
    type Output = Money;
    fn sub(self, rhs: Money) -> Money {
        Money(self.0 - rhs.0)
    }
}

impl Neg for Money {
    type Output = Money;
    fn neg(self) -> Money {
        Money(-self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_forms() {
        assert_eq!(Money::parse("1,234.56"), Some(Money(123456)));
        assert_eq!(Money::parse("-1,234.56"), Some(Money(-123456)));
        assert_eq!(Money::parse("(1,234.56)"), Some(Money(-123456)));
        assert_eq!(Money::parse("1,234.56-"), Some(Money(-123456)));
        assert_eq!(Money::parse("1,234.56 DR"), Some(Money(-123456)));
        assert_eq!(Money::parse("1,234.56CR"), Some(Money(123456)));
        assert_eq!(Money::parse("1234"), Some(Money(123400)));
        assert_eq!(Money::parse("0.5"), Some(Money(50)));
        assert_eq!(Money::parse(".50"), Some(Money(50)));
    }

    #[test]
    fn rejects_non_amounts() {
        for s in ["", "abc", "12/10/2026", "1.234", "-", "1.2.3", "KES"] {
            assert_eq!(Money::parse(s), None, "{s}");
        }
    }

    #[test]
    fn displays() {
        assert_eq!(Money(-123450).to_string(), "-1234.50");
        assert_eq!(Money(5).to_string(), "0.05");
        assert_eq!(Money(0).to_string(), "0.00");
    }
}
