//! Integration tests of the verifiers of group g4b (wave D12): the bool
//! expression and nullable dereference verifiers (called by the resolver),
//! the unicode text and language version override verifiers (library-wide
//! steps), and the verifiers that the error verifier and the best
//! practices verifier call on single nodes (const arguments, error
//! handlers, async returns, doc comments).
//!
//! The cases are the analyzer tests themselves: each `test_*` method of the
//! Dart test files in `pkg/analyzer/test/src/diagnostics/` that calls
//! `resolveTestCodeWithDiagnostics(r'''...''')` (or
//! `resolveFilesWithDiagnostics`) is read from `third_party/dart-sdk`, its
//! inline expectation markers (`//  ^^^` and `// [diag.x][column c][length
//! l] ...`, see `analyzer_testing/lib/src/expected_diagnostics.dart`) give
//! the expected diagnostics, and the cleaned code is analyzed. Only the
//! codes of the test file are compared (other wave D codes are not
//! reported yet). Verifiers that are not wired yet (their caller is the
//! error verifier or the best practices verifier) are called on every node
//! of the resolved unit through a test [`VerifierHost`].

mod support;

use std::path::PathBuf;

use dartr_ast::{Ast, AstVisitor, Comment, Id, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, LocatedDiagnostic};
use dartr_element::{Ctx, EId, FId, LibraryElement, LibraryFragment, ResolutionTables};
use dartr_parser::experimental_features::ExperimentalFeatures;
use dartr_resolver::error::{
    VerifierHost, async_return_visitor, const_argument_verifier, doc_comment_verifier,
    error_handler_verifier,
};
use dartr_resolver::library_analyzer::ResolvedUnit;
use dartr_resolver::options::AnalysisOptions;
use dartr_resolver::tables::ResolverTables;
use dartr_typesystem::TypeSystem;

/// One analyzer test: the files (name, cleaned code) and the expected
/// diagnostics (file, camel case code name, offset, length).
struct Case {
    name: String,
    files: Vec<(String, String)>,
    expected: Vec<(String, String, usize, usize)>,
}

fn sdk_test_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/dart-sdk/pkg/analyzer/test/src/diagnostics")
}

/// The UTF-16 length of [s].
fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Dart `removeDiagnosticExpectations` and the parse of the markers:
/// returns the cleaned code and the expected (code, offset, length).
fn parse_markers(code: &str) -> (String, Vec<(String, usize, usize)>) {
    let is_caret = |l: &str| {
        let t = l.trim_start_matches([' ', '\t']);
        t.strip_prefix("//").is_some_and(|r| {
            let r = r.trim_matches([' ', '\t']);
            !r.is_empty() && r.chars().all(|c| c == '^')
        })
    };
    let expectation = |l: &str| -> Option<String> {
        let t = l.trim_start_matches([' ', '\t']).strip_prefix("//")?;
        let t = t.trim_start_matches([' ', '\t']).strip_prefix('[')?;
        if let Some(rest) = t.strip_prefix("diag.") {
            let end = rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
            (rest.as_bytes()[end] == b']').then(|| rest.to_string())
        } else if t.starts_with("context") {
            Some(String::new())
        } else {
            None
        }
    };
    let mut clean_lines: Vec<&str> = Vec::new();
    let mut expected = Vec::new();
    // The start offset of the last code line in the cleaned code.
    let mut offset = 0usize;
    let mut last_line_start = 0usize;
    let mut caret: Option<(usize, usize)> = None;
    for line in code.split_inclusive('\n') {
        let text = line.trim_end_matches('\n');
        if is_caret(text) {
            let column = text.find('^').unwrap();
            let length = text.matches('^').count();
            caret = Some((column, length));
            continue;
        }
        if let Some(rest) = expectation(text) {
            if rest.is_empty() {
                continue;
            }
            let end = rest.find(']').unwrap();
            let name = rest[..end].to_string();
            let after = &rest[end + 1..];
            let location = after.strip_prefix("[column ").and_then(|r| {
                let (column, r) = r.split_once(']')?;
                let length = r.strip_prefix("[length ")?.split_once(']')?.0;
                Some((
                    column.parse::<usize>().ok()? - 1,
                    length.parse::<usize>().ok()?,
                ))
            });
            if let Some((column, length)) = location.or(caret) {
                expected.push((name, last_line_start + column, length));
            }
            continue;
        }
        caret = None;
        last_line_start = offset;
        offset += utf16_len(line);
        clean_lines.push(line);
    }
    let mut clean = clean_lines.concat();
    // Terminators separate the retained lines.
    if clean.ends_with('\n') && !code.ends_with('\n') {
        clean.pop();
    }
    (clean, expected)
}

