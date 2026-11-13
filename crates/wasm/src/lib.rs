//! JavaScript API.
//!
//! ```js
//! import init, { convert, detect, listBanks, version } from "./bank2csv.js";
//! await init();
//! try {
//!   const { csv, meta, warnings } = convert(bytes, { password, bank, lenient, format: "csv" });
//! } catch (e) {
//!   if (e.code === "ENCRYPTED") askForPassword();
//! }
//! ```
//!
//! `convert` is synchronous and CPU-bound: call it from a Web Worker.
//! The module has no network or filesystem access.

use bank2csv_core::{output, registry, ConvertOptions, Error};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct JsOptions {
    password: Option<String>,
    bank: Option<String>,
    lenient: bool,
    /// `csv` (default) or `json`.
    format: Option<String>,
    delimiter: Option<char>,
    split_debit_credit: Option<bool>,
}

#[derive(Serialize)]
struct JsOutput<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    csv: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    statement: Option<&'a bank2csv_core::Statement>,
    meta: &'a registry::Meta,
    warnings: &'a [String],
}

#[derive(Serialize)]
struct JsBank {
    id: &'static str,
    name: &'static str,
    layouts: &'static [&'static str],
}

fn js_error(e: Error) -> JsValue {
    let err = js_sys_error(&e.to_string());
    let _ = js_set(&err, "code", &JsValue::from_str(e.code()));
    err
}

fn js_sys_error(msg: &str) -> JsValue {
    // `Error` objects keep stack traces and work with `instanceof Error`.
    wasm_bindgen::JsError::new(msg).into()
}

fn js_set(target: &JsValue, key: &str, value: &JsValue) -> Result<(), JsValue> {
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = Reflect, js_name = set, catch)]
        fn reflect_set(target: &JsValue, key: &JsValue, value: &JsValue) -> Result<bool, JsValue>;
    }
    reflect_set(target, &JsValue::from_str(key), value).map(|_| ())
}

fn options(value: JsValue) -> Result<JsOptions, JsValue> {
    if value.is_undefined() || value.is_null() {
        return Ok(JsOptions::default());
    }
    serde_wasm_bindgen::from_value(value).map_err(|e| js_sys_error(&format!("invalid options: {e}")))
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| js_sys_error(&e.to_string()))
}

/// Converts a statement file to CSV (or JSON with `format: "json"`).
#[wasm_bindgen]
pub fn convert(bytes: &[u8], opts: JsValue) -> Result<JsValue, JsValue> {
    let opts = options(opts)?;
    let doc = bank2csv_extract::extract(bytes, opts.password.as_deref()).map_err(js_error)?;
    let parsers = bank2csv_banks::parsers();
    let parsed = registry::parse(&parsers, &doc, &ConvertOptions { bank: opts.bank.clone(), lenient: opts.lenient })
        .map_err(js_error)?;
    let json = opts.format.as_deref() == Some("json");
    let csv = (!json).then(|| {
        let mut csv_opts = output::CsvOptions::default();
        if let Some(d) = opts.delimiter {
            csv_opts.delimiter = d;
        }
        if let Some(s) = opts.split_debit_credit {
            csv_opts.split_debit_credit = s;
        }
        output::to_csv(&parsed.statement, &csv_opts)
    });
    to_js(&JsOutput {
        csv,
        statement: json.then_some(&parsed.statement),
        meta: &parsed.meta,
        warnings: &parsed.warnings,
    })
}

/// Returns the ids of parsers that recognise the file, best first.
#[wasm_bindgen]
pub fn detect(bytes: &[u8], opts: JsValue) -> Result<JsValue, JsValue> {
    let opts = options(opts)?;
    let doc = bank2csv_extract::extract(bytes, opts.password.as_deref()).map_err(js_error)?;
    let parsers = bank2csv_banks::parsers();
    let ids: Vec<&str> = match registry::candidates(&parsers, &doc) {
        Ok(found) => found.iter().map(|(p, _)| p.id()).collect(),
        Err(Error::NoParserMatched) => Vec::new(),
        Err(e) => return Err(js_error(e)),
    };
    to_js(&ids)
}

#[wasm_bindgen(js_name = listBanks)]
pub fn list_banks() -> Result<JsValue, JsValue> {
    let banks: Vec<JsBank> =
        bank2csv_banks::parsers().iter().map(|p| JsBank { id: p.id(), name: p.name(), layouts: p.layouts() }).collect();
    to_js(&banks)
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
