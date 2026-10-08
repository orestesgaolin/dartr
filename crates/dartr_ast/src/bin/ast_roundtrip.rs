#![allow(clippy::collapsible_if)]
//! Round trip test of the AST model without a parser.
//!
//! Reads the oracle output of mode `ast` (and optionally of mode
//! `tosource`), builds each AST from the dump with
//! [`dartr_ast::testing::load`], and compares:
//! - the `ast` dump of the Rust AST with the oracle dump, byte for byte;
//! - `to_source` of the Rust AST with Dart `toSource()`.
//!
//! Usage:
//!   ast_roundtrip <ast.jsonl> [<tosource.jsonl>] [--failures <dir>]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use rayon::prelude::*;
use serde_json::Value;
use serde_json::value::RawValue;

#[derive(serde::Deserialize)]
struct AstLine<'a> {
    path: String,
    #[serde(borrow)]
    ast: Option<&'a RawValue>,
}

#[derive(serde::Deserialize)]
struct SourceLine {
    path: String,
    source: Option<String>,
}

enum Outcome {
    Skipped,
    LoadError(String),
    Result { dump_ok: bool, dump_diff: String, source: Option<(bool, String)> },
}

fn read_source(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    Some(dartr_syntax::strip_bom(&text).to_string())
}

fn first_diff(a: &str, b: &str) -> String {
    let i = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    let ctx = |s: &str| {
        let mut lo = i.saturating_sub(150);
        while !s.is_char_boundary(lo) {
            lo -= 1;
        }
        let mut hi = (i + 150).min(s.len());
        while !s.is_char_boundary(hi) {
            hi += 1;
        }
        s[lo..hi].to_string()
    };
    format!("at byte {i}\n  oracle: {}\n  dartr:  {}", ctx(a), ctx(b))
}

fn run_one(line: &str, sources: &HashMap<String, String>) -> (String, Outcome) {
    let mut de = serde_json::Deserializer::from_str(line);
    de.disable_recursion_limit();
    let parsed: AstLine = match serde::Deserialize::deserialize(&mut de) {
        Ok(p) => p,
        Err(e) => return (String::new(), Outcome::LoadError(format!("bad line: {e}"))),
    };
    let path = parsed.path.clone();
    let Some(raw) = parsed.ast else {
        return (path, Outcome::Skipped);
    };
    let Some(source) = read_source(&path) else {
        return (path, Outcome::Skipped);
    };
    let mut de = serde_json::Deserializer::from_str(raw.get());
    de.disable_recursion_limit();
    let value: Value = serde::Deserialize::deserialize(&mut de).unwrap();
    let loaded = match dartr_ast::testing::load(&value, &source) {
        Ok(l) => l,
        Err(e) => return (path, Outcome::LoadError(e)),
    };
    let dump = dartr_ast::dump::node_json(&loaded.ast, loaded.unit);
    let dump_ok = dump == raw.get();
    let dump_diff = if dump_ok { String::new() } else { first_diff(raw.get(), &dump) };
    let source = sources.get(&path).map(|expected| {
        let actual = dartr_ast::to_source::to_source(&loaded.ast, loaded.unit);
        let ok = &actual == expected;
        (ok, if ok { String::new() } else { first_diff(expected, &actual) })
    });
    (path, Outcome::Result { dump_ok, dump_diff, source })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut positional = Vec::new();
    let mut failures_dir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--failures" {
            failures_dir = Some(PathBuf::from(&args[i + 1]));
            i += 2;
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }
    if positional.is_empty() {
        eprintln!("usage: ast_roundtrip <ast.jsonl> [<tosource.jsonl>] [--failures <dir>]");
        std::process::exit(64);
    }
    let lines: Vec<String> = BufReader::new(std::fs::File::open(&positional[0]).expect("ast.jsonl"))
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.is_empty())
        .collect();
    let mut sources = HashMap::new();
    if let Some(p) = positional.get(1) {
        for l in BufReader::new(std::fs::File::open(p).expect("tosource.jsonl")).lines().map_while(Result::ok) {
            if let Ok(s) = serde_json::from_str::<SourceLine>(&l) {
                if let Some(src) = s.source {
                    sources.insert(s.path, src);
                }
            }
        }
    }
    let pool = rayon::ThreadPoolBuilder::new().stack_size(256 << 20).build().unwrap();
    let results: Vec<(String, Outcome)> =
        pool.install(|| lines.par_iter().map(|l| run_one(l, &sources)).collect());

    let (mut files, mut skipped, mut load_errors, mut dump_ok, mut src_total, mut src_ok) = (0, 0, 0, 0, 0, 0);
    let mut report = String::new();
    let mut shown = 0;
    for (path, outcome) in &results {
        files += 1;
        match outcome {
            Outcome::Skipped => skipped += 1,
            Outcome::LoadError(e) => {
                load_errors += 1;
                if shown < 30 {
                    shown += 1;
                    report.push_str(&format!("LOAD {path}\n  {e}\n"));
                }
            }
            Outcome::Result { dump_ok: ok, dump_diff, source } => {
                if *ok {
                    dump_ok += 1;
                } else if shown < 30 {
                    shown += 1;
                    report.push_str(&format!("DUMP {path}\n  {dump_diff}\n"));
                }
                if let Some((sok, sdiff)) = source {
                    src_total += 1;
                    if *sok {
                        src_ok += 1;
                    } else if shown < 30 {
                        shown += 1;
                        report.push_str(&format!("TOSOURCE {path}\n  {sdiff}\n"));
                    }
                }
            }
        }
        if let Some(dir) = &failures_dir {
            let bad = match outcome {
                Outcome::LoadError(e) => Some(e.clone()),
                Outcome::Result { dump_ok, dump_diff, source } => {
                    let mut s = String::new();
                    if !dump_ok {
                        s.push_str(dump_diff);
                    }
                    if let Some((false, d)) = source {
                        s.push_str("\nTOSOURCE ");
                        s.push_str(d);
                    }
                    if s.is_empty() { None } else { Some(s) }
                }
                _ => None,
            };
            if let Some(b) = bad {
                std::fs::create_dir_all(dir).unwrap();
                let name = path.trim_start_matches('/').replace('/', "__");
                std::fs::write(dir.join(name + ".txt"), format!("{path}\n{b}\n")).unwrap();
            }
        }
    }
    print!("{report}");
    let checked = files - skipped;
    let pct = |a: usize, b: usize| if b == 0 { 100.0 } else { a as f64 * 100.0 / b as f64 };
    println!("files:        {files} ({skipped} without AST or source)");
    println!("load errors:  {load_errors}");
    println!("ast dump:     {dump_ok}/{checked} identical ({:.2}%)", pct(dump_ok, checked));
    if src_total > 0 || positional.len() > 1 {
        println!("to_source:    {src_ok}/{src_total} identical ({:.2}%)", pct(src_ok, src_total));
    }
}
