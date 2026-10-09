//! Differential output for `pkg/analysis_server/lib/src/context_manager.dart`
//! non-Dart validators. Usage: dartr-project-diagnostics <path> [path ...].

fn main() {
    let paths: Vec<_> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: dartr-project-diagnostics <path> [path ...]");
        std::process::exit(64);
    }
    let collection = dartr_project::AnalysisContextCollection::new(&paths, &Default::default());
    println!(
        "{}",
        dartr_project::non_dart::collection_cli_diagnostics_json(&collection)
    );
}
