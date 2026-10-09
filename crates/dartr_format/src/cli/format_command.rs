// Dart source: dart_style lib/src/cli/format_command.dart
// Dart source: package:args lib/src/parser.dart (parse rules and error messages)
// Dart source: package:args lib/command_runner.dart (help, usage errors)
// Dart source: sdk pkg/dartdev/lib/dartdev.dart (exit code 64 for usage errors)

//! The `format` command: parses the command line with the rules and error
//! messages of `package:args` (as `dart format` does) and runs the formatter
//! on stdin or on the given paths.

use std::io::Write;
use std::time::Instant;

use crate::cli::formatter_options::{DART_STYLE_VERSION, FormatterOptions};
use crate::cli::output::Output;
use crate::cli::show::Show;
use crate::cli::summary::Summary;
use crate::dart_formatter::{DartFormatter, TrailingCommas};
use crate::dart_version_history::{DartVersionHistory, Version};
use crate::io::{Console, format_paths, format_stdin};

/// The name of the program in usage text.
pub const PROGRAM: &str = "dartr";

/// Dart `FormatCommand.description`.
pub const DESCRIPTION: &str = "Idiomatically format Dart source code.";

/// `ArgParser.usage` of the command (non-verbose), with the invocation line
/// and the footer that `CommandRunner` adds.
const USAGE: &str = include_str!("usage.txt");

/// `ArgParser.usage` of the command when `-v` or `--verbose` is anywhere on
/// the command line (dartdev creates the command with `verbose: true`).
const USAGE_VERBOSE: &str = include_str!("usage_verbose.txt");

/// The usage text, without a trailing newline.
pub fn usage(verbose: bool) -> &'static str {
    let text = if verbose { USAGE_VERBOSE } else { USAGE };
    text.strip_suffix('\n').unwrap_or(text)
}

/// A usage error (`UsageException`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageError(pub String);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Flag,
    /// An option with the allowed values (empty: any value).
    Option(&'static [&'static str]),
    MultiOption,
}

/// The options of the format command, with their abbreviations.
const OPTIONS: &[(&str, Option<char>, Kind)] = &[
    ("help", Some('h'), Kind::Flag),
    ("verbose", Some('v'), Kind::Flag),
    (
        "output",
        Some('o'),
        Kind::Option(&["write", "show", "json", "none"]),
    ),
    ("show", None, Kind::Option(&["all", "changed", "none"])),
    ("summary", None, Kind::Option(&["line", "profile", "none"])),
    ("language-version", None, Kind::Option(&[])),
    ("set-exit-if-changed", None, Kind::Flag),
    ("page-width", None, Kind::Option(&[])),
    ("line-length", Some('l'), Kind::Option(&[])),
    ("trailing-commas", None, Kind::Option(&[])),
    ("indent", Some('i'), Kind::Option(&[])),
    ("follow-links", None, Kind::Flag),
    ("version", None, Kind::Flag),
    ("enable-experiment", None, Kind::MultiOption),
    ("selection", None, Kind::Option(&[])),
    ("stdin-name", None, Kind::Option(&[])),
];

/// The flags of the parent parser (`globalDartdevOptionsParser`) that the
/// format command does not define itself. `package:args` falls back to the
/// parent parser for them; they have no effect on formatting. The `bool` is
/// `negatable`.
const PARENT_FLAGS: &[(&str, bool)] = &[
    ("enable-analytics", false),
    ("disable-analytics", false),
    ("disable-telemetry", false),
    ("diagnostics", false),
    ("analytics", true),
    ("suppress-analytics", false),
];

fn find_by_name(name: &str) -> Option<(&'static str, Kind)> {
    OPTIONS.iter().find(|o| o.0 == name).map(|o| (o.0, o.2))
}

fn find_by_abbreviation(c: char) -> Option<(&'static str, Kind)> {
    OPTIONS.iter().find(|o| o.1 == Some(c)).map(|o| (o.0, o.2))
}

