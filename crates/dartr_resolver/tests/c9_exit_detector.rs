//! Representative ports of analyzer `exit_detector_test.dart` scenarios.

mod support;

use dartr_ast::NodeKind;
use dartr_element::Tag;

use support::{analyze, Analyzed};

fn run(source: &str) -> Option<Analyzed> {
    let analyzed = analyze(&[("main.dart", source)]);
    if analyzed.is_none() {
        eprintln!("skipped: no Dart SDK on PATH");
    }
    analyzed
}

fn exits_resolved(analyzed: &Analyzed, node: dartr_ast::NodeId) -> bool {
    let unit = analyzed.unit();
    dartr_resolver::exit_detector::exits_resolved(
        &unit.ast,
        &unit.tables,
        &unit.rt,
        analyzed.ctx(unit),
        node,
    )
}

#[test]
fn detects_returns_branches_and_simple_infinite_loops() {
    let source = r#"
void returns() {
  return;
}

void branches(bool condition) {
  if (condition) return; else throw 0;
}

void forever() {
  while (true) {}
}

void breaks() {
  while (true) { break; }
}
"#;
    let Some(analyzed) = run(source) else {
        return;
    };

    let return_statement = analyzed.node_at(NodeKind::ReturnStatement, "return;", 0, 0);
    assert!(dartr_resolver::exit_detector::exits(
        &analyzed.unit().ast,
        return_statement
    ));

    let if_statement = analyzed.node_at(NodeKind::IfStatement, "if (condition)", 0, 0);
    assert!(exits_resolved(&analyzed, if_statement));

    let infinite = analyzed.node_at(NodeKind::WhileStatement, "while (true) {}", 0, 0);
    assert!(exits_resolved(&analyzed, infinite));

    let with_break = analyzed.node_at(NodeKind::WhileStatement, "while (true) { break; }", 0, 0);
    assert!(!exits_resolved(&analyzed, with_break));
}

#[test]
fn detects_never_calls_and_short_circuit_boundaries() {
    let source = r#"
Never abort() => throw 0;

void callsNever() {
  abort();
}

void nullAware(Object? target) {
  target?.toString(throw 0);
}

void eagerArgument() {
  print(throw 0);
}
"#;
    let Some(analyzed) = run(source) else {
        return;
    };

    let method_name = analyzed.node_at(NodeKind::SimpleIdentifier, "abort();", 0, 0);
    let abort_reference = *analyzed
        .unit()
        .tables
        .element
        .get(method_name)
        .expect("resolved abort invocation");
    assert_eq!(
        analyzed
            .element_of(method_name)
            .expect("abort executable")
            .tag(),
        Tag::TopLevelFunction
    );
    assert_eq!(
        dartr_typesystem::member::return_type(&analyzed.ctx(analyzed.unit()), abort_reference,),
        dartr_element::TypeId::NEVER
    );

    let never_call = analyzed.node_at(NodeKind::ExpressionStatement, "abort();", 0, 0);
    assert!(exits_resolved(&analyzed, never_call));

    let null_aware = analyzed.node_at(NodeKind::ExpressionStatement, "target?.toString", 0, 0);
    assert!(!exits_resolved(&analyzed, null_aware));

    let eager = analyzed.node_at(NodeKind::ExpressionStatement, "print(throw 0)", 0, 0);
    assert!(exits_resolved(&analyzed, eager));
}

#[test]
fn detects_for_switch_try_and_label_control_flow() {
    let source = r#"
void foreverFor() {
  for (;;) {}
}

void exitingSwitch(int value) {
  switch (value) {
    case 0: return;
    default: throw 0;
  }
}

void finallyExits() {
  try {} finally { return; }
}

void labelledBreak() {
  outer: while (true) { break outer; }
}
"#;
    let Some(analyzed) = run(source) else {
        return;
    };

    let for_statement = analyzed.node_at(NodeKind::ForStatement, "for (;;) {}", 0, 0);
    assert!(exits_resolved(&analyzed, for_statement));

    let switch_statement = analyzed.node_at(NodeKind::SwitchStatement, "switch (value)", 0, 0);
    assert!(exits_resolved(&analyzed, switch_statement));

    let try_statement = analyzed.node_at(NodeKind::TryStatement, "try {} finally", 0, 0);
    assert!(exits_resolved(&analyzed, try_statement));

    let labelled = analyzed.node_at(NodeKind::LabeledStatement, "outer: while", 0, 0);
    assert!(!exits_resolved(&analyzed, labelled));
}
