//! `dartr`: Dart static analyzer written in Rust.

mod dump;
mod elements;
mod elements_const;
mod json;
mod resolved;

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
    /// Analyzes Dart code like `dart analyze` (same options, output formats
    /// and exit codes). Diagnostics: parse diagnostics only for now.
    Analyze {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Formats Dart code like `dart format` (same options, output and exit
    /// codes).
    #[command(disable_help_flag = true)]
    Format {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Writes internal data structures as JSON Lines (same format as
    /// tools/oracle), one object per file.
    Dump {
        mode: dump::DumpMode,
        /// Dart files. When there are none, paths are read from stdin, one
        /// per line.
        files: Vec<PathBuf>,
        /// Mode `elements`: add the constant value `"const"` to const
        /// top-level variables and fields (docs/design/semantics.md §5.1).
        #[arg(long)]
        with_const: bool,
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
    // `analyze` has the command line of `dart analyze` (package:args rules
    // and messages), so it is parsed by dartr_cli and not by clap.
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("analyze") {
        let stdout = std::io::stdout();
        let stderr = std::io::stderr();
        let code = dartr_cli::run(
            &argv[1..],
            &dartr_cli::ParseOnlyProvider,
            dartr_cli::Terminal::detect(),
            &mut stdout.lock(),
            &mut stderr.lock(),
        );
        std::process::exit(code);
    }
    // `format` has the command line of `dart format` (package:args rules and
    // messages), so it is parsed by dartr_format and not by clap.
    if argv.first().map(String::as_str) == Some("format") {
        use std::io::IsTerminal;
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        let stderr = std::io::stderr();
        let code = {
            let mut stdout = std::io::BufWriter::new(stdout.lock());
            let mut stderr = stderr.lock();
            let code = dartr_format::cli::run(
                &argv[1..],
                dartr_format::cli::CommandIo {
                    stdin_has_terminal: stdin.is_terminal(),
                    stdin: &mut stdin.lock(),
                    stderr_is_terminal: std::io::stderr().is_terminal(),
                    stdout: &mut stdout,
                    stderr: &mut stderr,
                },
            );
            use std::io::Write;
            let _ = stdout.flush();
            code
        };
        std::process::exit(code);
    }
    let cli = Cli::parse();
    match cli.command {
        Command::Dump {
            mode,
            files,
            with_const,
        } => dump::run(mode, files, with_const),
        Command::Analyze { .. } | Command::Format { .. } => unreachable!("handled before clap"),
        Command::LanguageServer { args } => {
            std::process::exit(dartr_legacy::run_with_args(&args, true))
        }
        Command::AnalysisServer { args } => {
            std::process::exit(dartr_legacy::run_with_args(&args, false))
        }
    }
}
