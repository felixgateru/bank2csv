//! A minimal calendar date. We avoid `chrono` to keep the WASM binary small;
//! statements only need parsing, ordering and ISO output.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

/// Day-first or month-first interpretation of all-numeric dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateOrder {
    DayFirst,
    MonthFirst,
}

impl Date {
    pub fn new(year: u16, month: u8, day: u8) -> Option<Date> {
        if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
            return None;
        }
        Some(Date { year, month, day })
    }

    /// Parses `2026-11-02`, `02/11/2026`, `02-11-2026`, `02.11.26`, `02-Nov-2026`,
    /// `02 Nov 2026`, `02NOV26`, `Nov 02, 2026`. Numeric dates use `order`.
    pub fn parse(text: &str, order: DateOrder) -> Option<Date> {
        let s = text.trim().to_ascii_uppercase().replace(',', " ");
        if s.is_empty() {
            return None;
        }
        // Split into alternating runs of digits and letters, ignoring separators.
        let mut parts: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut cur_kind = 0u8; // 1 digit, 2 alpha
        for c in s.chars() {
            let kind = if c.is_ascii_digit() {
                1
            } else if c.is_ascii_alphabetic() {
                2
            } else if matches!(c, '-' | '/' | '.' | ' ') {
                0
            } else {
                return None;
            };
            if kind != cur_kind && !cur.is_empty() {
                parts.push(std::mem::take(&mut cur));
            }
            if kind != 0 {
                cur.push(c);
            }
            cur_kind = kind;
        }
        if !cur.is_empty() {
            parts.push(cur);
        }
        if parts.len() != 3 {
            return None;
        }
        let month_name = |p: &str| -> Option<u8> {
            if p.len() < 3 {
                return None;
            }
            MONTHS.iter().position(|m| p.starts_with(m)).map(|i| i as u8 + 1)
        };
        let num = |p: &str| -> Option<u32> {
            if p.chars().all(|c| c.is_ascii_digit()) && p.len() <= 4 {
                p.parse().ok()
            } else {
                None
            }
        };
        let year = |v: u32, len: usize| -> Option<u16> {
            match len {
                4 => Some(v as u16),
                2 => Some(2000 + v as u16),
                _ => None,
            }
        };
        let (a, b, c) = (&parts[0], &parts[1], &parts[2]);
        if let Some(m) = month_name(b) {
            // 02 NOV 2026
            return Date::new(year(num(c)?, c.len())?, m, num(a)? as u8);
        }
        if let Some(m) = month_name(a) {
            // NOV 02 2026
            return Date::new(year(num(c)?, c.len())?, m, num(b)? as u8);
        }
        let (na, nb, nc) = (num(a)?, num(b)?, num(c)?);
        if a.len() == 4 {
            return Date::new(na as u16, nb as u8, nc as u8);
        }
        let y = year(nc, c.len())?;
        match order {
            DateOrder::DayFirst => Date::new(y, nb as u8, na as u8),
            DateOrder::MonthFirst => Date::new(y, na as u8, nb as u8),
        }
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => 29,
        2 => 28,
        _ => 0,
    }
}

impl fmt::Display for Date {
    /// ISO 8601: `2026-11-02`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DateOrder::*;

    #[test]
    fn parses_formats() {
        let d = Date::new(2026, 11, 2).unwrap();
        for s in ["2026-11-02", "02/11/2026", "02-11-2026", "02.11.26", "02-Nov-2026", "02 Nov 2026", "02NOV26", "Nov 02, 2026", "2 November 2026"] {
            assert_eq!(Date::parse(s, DayFirst), Some(d), "{s}");
        }
        assert_eq!(Date::parse("11/02/2026", MonthFirst), Some(d));
    }

    #[test]
    fn rejects_invalid() {
        for s in ["", "31/02/2026", "1,234.56", "hello", "2026-13-01"] {
            assert_eq!(Date::parse(s, DayFirst), None, "{s}");
        }
    }
}