/// The result of parsing the command line (`ArgResults`).
#[derive(Clone, Debug, Default)]
pub struct ArgResults {
    flags: Vec<&'static str>,
    /// Single-value options, last value wins.
    options: Vec<(&'static str, String)>,
    multi_options: Vec<(&'static str, Vec<String>)>,
    /// Positional arguments.
    pub rest: Vec<String>,
}

impl ArgResults {
    pub fn flag(&self, name: &str) -> bool {
        self.flags.contains(&name)
    }

    pub fn was_parsed(&self, name: &str) -> bool {
        self.flags.contains(&name)
            || self.options.iter().any(|o| o.0 == name)
            || self.multi_options.iter().any(|o| o.0 == name)
    }

    /// The value of a single-value option, or its default.
    pub fn option(&self, name: &str) -> Option<String> {
        if let Some((_, value)) = self.options.iter().find(|o| o.0 == name) {
            return Some(value.clone());
        }
        match name {
            "output" => Some("write".into()),
            "show" => Some("changed".into()),
            "summary" => Some("line".into()),
            "page-width" | "line-length" => Some("80".into()),
            "trailing-commas" => Some("automate".into()),
            "indent" => Some("0".into()),
            _ => None,
        }
    }

    pub fn multi_option(&self, name: &str) -> Vec<String> {
        self.multi_options
            .iter()
            .find(|o| o.0 == name)
            .map(|o| o.1.clone())
            .unwrap_or_default()
    }

    fn set_flag(&mut self, name: &'static str, value: bool) {
        self.flags.retain(|f| *f != name);
        if value {
            self.flags.push(name);
        }
    }

    /// Dart `Parser._setOption`.
    fn set_option(
        &mut self,
        name: &'static str,
        kind: Kind,
        value: String,
        arg: &str,
    ) -> Result<(), UsageError> {
        match kind {
            Kind::Option(allowed) => {
                validate_allowed(allowed, &value, arg)?;
                self.options.retain(|o| o.0 != name);
                self.options.push((name, value));
            }
            Kind::MultiOption => {
                let index = match self.multi_options.iter().position(|o| o.0 == name) {
                    Some(index) => index,
                    None => {
                        self.multi_options.push((name, Vec::new()));
                        self.multi_options.len() - 1
                    }
                };
                // `splitCommas: true`.
                for element in value.split(',') {
                    self.multi_options[index].1.push(element.to_string());
                }
            }
            Kind::Flag => unreachable!("set_option on a flag"),
        }
        Ok(())
    }
}

fn validate_allowed(allowed: &[&str], value: &str, arg: &str) -> Result<(), UsageError> {
    if allowed.is_empty() || allowed.contains(&value) {
        Ok(())
    } else {
        Err(UsageError(format!(
            "\"{value}\" is not an allowed value for option \"{arg}\"."
        )))
    }
}

fn is_letter_or_digit(c: char) -> bool {
    c.is_ascii_alphanumeric()
}

fn is_letter_digit_hyphen_or_underscore(c: char) -> bool {
    is_letter_or_digit(c) || c == '-' || c == '_'
}

/// Parses the arguments after `format` (Dart `Parser.parse` with
/// `allowTrailingOptions: true` and the dartdev parser as parent).
pub fn parse(argv: &[String]) -> Result<ArgResults, UsageError> {
    let mut results = ArgResults::default();
    let mut args: std::collections::VecDeque<String> = argv.iter().cloned().collect();

    while let Some(current) = args.front().cloned() {
        if current == "--" {
            args.pop_front();
            break;
        }

        // Dart `_parseSoloOption`.
        let chars: Vec<char> = current.chars().collect();
        if chars.len() == 2 && chars[0] == '-' && is_letter_or_digit(chars[1]) {
            let c = chars[1];
            let Some((name, kind)) = find_by_abbreviation(c) else {
                return Err(UsageError(format!(
                    "Could not find an option or flag \"-{c}\"."
                )));
            };
            args.pop_front();
            if kind == Kind::Flag {
                results.set_flag(name, true);
            } else {
                let arg = format!("-{c}");
                let Some(value) = args.pop_front() else {
                    return Err(UsageError(format!("Missing argument for \"{arg}\".")));
                };
                results.set_option(name, kind, value, &arg)?;
            }
            continue;
        }

        // Dart `_parseAbbreviation`.
        if chars.len() >= 2 && chars[0] == '-' {
            let mut index = 1;
            while index < chars.len() && is_letter_or_digit(chars[index]) {
                index += 1;
            }
            if index != 1 {
                let letters: String = chars[1..index].iter().collect();
                let rest: String = chars[index..].iter().collect();
                if !rest.contains('\n') && !rest.contains('\r') {
                    let c = chars[1];
                    match find_by_abbreviation(c) {
                        None => {
                            return Err(UsageError(format!(
                                "Could not find an option with short name \"-{c}\"."
                            )));
                        }
                        Some((name, kind)) if kind != Kind::Flag => {
                            let value = format!("{}{rest}", &letters[c.len_utf8()..]);
                            results.set_option(name, kind, value, &format!("-{c}"))?;
                        }
                        Some(_) => {
                            if !rest.is_empty() {
                                return Err(UsageError(format!(
                                    "Option \"-{c}\" is a flag and cannot handle value \"{}{rest}\".",
                                    &letters[c.len_utf8()..]
                                )));
                            }
                            for c in letters.chars() {
                                match find_by_abbreviation(c) {
                                    None => {
                                        return Err(UsageError(format!(
                                            "Could not find an option with short name \"-{c}\"."
                                        )));
                                    }
                                    Some((name, Kind::Flag)) => results.set_flag(name, true),
                                    Some(_) => {
                                        return Err(UsageError(format!(
                                            "Option \"-{c}\" must be a flag to be in a collapsed \"-\"."
                                        )));
                                    }
                                }
                            }
                        }
                    }
                    args.pop_front();
                    continue;
                }
            }
        }

        // Dart `_parseLongOption`.
        if let Some(body) = current.strip_prefix("--") {
            let (name, value) = match body.find('=') {
                Some(i) => (&body[..i], Some(body[i + 1..].to_string())),
                None => (body, None),
            };
            let valid_name = name.chars().all(is_letter_digit_hyphen_or_underscore);
            let valid_value = value
                .as_ref()
                .is_none_or(|v| !v.contains('\n') && !v.contains('\r'));
            if valid_name && valid_value {
                handle_long_option(&mut results, &mut args, name, value)?;
                continue;
            }
        }

        // `allowTrailingOptions: true`.
        results.rest.push(args.pop_front().unwrap());
    }

    results.rest.extend(args);
    Ok(results)
}

/// Dart `Parser._handleLongOption`, including the fallback to the parent
/// (dartdev) parser.
fn handle_long_option(
    results: &mut ArgResults,
    args: &mut std::collections::VecDeque<String>,
    name: &str,
    value: Option<String>,
) -> Result<(), UsageError> {
    if let Some((option_name, kind)) = find_by_name(name) {
        args.pop_front();
        if kind == Kind::Flag {
            if value.is_some() {
                return Err(UsageError(format!(
                    "Flag option \"--{name}\" should not be given a value."
                )));
            }
            results.set_flag(option_name, true);
        } else {
            let arg = format!("--{name}");
            let value = match value {
                Some(value) => value,
                None => match args.pop_front() {
                    Some(value) => value,
                    None => {
                        return Err(UsageError(format!("Missing argument for \"{arg}\".")));
                    }
                },
            };
            results.set_option(option_name, kind, value, &arg)?;
        }
        return Ok(());
    }
    if let Some(positive) = name.strip_prefix("no-") {
        if let Some((_, kind)) = find_by_name(positive) {
            args.pop_front();
            if kind != Kind::Flag {
                return Err(UsageError(format!(
                    "Cannot negate non-flag option \"--{name}\"."
                )));
            }
            // None of the flags of the format command is negatable.
            return Err(UsageError(format!("Cannot negate option \"--{name}\".")));
        }
        // Parent parser.
        if let Some((_, negatable)) = PARENT_FLAGS.iter().find(|f| f.0 == positive) {
            args.pop_front();
            if !negatable {
                return Err(UsageError(format!("Cannot negate option \"--{name}\".")));
            }
            return Ok(());
        }
        return Err(UsageError(format!(
            "Could not find an option named \"--{name}\"."
        )));
    }
    // Parent parser: global dartdev flags have no effect on formatting.
    if PARENT_FLAGS.iter().any(|f| f.0 == name) {
        args.pop_front();
        if value.is_some() {
            return Err(UsageError(format!(
                "Flag option \"--{name}\" should not be given a value."
            )));
        }
        return Ok(());
    }
    Err(UsageError(format!(
        "Could not find an option named \"--{name}\"."
    )))
}

/// Dart `int.tryParse`: optional sign, decimal digits or `0x` hex digits,
/// surrounding whitespace allowed.
pub fn dart_int_try_parse(text: &str) -> Option<i64> {
    let text = text.trim();
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let value = if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        i64::from_str_radix(hex, 16).ok()?
    } else {
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse::<i64>().ok()?
    };
    Some(if negative { -value } else { value })
}

/// The streams and environment that the command uses.
pub struct CommandIo<'a> {
    pub stdin: &'a mut dyn std::io::Read,
    /// Dart `stdin.hasTerminal`.
    pub stdin_has_terminal: bool,
    /// Whether stderr is a terminal (colors in syntax error messages).
    pub stderr_is_terminal: bool,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
}

