//! Measures the throughput of scan + parse + AST build (`parse_string`).
//!
//! `cargo run --release -p dartr_ast_builder --example build_throughput -- <dir>... [--iterations N]`
//!
//! Reads all `.dart` files into memory first, then runs `parse_string` on
//! every file N times on one thread, and N times with rayon over the files
//! (all cores), and reports MB/s of source (UTF-8). The Dart counterpart is
//! `tools/oracle/bin/parse_string_bench.dart` (the analyzer `parseString`;
//! compile it with `dart compile exe` and run it on the same directories).

use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;

fn dart_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dart_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "dart") {
            out.push(path);
        }
    }
}

fn parse_one(source: &str) -> usize {
    let parsed = dartr_ast_builder::parse_string(source, "/bench.dart");
    parsed.ast.node_count() ^ parsed.diagnostics.len()
}

fn report(label: &str, mb: f64, times: &[f64]) {
    let best = times.iter().cloned().fold(f64::MAX, f64::min);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "{label:<10} mean {:.1} ms/iteration, {:.1} MB/s; best {:.1} ms, {:.1} MB/s",
        mean * 1e3,
        mb / mean,
        best * 1e3,
        mb / best
    );
}

fn run() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut iterations = 10;
    if let Some(i) = args.iter().position(|a| a == "--iterations") {
        iterations = args[i + 1].parse().unwrap();
        args.drain(i..i + 2);
    }
    let mut files = Vec::new();
    for dir in &args {
        let p = Path::new(dir);
        if p.is_file() {
            files.push(p.to_path_buf());
        } else {
            dart_files(p, &mut files);
        }
    }
    files.sort();
    // Like Dart `File.readAsStringSync`: files that are not UTF-8 are
    // skipped (the Dart benchmark skips them too).
    let sources: Vec<String> = files
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .map(|s| dartr_syntax::strip_bom(&s).to_string())
        .collect();
    let bytes: usize = sources.iter().map(|s| s.len()).sum();
    let mb = bytes as f64 / 1e6;

    // Warm up.
    let mut sink = 0usize;
    let mut nodes = 0usize;
    for s in &sources {
        let parsed = dartr_ast_builder::parse_string(s, "/bench.dart");
        nodes += parsed.ast.node_count();
    }

    let mut single = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        for s in &sources {
            sink ^= parse_one(s);
        }
        single.push(start.elapsed().as_secs_f64());
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .stack_size(256 << 20)
        .build()
        .unwrap();
    let threads = pool.current_num_threads();
    let mut parallel = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let x: usize = pool.install(|| sources.par_iter().map(|s| parse_one(s)).sum());
        sink ^= x;
        parallel.push(start.elapsed().as_secs_f64());
    }

    println!("files:      {}", sources.len());
    println!("source:     {mb:.2} MB (UTF-8), {nodes} AST nodes");
    println!("iterations: {iterations}");
    report("1 thread", mb, &single);
    report(&format!("{threads} threads"), mb, &parallel);
    std::hint::black_box(sink);
}

fn main() {
    // Deeply nested code recurses deeply.
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
