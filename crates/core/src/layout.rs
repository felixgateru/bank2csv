//! Table recovery from positioned spans.
//!
//! PDF statements are tables without table structure. This layer groups spans
//! into lines by baseline and assigns them to columns anchored on a header row.
//! Column anchoring is alignment-aware: numbers are usually right-aligned under
//! their header, text left-aligned, so a span is matched against the edge its
//! column aligns to rather than against fixed x-bands.
//!
//! It lives in `core` (not `extract`) because bank parsers call it with their own
//! header definitions, and fixture tests must not need the PDF engine.

use crate::{Page, Span};

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// 1-based index within the page.
    pub index: u32,
    pub y: f32,
    /// Spans sorted left to right.
    pub spans: Vec<Span>,
}

impl Line {
    pub fn text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ")
    }
    pub fn x0(&self) -> f32 {
        self.spans.first().map(|s| s.x0).unwrap_or(0.0)
    }
}

/// Groups a page's spans into lines. Spans whose baselines differ by less than
/// `tolerance` points share a line.
pub fn lines(page: &Page, tolerance: f32) -> Vec<Line> {
    let mut spans: Vec<&Span> = page.spans.iter().filter(|s| !s.text.trim().is_empty()).collect();
    spans.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x0.total_cmp(&b.x0)));
    let mut out: Vec<Line> = Vec::new();
    for s in spans {
        match out.last_mut() {
            Some(line) if (s.y - line.y).abs() <= tolerance => line.spans.push(s.clone()),
            _ => out.push(Line { index: 0, y: s.y, spans: vec![s.clone()] }),
        }
    }
    for (i, line) in out.iter_mut().enumerate() {
        line.index = i as u32 + 1;
        line.spans.sort_by(|a, b| a.x0.total_cmp(&b.x0));
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
    Center,
}

/// A column a parser expects, with the header labels that identify it.
#[derive(Debug, Clone, Copy)]
pub struct ColumnSpec {
    pub key: &'static str,
    /// Header text variants, compared case-insensitively after trimming.
    pub labels: &'static [&'static str],
    pub align: Align,
}

#[derive(Debug, Clone)]
pub struct Column {
    pub key: &'static str,
    pub align: Align,
    pub x0: f32,
    pub x1: f32,
}

impl Column {
    fn anchor(&self) -> f32 {
        match self.align {
            Align::Left => self.x0,
            Align::Right => self.x1,
            Align::Center => (self.x0 + self.x1) / 2.0,
        }
    }
    fn distance(&self, s: &Span) -> f32 {
        let edge = match self.align {
            Align::Left => s.x0,
            Align::Right => s.x1,
            Align::Center => s.center_x(),
        };
        (edge - self.anchor()).abs()
    }
}

#[derive(Debug, Clone)]
pub struct Columns {
    pub columns: Vec<Column>,
}

impl Columns {
    /// Builds columns from a header line. Header labels may be split across
    /// several spans (e.g. `Value` `Date`), so consecutive spans are joined while
    /// looking for a label. Returns `None` unless every spec is found.
    pub fn from_header(line: &Line, specs: &[ColumnSpec]) -> Option<Columns> {
        let mut columns = Vec::new();
        let spans = &line.spans;
        for spec in specs {
            let mut found = None;
            'outer: for start in 0..spans.len() {
                let mut text = String::new();
                for end in start..spans.len().min(start + 4) {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(spans[end].text.trim());
                    if spec.labels.iter().any(|l| l.eq_ignore_ascii_case(&text)) {
                        found = Some(Column { key: spec.key, align: spec.align, x0: spans[start].x0, x1: spans[end].x1 });
                        break 'outer;
                    }
                }
            }
            columns.push(found?);
        }
        columns.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        Some(Columns { columns })
    }

    /// Splits a line into per-column text, keyed like the specs.
    /// Spans in the same column are joined with a space.
    pub fn assign(&self, line: &Line) -> Row {
        let mut cells: Vec<(&'static str, String)> = self.columns.iter().map(|c| (c.key, String::new())).collect();
        for s in &line.spans {
            let best = self
                .columns
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.distance(s).total_cmp(&b.1.distance(s)))
                .map(|(i, _)| i)
                .expect("at least one column");
            let cell = &mut cells[best].1;
            if !cell.is_empty() {
                cell.push(' ');
            }
            cell.push_str(s.text.trim());
        }
        Row { cells }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub cells: Vec<(&'static str, String)>,
}

impl Row {
    pub fn get(&self, key: &str) -> &str {
        self.cells.iter().find(|(k, _)| *k == key).map(|(_, v)| v.as_str()).unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(text: &str, x0: f32, x1: f32, y: f32) -> Span {
        Span { text: text.into(), x0, x1, y, font_size: 8.0 }
    }

    #[test]
    fn groups_and_assigns() {
        let page = Page {
            number: 1,
            width: 600.0,
            height: 800.0,
            spans: vec![
                span("Date", 20.0, 40.0, 100.0),
                span("Details", 80.0, 110.0, 100.2),
                span("Debit", 300.0, 330.0, 100.0),
                span("Credit", 380.0, 410.0, 100.0),
                span("02/11/2026", 20.0, 60.0, 120.0),
                span("POS", 80.0, 95.0, 120.0),
                span("PURCHASE", 98.0, 140.0, 120.0),
                span("1,250.00", 295.0, 330.0, 120.0),
            ],
        };
        let ls = lines(&page, 1.0);
        assert_eq!(ls.len(), 2);
        let specs = [
            ColumnSpec { key: "date", labels: &["Date"], align: Align::Left },
            ColumnSpec { key: "details", labels: &["Details"], align: Align::Left },
            ColumnSpec { key: "debit", labels: &["Debit"], align: Align::Right },
            ColumnSpec { key: "credit", labels: &["Credit"], align: Align::Right },
        ];
        let cols = Columns::from_header(&ls[0], &specs).unwrap();
        let row = cols.assign(&ls[1]);
        assert_eq!(row.get("date"), "02/11/2026");
        assert_eq!(row.get("details"), "POS PURCHASE");
        assert_eq!(row.get("debit"), "1,250.00");
        assert_eq!(row.get("credit"), "");
    }
}
