//! `difftest <mode> <path-or-dir>...`: compares `dartr dump <mode>` with the
//! Dart oracle. See the crate documentation.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use dartr_difftest::{Options, run};

#[derive(Parser)]
#[command(
    name = "difftest",
    about = "Compare `dartr dump <mode>` with the Dart analyzer oracle"
)]
struct Cli {
    /// Dump mode: tokens, ast or resolved.
    mode: String,
    /// Dart files or directories (searched recursively).
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Number of batches to run in parallel.
    #[arg(long, short = 'j', default_value_t = default_jobs())]
    jobs: usize,
    /// Files per batch (default: files / jobs).
    #[arg(long, default_value_t = 0)]
    batch_size: usize,
    /// Write the oracle and dartr output of each differing file to this
    /// directory.
    #[arg(long)]
    write_failures: Option<PathBuf>,
    /// The dartr binary (default: `dartr` next to this binary).
    #[arg(long)]
    dartr: Option<PathBuf>,
    /// The oracle command, for example `dart run tools/oracle/bin/oracle.dart`
    /// (default: the oracle compiled to target/oracle/oracle).
    #[arg(long)]
    oracle: Option<String>,
    /// Number of differing files to report.
    #[arg(long, default_value_t = 20)]
    max_report: usize,
}

fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let dartr = match cli.dartr {
        Some(p) => p,
        None => std::env::current_exe()?
            .with_file_name(format!("dartr{}", std::env::consts::EXE_SUFFIX)),
    };
    let options = Options {
        mode: cli.mode,
        inputs: cli.paths,
        jobs: cli.jobs,
        batch_size: cli.batch_size,
        write_failures: cli.write_failures,
        dartr,
        oracle: cli
            .oracle
            .map(|s| s.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default(),
    };
    let report = run(&options)?;
    print!("{}", report.difference_report(cli.max_report));
    print!("{}", report.summary());
    Ok(if report.different() == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
