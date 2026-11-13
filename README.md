# bank2csv

Converts bank statements to CSV, as a Rust library compiled to WebAssembly.
Each bank has its own parser module, and every result is checked against the
statement's own balances before it is returned.

Supported in v0.1:

| Bank | Id | Input | Layouts |
|------|----|-------|---------|
| I&M Bank (Kenya) | `im_bank_ke` | PDF (password-protected) | `email_v1` |

## Use from JavaScript

```js
import init, { convert } from "./pkg/web/bank2csv_wasm.js";
await init();
try {
  const { csv, meta, warnings } = convert(new Uint8Array(buffer), { password });
} catch (e) {
  // e.code: ENCRYPTED, WRONG_PASSWORD, NEEDS_OCR, NO_PARSER_MATCHED, UNRECONCILED, ...
}
```

`convert` is synchronous; run it in a Web Worker. Options: `password`, `bank`,
`lenient`, `format` (`"csv"` or `"json"`), `delimiter`, `splitDebitCredit`.

CSV columns: `date,value_date,description,reference,debit,credit,amount,balance`.
`amount` is signed (negative is money out), and dates are ISO 8601.

## Develop

```sh
cargo test --workspace
cargo run -p bank2csv-cli -- convert statement.pdf --password-file pw.txt
cargo run -p bank2csv-cli -- dump statement.pdf --lines --password-file pw.txt
scripts/build-wasm.sh
```

Keep real statements in `fixtures/private/`. It is git-ignored, and the
end-to-end test picks up any `<name>.pdf` and `<name>.pw` there.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design and how to add
a bank, and [docs/decisions/](docs/decisions/) for the decision log.
