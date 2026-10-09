//! Differential test of `dartr format` against `dart format` (the pinned SDK
//! 3.13.3 on PATH): stdout, stderr, the exit code and the files on disk after
//! the run must be equal. Usage text names the program, so `dart ` is
//! replaced by `dartr ` in the output of the real tool, and the time in the
//! summary line ("in 0.01 seconds.") is replaced by a placeholder.
//!
//! Skipped (with a message) when `dart` is not on PATH.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(program: &str, args: &[&str], cwd: &Path, stdin: Option<&str>) -> Output {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env_remove("COLUMNS")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .unwrap_or_else(|e| panic!("run {program}: {e}"));
    if let Some(input) = stdin {
        let mut pipe = child.stdin.take().unwrap();
        pipe.write_all(input.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    Output {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn dart_available() -> bool {
    Command::new("dart")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("3.13.3"))
        .unwrap_or(false)
}

fn write_project(root: &Path, files: &[(&str, &str)]) {
    if root.exists() {
        fs::remove_dir_all(root).unwrap();
    }
    fs::create_dir_all(root).unwrap();
    for (path, content) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

/// All files below [root] with their content, sorted by path.
fn snapshot(root: &Path) -> Vec<(String, String)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).unwrap().display().to_string();
                out.push((rel, fs::read_to_string(&path).unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("format_parity");
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// Replaces the time of the summary line ("in 0.01 seconds.") with "in T
/// seconds.".
fn normalize_time(text: &str) -> String {
    let mut out = String::new();
    for line in text.split_inclusive('\n') {
        if line.starts_with("Formatted ")
            && let Some(index) = line.rfind(" in ")
            && line.trim_end().ends_with(" seconds.")
        {
            out.push_str(&line[..index]);
            out.push_str(" in T seconds.\n");
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Cuts a Dart stack trace ("#0      ...") from the output of an uncaught
/// exception: dartr prints the exception text only.
fn cut_stack_trace(text: &str) -> String {
    match text.find("\n#0 ") {
        Some(index) => text[..=index].to_string(),
        None => text.to_string(),
    }
}

struct Case {
    name: &'static str,
    files: &'static [(&'static str, &'static str)],
    args: &'static [&'static str],
    stdin: Option<&'static str>,
    /// The case needs formatted output (the ported formatting styles).
    needs_formatter: bool,
    /// Dart crashes with an uncaught exception: compare without the stack
    /// trace.
    crash: bool,
}

const fn case(
    name: &'static str,
    files: &'static [(&'static str, &'static str)],
    args: &'static [&'static str],
) -> Case {
    Case {
        name,
        files,
        args,
        stdin: None,
        needs_formatter: false,
        crash: false,
    }
}

const fn formatted(mut c: Case) -> Case {
    c.needs_formatter = true;
    c
}

const fn stdin(mut c: Case, input: &'static str) -> Case {
    c.stdin = Some(input);
    c
}

const PROJECT: &[(&str, &str)] = &[
    ("lib/a.dart", "void   main( ){print( 'a' );}\n"),
    ("lib/b.dart", "void main() {}\n"),
    ("lib/sub/c.dart", "class C{int x=1;}\n"),
    ("lib/.hidden/h.dart", "void   main(){}\n"),
    ("lib/z.txt", "not dart\n"),
    (
        "lib/long.dart",
        "void f() { g(aaaaaaaaaaaa, bbbbbbbbbbbbbbbb, cccccccccccccccc, dddddddddddddddd, eeeeeeeeee); }\n",
    ),
];

const BAD: &[(&str, &str)] = &[
    ("lib/a.dart", "void   main(){}\n"),
    (
        "lib/bad.dart",
        "void main() {\n  var x = ;\n  int y = 1 +;\n}\n",
    ),
    ("lib/ok.dart", "void main() {}\n"),
];

const MANY_ERRORS: &[(&str, &str)] = &[(
    "lib/many.dart",
    "void main() {\n  var a = ;\n  var b = ;\n  var c = ;\n  var d = ;\n  var e = ;\n  var f = ;\n  var g = ;\n  var h = ;\n  var i = ;\n  var j = ;\n  var k = ;\n  var l = ;\n}\n",
)];

const OPTIONS_PROJECT: &[(&str, &str)] = &[
    (
        "analysis_options.yaml",
        "include: base.yaml\nformatter:\n  trailing_commas: preserve\n",
    ),
    ("base.yaml", "formatter:\n  page_width: 40\n"),
    (
        "lib/a.dart",
        "void f() { g(aaaaaaaa, bbbbbbbb, cccccccc, dddddddd,); }\n",
    ),
    (
        ".dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"p","rootUri":"../","packageUri":"lib/","languageVersion":"3.6"}]}"#,
    ),
];

const BAD_OPTIONS: &[(&str, &str)] = &[
    (
        "analysis_options.yaml",
        "include: [missing.yaml, package:nope/x.yaml]\nformatter:\n  trailing_commas: [1, 2]\n",
    ),
    ("lib/a.dart", "void main() {}\n"),
];

const MALFORMED_OPTIONS: &[(&str, &str)] = &[
    ("analysis_options.yaml", "formatter:\n  page_width: [1\n"),
    ("lib/a.dart", "void main() {}\n"),
];

const CASES: &[Case] = &[
    case("help", PROJECT, &["--help"]),
    case("verbose help", PROJECT, &["-h", "-v"]),
    case("version", PROJECT, &["--version"]),
    case("verbose version", PROJECT, &["--version", "-v"]),
    case("bad output", PROJECT, &["-o", "bad", "lib"]),
    case("bad output long", PROJECT, &["--output=bad", "lib"]),
    case("missing option value", PROJECT, &["lib", "-o"]),
    case("unknown option", PROJECT, &["--foo", "lib"]),
    case("unknown abbreviation", PROJECT, &["-x", "lib"]),
    case("verbose without help", PROJECT, &["-v", "lib"]),
    case("page width not int", PROJECT, &["--page-width=abc", "lib"]),
    case("page width zero", PROJECT, &["--line-length=0", "lib"]),
    case("negative indent", PROJECT, &["--indent=-1", "lib"]),
    case(
        "bad language version",
        PROJECT,
        &["--language-version=3", "lib"],
    ),
    case(
        "bad trailing commas",
        PROJECT,
        &["--trailing-commas=x", "lib"],
    ),
    case(
        "selection with paths",
        PROJECT,
        &["--selection", "1:2", "lib"],
    ),
    case(
        "json with summary",
        PROJECT,
        &["-o", "json", "--summary", "line", "lib"],
    ),
    case(
        "stdin name with paths",
        PROJECT,
        &["--stdin-name", "a", "lib"],
    ),
    stdin(
        case("write from stdin", PROJECT, &["-o", "write"]),
        "void main() {}\n",
    ),
    stdin(
        case("bad selection", PROJECT, &["--selection", "1:x"]),
        "void main() {}\n",
    ),
    case("missing path", PROJECT, &["nonexist.dart"]),
    case("syntax errors", BAD, &["-o", "none", "lib/bad.dart"]),
    case("syntax errors json", BAD, &["-o", "json", "lib/bad.dart"]),
    case("more than 10 errors", MANY_ERRORS, &["lib"]),
    stdin(
        case("stdin syntax error", PROJECT, &[]),
        "void main() { var x = ; }",
    ),
    stdin(
        case(
            "stdin syntax error with name",
            PROJECT,
            &["--stdin-name", "lib/x.dart"],
        ),
        "void main() {\n  var x = ;\n}\n",
    ),
    Case {
        crash: true,
        ..case(
            "malformed analysis options",
            MALFORMED_OPTIONS,
            &["-o", "none", "lib"],
        )
    },
    // Formatted output.
    formatted(case("write", PROJECT, &["lib"])),
    formatted(case(
        "write file and dir",
        PROJECT,
        &["lib/sub", "lib/a.dart", "./lib//"],
    )),
    formatted(case("show", PROJECT, &["-o", "show", "lib"])),
    formatted(case("json", PROJECT, &["-o", "json", "lib"])),
    formatted(case(
        "show all",
        PROJECT,
        &["--show", "all", "-o", "none", "lib"],
    )),
    formatted(case("show none", PROJECT, &["--show", "none", "lib"])),
    formatted(case("no summary", PROJECT, &["--summary", "none", "lib"])),
    formatted(case(
        "set exit if changed",
        PROJECT,
        &["--set-exit-if-changed", "-o", "none", "lib"],
    )),
    formatted(case(
        "page width",
        PROJECT,
        &["--page-width", "40", "-o", "show", "lib"],
    )),
    formatted(case(
        "line length",
        PROJECT,
        &["-l", "40", "-o", "show", "lib"],
    )),
    formatted(case("indent", PROJECT, &["-i", "2", "-o", "show", "lib"])),
    formatted(case(
        "language version",
        PROJECT,
        &["--language-version=3.6", "-o", "show", "lib"],
    )),
    formatted(case(
        "latest language version",
        PROJECT,
        &["--language-version=latest", "-o", "show", "lib"],
    )),
    formatted(case(
        "trailing commas",
        OPTIONS_PROJECT,
        &["--trailing-commas=automate", "-o", "show", "lib"],
    )),
    formatted(case("syntax errors and files", BAD, &["lib"])),
    formatted(case(
        "syntax errors exit if changed",
        BAD,
        &["--set-exit-if-changed", "lib/a.dart", "lib/bad.dart"],
    )),
    formatted(case(
        "analysis options and package config",
        OPTIONS_PROJECT,
        &["-o", "show", "lib"],
    )),
    formatted(case(
        "analysis options warnings",
        BAD_OPTIONS,
        &["-o", "none", "lib"],
    )),
    formatted(stdin(case("stdin", PROJECT, &[]), "void   main( ){}")),
    formatted(stdin(
        case(
            "stdin json selection",
            PROJECT,
            &["-o", "json", "--selection", "7:3"],
        ),
        "void   main( ){ }\n",
    )),
    formatted(stdin(
        case(
            "stdin name finds options",
            OPTIONS_PROJECT,
            &["--stdin-name", "lib/x.dart", "--show", "all"],
        ),
        "void f() { g(aaaaaaaa, bbbbbbbb, cccccccc, dddddddd,); }\n",
    )),
];

/// Runs [case] with both tools; returns a description of the difference, or
/// `None`. Returns `Some("SKIP...")` for a case that needs the formatter
/// while it is not ported.
fn compare(root: &Path, c: &Case, force: bool) -> Option<String> {
    let mut args = vec!["format"];
    args.extend(c.args);

    write_project(root, c.files);
    let dart = run("dart", &args, root, c.stdin);
    let dart_files = snapshot(root);

    write_project(root, c.files);
    let dartr = run(env!("CARGO_BIN_EXE_dartr"), &args, root, c.stdin);
    let dartr_files = snapshot(root);

    if c.needs_formatter && !force && dartr.stderr.contains("not ported yet") {
        return Some(format!("SKIP {}", c.name));
    }

    let normalize = |s: &str| {
        let s = s
            .replace("dart format", "dartr format")
            .replace("\"dart help\"", "\"dartr help\"");
        let s = normalize_time(&s);
        if c.crash { cut_stack_trace(&s) } else { s }
    };
    let expected_stdout = normalize(&dart.stdout);
    let expected_stderr = normalize(&dart.stderr);
    let actual_stdout = normalize_time(&dartr.stdout);
    let actual_stderr = dartr.stderr.clone();
    eprintln!("{}: exit dart={} dartr={}", c.name, dart.code, dartr.code);
    if dart.code == dartr.code
        && expected_stdout == actual_stdout
        && expected_stderr == actual_stderr
        && dart_files == dartr_files
    {
        return None;
    }
    let files_note = if dart_files == dartr_files {
        String::new()
    } else {
        format!("--- files differ\n  dart:  {dart_files:?}\n  dartr: {dartr_files:?}\n")
    };
    Some(format!(
        "{} (format {})\n  exit: dart={} dartr={}\n--- dart stdout\n{expected_stdout}--- dartr stdout\n{actual_stdout}--- dart stderr\n{expected_stderr}--- dartr stderr\n{actual_stderr}{files_note}",
        c.name,
        c.args.join(" "),
        dart.code,
        dartr.code,
    ))
}

#[test]
fn format_matches_dart_format() {
    if !dart_available() {
        eprintln!("skipped: dart 3.13.3 is not on PATH");
        return;
    }
    let force = true;
    let base = scratch();
    let mut failures = Vec::new();
    let mut skipped = Vec::new();
    for (i, c) in CASES.iter().enumerate() {
        let root = base.join(format!("case{i}"));
        match compare(&root, c, force) {
            None => {}
            Some(s) if s.starts_with("SKIP ") => skipped.push(c.name),
            Some(failure) => failures.push(failure),
        }
    }
    if !skipped.is_empty() {
        eprintln!(
            "skipped {} cases that need the ported formatting styles (set DARTR_FORMAT_PARITY=1 to run them): {}",
            skipped.len(),
            skipped.join(", ")
        );
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}
