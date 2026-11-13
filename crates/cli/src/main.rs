//! Development CLI.
//!
//!   bank2csv convert <file> [--bank ID] [--lenient] [--no-ids] [--json] [--password-file F]
//!   bank2csv dump    <file> [--lines] [--password-file F]   # RawDocument JSON or grouped lines
//!   bank2csv parse-raw <rawdoc.json> [--bank ID]            # run parsers on a fixture
//!   bank2csv banks
//!
//! The password is read from `--password-file` or the `BANK2CSV_PASSWORD`
//! environment variable, never from a command-line argument.

use bank2csv_core::{layout, output, registry, ConvertOptions, RawDocument};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn password(args: &[String]) -> Result<Option<String>, String> {
    if let Some(path) = value(args, "--password-file") {
        let pw = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        return Ok(Some(pw.trim_end_matches(['\r', '\n']).to_string()));
    }
    Ok(std::env::var("BANK2CSV_PASSWORD").ok().filter(|s| !s.is_empty()))
}

fn load(args: &[String]) -> Result<RawDocument, String> {
    let path = args.get(1).ok_or("missing file argument")?;
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let pw = password(args)?;
    bank2csv_extract::extract(&bytes, pw.as_deref()).map_err(|e| format!("[{}] {e}", e.code()))
}

fn run(args: &[String]) -> Result<(), String> {
    let parsers = bank2csv_banks::parsers();
    match args.first().map(|s| s.as_str()) {
        Some("banks") => {
            for p in &parsers {
                println!("{}\t{}\tlayouts: {}", p.id(), p.name(), p.layouts().join(", "));
            }
            Ok(())
        }
        Some("dump") => {
            let doc = load(args)?;
            if flag(args, "--lines") {
                if let RawDocument::Spans(d) = &doc {
                    for page in &d.pages {
                        println!("=== page {} ({}x{}) ===", page.number, page.width, page.height);
                        for line in layout::lines(page, 2.0) {
                            let cells: Vec<String> =
                                line.spans.iter().map(|s| format!("[{:.1}-{:.1}]{}", s.x0, s.x1, s.text)).collect();
                            println!("{:3} y={:6.1} {}", line.index, line.y, cells.join("  "));
                        }
                    }
                }
            } else {
                println!("{}", serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?);
            }
            Ok(())
        }
        Some(cmd @ ("convert" | "parse-raw")) => {
            let doc = if cmd == "convert" {
                load(args)?
            } else {
                let path = args.get(1).ok_or("missing file argument")?;
                let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?
            };
            let opts = ConvertOptions {
                bank: value(args, "--bank").map(String::from),
                lenient: flag(args, "--lenient"),
                generate_ids: !flag(args, "--no-ids"),
            };
            let parsed = registry::parse(&parsers, &doc, &opts).map_err(|e| format!("[{}] {e}", e.code()))?;
            for w in &parsed.warnings {
                eprintln!("warning: {w}");
            }
            eprintln!("meta: {}", serde_json::to_string(&parsed.meta).unwrap());
            if flag(args, "--json") {
                println!("{}", serde_json::to_string_pretty(&parsed.statement).unwrap());
            } else {
                print!("{}", output::to_csv(&parsed.statement, &output::CsvOptions::default()));
            }
            Ok(())
        }
        _ => Err("usage: bank2csv <convert|dump|parse-raw|banks> <file> [options]".into()),
    }
}