/// Runs `format` with the arguments after the command name and returns the
/// exit code (Dart `FormatCommand.run` inside the dartdev runner).
pub fn run(argv: &[String], io: CommandIo<'_>) -> i32 {
    // dartdev: `verbose = args.contains('-v') || args.contains('--verbose')`.
    let verbose_usage = argv.iter().any(|a| a == "-v" || a == "--verbose");
    let CommandIo {
        stdin,
        stdin_has_terminal,
        stderr_is_terminal,
        stdout,
        stderr,
    } = io;

    let usage_error = |stderr: &mut dyn Write, error: UsageError| -> i32 {
        let _ = writeln!(stderr, "{}\n\n{}", error.0, usage(verbose_usage));
        64
    };

    let arg_results = match parse(argv) {
        Ok(results) => results,
        Err(error) => return usage_error(stderr, error),
    };

    // `CommandRunner.runCommand`: `--help` prints the usage of the command.
    if arg_results.flag("help") {
        let _ = writeln!(stdout, "{DESCRIPTION}\n\n{}", usage(verbose_usage));
        return 0;
    }

    let mut console = Console {
        stdout,
        stderr,
        stderr_is_terminal,
    };
    match run_parsed(&arg_results, stdin, stdin_has_terminal, &mut console) {
        Ok(code) => code,
        Err(RunError::Usage(error)) => usage_error(console.stderr, error),
        Err(RunError::Crash(message)) => {
            // dartdev: an unexpected exception is printed and exits with 1.
            let _ = writeln!(console.stderr, "{message}");
            1
        }
    }
}

