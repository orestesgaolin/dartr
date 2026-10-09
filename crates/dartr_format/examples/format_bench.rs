// Dart source: dart_style benchmark/run.dart

//! Times formatting a file in the tall style, like dart_style's
//! `benchmark/run.dart`.
//!
//! `cargo run --release -p dartr_format --example format_bench -- <file.unit|file.dart> [iterations]`

use std::time::Instant;

use dartr_format::{DartFormatter, SourceCode};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let iterations: usize = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(100);
    let content = std::fs::read_to_string(path).unwrap();
    let mut lines: Vec<&str> = content.lines().collect();
    let mut page_width = 80;
    if lines[0].ends_with('|') {
        page_width = lines[0].find('|').unwrap();
        lines.remove(0);
    }
    let input = lines.join("\n");
    let mut formatter = DartFormatter::new(DartFormatter::LATEST_LANGUAGE_VERSION);
    formatter.page_width = page_width;
    let source = SourceCode::new(input, None, true, None, None).unwrap();

    let mut times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let result = formatter.format_source(&source).unwrap();
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        std::hint::black_box(result);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{path}: best {:.3} ms, median {:.3} ms ({iterations} iterations)",
        times[0],
        times[times.len() / 2]
    );
}
