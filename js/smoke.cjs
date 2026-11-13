// Smoke test of the Node build: node js/smoke.cjs <statement.pdf> [password-file]
const fs = require("fs");
const b2c = require("../pkg/node/bank2csv_wasm.js");

const [file, pwFile] = process.argv.slice(2);
const bytes = fs.readFileSync(file);
const password = pwFile ? fs.readFileSync(pwFile, "utf8").replace(/\r?\n$/, "") : undefined;

console.log("version", b2c.version(), "banks", JSON.stringify(b2c.listBanks()));
for (const [label, opts] of [["no password", {}], ["wrong password", { password: "nope" }]]) {
  try { b2c.convert(bytes, opts); console.log(label, "-> unexpected success"); }
  catch (e) { console.log(label, "->", e instanceof Error, e.code, e.message); }
}
if (password !== undefined) {
  console.log("detect", JSON.stringify(b2c.detect(bytes, { password })));
  const t = Date.now();
  const out = b2c.convert(bytes, { password });
  console.log("converted in", Date.now() - t, "ms; meta", JSON.stringify(out.meta), "warnings", out.warnings);
  fs.writeFileSync(process.env.OUT || "/dev/null", out.csv);
}
