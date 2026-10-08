//! Writes the JSON of the analysis contexts for the given paths, in the format
//! of `tools/oracle/bin/contexts.dart`.
//!
//! Usage: dartr-contexts <path> [path ...]

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: dartr-contexts <path> [path ...]");
        std::process::exit(64);
    }
    let collection = dartr_project::AnalysisContextCollection::new(&paths, &Default::default());
    let json = dartr_project::dump::collection_json(&collection);
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
}
