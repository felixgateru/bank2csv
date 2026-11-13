//! End-to-end tests against real statements kept in the git-ignored
//! `fixtures/private/`. For each `<name>.pdf` there:
//!
//! - `<name>.pw`            password (optional)
//! - `<name>.expected.csv`  expected output (optional; created with `BLESS=1`)
//!
//! With no private files the test does nothing, so CI and fresh clones pass.

use bank2csv_core::validate::Reconciliation;
use bank2csv_core::{output, registry, ConvertOptions};
use std::path::Path;

#[test]
fn private_statements_convert_and_reconcile() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/private");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("no fixtures/private; skipping");
        return;
    };
    let parsers = bank2csv_banks::parsers();
    let bless = std::env::var("BLESS").is_ok();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pdf") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let pw = std::fs::read_to_string(dir.join(format!("{stem}.pw"))).ok();
        let pw = pw.as_deref().map(|p| p.trim_end_matches(['\r', '\n']));
        let bytes = std::fs::read(&path).unwrap();
        let doc = bank2csv_extract::extract(&bytes, pw).unwrap_or_else(|e| panic!("{stem}: {e}"));
        let parsed = registry::parse(&parsers, &doc, &ConvertOptions::default()).unwrap_or_else(|e| panic!("{stem}: {e}"));
        assert_eq!(parsed.meta.reconciled, Reconciliation::Reconciled, "{stem}");
        assert_eq!(parsed.meta.candidates_tried.len(), 1, "{stem}: needed a fallback");
        let csv = output::to_csv(&parsed.statement, &output::CsvOptions::default());
        let expected_path = dir.join(format!("{stem}.expected.csv"));
        if bless {
            std::fs::write(&expected_path, &csv).unwrap();
        } else if let Ok(expected) = std::fs::read_to_string(&expected_path) {
            assert_eq!(csv, expected, "{stem}: CSV differs from {}", expected_path.display());
        }
        eprintln!("{stem}: ok, {} transactions", parsed.meta.transaction_count);
    }
}