/// The multi-line string literal (`r'''...'''` or `'''...'''`) that starts at
/// [start] in [text]: its value and the end of the literal. `None` for an
/// interpolation. Dart drops a line break right after the opening quotes.
fn string_at(text: &str, start: usize) -> Option<(String, usize)> {
    let (raw, rest) = match text[start..].strip_prefix("r'''") {
        Some(rest) => (true, rest),
        None => (false, text[start..].strip_prefix("'''")?),
    };
    let end = rest.find("'''")?;
    let literal = &rest[..end];
    let literal_end = text.len() - rest.len() + end + 3;
    let mut value = String::new();
    if raw {
        value.push_str(literal);
    } else {
        let mut chars = literal.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '$' => return None,
                '\\' => match chars.next()? {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'u' => {
                        let hex: String = if chars.peek() == Some(&'{') {
                            chars.next();
                            chars.by_ref().take_while(|&c| c != '}').collect()
                        } else {
                            chars.by_ref().take(4).collect()
                        };
                        value.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    other => value.push(other),
                },
                _ => value.push(c),
            }
        }
    }
    if let Some(v) = value.strip_prefix('\n') {
        value = v.to_string();
    }
    Some((value, literal_end))
}

/// The test cases of the Dart test file [file].
fn cases(file: &str) -> Vec<Case> {
    let path = sdk_test_dir().join(file);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    // The lines `  [ReturnType ]test_name() async {`.
    let mut methods = Vec::new();
    let mut line_start = 0;
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        if t.ends_with("async {")
            && let Some(paren) = t.find('(')
            && t[..paren]
                .rsplit(' ')
                .next()
                .is_some_and(|n| n.starts_with("test_"))
        {
            methods.push(line_start + line.find("test_").unwrap() - 2);
        }
        line_start += line.len();
    }
    for (k, &start) in methods.iter().enumerate() {
        let end = methods.get(k + 1).copied().unwrap_or(text.len());
        let body = &text[start..end];
        let name = body[2..body.find('(').unwrap_or(2)].to_string();
        if body.contains("newFile") || body.contains("writeTestPackageConfig") {
            continue;
        }
        let mut files = Vec::new();
        let mut expected = Vec::new();
        let mut ok = true;
        if let Some(i) = body.find("resolveTestCodeWithDiagnostics(") {
            let s = i + "resolveTestCodeWithDiagnostics(".len();
            match string_at(body, s) {
                Some((code, _)) => files.push(("main.dart".to_string(), code)),
                None => ok = false,
            }
        } else if let Some(i) = body.find("resolveFilesWithDiagnostics({") {
            // `x: r'''...''',` entries; the first file is the library.
            let mut p = i;
            let mut first = true;
            while let Some(j) = body[p..].find(": r'''") {
                let var_start = body[..p + j]
                    .rfind(|c: char| c.is_whitespace())
                    .map_or(0, |v| v + 1);
                let var = &body[var_start..p + j];
                let Some((code, e)) = string_at(body, p + j + 2) else {
                    ok = false;
                    break;
                };
                let name = if first {
                    "main".to_string()
                } else {
                    var.to_string()
                };
                first = false;
                files.push((format!("{name}.dart"), format!("{var}\u{0}{code}")));
                p = e;
            }
            // The library is `main.dart`: rename the first file in the
            // `part of` URIs.
            if let Some((_, first_code)) = files.first().cloned() {
                let first_var = first_code.split('\u{0}').next().unwrap().to_string();
                for (_, code) in &mut files {
                    let c = code.split_once('\u{0}').unwrap().1.to_string();
                    *code = c.replace(&format!("'{first_var}.dart'"), "'main.dart'");
                }
            }
        } else {
            continue;
        }
        if !ok || files.is_empty() {
            continue;
        }
        let mut clean_files = Vec::new();
        for (name, code) in files {
            // `package:meta` is a local library with the same URI length.
            let code = code.replace("'package:meta/meta.dart'", "'meta_package_meta.dart'");
            if code.contains("package:") {
                ok = false;
            }
            let (clean, e) = parse_markers(&code);
            expected.extend(e.into_iter().map(|(c, o, l)| (name.clone(), c, o, l)));
            clean_files.push((name, clean));
        }
        if ok {
            out.push(Case {
                name,
                files: clean_files,
                expected,
            });
        }
    }
    out
}