/// Why [run_parsed] stopped.
pub enum RunError {
    Usage(UsageError),
    /// An uncaught exception (Dart prints it with a stack trace).
    Crash(String),
}

impl From<UsageError> for RunError {
    fn from(error: UsageError) -> Self {
        RunError::Usage(error)
    }
}

fn usage_exception(message: impl Into<String>) -> RunError {
    RunError::Usage(UsageError(message.into()))
}

/// Dart `FormatCommand.run` after argument parsing.
fn run_parsed(
    arg_results: &ArgResults,
    stdin: &mut dyn std::io::Read,
    stdin_has_terminal: bool,
    console: &mut Console<'_>,
) -> Result<i32, RunError> {
    if arg_results.flag("version") {
        if arg_results.flag("verbose") {
            let _ = writeln!(
                console.stdout,
                "dart_style {DART_STYLE_VERSION} supporting Dart language versions {} through {}",
                DartVersionHistory::EARLIEST.major_minor(),
                DartVersionHistory::LATEST.major_minor()
            );
        } else {
            let _ = writeln!(console.stdout, "{DART_STYLE_VERSION}");
        }
        return Ok(0);
    }

    let start = Instant::now();

    let mut show = Show::by_name(&arg_results.option("show").unwrap());
    let mut output = Output::by_name(&arg_results.option("output").unwrap());
    let mut summary = match arg_results.option("summary").as_deref() {
        Some("line") => Summary::line(start),
        Some("profile") => Summary::profile(),
        _ => Summary::None,
    };

    // If the user is sending code through stdin, default the output to stdout.
    if !arg_results.was_parsed("output") && arg_results.rest.is_empty() {
        output = Output::Show;
    }

    // If the user wants to print the code and didn't indicate how the files
    // should be printed, default to only showing the code.
    if !arg_results.was_parsed("show") && (output == Output::Show || output == Output::Json) {
        show = Show::None;
    }

    // If the user wants JSON output, default to no summary.
    if !arg_results.was_parsed("summary") && output == Output::Json {
        summary = Summary::None;
    }

    // Can't use --verbose with anything but --help.
    if arg_results.flag("verbose") && !arg_results.flag("help") {
        return Err(usage_exception("Can only use --verbose with --help."));
    }

    // Can't use any summary with JSON output.
    if output == Output::Json && !summary.is_none() {
        return Err(usage_exception("Cannot print a summary with JSON output."));
    }

    let mut language_version: Option<Version> = None;
    if let Some(version) = arg_results.option("language-version") {
        if version == "latest" {
            language_version = Some(DartFormatter::LATEST_LANGUAGE_VERSION);
        } else if let Some(parsed) = match_version(&version) {
            language_version = Some(parsed);
        } else {
            return Err(usage_exception(format!(
                "--language-version must be a version like \"3.2\" or \"latest\", was \"{version}\"."
            )));
        }
    }

    // Allow the old option name if the new one wasn't passed.
    let page_width_string = if arg_results.was_parsed("page-width") {
        arg_results.option("page-width")
    } else if arg_results.was_parsed("line-length") {
        arg_results.option("line-length")
    } else {
        None
    };

    let mut page_width: Option<usize> = None;
    if let Some(text) = page_width_string {
        match dart_int_try_parse(&text) {
            None => {
                return Err(usage_exception(format!(
                    "Page width must be an integer, was \"{text}\"."
                )));
            }
            Some(width) if width <= 0 => {
                return Err(usage_exception(format!(
                    "Page width must be a positive number, was {width}."
                )));
            }
            Some(width) => page_width = Some(width as usize),
        }
    }

    let mut trailing_commas: Option<TrailingCommas> = None;
    if arg_results.was_parsed("trailing-commas") {
        // We check the values explicitly here instead of using `allowedValues`
        // from [ArgParser] because this provides a better error message.
        trailing_commas = Some(match arg_results.option("trailing-commas").as_deref() {
            Some("automate") => TrailingCommas::Automate,
            Some("preserve") => TrailingCommas::Preserve,
            mode => {
                return Err(usage_exception(format!(
                    "--trailing-commas must be \"automate\" or \"preserve\", was \"{}\".",
                    mode.unwrap_or_default()
                )));
            }
        });
    }

    let indent_text = arg_results.option("indent").unwrap();
    let Some(indent) = dart_int_try_parse(&indent_text) else {
        return Err(usage_exception(format!(
            "--indent must be an integer, was \"{indent_text}\"."
        )));
    };
    if indent < 0 {
        return Err(usage_exception(format!(
            "--indent must be non-negative, was \"{indent_text}\"."
        )));
    }

    let selection = parse_selection(arg_results, "selection")?;

    let follow_links = arg_results.flag("follow-links");
    let set_exit_if_changed = arg_results.flag("set-exit-if-changed");

    let experiment_flags = arg_results.multi_option("enable-experiment");

    // If stdin isn't connected to a pipe, then the user is not passing
    // anything to stdin, so let them know they made a mistake.
    if arg_results.rest.is_empty() && stdin_has_terminal {
        return Err(usage_exception("Missing paths to code to format."));
    }

    if arg_results.rest.is_empty() && output == Output::Write {
        return Err(usage_exception(
            "Cannot use --output=write when reading from stdin.",
        ));
    }

    if arg_results.was_parsed("stdin-name") && !arg_results.rest.is_empty() {
        return Err(usage_exception(
            "Cannot pass --stdin-name when not reading from stdin.",
        ));
    }
    let stdin_name = arg_results.option("stdin-name");

    let mut options = FormatterOptions {
        language_version,
        indent: indent as usize,
        page_width,
        trailing_commas,
        follow_links,
        show,
        output,
        summary,
        set_exit_if_changed,
        experiment_flags,
        exit_code: 0,
    };

    if arg_results.rest.is_empty() {
        format_stdin(
            &mut options,
            selection,
            stdin_name.as_deref(),
            stdin,
            console,
        );
    } else {
        format_paths(&mut options, &arg_results.rest, console).map_err(RunError::Crash)?;
        options.summary.show(console.stdout);
    }

    // Return the exitCode explicitly for tools which embed dart_style
    // and set their own exitCode.
    Ok(options.exit_code)
}

