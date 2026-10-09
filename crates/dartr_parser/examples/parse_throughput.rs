//! Measures scan + parse throughput (single thread) with a no-op listener.
//!
//! `cargo run --release -p dartr_parser --example parse_throughput -- <dir>... [--iterations N]`
//!
//! Reads all `.dart` files into memory first, then scans and parses every
//! file N times the way the analyzer does (`parse_for_analyzer`: analyzer
//! scanner, language version features, `Parser::parse_unit`) with a
//! listener that does nothing, and reports MB/s of source. The Dart
//! counterpart is `tools/oracle/bin/parse_bench.dart` (compile it with
//! `dart compile exe` and run it on the same directories).

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// Counts heap allocations, to check that parsing does not allocate per
/// event.
struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

use dartr_parser::{Listener, parse_for_analyzer};

/// A listener that ignores every event (all trait defaults).
struct NoopListener;

impl Listener for NoopListener {}

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

    // Warm up, and count the allocations of scan + parse and of the scan
    // alone.
    let mut tokens = 0usize;
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for s in &sources {
        tokens += parse_for_analyzer(s, NoopListener).tokens.len();
    }
    let parse_allocations = ALLOCATIONS.load(Ordering::Relaxed) - before;
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    for s in &sources {
        std::hint::black_box(dartr_syntax::scan_for_analyzer(s));
    }
    let scan_allocations = ALLOCATIONS.load(Ordering::Relaxed) - before;

    let mut best = f64::MAX;
    let mut total = 0.0;
    for _ in 0..iterations {
        let start = Instant::now();
        for s in &sources {
            let result = parse_for_analyzer(s, NoopListener);
            std::hint::black_box(&result.tokens);
        }
        let t = start.elapsed().as_secs_f64();
        best = best.min(t);
        total += t;
    }
    let mb = bytes as f64 / 1e6;
    let mean = total / iterations as f64;
    println!("files:      {}", sources.len());
    println!(
        "source:     {:.2} MB, {} tokens (arena entries incl. comments)",
        mb, tokens
    );
    println!(
        "allocs:     {} scan + parse, {} scan only ({} in the parser for {} tokens)",
        parse_allocations,
        scan_allocations,
        parse_allocations.saturating_sub(scan_allocations),
        tokens
    );
    println!("iterations: {iterations}");
    println!(
        "mean:       {:.1} ms/iteration, {:.1} MB/s",
        mean * 1e3,
        mb / mean
    );
    println!(
        "best:       {:.1} ms/iteration, {:.1} MB/s",
        best * 1e3,
        mb / best
    );
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
