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
    /// Starts the language server (LSP over stdin and stdout). Accepts the
    /// options of `dart language-server`.
    #[command(name = "language-server", disable_help_flag = true)]
    LanguageServer {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Starts the analysis server like the server snapshot (`dart
    /// analysis_server.dart.snapshot`): the default protocol is the legacy
    /// protocol, `--lsp` selects LSP.
    #[command(name = "analysis-server", hide = true, disable_help_flag = true)]
    AnalysisServer {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Dump { mode, files } => dump::run(mode, files),
        Command::LanguageServer { args } => {
            std::process::exit(dartr_server::run_with_args(&args, true))
        }
        Command::AnalysisServer { args } => {
            std::process::exit(dartr_server::run_with_args(&args, false))
        }
    }
}
