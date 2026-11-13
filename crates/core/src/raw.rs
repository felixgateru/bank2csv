//! `RawDocument`: what extractors produce and parsers consume.
//! It is serializable so fixtures can be stored as anonymised JSON and parsers
//! can be tested without the PDF engine.

use crate::model::Statement;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RawDocument {
    /// Positioned text from a PDF. Table structure must be recovered from coordinates.
    Spans(SpanDocument),
    /// Cells from CSV or spreadsheets.
    Grid(GridDocument),
    /// Already-structured formats (OFX, MT940) that map straight to the model.
    Structured(Statement),
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SpanDocument {
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Page {
    /// 1-based page number.
    pub number: u32,
    pub width: f32,
    pub height: f32,
    /// Spans in reading order (top to bottom, then left to right).
    pub spans: Vec<Span>,
}

/// A run of text on one baseline with no large gaps inside it.
/// Coordinates are in PDF points with the origin at the top-left of the page,
/// so `y` grows downward.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub text: String,
    pub x0: f32,
    pub x1: f32,
    /// Baseline y.
    pub y: f32,
    pub font_size: f32,
}

impl Span {
    pub fn center_x(&self) -> f32 {
        (self.x0 + self.x1) / 2.0
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GridDocument {
    pub sheets: Vec<Sheet>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
}

impl SpanDocument {
    /// All text joined, for cheap detection checks.
    pub fn full_text(&self) -> String {
        let mut out = String::new();
        for page in &self.pages {
            for span in &page.spans {
                out.push_str(&span.text);
                out.push('\n');
            }
        }
        out
    }
}

impl RawDocument {
    pub fn kind(&self) -> &'static str {
        match self {
            RawDocument::Spans(_) => "spans",
            RawDocument::Grid(_) => "grid",
            RawDocument::Structured(_) => "structured",
        }
    }
}
