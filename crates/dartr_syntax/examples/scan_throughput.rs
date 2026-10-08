//! Measures scanner throughput (single thread).
//!
//! `cargo run --release -p dartr_syntax --example scan_throughput -- <dir>... [--iterations N]`
//!
//! Reads all `.dart` files into memory first, then scans every file N times
//! with `scan_for_analyzer` (the analyzer path, with comments and the
//! translation of error tokens) and reports MB/s of source and tokens/s.

use std::path::{Path, PathBuf};
use std::time::Instant;

fn dart_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            dart_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "dart") {
            out.push(path);
        }
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut iterations = 10;
    if let Some(i) = args.iter().position(|a| a == "--iterations") {
        iterations = args[i + 1].parse().unwrap();
        args.drain(i..i + 2);
    }
    let mut files = Vec::new();
    for dir in &args {
        dart_files(Path::new(dir), &mut files);
    }
    let sources: Vec<String> = files
        .iter()
        .map(|p| dartr_syntax::strip_bom(&std::fs::read_to_string(p).unwrap()).to_string())
        .collect();
    let bytes: usize = sources.iter().map(|s| s.len()).sum();

    // Warm up.
    let mut tokens = 0usize;
    for s in &sources {
        tokens += dartr_syntax::scan_for_analyzer(s).tokens().len();
    }

    let mut best = f64::MAX;
    let mut total = 0.0;
    for _ in 0..iterations {
        let start = Instant::now();
        for s in &sources {
            let result = dartr_syntax::scan_for_analyzer(s);
            std::hint::black_box(&result);
        }
        let t = start.elapsed().as_secs_f64();
        best = best.min(t);
        total += t;
    }
    let mb = bytes as f64 / 1e6;
    let mean = total / iterations as f64;
    println!("files:      {}", files.len());
    println!(
        "source:     {:.2} MB, {} tokens (arena entries incl. comments)",
        mb, tokens
    );
    println!("iterations: {iterations}");
    println!(
        "mean:       {:.1} ms/iteration, {:.0} MB/s, {:.1} M tokens/s",
        mean * 1e3,
        mb / mean,
        tokens as f64 / mean / 1e6
    );
    println!(
        "best:       {:.1} ms/iteration, {:.0} MB/s",
        best * 1e3,
        mb / best
    );
}
