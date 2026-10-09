//! Differential test of `dartr analyze` against `dart analyze` (the pinned
//! SDK 3.13.3 on PATH): stdout, stderr and the exit code must be equal for
//! projects whose diagnostics are all syntactic (dartr has the parse-only
//! provider for now). Usage text names the program, so `dart ` is replaced
//! by `dartr ` in the output of the real tool before comparing.
//!
//! Skipped (with a message) when `dart` is not on PATH.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(program: &str, args: &[&str], cwd: &Path) -> Output {
    let out = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env_remove("COLUMNS")
        .output()
        .unwrap_or_else(|e| panic!("run {program}: {e}"));
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

fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("analyze_parity");
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

/// Compares `dart analyze <args>` and `dartr analyze <args>` in [cwd].
/// Returns a description of the difference, or `None`.
fn compare(cwd: &Path, args: &[&str]) -> Option<String> {
    let mut dart_args = vec!["analyze"];
    dart_args.extend(args);
    let dart = run("dart", &dart_args, cwd);
    let dartr = run(env!("CARGO_BIN_EXE_dartr"), &dart_args, cwd);
    let normalize = |s: &str| {
        s.replace("dart analyze", "dartr analyze")
            .replace("\"dart help\"", "\"dartr help\"")
    };
    let expected_stdout = normalize(&dart.stdout);
    let expected_stderr = normalize(&dart.stderr);
    let label = format!("analyze {} (in {})", args.join(" "), cwd.display());
    eprintln!("{label}: exit dart={} dartr={}", dart.code, dartr.code);
    if dart.code == dartr.code && expected_stdout == dartr.stdout && expected_stderr == dartr.stderr
    {
        return None;
    }
    Some(format!(
        "{label}\n  exit: dart={} dartr={}\n--- dart stdout\n{expected_stdout}--- dartr stdout\n{}--- dart stderr\n{expected_stderr}--- dartr stderr\n{}",
        dart.code, dartr.code, dartr.stdout, dartr.stderr
    ))
}

const A_DART: &str = "void main() {\n  var x = 1\n  print(x);\n}\nclass {\n";

const B_DART: &str = "\
// ignore_for_file: missing_identifier
// ignore: expected_token
int f() { return 1 }
int g() { return 1 } // ignore: expected_token, expected_token
// ignore: type=warning, type=warning, unexpected_token
int h() { return 2 }
// TODO: not reported by dart analyze
var s = 'a|b' +;
int k() { return 3 }
";

const PIPE_DART: &str = "var a = [1, 2;\n\r\nvar b = 'x\\\\y';\nint c() => 1\n";

const OLD_DART: &str =
    "var r = (1, 2);\nsealed class S {}\nvar t = 1 >>> 2;\nvoid main() { print(1) }\n";

fn package_config(language_version: &str) -> String {
    format!(
        "{{\"configVersion\":2,\"packages\":[{{\"name\":\"p\",\"rootUri\":\"../\",\"packageUri\":\"lib/\",\"languageVersion\":\"{language_version}\"}}]}}"
    )
}

fn check_all(cases: &[(&Path, Vec<&str>)]) {
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|(cwd, args)| compare(cwd, args))
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn analyze_parity_syntax_errors() {
    if !dart_available() {
        eprintln!("skipped: dart 3.13.3 is not on PATH");
        return;
    }
    let root = scratch().join("syntax");
    let pc = package_config("3.13");
    write_project(
        &root,
        &[
            ("pubspec.yaml", "name: p\nenvironment:\n  sdk: ^3.9.0\n"),
            (".dart_tool/package_config.json", &pc),
            ("lib/a.dart", A_DART),
            ("lib/b.dart", B_DART),
            ("lib/we|ird.dart", PIPE_DART),
            ("lib/clean.dart", "int x = 1;\n"),
            ("bin/main.dart", "void main() {\n  print('x')\n}\n"),
        ],
    );
    let r = root.as_path();
    let lib = root.join("lib");
    let cases: Vec<(&Path, Vec<&str>)> = vec![
        (r, vec![]),
        (r, vec!["--format=machine"]),
        (r, vec!["--format=json"]),
        (r, vec!["--format", "json", "--fatal-infos"]),
        (r, vec!["lib"]),
        (r, vec!["lib/"]),
        (r, vec!["."]),
        (r, vec!["lib/a.dart"]),
        (r, vec!["lib/a.dart", "bin"]),
        (r, vec!["lib/clean.dart"]),
        (r, vec!["lib/clean.dart", "--format=json"]),
        (r, vec!["lib/clean.dart", "--format=machine"]),
        (&lib, vec![]),
        (&lib, vec!["../bin", "a.dart"]),
        (r, vec!["--enable-experiment=records"]),
    ];
    check_all(&cases);
}

