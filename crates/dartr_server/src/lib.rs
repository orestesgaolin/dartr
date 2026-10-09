//! dartr_server: the language server of dartr (`dartr language-server`), a
//! port of the LSP server of `pkg/analysis_server` (SDK 3.13.3).
//!
//! # Structure
//!
//! - [`args`]: the command line options of the Dart server.
//! - [`transport`]: JSON-RPC with LSP framing over stdio, and the protocol
//!   traffic log.
//! - [`server`]: the main loop, server states, overlays, analysis and
//!   notifications.
//! - [`capabilities`]: client capabilities, static capabilities and
//!   dynamic registrations.
//! - [`features`]: the LSP results of the AST features.
//! - [`computer`]: ports of the Dart computers (folding, selection ranges,
//!   closing labels, outline).
//! - [`mapping`]: positions, ranges, diagnostics, symbol kinds, errors.
//! - [`source_edits`], [`uri`]: document changes and file URIs.
//!
//! # Implemented methods
//!
//! `initialize`, `initialized`, `shutdown`, `exit`, `$/cancelRequest`,
//! `textDocument/{didOpen,didChange,didClose}` (incremental),
//! `workspace/{didChangeWorkspaceFolders,didChangeConfiguration}`,
//! `textDocument/{documentSymbol,foldingRange,selectionRange}`,
//! `dart/workspace/analysis/complete`; server to client:
//! `textDocument/publishDiagnostics` (parse diagnostics),
//! `dart/textDocument/{publishClosingLabels,publishOutline}`, `$/progress`
//! with `window/workDoneProgress/create`, `$/analyzerStatus`,
//! `workspace/configuration`, `client/registerCapability`.

#![allow(clippy::collapsible_if, clippy::collapsible_else_if)]

pub mod args;
pub mod capabilities;
pub mod computer;
pub mod features;
pub mod mapping;
pub mod server;
pub mod source_edits;
pub mod transport;
pub mod uri;

use std::io;

use args::{Protocol, ServerOptions, USAGE};

/// Runs the server with the command line [args] (without the command
/// name) on stdin and stdout. [default_to_lsp] is `true` for
/// `language-server`. Returns the exit code.
pub fn run_with_args(args: &[String], default_to_lsp: bool) -> i32 {
    let options = match ServerOptions::parse(args, default_to_lsp) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{}\n\n{USAGE}", e.0);
            return 64;
        }
    };
    if options.help {
        println!("{USAGE}");
        return 0;
    }
    if options.protocol == Protocol::Analyzer {
        eprintln!(
            "dartr: the legacy analysis server protocol (--protocol=analyzer) is not implemented yet"
        );
        return 64;
    }
    let channel = transport::Channel::new(
        Box::new(io::stdout()),
        options.protocol_traffic_log.as_deref(),
    );
    server::run(options, Box::new(io::stdin()), channel)
}
