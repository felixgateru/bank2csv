# 0003 — Deterministic transaction ids

- Date: 2026-11-13
- Status: Accepted

## Context

I&M Bank (Kenya) statements print a `Ref No` column that is always empty, so
transactions have no identifier. Downstream consumers need one to deduplicate
re-imports and overlapping statements. Other banks will have the same gap, so
the solution must be generic.

## Decisions

1. **`reference` stays bank-only.** It holds exactly what the bank printed and is
   never generated or overwritten. Generated ids go in a separate field,
   `transaction_id`, so consumers can always tell the two apart.

2. **Ids are deterministic, not random.** Random UUIDs (v4) would produce a new id
   on every import and defeat deduplication. Row-number ids break as soon as two
   statements overlap.

3. **Scheme `v1`:**

   ```
   key = JSON ["v1", bank, account, date, amount, basis, value, occurrence]
   id  = UUIDv5(206120a8-825d-4e8c-8080-bf08a9940efc, key)
   ```

   | Field | Value |
   |-------|-------|
   | `bank` | parser id, e.g. `im_bank_ke` |
   | `account` | account number as printed, upper-cased, whitespace removed (masks are fine) |
   | `date` | posting date, ISO 8601 (not the value date) |
   | `amount` | signed integer minor units, e.g. `-1000000` |
   | `basis`, `value` | `"bal"` + balance after the transaction in minor units; or, when no balance is shown, `"desc"` + description upper-cased with whitespace collapsed |
   | `occurrence` | `"1"`, `"2"`, ... for repeats of the same key, in chronological order |

   - The running balance is preferred because it distinguishes identical-looking
     transactions on the same day (two KES 10,000 transfers) and does not depend
     on how wrapped narrative text was joined or truncated. The PDF and a CSV
     export of the same transaction therefore get the same id.
   - The occurrence number covers a balance returning to the same value on one
     day (+100, −100, +100).
   - JSON-encoding the key makes it unambiguous whatever characters the fields
     contain.

4. **UUIDv5 format**, so ids fit a standard database `uuid` column.

5. **On by default**, with an opt-out: `generateIds: false` (JS),
   `generate_ids` (Rust), `--no-ids` (CLI). The CSV always has a
   `transaction_id` first column, empty when ids are off, so the column set does
   not depend on options.

6. **Reported in meta:** `id_scheme` (`"v1"`) and `id_basis` (`balance`,
   `description` or `mixed`); both are `null` when ids are off.

7. **The scheme is frozen.** A unit test pins an exact v1 id. The synthetic
   fixture generator computes ids with an independent Python implementation, so
   the golden test cross-checks the Rust code. Any change to the inputs is a new
   scheme version with its own decision record. The old version stays available.

## Known limits

- **No account number:** ids are only unique within one statement. A warning is
  emitted.
- **Description basis** (no running balance): weaker, since the same transaction
  can be worded differently across export formats. A warning is emitted.
- **Statements split mid-day:** if a statement starts partway through a day that
  has repeated identical keys, occurrence numbers can shift between statements.
  I&M statements end on month boundaries.
- **Bank corrections** that rewrite history change the affected balances, and so
  the ids. This is arguably correct, since the record itself changed.