#[test]
fn analyze_parity_language_version() {
    if !dart_available() {
        eprintln!("skipped: dart 3.13.3 is not on PATH");
        return;
    }
    let root = scratch().join("language_version");
    let pc = package_config("2.19");
    write_project(
        &root,
        &[
            (
                "pubspec.yaml",
                "name: p\nenvironment:\n  sdk: '>=2.19.0 <4.0.0'\n",
            ),
            (".dart_tool/package_config.json", &pc),
            ("lib/old.dart", OLD_DART),
            ("lib/new.dart", &format!("// @dart = 3.0\n{OLD_DART}")),
        ],
    );
    let r = root.as_path();
    check_all(&[
        (r, vec![]),
        (r, vec!["--format=json"]),
        (r, vec!["--format=machine"]),
    ]);
}

#[test]
fn analyze_parity_error_processors() {
    if !dart_available() {
        eprintln!("skipped: dart 3.13.3 is not on PATH");
        return;
    }
    let base = scratch();
    let pc = package_config("3.13");
    let project = |name: &str, options: &str| -> PathBuf {
        let root = base.join(name);
        write_project(
            &root,
            &[
                ("pubspec.yaml", "name: p\nenvironment:\n  sdk: ^3.9.0\n"),
                (".dart_tool/package_config.json", &pc),
                ("analysis_options.yaml", options),
                ("lib/a.dart", A_DART),
                ("lib/b.dart", B_DART),
            ],
        );
        root
    };
    let warning = project(
        "warning",
        "analyzer:\n  errors:\n    expected_token: warning\n    missing_identifier: warning\n    unexpected_token: warning\n",
    );
    let info = project(
        "info",
        "analyzer:\n  errors:\n    expected_token: info\n    missing_identifier: info\n    unexpected_token: info\n",
    );
    let mixed = project(
        "mixed",
        "analyzer:\n  errors:\n    expected_token: info\n    missing_identifier: warning\n    unexpected_token: error\n",
    );
    let ignore = project(
        "ignore",
        "analyzer:\n  errors:\n    expected_token: ignore\n    missing_identifier: false\n    unexpected_token: ignore\n    duplicate_ignore: ignore\n",
    );
    let cannot_ignore = project(
        "cannot_ignore",
        "analyzer:\n  cannot-ignore:\n    - expected_token\n    - missing_identifier\n",
    );
    let mut cases: Vec<(&Path, Vec<&str>)> = Vec::new();
    for p in [&warning, &info, &mixed, &ignore, &cannot_ignore] {
        for args in [
            vec![],
            vec!["--fatal-infos"],
            vec!["--no-fatal-warnings"],
            vec!["--fatal-infos", "--no-fatal-warnings"],
            vec!["--format=json"],
            vec!["--format=machine", "--fatal-infos"],
        ] {
            cases.push((p.as_path(), args));
        }
    }
    check_all(&cases);
}

