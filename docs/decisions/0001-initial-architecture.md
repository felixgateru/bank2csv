# 0001 — Initial architecture

- Date: 2026-11-13
- Status: Accepted

## Context

We need a WASM module that converts statements from many banks into CSV. Each
bank has its own format, and formats change over time. The first test case is a
password-protected I&M Bank Kenya PDF (AES-128, `/AESV2`, revision 4).

## Decisions

1. **Language: Rust**, compiled to `wasm32-unknown-unknown` with `wasm-bindgen`.
2. **Extraction is separate from parsing.** Bank parsers only see `RawDocument`.
3. **`RawDocument` is an enum** (`Spans`, `Grid`, `Structured`). PDF statements
   are tables without table structure; only x-positions separate debit from
   credit, so the PDF variant keeps positioned text spans and the table finder
   is its own layer.
4. **Money is `i64` minor units**, one signed amount per transaction. Debit and
   credit columns are derived by the writer.
5. **Reconciliation failure is an error** unless the caller passes
   `lenient: true`. Statements with no balances are `Unverifiable`, not errors.
6. **Detection uses tiers** (`Definitive`, `Probable`, `Fallback`), not floats.
   A tie at the top tier is an `AmbiguousBank` error. If the top candidate fails
   reconciliation the next is tried, and `meta.candidates_tried` records it.
7. **Explicit, feature-gated parser list.** No auto-registration macros; constructor
   tricks are unreliable on `wasm32-unknown-unknown`.
8. **One parser per bank, multiple layouts per parser.** The matched layout is
   reported in `meta.layout`.
9. **Typed errors** with stable codes for the JS host.
10. **Fixtures are serialized `RawDocument` JSON**, anonymised by hand. Real
    statements stay in git-ignored `fixtures/private/`.
11. **JSON for profiles**, not YAML (`serde_yaml` is unmaintained, and it adds size).

## Consequences

- PDF support depends on a crate that exposes glyph positions and decrypts
  AES-128. v0.1 uses `lopdf` for decryption and `pdf-extract`'s output-device
  hook for positions; see the v0.1 notes in the changelog.
- `convert()` is synchronous; hosts must call it from a Web Worker for large files.