/// Dart `RegExp(r'^([0-9]+)\.([0-9]+)$')` and `Version(major, minor, 0)`.
fn match_version(text: &str) -> Option<Version> {
    let (major, minor) = text.split_once('.')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(major) || !digits(minor) {
        return None;
    }
    Some(Version::new(major.parse().ok()?, minor.parse().ok()?))
}

/// Dart `FormatCommand._parseSelection`.
fn parse_selection(
    arg_results: &ArgResults,
    option_name: &str,
) -> Result<Option<(i64, i64)>, RunError> {
    let Some(option) = arg_results.option(option_name) else {
        return Ok(None);
    };

    // Can only preserve a selection when parsing from stdin.
    if !arg_results.rest.is_empty() {
        return Err(usage_exception(format!(
            "Can only use --{option_name} when reading from stdin."
        )));
    }

    let error = || {
        usage_exception(format!(
            "--{option_name} must be a colon-separated pair of integers, was \"{option}\"."
        ))
    };
    let coordinates: Vec<&str> = option.split(':').collect();
    if coordinates.len() != 2 {
        return Err(error());
    }
    // Dart `int.parse(coord.trim())`.
    let start = dart_int_try_parse(coordinates[0]).ok_or_else(error)?;
    let length = dart_int_try_parse(coordinates[1]).ok_or_else(error)?;
    Ok(Some((start, length)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<ArgResults, UsageError> {
        parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn parses_like_package_args() {
        let r = p(&["lib", "-ojson", "--page-width=100", "-i", "2", "a.dart"]).unwrap();
        assert_eq!(r.rest, ["lib", "a.dart"]);
        assert_eq!(r.option("output").as_deref(), Some("json"));
        assert_eq!(r.option("indent").as_deref(), Some("2"));
        assert!(r.was_parsed("page-width"));
        assert!(!r.was_parsed("show"));
        assert_eq!(
            p(&["-o", "bad"]).unwrap_err().0,
            "\"bad\" is not an allowed value for option \"-o\"."
        );
        assert_eq!(
            p(&["--output=bad"]).unwrap_err().0,
            "\"bad\" is not an allowed value for option \"--output\"."
        );
        assert_eq!(p(&["-o"]).unwrap_err().0, "Missing argument for \"-o\".");
        assert_eq!(
            p(&["--foo"]).unwrap_err().0,
            "Could not find an option named \"--foo\"."
        );
        assert_eq!(
            p(&["-x"]).unwrap_err().0,
            "Could not find an option or flag \"-x\"."
        );
        assert_eq!(
            p(&["--no-follow-links"]).unwrap_err().0,
            "Cannot negate option \"--no-follow-links\"."
        );
        assert_eq!(
            p(&["-hv2"]).unwrap_err().0,
            "Could not find an option with short name \"-2\"."
        );
        let r = p(&[
            "--enable-experiment=a,b",
            "--enable-experiment",
            "c",
            "--",
            "-o",
        ])
        .unwrap();
        assert_eq!(r.multi_option("enable-experiment"), ["a", "b", "c"]);
        assert_eq!(r.rest, ["-o"]);
        assert!(p(&["--suppress-analytics", "x"]).is_ok());
    }

    #[test]
    fn int_parse_like_dart() {
        assert_eq!(dart_int_try_parse(" 12 "), Some(12));
        assert_eq!(dart_int_try_parse("+3"), Some(3));
        assert_eq!(dart_int_try_parse("-0x10"), Some(-16));
        assert_eq!(dart_int_try_parse("1.0"), None);
        assert_eq!(dart_int_try_parse(""), None);
    }
}
