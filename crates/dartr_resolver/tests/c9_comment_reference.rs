//! Integration coverage for documentation-comment reference resolution.

mod support;

use dartr_ast::NodeKind;
use dartr_element::{ElemRef, Tag};

use support::{Analyzed, analyze};

fn run(files: &[(&str, &str)]) -> Option<Analyzed> {
    let analyzed = analyze(files);
    if analyzed.is_none() {
        eprintln!("skipped: no Dart SDK on PATH");
    }
    analyzed
}

#[test]
fn resolves_simple_and_interface_member_references() {
    let source = r#"
void top() {}

class C {
  C();
  C.named();
  void member() {}
}

class NoDefault {
  NoDefault.named();
}

class Box<T> {
  T get value => throw 0;
}

class StringBox extends Box<String> {}

/// [top]
/// [new C]
/// [new top]
/// [new NoDefault]
/// [C.named]
/// [C.member]
/// [StringBox.value]
void documented() {}
"#;
    let Some(analyzed) = run(&[("main.dart", source)]) else {
        return;
    };

    let top = analyzed.node_at(NodeKind::SimpleIdentifier, "[top]", 0, 1);
    assert_eq!(
        analyzed.element_name(analyzed.element_of(top).expect("top element")),
        Some("top".to_string())
    );

    let constructor = analyzed.node_at(NodeKind::SimpleIdentifier, "[new C]", 0, 5);
    assert_eq!(
        analyzed
            .element_of(constructor)
            .map(|element| element.tag()),
        Some(Tag::Constructor)
    );

    let new_function = analyzed.node_at(NodeKind::SimpleIdentifier, "[new top]", 0, 5);
    assert_eq!(
        analyzed.element_name(analyzed.element_of(new_function).expect("top element")),
        Some("top".to_string())
    );
    let no_default = analyzed.node_at(NodeKind::SimpleIdentifier, "[new NoDefault]", 0, 5);
    assert_eq!(
        analyzed.element_of(no_default).map(|element| element.tag()),
        Some(Tag::Class)
    );

    let named = analyzed.node_at(NodeKind::SimpleIdentifier, "[C.named]", 0, 3);
    assert_eq!(
        analyzed.element_of(named).map(|element| element.tag()),
        Some(Tag::Constructor)
    );
    assert_eq!(
        analyzed.element_name(analyzed.element_of(named).expect("named constructor")),
        Some("named".to_string())
    );

    let member = analyzed.node_at(NodeKind::SimpleIdentifier, "[C.member]", 0, 3);
    assert_eq!(
        analyzed.element_name(analyzed.element_of(member).expect("member element")),
        Some("member".to_string())
    );

    let inherited = analyzed.node_at(NodeKind::SimpleIdentifier, "[StringBox.value]", 0, 11);
    assert!(matches!(
        analyzed.unit().tables.element.get(inherited),
        Some(ElemRef::Member(_))
    ));
}

#[test]
fn resolves_import_prefix_and_prefixed_type_member_references() {
    let main = r#"
import 'remote.dart' as p;

/// [p.Remote]
/// [p.Remote.member]
/// [p.Derived.member]
void documented() {}
"#;
    let remote = r#"
class Remote {
  void member() {}
}
class Derived extends Remote {}
"#;
    let Some(analyzed) = run(&[("main.dart", main), ("remote.dart", remote)]) else {
        return;
    };

    let prefix = analyzed.node_at(NodeKind::SimpleIdentifier, "[p.Remote]", 0, 1);
    assert_eq!(
        analyzed.element_of(prefix).map(|element| element.tag()),
        Some(Tag::Prefix)
    );
    let remote_type = analyzed.node_at(NodeKind::SimpleIdentifier, "[p.Remote]", 0, 3);
    assert_eq!(
        analyzed.element_name(analyzed.element_of(remote_type).expect("Remote element")),
        Some("Remote".to_string())
    );

    let property = analyzed.node_at(NodeKind::SimpleIdentifier, "[p.Remote.member]", 0, 10);
    assert_eq!(
        analyzed.element_name(analyzed.element_of(property).expect("member element")),
        Some("member".to_string())
    );

    let inherited_property =
        analyzed.node_at(NodeKind::SimpleIdentifier, "[p.Derived.member]", 0, 11);
    assert_eq!(analyzed.element_of(inherited_property), None);
}
