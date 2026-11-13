//! Golden tests: every `fixtures/<bank>/<layout>/input.rawdoc.json` must parse,
//! reconcile with the first candidate (no fallback), and match `expected.csv`.

use bank2csv_core::validate::Reconciliation;
use bank2csv_core::{output, registry, ConvertOptions, Error, RawDocument};
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn fixture_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for bank in std::fs::read_dir(fixtures_root()).unwrap().flatten() {
        let name = bank.file_name();
        let name = name.to_string_lossy();
        if !bank.path().is_dir() || name == "private" || name.starts_with('_') {
            continue;
        }
        for layout in std::fs::read_dir(bank.path()).unwrap().flatten() {
            if layout.path().join("input.rawdoc.json").exists() {
                dirs.push(layout.path());
            }
        }
    }
    dirs.sort();
    dirs
}

fn load(dir: &Path) -> RawDocument {
    let text = std::fs::read_to_string(dir.join("input.rawdoc.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn all_fixtures_match_expected_csv() {
    let parsers = bank2csv_banks::parsers();
    let dirs = fixture_dirs();
    assert!(!dirs.is_empty(), "no fixtures found");
    for dir in dirs {
        let bank = dir.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
        let parsed = registry::parse(&parsers, &load(&dir), &ConvertOptions::default())
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        assert_eq!(parsed.meta.bank, bank, "{}: detected the wrong bank", dir.display());
        assert_eq!(parsed.meta.candidates_tried.len(), 1, "{}: needed a fallback parser", dir.display());
        assert_eq!(parsed.meta.reconciled, Reconciliation::Reconciled, "{}", dir.display());
        let csv = output::to_csv(&parsed.statement, &output::CsvOptions::default());
        let expected = std::fs::read_to_string(dir.join("expected.csv")).unwrap();
        assert_eq!(csv, expected, "{}: CSV differs", dir.display());
    }
}

// --- I&M-specific negative tests, built by mutating the synthetic fixture. ---

fn im_fixture() -> RawDocument {
    load(&fixtures_root().join("im_bank_ke/email_v1"))
}

fn spans_mut(doc: &mut RawDocument) -> &mut bank2csv_core::SpanDocument {
    match doc {
        RawDocument::Spans(d) => d,
        _ => unreachable!(),
    }
}

#[test]
fn im_dropped_row_is_caught() {
    let mut doc = im_fixture();
    // Remove every span on the 05-10-26 amount row (the single-line EXCISE DUTY row).
    let page = &mut spans_mut(&mut doc).pages[0];
    let y = page.spans.iter().find(|s| s.text == "05-10-26").unwrap().y;
    page.spans.retain(|s| (s.y - y).abs() > 0.5);
    let err = registry::parse(&bank2csv_banks::parsers(), &doc, &ConvertOptions::default()).unwrap_err();
    assert!(err.is_validation() || matches!(err, Error::ParseFailed { .. }), "{err:?}");
}

#[test]
fn im_misread_amount_is_caught() {
    let mut doc = im_fixture();
    let page = &mut spans_mut(&mut doc).pages[0];
    let s = page.spans.iter_mut().find(|s| s.text == "2,500.50").unwrap();
    s.text = "2,500.05".into();
    let err = registry::parse(&bank2csv_banks::parsers(), &doc, &ConvertOptions::default()).unwrap_err();
    assert_eq!(err.code(), "BALANCE_MISMATCH", "{err:?}");
}

#[test]
fn im_lenient_returns_output_marked_failed() {
    let mut doc = im_fixture();
    let page = &mut spans_mut(&mut doc).pages[0];
    page.spans.iter_mut().find(|s| s.text == "2,500.50").unwrap().text = "2,500.05".into();
    let opts = ConvertOptions { lenient: true, ..Default::default() };
    let parsed = registry::parse(&bank2csv_banks::parsers(), &doc, &opts).unwrap();
    assert_eq!(parsed.meta.reconciled, Reconciliation::Failed);
    assert!(!parsed.warnings.is_empty());
}

#[test]
fn unknown_document_matches_no_parser() {
    let doc = RawDocument::Spans(bank2csv_core::SpanDocument { pages: vec![] });
    let err = registry::parse(&bank2csv_banks::parsers(), &doc, &ConvertOptions::default()).unwrap_err();
    assert_eq!(err.code(), "NO_PARSER_MATCHED");
}
