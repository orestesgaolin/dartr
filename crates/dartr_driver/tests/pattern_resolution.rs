//! Integration tests of the pattern resolution (`dartr_resolver`
//! `pattern_resolver.rs`, `resolver/patterns.rs`,
//! `list_pattern_resolver.rs`): Dart source analyzed with
//! `Driver::analyze_library`, checked against the static types, matched
//! value types and variable types that the Dart analyzer 3.13.3 gives.
//!
//! The tests need the Dart SDK (`dart` on `PATH`); without it they return
//! early.
//!
//! The resolver resolves a function body only when the element binding pass
//! bound the declaration (`ResolutionTables.declared_fragment`), and a
//! pattern variable only when the pass bound the `DeclaredVariablePattern`.
//! Until that pass is ported (`element_binding_visitor.rs`), the tests are
//! ignored. The second group also needs the resolution visitor (scopes, type
//! annotations, identifiers) and the record and collection literal
//! resolvers.

use std::sync::Arc;

use dartr_ast::{Ast, NodeId, NodeKind};
use dartr_driver::driver::Driver;
use dartr_driver::file_state::{FileConfig, FileSystemState, SourceFactory};
use dartr_element::{
    Ctx, DisplayOptions, ElemRef, FeatureSet, Generation, LocalVariableElement, NoopSink, TypeId,
    TypeProvider, type_display_string_with,
};
use dartr_project::{DartSdk, Packages, Workspace};
use dartr_resolver::library_analyzer::{ResolvedLibrary, ResolvedUnit};
use dartr_resolver::options::AnalysisOptions;

/// One analyzed library with one unit.
struct Analyzed {
    driver: Driver,
    library: ResolvedLibrary,
    source: String,
    tp: TypeProvider,
}

/// Analyzes [source] as the library `<tmp>/<name>/lib.dart`. Returns `None`
/// when no Dart SDK is found.
fn analyze(name: &str, source: &str) -> Option<Analyzed> {
    let sdk_path = dartr_project::sdk::find_sdk_path()?;
    let sdk = DartSdk::new(&sdk_path);
    let version = sdk
        .language_version()
        .map(|v| (v.major, v.minor))
        .unwrap_or((3, 13));
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("pattern_resolution")
        .join(name);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join("lib.dart");
    std::fs::write(&path, source).expect("write source");
    let path = dartr_project::paths::normalize(path.to_str().expect("utf-8 path"));

    let source_factory = SourceFactory {
        workspace: Workspace::basic(Packages::empty(), "/"),
        sdk: Some(sdk),
    };
    let config_for = Box::new(move |_: &str, _: &str| FileConfig {
        package_language_version: version,
        experiments: Vec::new(),
    });
    let mut driver = Driver::new(
        FileSystemState::new(source_factory, config_for),
        Arc::new(Generation::new(0)),
    );
    let file = driver.fs.get_file_for_path(&path);
    driver.fs.discover();
    driver.link_libraries(&[file]);
    let library = driver
        .analyze_library(file, AnalysisOptions::default())
        .expect("linked library");
    let tp = dartr_link::types_builder::world_type_provider(&driver.state.world);
    Some(Analyzed {
        driver,
        library,
        source: source.to_string(),
        tp,
    })
}

impl Analyzed {
    fn unit(&self) -> &ResolvedUnit {
        let unit = &self.library.units[0];
        assert_eq!(unit.panic, None, "the resolver panicked");
        unit
    }

