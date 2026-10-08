//! `dartr`: Dart static analyzer written in Rust.

mod dump;
mod elements;
mod json;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "dartr",
    version,
    about = "Dart static analyzer written in Rust"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Writes internal data structures as JSON Lines (same format as
    /// tools/oracle), one object per file.
    Dump {
        mode: dump::DumpMode,
        /// Dart files. When there are none, paths are read from stdin, one
        /// per line.
        files: Vec<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Dump { mode, files } => dump::run(mode, files),
    }
}
