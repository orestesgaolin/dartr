// Dart source: pkg/analysis_server/lib/src/server/driver.dart (Driver.start)
// Dart source: pkg/dartdev/lib/src/commands/language_server.dart

//! Legacy analysis server protocol. The LSP implementation remains in dartr_server.

pub mod protocol;
pub mod server;
pub mod transport;

use dartr_server::args::{Protocol, ServerOptions, USAGE};

/// Dispatches both command aliases with Dart's default protocol selection.
pub fn run_with_args(args: &[String], default_to_lsp: bool) -> i32 {
    let options = match ServerOptions::parse(args, default_to_lsp) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{}\n\n{USAGE}", error.0);
            return 64;
        }
    };
    if options.help {
        println!("{}", USAGE.replace(" (not implemented by dartr yet)", ""));
        return 0;
    }
    if options.protocol == Protocol::Lsp {
        return dartr_server::run_with_args(args, default_to_lsp);
    }
    match transport::Channel::new(std::io::stdout(), options.protocol_traffic_log.as_deref()) {
        Ok(channel) => server::run(options, std::io::stdin().lock(), channel),
        Err(error) => {
            eprintln!("dartr: {error}");
            1
        }
    }
}