    fn with_ctx<R>(&self, f: impl FnOnce(&Ctx<'_>, &ResolvedUnit) -> R) -> R {
        let unit = self.unit();
        let features = FeatureSet::default();
        let sink = NoopSink;
        let ctx = Ctx {
            world: &self.driver.state.world,
            current: None,
            local: Some(&unit.local),
            tp: &self.tp,
            features: &features,
            req: &sink,
        };
        f(&ctx, unit)
    }

    fn type_string(&self, ty: TypeId) -> String {
        self.with_ctx(|ctx, _| type_display_string_with(ctx, ty, DisplayOptions::default()))
    }

    /// The node of [kind] that starts where [needle] starts in the source
    /// (the first occurrence of [needle]).
    fn find(&self, kind: NodeKind, needle: &str) -> NodeId {
        let offset = self
            .source
            .find(needle)
            .unwrap_or_else(|| panic!("`{needle}` is not in the source")) as u32;
        let unit = self.unit();
        find_node(&unit.ast, unit.unit.raw(), kind, offset)
            .unwrap_or_else(|| panic!("no {kind:?} at `{needle}`"))
    }

    /// `staticType` of the expression of [kind] at [needle].
    fn static_type(&self, kind: NodeKind, needle: &str) -> String {
        let node = self.find(kind, needle);
        let ty = *self
            .unit()
            .tables
            .static_type
            .get(node)
            .unwrap_or_else(|| panic!("no static type for {kind:?} at `{needle}`"));
        self.type_string(ty)
    }

    /// `matchedValueType` of the pattern of [kind] at [needle].
    fn matched_value_type(&self, kind: NodeKind, needle: &str) -> String {
        let node = self.find(kind, needle);
        let ty = self
            .unit()
            .tables
            .pattern_info
            .get(node)
            .and_then(|i| i.matched_value_type)
            .unwrap_or_else(|| panic!("no matched value type for {kind:?} at `{needle}`"));
        self.type_string(ty)
    }

    /// `requiredType` of the list or map pattern at [needle].
    fn required_type(&self, kind: NodeKind, needle: &str) -> String {
        let node = self.find(kind, needle);
        let ty = self
            .unit()
            .tables
            .pattern_info
            .get(node)
            .and_then(|i| i.required_type)
            .unwrap_or_else(|| panic!("no required type for {kind:?} at `{needle}`"));
        self.type_string(ty)
    }

    /// The type of the variable declared by the `DeclaredVariablePattern`
    /// at [needle].
    fn variable_type(&self, needle: &str) -> String {
        let node = self.find(NodeKind::DeclaredVariablePattern, needle);
        self.with_ctx(|ctx, unit| {
            let fragment = *unit
                .tables
                .declared_fragment
                .get(node)
                .unwrap_or_else(|| panic!("`{needle}` is not bound"));
            let element = *ctx
                .fragment_data(fragment)
                .expect("fragment")
                .element
                .get();
            let local = element
                .cast::<LocalVariableElement>()
                .expect("local variable");
            let ty = ctx.get(local).type_.get().expect("variable type");
            type_display_string_with(ctx, ty, DisplayOptions::default())
        })
    }

    /// The display string of the element of the node of [kind] at
    /// [needle] (Dart `node.element`).
    fn element_name(&self, kind: NodeKind, needle: &str) -> Option<String> {
        let node = self.find(kind, needle);
        self.with_ctx(|ctx, unit| {
            let element = *unit.tables.element.get(node)?;
            let base = match element {
                ElemRef::Base(e) => e,
                ElemRef::Member(m) => ctx.member(m).base,
            };
            Some(dartr_element::element_display_string_with(
                ctx,
                base,
                DisplayOptions::default(),
            ))
        })
    }

    /// The names of the diagnostics of the unit.
    fn diagnostic_names(&self) -> Vec<String> {
        self.unit()
            .diagnostics
            .iter()
            .map(|d| d.code.name.to_string())
            .collect()
    }
}

fn find_node(ast: &Ast, node: NodeId, kind: NodeKind, offset: u32) -> Option<NodeId> {
    if ast.kind(node) == kind && ast.offset(node) == offset {
        return Some(node);
    }
    for child in ast.children(node) {
        if ast.offset(child) <= offset
            && offset <= ast.end(child)
            && let Some(found) = find_node(ast, child, kind, offset)
        {
            return Some(found);
        }
    }
    None
}

// ------------------------------------------------------------ literal scrutinees

#[test]
#[ignore = "needs the element binding pass (element_binding_visitor.rs)"]
fn switch_expression_constant_relational_wildcard() {
    let Some(a) = analyze(
        "switch_expression_constant_relational_wildcard",
        r#"
String f() => switch (1) {
  0 => 'zero',
  > 0 => 'positive',
  _ => 'negative',
};
"#,
    ) else {
        return;
    };
    assert_eq!(a.static_type(NodeKind::SwitchExpression, "switch (1)"), "String");
    assert_eq!(a.matched_value_type(NodeKind::ConstantPattern, "0 =>"), "int");
    assert_eq!(a.matched_value_type(NodeKind::RelationalPattern, "> 0"), "int");
    assert_eq!(a.matched_value_type(NodeKind::WildcardPattern, "_ =>"), "int");
    // The operator `>` of `num` (Dart `RelationalPatternImpl.element`).
    assert_eq!(
        a.element_name(NodeKind::RelationalPattern, "> 0").as_deref(),
        Some("bool >(num other)")
    );
}

#[test]
#[ignore = "needs the element binding pass (element_binding_visitor.rs)"]
fn switch_expression_least_upper_bound_of_cases() {
    let Some(a) = analyze(
        "switch_expression_least_upper_bound_of_cases",
        r#"
Object f() => switch ('s') {
  'a' => 1,
  'b' => 2.5,
  _ => 3,
};
"#,
    ) else {
        return;
    };
    // The type of a switch expression is the least upper bound of the case
    // bodies: LUB(int, double, int) = num.
    assert_eq!(a.static_type(NodeKind::SwitchExpression, "switch ('s')"), "num");
    assert_eq!(a.matched_value_type(NodeKind::ConstantPattern, "'a' =>"), "String");
}

#[test]
#[ignore = "needs the element binding pass (element_binding_visitor.rs)"]
fn switch_statement_declared_variable_patterns() {
    let Some(a) = analyze(
        "switch_statement_declared_variable_patterns",
        r#"
void f() {
  switch (1) {
    case final first when first > 0:
      break;
    case var second:
      break;
  }
}
"#,
    ) else {
        return;
    };
    // The type of an untyped variable pattern is the matched value type.
    assert_eq!(a.variable_type("first when"), "int");
    assert_eq!(a.matched_value_type(NodeKind::DeclaredVariablePattern, "first when"), "int");
    assert_eq!(a.variable_type("second:"), "int");
}

#[test]
#[ignore = "needs the element binding pass (element_binding_visitor.rs)"]
fn switch_statement_shared_body_and_default() {
    let Some(a) = analyze(
        "switch_statement_shared_body_and_default",
        r#"
void f() {
  switch ('s') {
    case 'a':
    case 'b':
      break;
    default:
      break;
  }
}
"#,
    ) else {
        return;
    };
    assert_eq!(a.matched_value_type(NodeKind::ConstantPattern, "'a':"), "String");
    assert_eq!(a.matched_value_type(NodeKind::ConstantPattern, "'b':"), "String");
    assert!(a.diagnostic_names().is_empty(), "{:?}", a.diagnostic_names());
}

#[test]
#[ignore = "needs the element binding pass (element_binding_visitor.rs)"]
fn if_case_statement_with_null_check_pattern() {
    let Some(a) = analyze(
        "if_case_statement_with_null_check_pattern",
        r#"
void f() {
  if (1 case final y) {}
  if ('s' case var z?) {}
}
"#,
    ) else {
        return;
    };
    assert_eq!(a.variable_type("y)"), "int");
    assert_eq!(a.variable_type("z?"), "String");
    assert_eq!(a.matched_value_type(NodeKind::NullCheckPattern, "z?"), "String");
    // `?` on a value of a non-nullable type.
    assert_eq!(a.diagnostic_names(), vec!["unnecessary_null_check_pattern"]);
}

#[test]
#[ignore = "needs the element binding pass (element_binding_visitor.rs)"]
fn pattern_variable_declaration_parenthesized() {
    let Some(a) = analyze(
        "pattern_variable_declaration_parenthesized",
        r#"
void f() {
  var (p) = 1;
  final (q) = 'q';
}
"#,
    ) else {
        return;
    };
    assert_eq!(a.variable_type("p)"), "int");
    assert_eq!(a.variable_type("q)"), "String");
    assert_eq!(a.matched_value_type(NodeKind::ParenthesizedPattern, "(p)"), "int");
}

// ------------------------------------------------------------ realistic code

#[test]
#[ignore = "needs the element binding pass and the resolution visitor (scopes, type annotations), and the record literal resolver"]
fn switch_statement_record_and_typed_patterns() {
    let Some(a) = analyze(
        "switch_statement_record_and_typed_patterns",
        r#"
void f(Object o) {
  switch (o) {
    case (int a, String b):
      break;
    case int x when x > 0:
      break;
    case (final c, d: final e):
      break;
  }
}
"#,
    ) else {
        return;
    };
    assert_eq!(a.matched_value_type(NodeKind::RecordPattern, "(int a"), "Object");
    assert_eq!(a.variable_type("a, String"), "int");
    assert_eq!(a.variable_type("b)"), "String");
    // The fields of a record pattern match `Object?` when the scrutinee is
    // not a record type.
    assert_eq!(a.matched_value_type(NodeKind::DeclaredVariablePattern, "a, String"), "Object?");
    assert_eq!(a.variable_type("x when"), "int");
    assert_eq!(a.variable_type("c, d"), "Object?");
    assert_eq!(a.variable_type("e)"), "Object?");
}

#[test]
#[ignore = "needs the element binding pass and the resolution visitor (scopes, type annotations), and the collection literal resolvers"]
fn if_case_list_and_map_patterns() {
    let Some(a) = analyze(
        "if_case_list_and_map_patterns",
        r#"
void f(List<int> list, Map<String, double> map) {
  if (list case [var first, ...]) {}
  if (map case {'k': var v}) {}
}
"#,
    ) else {
        return;
    };
    assert_eq!(a.required_type(NodeKind::ListPattern, "[var first"), "List<int>");
    assert_eq!(a.variable_type("first,"), "int");
    assert_eq!(a.required_type(NodeKind::MapPattern, "{'k'"), "Map<String, double>");
    assert_eq!(a.variable_type("v}"), "double");
}

#[test]
#[ignore = "needs the element binding pass and the resolution visitor (scopes, type annotations)"]
fn object_pattern_fields_and_inferred_type_arguments() {
    let Some(a) = analyze(
        "object_pattern_fields_and_inferred_type_arguments",
        r#"
class Point {
  final int x;
  final int y;
  Point(this.x, this.y);
}

class Box<T> {
  final T value;
  Box(this.value);
}

void f(Object o, Box<String> box) {
  if (o case Point(x: var px, :var y)) {}
  if (box case Box(value: var inner)) {}
}
"#,
    ) else {
        return;
    };
    assert_eq!(a.variable_type("px"), "int");
    assert_eq!(a.variable_type("y))"), "int");
    assert_eq!(
        a.element_name(NodeKind::PatternField, "x: var px").as_deref(),
        Some("int get x")
    );
    // `Box` without type arguments: inferred from the matched value type.
    assert_eq!(a.variable_type("inner"), "String");
}

#[test]
#[ignore = "needs the element binding pass and the resolution visitor (scopes, join variables)"]
fn logical_or_pattern_join_variable() {
    let Some(a) = analyze(
        "logical_or_pattern_join_variable",
        r#"
void f((int, int) r) {
  switch (r) {
    case (0, var v) || (var v, 0):
      break;
  }
}
"#,
    ) else {
        return;
    };
    assert_eq!(a.variable_type("v) ||"), "int");
    assert_eq!(a.variable_type("v, 0)"), "int");
    assert!(a.diagnostic_names().is_empty(), "{:?}", a.diagnostic_names());
}

#[test]
#[ignore = "needs the element binding pass and the resolution visitor (assigned variable patterns), and the record literal resolver"]
fn pattern_assignment_static_type() {
    let Some(a) = analyze(
        "pattern_assignment_static_type",
        r#"
void f() {
  int a;
  String b;
  (a, b) = (1, 'x');
}
"#,
    ) else {
        return;
    };
    assert_eq!(a.static_type(NodeKind::PatternAssignment, "(a, b) ="), "(int, String)");
    assert_eq!(a.matched_value_type(NodeKind::AssignedVariablePattern, "a, b)"), "int");
}