const META: &str = "library meta;\n\
class _MustBeConst { const _MustBeConst(); }\n\
const _MustBeConst mustBeConst = _MustBeConst();\n";

/// The verifiers that are not wired yet, called the way their Dart callers
/// call them.
#[derive(Clone, Copy)]
enum Direct {
    /// Wired: only the diagnostics of the analysis.
    None,
    /// `BestPracticesVerifier.visitComment`.
    DocComment,
    /// `BestPracticesVerifier.visitMethodInvocation`.
    ErrorHandler,
    /// `ErrorVerifier.visitX` -> `_constArgumentsVerifier.visitX`.
    ConstArguments,
    /// `ErrorVerifier.visitReturnStatement`.
    AsyncReturn,
}

/// A [`VerifierHost`] over a resolved unit of a test.
struct TestHost<'a> {
    ctx: Ctx<'a>,
    unit: &'a ResolvedUnit,
    library: EId<LibraryElement>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> VerifierHost<'a> for TestHost<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }
    fn type_system(&self) -> TypeSystem<'a> {
        TypeSystem::new(self.ctx)
    }
    fn ast(&self) -> &Ast {
        &self.unit.ast
    }
    fn tables(&self) -> &ResolutionTables {
        &self.unit.tables
    }
    fn rt(&self) -> &ResolverTables {
        &self.unit.rt
    }
    fn library(&self) -> EId<LibraryElement> {
        self.library
    }
    fn fragment(&self) -> FId<LibraryFragment> {
        self.unit.fragment
    }
    fn options(&self) -> AnalysisOptions {
        AnalysisOptions::default()
    }
    fn features(&self) -> ExperimentalFeatures {
        ExperimentalFeatures::latest()
    }
    fn report(&mut self, diagnostic: LocatedDiagnostic) {
        self.diagnostics.push(diagnostic.into_diagnostic());
    }
}

/// Collects every node in visit order.
struct AllNodes(Vec<NodeId>);

impl AstVisitor for AllNodes {
    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        self.0.push(node);
        ast.visit_children(node, self);
    }
}

