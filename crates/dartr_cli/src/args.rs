// Dart source: pkg/dartdev/lib/src/commands/analyze.dart (AnalyzeCommand options)
// Dart source: third_party/pkg/args (parser error messages)

//! The command line of `dartr analyze`, parsed with the rules and error
//! messages of `package:args` (as `dart analyze` uses it).

/// The name of the program in usage text.
pub const PROGRAM: &str = "dartr";

/// Output format (`--format`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Default,
    Json,
    Machine,
}

/// The parsed options of `analyze`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnalyzeArgs {
    pub help: bool,
    pub verbose: bool,
    pub fatal_infos: bool,
    pub fatal_warnings: bool,
    pub cache: Option<String>,
    pub memory: bool,
    pub format: Format,
    pub packages: Option<String>,
    pub sdk_path: Option<String>,
    pub use_aot_snapshot: bool,
    pub plugins: bool,
    /// `--enable-experiment` values, in order; `None` if not given.
    pub enable_experiment: Option<Vec<String>>,
    /// Positional arguments (`argResults.rest`).
    pub rest: Vec<String>,
}

impl Default for AnalyzeArgs {
    fn default() -> Self {
        AnalyzeArgs {
            help: false,
            verbose: false,
            fatal_infos: false,
            fatal_warnings: true,
            cache: None,
            memory: false,
            format: Format::Default,
            packages: None,
            sdk_path: None,
            use_aot_snapshot: true,
            plugins: true,
            enable_experiment: None,
            rest: Vec::new(),
        }
    }
}

/// A usage error (`UsageException`): the message, printed with the usage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageError(pub String);

/// The usage text of `analyze` (`ArgParser.usage` with the non-hidden
/// options), as printed by `--help` after the description.
pub fn usage() -> String {
    format!(
        "Usage: {PROGRAM} analyze [arguments] [<directory>]\n\
         -h, --help                   Print this usage information.\n\
         \x20   --fatal-infos            Treat info level issues as fatal.\n\
         \x20   --[no-]fatal-warnings    Treat warning level issues as fatal.\n\
         \x20                            (defaults to on)\n\
         \n\
         Run \"{PROGRAM} help\" to see global options."
    )
}

/// The `--help` output.
pub fn help() -> String {
    format!("Analyze Dart code in a directory.\n\n{}\n", usage())
}

/// The text of a usage error, as printed on stderr (`UsageException.toString`).
pub fn usage_error_text(error: &UsageError) -> String {
    format!("{}\n\n{}\n", error.0, usage())
}

#[derive(Clone, Copy)]
enum Kind {
    /// A flag; `true` if negatable.
    Flag(bool),
    Option,
    MultiOption,
}

const OPTIONS: &[(&str, Option<char>, Kind)] = &[
    ("help", Some('h'), Kind::Flag(false)),
    ("verbose", Some('v'), Kind::Flag(false)),
    ("fatal-infos", None, Kind::Flag(false)),
    ("fatal-warnings", None, Kind::Flag(true)),
    ("cache", None, Kind::Option),
    ("memory", None, Kind::Flag(true)),
    ("format", None, Kind::Option),
    ("packages", None, Kind::Option),
    ("sdk-path", None, Kind::Option),
    ("use-aot-snapshot", None, Kind::Flag(true)),
    ("plugins", None, Kind::Flag(true)),
    ("enable-experiment", None, Kind::MultiOption),
];

fn find(name: &str) -> Option<Kind> {
    OPTIONS.iter().find(|o| o.0 == name).map(|o| o.2)
}

fn set_flag(args: &mut AnalyzeArgs, name: &str, value: bool) {
    match name {
        "help" => args.help = value,
        "verbose" => args.verbose = value,
        "fatal-infos" => args.fatal_infos = value,
        "fatal-warnings" => args.fatal_warnings = value,
        "memory" => args.memory = value,
        "use-aot-snapshot" => args.use_aot_snapshot = value,
        "plugins" => args.plugins = value,
        _ => unreachable!("{name}"),
    }
}

