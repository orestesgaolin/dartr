//! Integration tests of unit C7: list, set-or-map and record literals,
//! collection elements (spread, null-aware, if, for) and for loops. Each
//! fixture in `tests/c7_fixtures` is ported from the analyzer resolution
//! tests (`list_literal_test.dart`, `set_or_map_literal_test.dart`,
//! `record_literal_test.dart`, `for_statement_test.dart`,
//! `for_element_test.dart`, `if_element_test.dart`). The expected static
//! types are the output of the Dart analyzer on the same fixtures
//! (`difftest resolved`); entries that depend on node kinds of other units
//! (invocations, prefixed identifiers, prefix and binary operators) are
//! left out.

mod support;

use dartr_ast::NodeKind;
use support::{Analyzed, analyze};

fn run(source: &str) -> Option<Analyzed> {
    let a = analyze(&[("main.dart", source)]);
    if a.is_none() {
        eprintln!("skipped: no Dart SDK on PATH");
    }
    a
}

/// Checks the static type of the node of each `(search, delta, kind,
/// type)`: the node of `kind` that starts `delta` characters after the
/// only occurrence of `search`.
fn check_types(source: &str, expected: &[(&str, usize, NodeKind, &str)]) {
    let Some(a) = run(source) else {
        return;
    };
    assert!(a.unit().panic.is_none(), "{:?}", a.unit().panic);
    let mut failures = Vec::new();
    for &(search, delta, kind, ty) in expected {
        let node = a.node_at(kind, search, 0, delta);
        let actual = a
            .unit()
            .tables
            .static_type
            .get(node)
            .map(|&t| a.type_str(t));
        if actual.as_deref() != Some(ty) {
            failures.push(format!("{search:?}: expected {ty}, got {actual:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn for_loops_have_the_types_of_the_dart_analyzer() {
    check_types(
        include_str!("c7_fixtures/for_loops.dart"),
        &[
            ("field;", 0, NodeKind::SimpleIdentifier, "int"),
            ("['a']", 0, NodeKind::ListLiteral, "List<String>"),
            ("x1;", 0, NodeKind::SimpleIdentifier, "String"),
            ("x2;", 0, NodeKind::SimpleIdentifier, "int"),
            ("x3;", 0, NodeKind::SimpleIdentifier, "String"),
            ("x4;", 0, NodeKind::SimpleIdentifier, "num"),
            ("x5;", 0, NodeKind::SimpleIdentifier, "dynamic"),
            (" m.", 1, NodeKind::SimpleIdentifier, "Map<int, String>"),
            (" []", 1, NodeKind::ListLiteral, "List<dynamic>"),
            ("x7;", 0, NodeKind::SimpleIdentifier, "dynamic"),
            ("x8;", 0, NodeKind::SimpleIdentifier, "double"),
            ("  y;", 2, NodeKind::SimpleIdentifier, "int"),
            ("top;", 0, NodeKind::SimpleIdentifier, "int"),
            (" i;", 1, NodeKind::SimpleIdentifier, "int"),
            (" j;", 1, NodeKind::SimpleIdentifier, "double"),
            ("  k;", 2, NodeKind::SimpleIdentifier, "int"),
            ("a1;", 0, NodeKind::SimpleIdentifier, "int"),
            ("b1;", 0, NodeKind::SimpleIdentifier, "String"),
            (
                "[(1, b: 's')]",
                0,
                NodeKind::ListLiteral,
                "List<(int, {String b})>",
            ),
            (
                "(1, b: 's')",
                0,
                NodeKind::RecordLiteral,
                "(int, {String b})",
            ),
            ("a2;", 0, NodeKind::SimpleIdentifier, "int"),
            ("b2;", 0, NodeKind::SimpleIdentifier, "String"),
            ("(0, 1)", 0, NodeKind::RecordLiteral, "(int, int)"),
            ("< j3;", 2, NodeKind::SimpleIdentifier, "int"),
            ("i3;", 0, NodeKind::SimpleIdentifier, "int"),
            ("  j3;", 2, NodeKind::SimpleIdentifier, "int"),
            ("x9;", 0, NodeKind::SimpleIdentifier, "int"),
            (
                "[for (var x in li) for (var y in is_) (x, y)]",
                0,
                NodeKind::ListLiteral,
                "List<(int, String)>",
            ),
            ("(x, y)", 0, NodeKind::RecordLiteral, "(int, String)"),
            (
                "{for (var x in li) x}",
                0,
                NodeKind::SetOrMapLiteral,
                "Set<int>",
            ),
            (
                "{for (var x in li) x: '$x'}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<int, String>",
            ),
            ("li.", 0, NodeKind::SimpleIdentifier, "List<int>"),
        ],
    );
}

#[test]
fn list_literals_have_the_types_of_the_dart_analyzer() {
    check_types(
        include_str!("c7_fixtures/list_literals.dart"),
        &[
            ("1 = []", 4, NodeKind::ListLiteral, "List<dynamic>"),
            ("[1, 2]", 0, NodeKind::ListLiteral, "List<int>"),
            ("[1, 2.0]", 0, NodeKind::ListLiteral, "List<num>"),
            ("[a, s]", 0, NodeKind::ListLiteral, "List<Object>"),
            ("<num>[1]", 0, NodeKind::ListLiteral, "List<num>"),
            ("<dynamic>[a]", 0, NodeKind::ListLiteral, "List<dynamic>"),
            ("= [1]", 2, NodeKind::ListLiteral, "List<num>"),
            ("8 = []", 4, NodeKind::ListLiteral, "List<int?>"),
            ("[...li]", 0, NodeKind::ListLiteral, "List<int>"),
            ("[...?lni]", 0, NodeKind::ListLiteral, "List<int>"),
            ("[...inum, 1]", 0, NodeKind::ListLiteral, "List<num>"),
            (
                "[if (a > 0) 1 else 's']",
                0,
                NodeKind::ListLiteral,
                "List<Object>",
            ),
            ("[if (a > 0) 1]", 0, NodeKind::ListLiteral, "List<int>"),
            (
                "[for (var i = 0; i < 3; i++) i]",
                0,
                NodeKind::ListLiteral,
                "List<int>",
            ),
            ("[?b]", 0, NodeKind::ListLiteral, "List<int>"),
            ("[...d]", 0, NodeKind::ListLiteral, "List<dynamic>"),
            ("[...nul]", 0, NodeKind::ListLiteral, "List<dynamic>"),
            ("[...?nul]", 0, NodeKind::ListLiteral, "List<Never>"),
            (
                "[for (final (x, y) in [(1, 's')]) y]",
                0,
                NodeKind::ListLiteral,
                "List<String>",
            ),
            (
                "[(1, 's')]",
                0,
                NodeKind::ListLiteral,
                "List<(int, String)>",
            ),
            ("(1, 's')", 0, NodeKind::RecordLiteral, "(int, String)"),
            (
                "[if (b case var c?) c]",
                0,
                NodeKind::ListLiteral,
                "List<int>",
            ),
            (
                "[if (b case int c when c > 0) c else 'x']",
                0,
                NodeKind::ListLiteral,
                "List<Object>",
            ),
            ("[...li, 's']", 0, NodeKind::ListLiteral, "List<Object>"),
            ("4 = []", 4, NodeKind::ListLiteral, "List<int>"),
            ("const [1, 'a']", 0, NodeKind::ListLiteral, "List<Object>"),
            ("[[], [1]]", 0, NodeKind::ListLiteral, "List<List<dynamic>>"),
            ("[[]", 1, NodeKind::ListLiteral, "List<dynamic>"),
            (", [1]", 2, NodeKind::ListLiteral, "List<int>"),
            (
                "<int>[...li, if (a > 1) a]",
                0,
                NodeKind::ListLiteral,
                "List<int>",
            ),
            ("[null, 1]", 0, NodeKind::ListLiteral, "List<int?>"),
            (" [a]", 1, NodeKind::ListLiteral, "List<int>"),
            ("[for (; a < 0;) a]", 0, NodeKind::ListLiteral, "List<int>"),
        ],
    );
}

#[test]
fn record_literals_have_the_types_of_the_dart_analyzer() {
    check_types(
        include_str!("c7_fixtures/record_literals.dart"),
        &[
            ("1 = (1, 2)", 4, NodeKind::RecordLiteral, "(int, int)"),
            (
                "(a: 1, b: 's')",
                0,
                NodeKind::RecordLiteral,
                "({int a, String b})",
            ),
            (
                "(1, b: s, 2.0)",
                0,
                NodeKind::RecordLiteral,
                "(int, double, {String b})",
            ),
            ("(1, 's')", 0, NodeKind::RecordLiteral, "(int, String)"),
            (
                "(a: 1, b: 2)",
                0,
                NodeKind::RecordLiteral,
                "({int a, int b})",
            ),
            ("(d, d)", 0, NodeKind::RecordLiteral, "(int, String)"),
            ("(x: d)", 0, NodeKind::RecordLiteral, "({int x})"),
            ("(1, z: 2)", 0, NodeKind::RecordLiteral, "(int, {int z})"),
            ("(d,)", 0, NodeKind::RecordLiteral, "(int,)"),
            ("(d, 1)", 0, NodeKind::RecordLiteral, "(dynamic, int)"),
            (
                "(a, (s, [1]))",
                0,
                NodeKind::RecordLiteral,
                "(int, (String, List<int>))",
            ),
            (
                "(s, [1])",
                0,
                NodeKind::RecordLiteral,
                "(String, List<int>)",
            ),
            ("[1]", 0, NodeKind::ListLiteral, "List<int>"),
            (
                "([], {})",
                0,
                NodeKind::RecordLiteral,
                "(List<num>, Set<int>)",
            ),
            ("([]", 1, NodeKind::ListLiteral, "List<num>"),
            (" {}", 1, NodeKind::SetOrMapLiteral, "Set<int>"),
            (
                "const (1, x: 'a')",
                0,
                NodeKind::RecordLiteral,
                "(int, {String x})",
            ),
            ("4 = (1, 2)", 4, NodeKind::RecordLiteral, "(int, int)"),
        ],
    );
}

#[test]
fn set_or_map_literals_have_the_types_of_the_dart_analyzer() {
    check_types(
        include_str!("c7_fixtures/set_or_map_literals.dart"),
        &[
            (
                "m1 = {}",
                5,
                NodeKind::SetOrMapLiteral,
                "Map<dynamic, dynamic>",
            ),
            ("{1: 's'}", 0, NodeKind::SetOrMapLiteral, "Map<int, String>"),
            (
                "{a: s, 1.0: 2}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<num, Object>",
            ),
            (" {1}", 1, NodeKind::SetOrMapLiteral, "Set<int>"),
            ("{1, 's'}", 0, NodeKind::SetOrMapLiteral, "Set<Object>"),
            ("<int>{}", 0, NodeKind::SetOrMapLiteral, "Set<int>"),
            (
                "<int, String>{}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<int, String>",
            ),
            ("4 = {}", 4, NodeKind::SetOrMapLiteral, "Set<num>"),
            ("m5 = {}", 5, NodeKind::SetOrMapLiteral, "Map<int, num>"),
            ("s5 = {}", 5, NodeKind::SetOrMapLiteral, "Set<int>"),
            ("{...m}", 0, NodeKind::SetOrMapLiteral, "Map<int, String>"),
            ("{...?mn}", 0, NodeKind::SetOrMapLiteral, "Map<int, String>"),
            ("{...si}", 0, NodeKind::SetOrMapLiteral, "Set<int>"),
            ("{...li, 1}", 0, NodeKind::SetOrMapLiteral, "Set<int>"),
            ("{...d}", 0, NodeKind::SetOrMapLiteral, "dynamic"),
            (
                "{...d, 1: 2}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<dynamic, dynamic>",
            ),
            ("{...d, 1}", 0, NodeKind::SetOrMapLiteral, "Set<dynamic>"),
            (
                "{if (a > 0) 1: 's' else 2: 3}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<int, Object>",
            ),
            (
                "{if (a > 0) 1 else 's'}",
                0,
                NodeKind::SetOrMapLiteral,
                "Set<Object>",
            ),
            (
                "{for (var i = 0; i < 2; i++) i: i}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<int, int>",
            ),
            (
                "{for (var x in li) x}",
                0,
                NodeKind::SetOrMapLiteral,
                "Set<int>",
            ),
            ("{?b: 1}", 0, NodeKind::SetOrMapLiteral, "Map<int, int>"),
            ("{1: ?b}", 0, NodeKind::SetOrMapLiteral, "Map<int, int>"),
            ("{?b}", 0, NodeKind::SetOrMapLiteral, "Set<int>"),
            ("2 = {}", 4, NodeKind::SetOrMapLiteral, "Set<int>"),
            ("3 = {}", 4, NodeKind::SetOrMapLiteral, "Map<int, int>"),
            (
                "o1 = {}",
                5,
                NodeKind::SetOrMapLiteral,
                "Map<dynamic, dynamic>",
            ),
            (
                "d1 = {}",
                5,
                NodeKind::SetOrMapLiteral,
                "Map<dynamic, dynamic>",
            ),
            (
                "const {1: 'a'}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<int, String>",
            ),
            ("const {1, 2}", 0, NodeKind::SetOrMapLiteral, "Set<int>"),
            (
                "{...m, if (a > 0) ...m}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<int, String>",
            ),
            ("{...n}", 0, NodeKind::SetOrMapLiteral, "dynamic"),
            (
                "{...m, ...d}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<dynamic, dynamic>",
            ),
            (
                "<int, String, bool>{}",
                0,
                NodeKind::SetOrMapLiteral,
                "Map<dynamic, dynamic>",
            ),
            (
                "<int, String, bool>{1}",
                0,
                NodeKind::SetOrMapLiteral,
                "Set<int>",
            ),
        ],
    );
}

/// The diagnostics of the C7 resolvers, with the codes and offsets of the
/// Dart analyzer on the same fixture.
#[test]
fn literal_and_for_loop_diagnostics_are_the_diagnostics_of_the_dart_analyzer() {
    let Some(a) = run(include_str!("c7_fixtures/diagnostics.dart")) else {
        return;
    };
    let own = [
        "ambiguous_set_or_map_literal_either",
        "ambiguous_set_or_map_literal_both",
        "invalid_field_name",
        "duplicate_field_name",
        "use_of_void_result",
        "unchecked_use_of_nullable_value",
    ];
    let actual: Vec<String> = a
        .diagnostic_names()
        .into_iter()
        .filter(|d| own.iter().any(|c| d.starts_with(&format!("{c}@"))))
        .collect();
    let expected = [
        "ambiguous_set_or_map_literal_either@240",
        "ambiguous_set_or_map_literal_either@257",
        "invalid_field_name@275",
        "duplicate_field_name@299",
        "invalid_field_name@319",
        "invalid_field_name@358",
        "invalid_field_name@371",
        "use_of_void_result@395",
        "use_of_void_result@398",
        "unchecked_use_of_nullable_value@418",
        "unchecked_use_of_nullable_value@436",
        "unchecked_use_of_nullable_value@477",
        "unchecked_use_of_nullable_value@524",
        "unchecked_use_of_nullable_value@569",
        "unchecked_use_of_nullable_value@617",
    ];
    pretty_assertions::assert_eq!(actual, expected);
}