#[test]
fn analyze_parity_usage_errors() {
    if !dart_available() {
        eprintln!("skipped: dart 3.13.3 is not on PATH");
        return;
    }
    let root = scratch().join("usage");
    write_project(&root, &[("lib/a.dart", "int x = 1;\n")]);
    let r = root.as_path();
    let cases: Vec<(&Path, Vec<&str>)> = vec![
        (r, vec![]),
        (r, vec!["--format=json"]),
        (r, vec!["--format=machine"]),
        (r, vec!["nope"]),
        (r, vec!["--bogus"]),
        (r, vec!["-x"]),
        (r, vec!["--format=xml"]),
        (r, vec!["--format"]),
        (
            r,
            vec!["--enable-experiment=foo,bar", "--enable-experiment=no-baz"],
        ),
        (r, vec!["--enable-experiment=macros"]),
        (r, vec!["--packages=nofile"]),
        (r, vec!["--sdk-path=/nonexistent"]),
        (r, vec!["--sdk-path=lib"]),
        (r, vec!["--fatal-infos=true"]),
        (r, vec!["--no-fatal-infos"]),
        (r, vec!["--no-format"]),
        (r, vec!["-h"]),
        (r, vec!["--help"]),
        (r, vec!["--", "lib"]),
    ];
    check_all(&cases);
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Compares `dart analyze` and `dartr analyze` (default, json and machine
/// output) for each project folder in [fixtures].
fn check_fixture_projects(fixtures: &Path, scratch_name: &str) {
    let base = scratch().join(scratch_name);
    if base.exists() {
        fs::remove_dir_all(&base).unwrap();
    }
    let mut names: Vec<String> = fs::read_dir(fixtures)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in &names {
        copy_dir(&fixtures.join(name), &base.join(name));
    }
    // Each `dart analyze` start takes seconds: run the projects in parallel.
    let count = names.len() * 3;
    let failures: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = names
            .iter()
            .map(|name| {
                let root = base.join(name);
                scope.spawn(move || {
                    [vec![], vec!["--format=json"], vec!["--format=machine"]]
                        .iter()
                        .filter_map(|args| compare(&root, args))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    assert!(
        failures.is_empty(),
        "{} of {count} cases differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The non-Dart diagnostics (`analysis_options.yaml`, `pubspec.yaml`,
/// `AndroidManifest.xml`), with the priority-file output of dartdev, for
/// the fixture projects of `dartr_project`.
#[test]
fn analyze_parity_non_dart_files() {
    if !dart_available() {
        eprintln!("skipped: dart 3.13.3 is not on PATH");
        return;
    }
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../dartr_project/tests/fixtures/diagnostics");
    check_fixture_projects(&fixtures, "non_dart");
}

/// The `fix_data.yaml` diagnostics (`TransformSetParser`): reported for
/// `lib/fix_data.yaml` and `lib/fix_data/**.yaml`, also when excluded.
#[test]
fn analyze_parity_fix_data_files() {
    if !dart_available() {
        eprintln!("skipped: dart 3.13.3 is not on PATH");
        return;
    }
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../dartr_project/tests/fixtures/fix_data");
    check_fixture_projects(&fixtures, "fix_data");
}

const LINT_RULES: &str = "\
linter:
  rules:
    - always_declare_return_types
    - avoid_empty_else
    - avoid_multiple_declarations_per_line
    - camel_case_types
    - constant_identifier_names
    - curly_braces_in_flow_control_structures
    - directives_ordering
    - empty_statements
    - eol_at_end_of_file
    - file_names
    - throw_in_finally
    - unnecessary_new
    - unawaited_futures
";

const LINT_A: &str = "\
import 'package:p/util.dart';
import 'dart:math';

class my_class {}
class other_class {} // ignore: camel_case_types

f() => 1;

int g(int a) {
  if (a > 1) return max(a, 2);
  if (a == 0) { return 1; } else ;
  return utilValue + Object().hashCode;
}

const my_const = 1;
// ignore: type=lint
const other_const = 2;
// ignore: unnecessary_new, duplicate_ignore
Object h() => new Object();
Object i() => new Object(); // ignore: avoid_empty_else, unnecessary_new
";

const LINT_UTIL: &str = "\
const int utilValue = 3;
int a = 1, b = 2;
void k() {
  ;
}
";

const LINT_LIB: &str = "\
part 'Bad_Name_part.dart';
part 'bad_name_two.dart';

class lib_class {}
f2() {}
";

const LINT_PART: &str = "\
part of 'Bad_Name.dart';

class part_class {}
// ignore: always_declare_return_types
f3() {}
Object p() => new Object();
";

const LINT_PART_TWO: &str = "\
// ignore_for_file: camel_case_types, unnecessary_new
part of 'Bad_Name.dart';

class part_two {}
f4() {}
Object q() => new Object();
";

/// The nearest `try` decides: the second `throw` is not reported.
const LINT_FINALLY: &str = "\
void a() {
  try {
    print(1);
  } finally {
    throw StateError('direct');
  }
}

void b() {
  try {
    print(1);
  } finally {
    try {
      print(2);
    } on Object {
      throw StateError('nested');
    }
    void inner() {
      throw StateError('in function');
    }
    inner();
  }
}
";

fn lint_project(base: &Path, name: &str, options: &str) -> PathBuf {
    let root = base.join(name);
    let pc = package_config("3.13");
    write_project(
        &root,
        &[
            ("pubspec.yaml", "name: p\nenvironment:\n  sdk: ^3.9.0\n"),
            (".dart_tool/package_config.json", &pc),
            ("analysis_options.yaml", options),
            ("lib/a.dart", LINT_A),
            ("lib/util.dart", LINT_UTIL),
            ("lib/Bad_Name.dart", LINT_LIB),
            ("lib/Bad_Name_part.dart", LINT_PART),
            ("lib/bad_name_two.dart", LINT_PART_TWO),
            ("lib/finally.dart", LINT_FINALLY),
            ("lib/no_eol.dart", "int z() => 1;"),
            ("lib/syntax.dart", "int y() => 1 // ignore: expected_token\nint w() { return 1 }\nclass sx {}\n"),
        ],
    );
    root
}

/// AST-only lint rules (the implemented ones): lints of a library with
/// parts, `// ignore:` comments for lints, `errors:` overrides of lint
/// severities, `cannot-ignore`, with parse errors in the same project.
#[test]
fn analyze_parity_lints() {
    if !dart_available() {
        eprintln!("skipped: dart 3.13.3 is not on PATH");
        return;
    }
    let base = scratch().join("lints");
    let plain = lint_project(&base, "plain", LINT_RULES);
    let overrides = lint_project(
        &base,
        "overrides",
        &format!(
            "analyzer:\n  errors:\n    camel_case_types: error\n    empty_statements: warning\n    unnecessary_new: ignore\n    avoid_empty_else: false\n    duplicate_ignore: info\n{LINT_RULES}"
        ),
    );
    let cannot_ignore = lint_project(
        &base,
        "cannot_ignore",
        &format!(
            "analyzer:\n  cannot-ignore:\n    - camel_case_types\n    - unnecessary_new\n  errors:\n    unnecessary_new: warning\n{LINT_RULES}"
        ),
    );
    let mut cases: Vec<(&Path, Vec<&str>)> = Vec::new();
    for p in [&plain, &overrides, &cannot_ignore] {
        for args in [
            vec![],
            vec!["--fatal-infos"],
            vec!["--format=json"],
            vec!["--format=machine"],
        ] {
            cases.push((p.as_path(), args));
        }
    }
    cases.push((plain.as_path(), vec!["lib/Bad_Name_part.dart"]));
    cases.push((plain.as_path(), vec!["lib/bad_name_two.dart", "lib/a.dart"]));
    let failures: Vec<String> = std::thread::scope(|scope| {
        let handles: Vec<_> = cases
            .iter()
            .map(|(cwd, args)| scope.spawn(move || compare(cwd, args)))
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().unwrap())
            .collect()
    });
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