fn set_option(args: &mut AnalyzeArgs, name: &str, value: String) -> Result<(), UsageError> {
    match name {
        "cache" => args.cache = Some(value),
        "packages" => args.packages = Some(value),
        "sdk-path" => args.sdk_path = Some(value),
        "format" => {
            args.format = match value.as_str() {
                "default" => Format::Default,
                "json" => Format::Json,
                "machine" => Format::Machine,
                _ => {
                    return Err(UsageError(format!(
                        "\"{value}\" is not an allowed value for option \"--format\"."
                    )));
                }
            }
        }
        "enable-experiment" => {
            // `splitCommas: true`.
            let list = args.enable_experiment.get_or_insert_with(Vec::new);
            list.extend(value.split(',').map(str::to_string));
        }
        _ => unreachable!("{name}"),
    }
    Ok(())
}

/// Parses the arguments after `analyze` (`ArgParser.parse` with
/// `allowTrailingOptions: true`).
pub fn parse(argv: &[String]) -> Result<AnalyzeArgs, UsageError> {
    let mut args = AnalyzeArgs::default();
    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        i += 1;
        if arg == "--" {
            args.rest.extend(argv[i..].iter().cloned());
            break;
        }
        if let Some(body) = arg.strip_prefix("--") {
            let (name, value) = match body.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (body, None),
            };
            match find(name) {
                Some(Kind::Flag(_)) => {
                    if value.is_some() {
                        return Err(UsageError(format!(
                            "Flag option \"--{name}\" should not be given a value."
                        )));
                    }
                    set_flag(&mut args, name, true);
                }
                Some(Kind::Option | Kind::MultiOption) => {
                    let value = match value {
                        Some(v) => v,
                        None => {
                            if i >= argv.len() {
                                return Err(UsageError(format!(
                                    "Missing argument for \"--{name}\"."
                                )));
                            }
                            i += 1;
                            argv[i - 1].clone()
                        }
                    };
                    set_option(&mut args, name, value)?;
                }
                None => {
                    let negated = name.strip_prefix("no-").map(|n| (n, find(n)));
                    match negated {
                        Some((n, Some(Kind::Flag(true)))) if value.is_none() => {
                            set_flag(&mut args, n, false)
                        }
                        Some((n, Some(Kind::Flag(false)))) => {
                            return Err(UsageError(format!(
                                "Cannot negate option \"--no-{n}\"."
                            )));
                        }
                        _ => {
                            return Err(UsageError(format!(
                                "Could not find an option named \"--{name}\"."
                            )));
                        }
                    }
                }
            }
        } else if arg.len() > 1 && arg.starts_with('-') {
            // Abbreviations: `-h`, `-v`, `-hv`.
            for c in arg[1..].chars() {
                let option = OPTIONS.iter().find(|o| o.1 == Some(c));
                match option {
                    Some((name, _, Kind::Flag(_))) => set_flag(&mut args, name, true),
                    _ => {
                        return Err(UsageError(format!(
                            "Could not find an option or flag \"-{c}\"."
                        )));
                    }
                }
            }
        } else {
            args.rest.push(arg.clone());
        }
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<AnalyzeArgs, UsageError> {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn parses_options() {
        let a = p(&["lib", "--no-fatal-warnings", "--format", "json", "--enable-experiment=a,b"]).unwrap();
        assert_eq!(a.rest, ["lib"]);
        assert!(!a.fatal_warnings);
        assert_eq!(a.format, Format::Json);
        assert_eq!(a.enable_experiment, Some(vec!["a".into(), "b".into()]));
        assert_eq!(
            p(&["--no-fatal-infos"]).unwrap_err().0,
            "Cannot negate option \"--no-fatal-infos\"."
        );
        assert_eq!(
            p(&["--format=xml"]).unwrap_err().0,
            "\"xml\" is not an allowed value for option \"--format\"."
        );
        assert_eq!(
            p(&["--bogus"]).unwrap_err().0,
            "Could not find an option named \"--bogus\"."
        );
        assert_eq!(
            p(&["--fatal-infos=true"]).unwrap_err().0,
            "Flag option \"--fatal-infos\" should not be given a value."
        );
    }
}
