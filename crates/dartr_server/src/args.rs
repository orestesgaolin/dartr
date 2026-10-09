// Dart source: pkg/analysis_server/lib/src/server/driver.dart (Driver.createArgParser, Driver.start)
// Dart source: pkg/dartdev/lib/src/commands/language_server.dart (LanguageServerCommand)

//! The command line options of the analysis server.
//!
//! dartr accepts every option of the 3.13.3 server (and of `dart
//! language-server`), so that every client can start it with its usual
//! arguments. Options that only change diagnostics of the Dart
//! implementation (logs, analytics, the diagnostic web server) are accepted
//! and ignored. The parser follows `package:args`: `--name=value`,
//! `--name value`, `--[no-]flag`, `-h`, and `--` ends the options.

/// The protocol of the server (Dart `--protocol`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Lsp,
    Analyzer,
}

/// The parsed options. Only the options that dartr reads are kept; the
/// others are validated and dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerOptions {
    pub protocol: Protocol,
    pub client_id: String,
    pub client_version: Option<String>,
    /// `--protocol-traffic-log` (alias `--instrumentation-log-file`).
    pub protocol_traffic_log: Option<String>,
    /// `--dart-sdk` (alias `--sdk`).
    pub dart_sdk: Option<String>,
    /// `--packages`.
    pub packages: Option<String>,
    /// `--enable-experiment`, all values.
    pub enabled_experiments: Vec<String>,
    pub disable_completion: bool,
    pub disable_search: bool,
    pub help: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A flag; `true` if `--no-<name>` is allowed.
    Flag {
        negatable: bool,
    },
    Option,
    MultiOption,
}

/// Every option of `Driver.createArgParser` (3.13.3) and the
/// `--[no-]use-aot-snapshot` flag of `dart language-server`.
const OPTIONS: &[(&str, Kind)] = &[
    ("help", Kind::Flag { negatable: false }),
    ("client-id", Kind::Option),
    ("client-version", Kind::Option),
    ("dart-sdk", Kind::Option),
    ("sdk", Kind::Option),
    ("cache", Kind::Option),
    ("packages", Kind::Option),
    ("enable-experiment", Kind::MultiOption),
    ("protocol", Kind::Option),
    ("lsp", Kind::Flag { negatable: false }),
    ("protocol-traffic-log", Kind::Option),
    ("instrumentation-log-file", Kind::Option),
    ("session-log", Kind::Option),
    ("performance-log", Kind::Option),
    ("analysis-driver-log", Kind::Option),
    ("new-analysis-driver-log", Kind::Option),
    ("diagnostic-port", Kind::Option),
    ("port", Kind::Option),
    ("analytics", Kind::Flag { negatable: true }),
    ("suppress-analytics", Kind::Flag { negatable: false }),
    ("train-using", Kind::Option),
    ("disable-file-byte-store", Kind::Flag { negatable: true }),
    ("with-fine-dependencies", Kind::Flag { negatable: true }),
    (
        "disable-server-exception-handling",
        Kind::Flag { negatable: true },
    ),
    (
        "disable-server-feature-completion",
        Kind::Flag { negatable: true },
    ),
    (
        "disable-server-feature-search",
        Kind::Flag { negatable: true },
    ),
    ("plugins", Kind::Flag { negatable: true }),
    (
        "disable-silent-analysis-exceptions",
        Kind::Flag { negatable: false },
    ),
    (
        "disable-status-notification-debouncing",
        Kind::Flag { negatable: false },
    ),
    ("internal-print-to-console", Kind::Flag { negatable: false }),
    ("report-protocol-version", Kind::Option),
    // Removed options that the parser still accepts.
    ("completion-model", Kind::Option),
    ("dartpad", Kind::Flag { negatable: true }),
    ("enable-completion-model", Kind::Flag { negatable: true }),
    ("enable-instrumentation", Kind::Flag { negatable: true }),
    ("file-read-mode", Kind::Option),
    ("ignore-unrecognized-flags", Kind::Flag { negatable: true }),
    ("preview-dart-2", Kind::Flag { negatable: true }),
    ("useAnalysisHighlight2", Kind::Flag { negatable: true }),
    ("use-new-relevance", Kind::Flag { negatable: true }),
    ("use-fasta-parser", Kind::Flag { negatable: true }),
    // `dart language-server`.
    ("use-aot-snapshot", Kind::Flag { negatable: true }),
];

