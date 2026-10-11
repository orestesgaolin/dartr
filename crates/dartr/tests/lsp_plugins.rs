//! Differential parity test of analyzer plugins in `dartr language-server`
//! against `dart language-server` (3.13.3):
//! - Starts a local path plugin (`my_lint`) configured under `plugins:` in
//!   `analysis_options.yaml` with a lint rule (`no_print_calls`), a quick fix
//!   (`Remove print call`), and an assist (`Add trailing comment`).
//! - Verifies published diagnostics (plugin lint merged with built-in server
//!   diagnostics, respecting `// ignore: my_lint/no_print_calls`).
//! - Verifies `textDocument/codeAction` at the lint range (`quickfix` and
//!   `refactor` kinds, including the plugin fix and assist).
//! - Verifies `textDocument/didChange` incremental overlay updates (`ChangeContentOverlay`)
//!   and `textDocument/didClose` (`RemoveContentOverlay`).

mod lsp_support;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use lsp_support::*;
use serde_json::{Value, json};

fn dart_3_13_available() -> bool {
    Command::new("dart")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("3.13.3"))
        .unwrap_or(false)
}

fn session_args() -> Vec<&'static str> {
    vec![
        "language-server",
        "--protocol=lsp",
        "--client-id=VS-Code",
        "--client-version=3.144.0",
    ]
}

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

const MAIN_INITIAL: &str = "\
void main() {
  print('hello');
  // ignore: my_lint/no_print_calls
  print('ignored');
  int x = 1;
}
";

const MAIN_EDITED_ONE_MORE: &str = "\
void main() {
  print('hello');
  print('second');
  // ignore: my_lint/no_print_calls
  print('ignored');
  int x = 1;
}
";

const MAIN_EDITED_CLEAN: &str = "\
void main() {
  // ignore: my_lint/no_print_calls
  print('ignored');
  int x = 1;
}
";

fn create_plugin_fixture() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lsp_plugins_parity");
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    let root = dir.canonicalize().unwrap();

    write_file(
        &root,
        "pubspec.yaml",
        "name: app\nenvironment:\n  sdk: ^3.8.0\n",
    );
    write_file(
        &root,
        ".dart_tool/package_config.json",
        "{\"configVersion\":2,\"packages\":[{\"name\":\"app\",\"rootUri\":\"../\",\"packageUri\":\"lib/\",\"languageVersion\":\"3.8\"}]}\n",
    );
    write_file(
        &root,
        "analysis_options.yaml",
        "analyzer:\n  exclude:\n    - my_lint/**\n\nlinter:\n  rules:\n    - prefer_final_locals\n\nplugins:\n  my_lint:\n    path: my_lint\n    diagnostics:\n      no_print_calls: true\n",
    );
    write_file(&root, "lib/main.dart", MAIN_INITIAL);

    write_file(
        &root,
        "my_lint/pubspec.yaml",
        "name: my_lint\nversion: 0.0.1\npublish_to: none\nenvironment:\n  sdk: ^3.8.0\ndependencies:\n  analysis_server_plugin: ^0.3.0\n  analyzer: ^12.1.0\n  analyzer_plugin: ^0.14.0\n",
    );
    write_file(
        &root,
        "my_lint/lib/main.dart",
        "\
import 'package:analysis_server_plugin/plugin.dart';
import 'package:analysis_server_plugin/registry.dart';

import 'src/no_print_calls.dart';

final plugin = MyLintPlugin();

class MyLintPlugin extends Plugin {
  @override
  String get name => 'my_lint';

  @override
  void register(PluginRegistry registry) {
    registry.registerLintRule(NoPrintCalls());
    registry.registerFixForRule(NoPrintCalls.code, RemovePrintFix.new);
    registry.registerAssist(AddCommentAssist.new);
  }
}
",
    );
    write_file(
        &root,
        "my_lint/lib/src/no_print_calls.dart",
        "\
import 'package:analysis_server_plugin/edit/dart/correction_producer.dart';
import 'package:analysis_server_plugin/edit/dart/dart_fix_kind_priority.dart';
import 'package:analyzer/analysis_rule/analysis_rule.dart';
import 'package:analyzer/analysis_rule/rule_context.dart';
import 'package:analyzer/analysis_rule/rule_visitor_registry.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/visitor.dart';
import 'package:analyzer/error/error.dart';
import 'package:analyzer/source/source_range.dart';
import 'package:analyzer_plugin/utilities/assist/assist.dart';
import 'package:analyzer_plugin/utilities/change_builder/change_builder_core.dart';
import 'package:analyzer_plugin/utilities/fixes/fixes.dart';

class NoPrintCalls extends AnalysisRule {
  static const LintCode code = LintCode(
    'no_print_calls',
    \"Don't call 'print'.\",
    correctionMessage: 'Try removing the print call.',
  );

  NoPrintCalls()
    : super(name: 'no_print_calls', description: \"Don't call 'print'.\");

  @override
  LintCode get diagnosticCode => code;

  @override
  void registerNodeProcessors(
    RuleVisitorRegistry registry,
    RuleContext context,
  ) {
    registry.addMethodInvocation(this, _Visitor(this));
  }
}

class _Visitor extends SimpleAstVisitor<void> {
  final NoPrintCalls rule;

  _Visitor(this.rule);

  @override
  void visitMethodInvocation(MethodInvocation node) {
    if (node.target == null && node.methodName.name == 'print') {
      rule.reportAtNode(node.methodName);
    }
  }
}

class RemovePrintFix extends ResolvedCorrectionProducer {
  static const _fixKind = FixKind(
    'my_lint.fix.removePrint',
    DartFixKindPriority.standard,
    'Remove print call',
  );

  RemovePrintFix({required super.context});

  @override
  CorrectionApplicability get applicability =>
      CorrectionApplicability.singleLocation;

  @override
  FixKind get fixKind => _fixKind;

  @override
  Future<void> compute(ChangeBuilder builder) async {
    final stmt = node.thisOrAncestorOfType<ExpressionStatement>();
    if (stmt == null) return;
    await builder.addDartFileEdit(file, (builder) {
      builder.addDeletion(SourceRange(stmt.offset, stmt.length));
    });
  }
}

class AddCommentAssist extends ResolvedCorrectionProducer {
  static const _assistKind = AssistKind(
    'my_lint.assist.addComment',
    30,
    'Add trailing comment',
  );

  AddCommentAssist({required super.context});

  @override
  CorrectionApplicability get applicability =>
      CorrectionApplicability.singleLocation;

  @override
  AssistKind get assistKind => _assistKind;

  @override
  Future<void> compute(ChangeBuilder builder) async {
    final stmt = node.thisOrAncestorOfType<ExpressionStatement>();
    if (stmt == null) return;
    await builder.addDartFileEdit(file, (builder) {
      builder.addSimpleInsertion(stmt.end, ' // comment');
    });
  }
}
",
    );
    root
}

