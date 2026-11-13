# bank2csv — Architecture

Status: living document. Significant changes are recorded as decision records in
[`docs/decisions/`](decisions/) and summarised in the changelog at the end of this file.

## Goal

A Rust library compiled to WebAssembly that converts bank statements (PDF, CSV,
XLSX, OFX, MT940) into a canonical transaction model and writes CSV or JSON.
Each bank's format is handled by its own module, so adding a bank does not touch
the rest of the system.

v0.1 scope: PDF input only, one bank (I&M Bank Kenya, layout `email_v1`).
CSV/XLSX, OFX/MT940 and JSON profiles are designed but not built yet.

Non-goals for now: OCR of scanned statements (we detect them and return
`NeedsOcr`), any network or filesystem access inside the WASM module.

## Pipeline

```
HOST (JS/TS) — calls from a Web Worker; convert() is synchronous and CPU-bound
  convert(bytes, { bank?, password?, lenient? })
        │
        ▼
1. API        convert · detect · list_banks · version
              Result<Output, Error> → typed JS error { code, message, ... }
        │
2. EXTRACT    sniff magic bytes
              PDF:   decrypt(password) → image-only? → NeedsOcr → positioned spans
              CSV/XLSX → cell grid
              OFX/MT940 → structured transactions (skip steps 3–5)
        │
              enum RawDocument { Spans(..), Grid(..), Structured(Statement) }
        │
3. LAYOUT     (PDF only, core::layout, called by each parser)
              spans → lines by baseline; columns anchored on the header row,
              matched by the edge each column aligns to (left/right/center).
              Parsers attach wrapped narrative lines to the nearest amount row.
        │
4. REGISTRY   explicit, feature-gated list of parsers (no auto-registration)
              detect → Tier { Definitive | Probable | Fallback }
              tie at the top tier → AmbiguousBank
        │
5. PARSERS    crates/banks/src/<bank>/ — each parser owns one or more layout variants
              generic profile parser driven by JSON column mappings
        │
6. MODEL      Statement { bank, layout, account, currency, period,
                          opening?, closing?, total_debits?, total_credits?,
                          transactions }
              Transaction { date, value_date?, description, reference?,
                            amount: Money (signed), balance?: Money }
              Money = i64 minor units. Never f64. Debit/credit are derived on output.
        │
7. VALIDATE   running balance row by row; opening + Σ amount == closing;
              printed debit/credit totals == Σ of transactions;
              dates monotonic in either direction (newest-first is allowed)
              Reconciled → ok · Unverifiable (no balances) → ok + warning
              Failed → try next candidate parser; all fail → Unreconciled error
              (lenient: true → return output with reconciled = false)
        │
8. OUTPUT     CSV / JSON; debit and credit columns derived from the signed amount
        │
        ▼
  { csv, meta: { bank, layout, reconciled, candidates_tried }, warnings }
```

### Errors

| Code | Meaning | Kind |
|------|---------|------|
| `ENCRYPTED` | password needed | extraction |
| `WRONG_PASSWORD` | password rejected | extraction |
| `NEEDS_OCR` | no text layer | extraction |
| `UNSUPPORTED_FORMAT` | not a format we read | extraction |
| `CORRUPT` | file could not be read | extraction |
| `NO_PARSER_MATCHED` | no parser recognised it | detection |
| `AMBIGUOUS_BANK` | tie at the top tier | detection |
| `UNKNOWN_BANK` | `bank` option names no parser | detection |
| `PARSE_FAILED` | layout not understood (`page`, `line`, `reason`) | parsing |
| `BALANCE_MISMATCH` | running balance broken at a row | validation |
| `UNRECONCILED` | opening + Σ ≠ closing | validation |
| `TOTALS_MISMATCH` | printed totals ≠ Σ of rows | validation |

Validation errors may trigger a fallback to the next candidate parser, and
are the only errors `lenient: true` turns into warnings.

Each maps to a stable string `code` on the JS side so hosts can react (for example,
prompt for a password on `Encrypted`).

## Repository layout

```
bank2csv/
├── crates/
│   ├── core/     model, Money, Date, errors, BankParser trait, registry,
│   │             layout (table recovery), validate, output
│   ├── extract/  format sniffing; pdf (decrypt, positioned spans, OCR check)
│   ├── banks/    one crate; one module per bank, each behind a Cargo feature
│   ├── wasm/     wasm-bindgen API
│   └── cli/      native CLI for development and debugging (not shipped in WASM)
├── profiles/     JSON column-mapping profiles for CSV-export banks (planned)
├── tools/fixtures/  generators for synthetic fixtures
├── scripts/build-wasm.sh
├── js/smoke.cjs     Node smoke test of the WASM build
├── fixtures/<bank>/<layout>/
│   ├── input.rawdoc.json   serialized RawDocument, anonymised by hand
│   └── expected.csv
└── docs/
    ├── ARCHITECTURE.md
    └── decisions/          numbered decision records
```

`fixtures/private/` is git-ignored. Real statements and their raw extractions
live there only, and must never be committed.

## Adding a bank

1. Create `crates/banks/src/<bank>/mod.rs` implementing `BankParser`.
2. Add a Cargo feature `<bank>` in `crates/banks/Cargo.toml` and add it to `default`.
3. Add one line to the list in `crates/banks/src/lib.rs::parsers()`.
4. Dump a real statement locally: `bank2csv dump file.pdf --lines --password-file F`
   (keep everything under `fixtures/private/`).
5. Add a synthetic fixture under `fixtures/<bank>/<layout>/` (`input.rawdoc.json`
   plus `expected.csv`). Copy the real coordinates and quirks; invent every name,
   number and amount. Commit the generator under `tools/fixtures/`.
6. Record the layout's quirks in the module's doc comment, and add a changelog row.

## Testing

- Golden tests: each `fixtures/<bank>/<layout>/input.rawdoc.json` is parsed and
  compared with `expected.csv`. These run without the PDF engine.
- CI fails if any fixture reconciled only through a fallback candidate.
- Negative tests mutate a fixture (drop a row, alter an amount) and expect a
  validation error.
- Private end-to-end test: every `fixtures/private/<name>.pdf` (with optional
  `<name>.pw`) must convert and reconcile; `BLESS=1` writes `<name>.expected.csv`
  and later runs compare against it. With no private files the test is a no-op.

## Building

```
cargo test --workspace
scripts/build-wasm.sh          # → pkg/web (ES module) and pkg/node (CommonJS)
node js/smoke.cjs statement.pdf password-file
```

v0.1 binary: 1.07 MB, or 505 KB gzipped, after `wasm-opt -Oz` (1.48 MB / 565 KB without it). PDF parsing dominates.

## Changelog

| Version | Date       | Change |
|---------|------------|--------|
| 0.1.0   | 2026-11-13 | Initial architecture; I&M Bank Kenya PDF parser as first bank. See [0001](decisions/0001-initial-architecture.md). |
| 0.1.0   | 2026-11-13 | Changes made while implementing v0.1: decrypt during load, layout in `core`, printed totals in the model. See [0002](decisions/0002-v0.1-implementation.md). |