/// A usage error (`package:args` `ArgParserException`): the message, as Dart
/// prints it.
#[derive(Debug, PartialEq, Eq)]
pub struct UsageError(pub String);

impl ServerOptions {
    /// Parses [args] (without the command name). [default_to_lsp] is `true`
    /// for `language-server` (dartdev adds `--protocol=lsp` when there is no
    /// `--protocol`).
    pub fn parse(args: &[String], default_to_lsp: bool) -> Result<ServerOptions, UsageError> {
        let mut values: Vec<(&'static str, Value)> = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            i += 1;
            if arg == "--" {
                break;
            }
            if arg == "-h" {
                values.push(("help", Value::Flag(true)));
                continue;
            }
            let Some(body) = arg.strip_prefix("--") else {
                // A rest argument; the server ignores them.
                continue;
            };
            let (name, inline_value) = match body.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (body, None),
            };
            if let Some((option, kind)) = OPTIONS.iter().find(|(n, _)| *n == name) {
                match kind {
                    Kind::Flag { .. } => {
                        if inline_value.is_some() {
                            return Err(UsageError(format!(
                                "Flag option \"--{name}\" should not be given a value."
                            )));
                        }
                        values.push((option, Value::Flag(true)));
                    }
                    Kind::Option | Kind::MultiOption => {
                        let value = match inline_value {
                            Some(v) => v,
                            None => {
                                if i >= args.len() {
                                    return Err(UsageError(format!(
                                        "Missing argument for \"--{name}\"."
                                    )));
                                }
                                i += 1;
                                args[i - 1].clone()
                            }
                        };
                        values.push((option, Value::Text(value)));
                    }
                }
                continue;
            }
            if let Some(positive) = name.strip_prefix("no-") {
                if let Some((option, kind)) = OPTIONS.iter().find(|(n, _)| *n == positive) {
                    match kind {
                        Kind::Flag { negatable: true } if inline_value.is_none() => {
                            values.push((option, Value::Flag(false)));
                            continue;
                        }
                        Kind::Flag { negatable: true } => {
                            return Err(UsageError(format!(
                                "Flag option \"--{name}\" should not be given a value."
                            )));
                        }
                        _ => {
                            return Err(UsageError(format!(
                                "Cannot negate option \"--{positive}\"."
                            )));
                        }
                    }
                }
            }
            return Err(UsageError(format!(
                "Could not find an option named \"--{name}\"."
            )));
        }

        let text = |name: &str| -> Option<String> {
            values.iter().rev().find_map(|(n, v)| match v {
                Value::Text(t) if *n == name => Some(t.clone()),
                _ => None,
            })
        };
        let flag = |name: &str| -> Option<bool> {
            values.iter().rev().find_map(|(n, v)| match v {
                Value::Flag(b) if *n == name => Some(*b),
                _ => None,
            })
        };

        let protocol = match flag("lsp") {
            Some(true) => Protocol::Lsp,
            _ => match text("protocol").as_deref() {
                Some("lsp") => Protocol::Lsp,
                Some("analyzer") => Protocol::Analyzer,
                Some(other) => {
                    return Err(UsageError(format!(
                        "\"{other}\" is not an allowed value for option \"--protocol\"."
                    )));
                }
                None if default_to_lsp => Protocol::Lsp,
                None => Protocol::Analyzer,
            },
        };
        let mut enabled_experiments = Vec::new();
        for (n, v) in &values {
            if let (&"enable-experiment", Value::Text(t)) = (n, v) {
                enabled_experiments.extend(t.split(',').map(str::to_string));
            }
        }
        let client_id = text("client-id").unwrap_or_else(|| {
            match protocol {
                Protocol::Lsp => "unknown.client.lsp",
                Protocol::Analyzer => "unknown.client.classic",
            }
            .to_string()
        });
        if let Some(port) = text("diagnostic-port").or_else(|| text("port")) {
            if port.parse::<u16>().is_err() {
                return Err(UsageError(format!("Invalid port number: {port}")));
            }
        }
        Ok(ServerOptions {
            protocol,
            client_id,
            client_version: text("client-version"),
            protocol_traffic_log: text("protocol-traffic-log")
                .or_else(|| text("instrumentation-log-file")),
            dart_sdk: text("dart-sdk").or_else(|| text("sdk")),
            packages: text("packages"),
            enabled_experiments,
            disable_completion: flag("disable-server-feature-completion").unwrap_or(false),
            disable_search: flag("disable-server-feature-search").unwrap_or(false),
            help: flag("help").unwrap_or(false),
        })
    }
}