/// Calls the verifier [direct] on the nodes of [unit].
fn run_direct(host: &mut TestHost<'_>, direct: Direct) {
    let ast = &host.unit.ast;
    let mut all = AllNodes(Vec::new());
    ast.accept(host.unit.unit, &mut all);
    for node in all.0 {
        let kind = ast.kind(node);
        match direct {
            Direct::None => {}
            Direct::DocComment => {
                if let Some(comment) = ast.cast::<Comment>(node) {
                    for doc_import in &ast[comment].doc_imports {
                        doc_comment_verifier::doc_import(host, doc_import);
                    }
                    for doc_directive in &ast[comment].doc_directives {
                        doc_comment_verifier::doc_directive(host, doc_directive);
                    }
                }
            }
            Direct::ErrorHandler => {
                if kind == NodeKind::MethodInvocation {
                    error_handler_verifier::verify_method_invocation(host, Id::from_raw(node));
                }
            }
            Direct::AsyncReturn => {
                if kind == NodeKind::ReturnStatement
                    && ast[Id::<dartr_ast::ReturnStatement>::from_raw(node)]
                        .expression
                        .is_some()
                {
                    async_return_visitor::report_missing_await_in_try_block(
                        host,
                        Id::from_raw(node),
                    );
                }
            }
            Direct::ConstArguments => {
                use const_argument_verifier as c;
                match kind {
                    NodeKind::AnonymousMethodInvocation => {
                        c::visit_anonymous_method_invocation(host, Id::from_raw(node))
                    }
                    NodeKind::AssignmentExpression => {
                        c::visit_assignment_expression(host, Id::from_raw(node))
                    }
                    NodeKind::BinaryExpression => {
                        c::visit_binary_expression(host, Id::from_raw(node))
                    }
                    NodeKind::ConstructorReference => {
                        c::visit_constructor_reference(host, Id::from_raw(node))
                    }
                    NodeKind::FunctionExpressionInvocation => {
                        c::visit_function_expression_invocation(host, Id::from_raw(node))
                    }
                    NodeKind::IndexExpression => {
                        c::visit_index_expression(host, Id::from_raw(node))
                    }
                    NodeKind::InstanceCreationExpression => {
                        c::visit_instance_creation_expression(host, Id::from_raw(node))
                    }
                    NodeKind::MethodInvocation => {
                        c::visit_method_invocation(host, Id::from_raw(node))
                    }
                    NodeKind::PrefixedIdentifier => {
                        c::visit_prefixed_identifier(host, Id::from_raw(node))
                    }
                    NodeKind::PropertyAccess => c::visit_property_access(host, Id::from_raw(node)),
                    NodeKind::RedirectingConstructorInvocation => {
                        c::visit_redirecting_constructor_invocation(host, Id::from_raw(node))
                    }
                    NodeKind::SimpleIdentifier => {
                        c::visit_simple_identifier(host, Id::from_raw(node))
                    }
                    NodeKind::SuperConstructorInvocation => {
                        c::visit_super_constructor_invocation(host, Id::from_raw(node))
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Runs the cases of the Dart test [file], comparing the codes for which
/// [is_code] holds. Returns (passed, total, failures).
fn run_file(
    file: &str,
    is_code: &dyn Fn(&str) -> bool,
    direct: Direct,
) -> Option<(usize, usize, Vec<String>)> {
    let cases = cases(file);
    assert!(!cases.is_empty(), "no test cases found in {file}");
    let mut passed = 0;
    let mut failures = Vec::new();
    for case in &cases {
        let mut files: Vec<(&str, &str)> = case
            .files
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();
        files.push(("meta_package_meta.dart", META));
        let a = support::analyze(&files)?;
        let mut actual = Vec::new();
        for unit in &a.library.units {
            let name = unit.path.rsplit('/').next().unwrap_or("").to_string();
            let mut diagnostics = unit.diagnostics.clone();
            if !matches!(direct, Direct::None) {
                let mut host = TestHost {
                    ctx: a.ctx(unit),
                    unit,
                    library: a.library.library,
                    diagnostics: Vec::new(),
                };
                run_direct(&mut host, direct);
                diagnostics.extend(host.diagnostics);
            }
            if let Some(panic) = &unit.panic {
                actual.push((name.clone(), format!("PANIC {panic}"), 0, 0));
            }
            for d in diagnostics {
                if is_code(d.code.camel_case_name) {
                    actual.push((
                        name.clone(),
                        d.code.camel_case_name.to_string(),
                        d.offset,
                        d.length,
                    ));
                }
            }
        }
        let mut expected: Vec<_> = case
            .expected
            .iter()
            .filter(|(_, c, _, _)| is_code(c))
            .cloned()
            .collect();
        expected.sort();
        actual.sort();
        if expected == actual {
            passed += 1;
        } else {
            failures.push(format!(
                "{file} {}:\n  expected {expected:?}\n  actual   {actual:?}",
                case.name
            ));
        }
    }
    Some((passed, cases.len(), failures))
}

/// Runs the Dart test files of one table row and checks that at least
/// [min_passed] cases pass (the others are listed).
fn check(rows: &[(&str, &dyn Fn(&str) -> bool, Direct, usize)]) {
    let mut report = Vec::new();
    let mut below = Vec::new();
    for &(file, is_code, direct, min_passed) in rows {
        let Some((passed, total, failures)) = run_file(file, is_code, direct) else {
            eprintln!("skipped: no Dart SDK on PATH");
            return;
        };
        report.push(format!("{file}: {passed}/{total}"));
        for f in &failures {
            report.push(f.clone());
        }
        if passed < min_passed {
            below.push(format!("{file}: {passed}/{total} < {min_passed}"));
        }
    }
    eprintln!("{}", report.join("\n"));
    assert!(
        below.is_empty(),
        "{}\n\n{}",
        below.join("\n"),
        report.join("\n")
    );
}

#[test]
fn markers_give_offsets_in_the_cleaned_code() {
    let (clean, expected) = parse_markers(
        "void f(Null a) {\n  if (a) {}\n//    ^\n// [diag.nonBoolCondition] Conditions.\n}\n\
         /// {@youtube 600}\n// [diag.docDirectiveMissingTwoArguments][column 5][length 15] M.\n",
    );
    assert_eq!(
        clean,
        "void f(Null a) {\n  if (a) {}\n}\n/// {@youtube 600}\n"
    );
    assert_eq!(
        expected,
        vec![
            ("nonBoolCondition".to_string(), 23, 1),
            ("docDirectiveMissingTwoArguments".to_string(), 35, 15),
        ]
    );
}

#[test]
fn bool_conditions_and_operands_match_the_analyzer_tests() {
    check(&[
        (
            "non_bool_condition_test.dart",
            &|c| c == "nonBoolCondition" || c == "uncheckedUseOfNullableValueAsCondition",
            Direct::None,
            20,
        ),
        (
            "non_bool_expression_test.dart",
            &|c| c == "nonBoolExpression",
            Direct::None,
            3,
        ),
        (
            "non_bool_negation_expression_test.dart",
            &|c| c == "nonBoolNegationExpression",
            Direct::None,
            2,
        ),
        (
            "non_bool_operand_test.dart",
            &|c| c == "nonBoolOperand",
            Direct::None,
            3,
        ),
    ]);
}

#[test]
fn unicode_text_and_language_version_overrides_match_the_analyzer_tests() {
    check(&[
        (
            "text_direction_code_point_test.dart",
            &|c| c.starts_with("textDirectionCodePoint"),
            Direct::None,
            5,
        ),
        (
            "invalid_language_override_test.dart",
            &|c| c.starts_with("invalidLanguageVersionOverride"),
            Direct::None,
            25,
        ),
        (
            "inconsistent_language_version_override_test.dart",
            &|c| c == "inconsistentLanguageVersionOverride",
            Direct::None,
            8,
        ),
    ]);
}

#[test]
fn doc_comments_match_the_analyzer_tests() {
    let doc = |c: &str| {
        c.starts_with("docDirective")
            && c != "docDirectiveMissingClosingBrace"
            && c != "docDirectiveMissingClosingTag"
            && c != "docDirectiveMissingOpeningTag"
            && c != "docDirectiveUnknown"
    };
    let doc_import = |c: &str| c.starts_with("docImportCannot");
    check(&[
        (
            "doc_directive_argument_wrong_format_test.dart",
            &doc,
            Direct::DocComment,
            5,
        ),
        (
            "doc_directive_has_extra_arguments_test.dart",
            &doc,
            Direct::DocComment,
            3,
        ),
        (
            "doc_directive_has_unexpected_named_argument_test.dart",
            &doc,
            Direct::DocComment,
            1,
        ),
        (
            "doc_directive_missing_one_argument_test.dart",
            &doc,
            Direct::DocComment,
            3,
        ),
        (
            "doc_directive_missing_two_arguments_test.dart",
            &doc,
            Direct::DocComment,
            3,
        ),
        (
            "doc_directive_missing_three_arguments_test.dart",
            &doc,
            Direct::DocComment,
            1,
        ),
        (
            "doc_import_cannot_be_deferred_test.dart",
            &doc_import,
            Direct::DocComment,
            1,
        ),
        (
            "doc_import_cannot_have_combinators_test.dart",
            &doc_import,
            Direct::DocComment,
            1,
        ),
        (
            "doc_import_cannot_have_configurations_test.dart",
            &doc_import,
            Direct::DocComment,
            1,
        ),
        (
            "doc_import_cannot_have_prefix_test.dart",
            &doc_import,
            Direct::DocComment,
            1,
        ),
    ]);
}

#[test]
fn error_handlers_match_the_analyzer_tests() {
    let codes = |c: &str| {
        matches!(
            c,
            "argumentTypeNotAssignableToErrorHandler"
                | "returnTypeInvalidForCatchError"
                | "returnOfInvalidTypeFromCatchError"
                | "returnTypeInvalidForThen"
                | "returnOfInvalidTypeFromThen"
        )
    };
    check(&[
        (
            "argument_type_not_assignable_to_error_handler_test.dart",
            &codes,
            Direct::ErrorHandler,
            40,
        ),
        (
            "return_type_invalid_for_catch_error_test.dart",
            &codes,
            Direct::ErrorHandler,
            3,
        ),
        (
            "return_of_invalid_type_from_catch_error_test.dart",
            &codes,
            Direct::ErrorHandler,
            5,
        ),
    ]);
}

#[test]
fn const_arguments_and_async_returns_match_the_analyzer_tests() {
    check(&[
        (
            "non_const_argument_for_const_parameter_test.dart",
            &|c| c == "nonConstArgumentForConstParameter",
            Direct::ConstArguments,
            25,
        ),
        (
            "tearoff_with_must_be_const_parameter_test.dart",
            &|c| c == "tearoffWithMustBeConstParameter",
            Direct::ConstArguments,
            3,
        ),
        (
            "unawaited_return_in_try_block_test.dart",
            &|c| c == "unawaitedReturnInTryBlock",
            Direct::AsyncReturn,
            10,
        ),
    ]);
}