fn diagnostics_for(c: &LspClient, uri: &str) -> Vec<Value> {
    let mut list = c.state.diagnostics.get(uri).cloned().unwrap_or_default();
    list.sort_by_key(|d| {
        (
            d["range"]["start"]["line"].as_u64().unwrap_or(0),
            d["range"]["start"]["character"].as_u64().unwrap_or(0),
            d["code"].as_str().unwrap_or("").to_string(),
        )
    });
    list
}

/// Waits until `predicate` holds on the diagnostics of `uri` (since `dart
/// language-server` starts plugin isolates in the background after its own
/// analysis `$/progress` ends).
fn wait_for_diagnostics(
    c: &mut LspClient,
    uri: &str,
    step: &str,
    predicate: impl Fn(&[Value]) -> bool,
) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let diags = diagnostics_for(c, uri);
        if predicate(&diags) {
            c.settle(false);
            return diagnostics_for(c, uri);
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for diagnostics in step '{step}'; current diagnostics: {diags:#?}"
        );
        c.settle_with_grace(Duration::from_millis(500));
    }
}

fn count_code(diags: &[Value], code: &str) -> usize {
    diags.iter().filter(|d| d["code"] == json!(code)).count()
}

fn run_plugin_session(program: &str, root: &Path) -> BTreeMap<String, Value> {
    let mut c = LspClient::spawn(program, &session_args(), &[]);
    c.step_timeout = Duration::from_secs(120);
    let main_path = root.join("lib/main.dart");
    let main_uri = file_uri(&main_path);

    let init = c.request("initialize", dart_code_initialize_params(root));
    assert!(init["result"]["capabilities"].is_object(), "{init}");
    c.notify("initialized", json!({}));
    c.settle(true);

    c.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": {
                "uri": main_uri,
                "languageId": "dart",
                "version": 1,
                "text": MAIN_INITIAL,
            }
        }),
    );

    // Wait for both built-in lint (`prefer_final_locals`) and plugin lint (`no_print_calls`).
    let initial_diags = wait_for_diagnostics(&mut c, &main_uri, "initial didOpen", |d| {
        count_code(d, "no_print_calls") == 1 && count_code(d, "prefer_final_locals") == 1
    });

    let print_diag = initial_diags
        .iter()
        .find(|d| d["code"] == json!("no_print_calls"))
        .expect("expected no_print_calls diagnostic")
        .clone();
    let print_range = print_diag["range"].clone();

    // Request quickfixes at the plugin lint diagnostic.
    let quickfixes_resp = c.request(
        "textDocument/codeAction",
        json!({
            "textDocument": {"uri": main_uri},
            "range": print_range,
            "context": {
                "diagnostics": [print_diag],
                "only": ["quickfix"],
            }
        }),
    );
    let quickfixes = quickfixes_resp["result"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    // Request refactor/assists at the plugin lint diagnostic.
    let assists_resp = c.request(
        "textDocument/codeAction",
        json!({
            "textDocument": {"uri": main_uri},
            "range": print_range,
            "context": {
                "diagnostics": [],
                "only": ["refactor"],
            }
        }),
    );
    let assists = assists_resp["result"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let plugin_assists: Vec<Value> = assists
        .into_iter()
        .filter(|a| {
            a.pointer("/command/arguments/0/action") == Some(&json!("assist from plugin"))
                || a.get("title") == Some(&json!("Add trailing comment"))
        })
        .collect();

    // Edit the open file to add a second `print('second');` call (`ChangeContentOverlay`).
    c.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": main_uri, "version": 2},
            "contentChanges": [{
                "range": {
                    "start": {"line": 1, "character": 17},
                    "end": {"line": 1, "character": 17},
                },
                "text": "\n  print('second');",
            }]
        }),
    );
    let after_add_diags = wait_for_diagnostics(&mut c, &main_uri, "didChange add print", |d| {
        count_code(d, "no_print_calls") == 2 && count_code(d, "prefer_final_locals") == 1
    });

    // Edit the open file to remove all unignored `print` calls.
    c.notify(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": main_uri, "version": 3},
            "contentChanges": [{
                "text": MAIN_EDITED_CLEAN,
            }]
        }),
    );
    let after_clean_diags =
        wait_for_diagnostics(&mut c, &main_uri, "didChange remove prints", |d| {
            count_code(d, "no_print_calls") == 0 && count_code(d, "prefer_final_locals") == 1
        });

    // Close the file (`RemoveContentOverlay`): disk still has `MAIN_INITIAL` with 1 `no_print_calls`.
    c.notify(
        "textDocument/didClose",
        json!({
            "textDocument": {"uri": main_uri},
        }),
    );
    let after_close_diags = wait_for_diagnostics(&mut c, &main_uri, "didClose", |d| {
        count_code(d, "no_print_calls") == 1 && count_code(d, "prefer_final_locals") == 1
    });

    let (_, exit_code) = c.shutdown_and_exit();
    assert_eq!(exit_code, 0, "{program} exited with {exit_code}");

    let _ = MAIN_EDITED_ONE_MORE;
    let mut out = BTreeMap::new();
    out.insert("1_initial_diagnostics".into(), Value::Array(initial_diags));
    out.insert("2_quickfixes".into(), Value::Array(quickfixes));
    out.insert("3_plugin_assists".into(), Value::Array(plugin_assists));
    out.insert(
        "4_after_add_diagnostics".into(),
        Value::Array(after_add_diags),
    );
    out.insert(
        "5_after_clean_diagnostics".into(),
        Value::Array(after_clean_diags),
    );
    out.insert(
        "6_after_close_diagnostics".into(),
        Value::Array(after_close_diags),
    );
    out
}

#[test]
fn lsp_plugins_parity() {
    assert!(
        dart_3_13_available(),
        "dart 3.13.3 must be on PATH for lsp_plugins_parity"
    );
    let root = create_plugin_fixture();
    let dart = run_plugin_session("dart", &root);
    let dartr = run_plugin_session(env!("CARGO_BIN_EXE_dartr"), &root);

    // Ensure Dart's answers are non-empty and include the plugin diagnostic, fix, and assist.
    assert!(
        !dart["1_initial_diagnostics"].as_array().unwrap().is_empty(),
        "Dart initial diagnostics must not be empty"
    );
    let has_plugin_fix = dart["2_quickfixes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["title"] == json!("Remove print call"));
    assert!(
        has_plugin_fix,
        "Dart quickfixes must include 'Remove print call': {:#?}",
        dart["2_quickfixes"]
    );
    let has_plugin_assist = dart["3_plugin_assists"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["title"] == json!("Add trailing comment"));
    assert!(
        has_plugin_assist,
        "Dart assists must include 'Add trailing comment': {:#?}",
        dart["3_plugin_assists"]
    );

    assert_eq!(dart, dartr);
}