enum Value {
    Flag(bool),
    Text(String),
}

/// The usage text printed for `--help` and after a usage error.
pub const USAGE: &str = "\
Usage: dartr language-server [arguments]
    --client-id=<name>                    An identifier for the analysis server client.
    --client-version=<version>            The version of the analysis server client.
    --dart-sdk=<path>                     Override the Dart SDK to use for analysis.
    --cache=<path>                        Override the location of the analysis server's cache.
    --packages=<path>                     The path to the package resolution configuration file.
    --protocol=<protocol>                 Specify the protocol to use to communicate with the analysis server.
          [analyzer]                      Dart's analysis server protocol (not implemented by dartr yet)
          [lsp] (default)                 The Language Server Protocol

Server diagnostics:
    --protocol-traffic-log=<file path>    Write server protocol traffic to the given file.
    --session-log=<file path>             Accepted and ignored.
    --performance-log=<file path>         Accepted and ignored.
    --analysis-driver-log=<file path>     Accepted and ignored.
    --diagnostic-port=<port>              Accepted and ignored.
";

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<ServerOptions, UsageError> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        ServerOptions::parse(&args, true)
    }

    #[test]
    fn dart_code_arguments() {
        let o = parse(&[
            "--lsp",
            "--port=8123",
            "--client-id=VS-Code",
            "--client-version=3.144.0",
            "--instrumentation-log-file=/tmp/x.txt",
            "--session-log=/tmp/s.txt",
        ])
        .unwrap();
        assert_eq!(o.protocol, Protocol::Lsp);
        assert_eq!(o.client_id, "VS-Code");
        assert_eq!(o.client_version.as_deref(), Some("3.144.0"));
        assert_eq!(o.protocol_traffic_log.as_deref(), Some("/tmp/x.txt"));
    }

    #[test]
    fn flutter_analyze_arguments() {
        let o = parse(&[
            "--dart-sdk",
            "/sdk",
            "--disable-server-feature-completion",
            "--disable-server-feature-search",
            "--no-with-fine-dependencies",
            "--no-plugins",
            "--suppress-analytics",
            "--protocol-traffic-log=/tmp/t",
        ])
        .unwrap();
        assert_eq!(o.dart_sdk.as_deref(), Some("/sdk"));
        assert!(o.disable_completion && o.disable_search);
        assert_eq!(o.protocol, Protocol::Lsp);
    }

    #[test]
    fn dart_analyze_arguments_default_to_legacy_protocol() {
        let args: Vec<String> = [
            "--client-id=dart-analyze",
            "--disable-server-feature-completion",
            "--disable-status-notification-debouncing",
            "--disable-silent-analysis-exceptions",
            "--sdk",
            "/sdk",
            "--enable-experiment=a,b",
            "--enable-experiment=c",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let o = ServerOptions::parse(&args, false).unwrap();
        assert_eq!(o.protocol, Protocol::Analyzer);
        assert_eq!(o.dart_sdk.as_deref(), Some("/sdk"));
        assert_eq!(o.enabled_experiments, ["a", "b", "c"]);
    }

    #[test]
    fn usage_errors() {
        assert_eq!(
            parse(&["--bogus"]).unwrap_err().0,
            "Could not find an option named \"--bogus\"."
        );
        assert_eq!(
            parse(&["--protocol=x"]).unwrap_err().0,
            "\"x\" is not an allowed value for option \"--protocol\"."
        );
        assert!(parse(&["--client-id"]).is_err());
        assert!(parse(&["--no-help"]).is_err());
        assert!(parse(&["--port=abc"]).is_err());
    }
}
