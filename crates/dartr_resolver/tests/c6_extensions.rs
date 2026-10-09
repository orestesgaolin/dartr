//! Integration coverage for extension applicability, member substitution,
//! most-specific selection, ambiguity, and explicit overrides.

mod support;

use dartr_ast::NodeKind;
use dartr_element::{ElemRef, FeatureSet, NoopSink};
use dartr_resolver::extension_member_resolver::{find_extension, get_override_member};
use dartr_resolver::options::AnalysisOptions;
use dartr_resolver::resolver::{ResolverVisitor, UnitContext};
use dartr_resolver::scope::LibraryScopes;
use dartr_typesystem::inheritance_manager3::Name;
use dartr_typesystem::member;

use support::{Analyzed, analyze};

fn run(source: &str) -> Option<Analyzed> {
    run_files(&[("main.dart", source)])
}

fn run_files(files: &[(&str, &str)]) -> Option<Analyzed> {
    let analyzed = analyze(files);
    if analyzed.is_none() {
        eprintln!("skipped: no Dart SDK on PATH");
    }
    analyzed
}

#[test]
fn honors_import_combinators_private_names_and_unnamed_extensions() {
    let main = r#"
import 'remote.dart' show Remote, PublicExt;
import 'other.dart' hide HiddenOther;

extension on Remote { double get localValue => 0; }

void f(Remote remote, Other other) {
  remote.visible;
  remote.hidden;
  remote.remoteUnnamed;
  remote._private;
  remote.localValue;
  other.available;
  other.secret;
}
"#;
    let remote = r#"
class Remote {}
extension PublicExt on Remote {
  int get visible => 0;
  int get _private => 0;
}
extension HiddenExt on Remote { String get hidden => ''; }
extension on Remote { bool get remoteUnnamed => false; }
"#;
    let other = r#"
class Other {}
extension PublicOther on Other { bool get available => false; }
extension HiddenOther on Other { int get secret => 0; }
"#;
    let Some(mut analyzed) = run_files(&[
        ("main.dart", main),
        ("remote.dart", remote),
        ("other.dart", other),
    ]) else {
        return;
    };
    let remote_node = analyzed.node_at(NodeKind::SimpleIdentifier, "remote.visible", 0, 0);
    let remote_type = *analyzed
        .unit()
        .tables
        .static_type
        .get(remote_node)
        .expect("Remote receiver type");
    let other_node = analyzed.node_at(NodeKind::SimpleIdentifier, "other.available", 0, 0);
    let other_type = *analyzed
        .unit()
        .tables
        .static_type
        .get(other_node)
        .expect("Other receiver type");

    with_resolver(&mut analyzed, |rv| {
        let find = |rv: &mut ResolverVisitor<'_>, ty, entity, text| {
            let name = Name::for_library(&rv.ctx, Some(rv.unit.library), text);
            find_extension(rv, ty, entity, &name)
        };

        let visible = find(rv, remote_type, remote_node, "visible");
        assert_eq!(
            member_return_type(rv, visible.getter.expect("shown extension")),
            "int"
        );
        assert!(
            find(rv, remote_type, remote_node, "hidden")
                .getter
                .is_none()
        );
        assert!(
            find(rv, remote_type, remote_node, "remoteUnnamed")
                .getter
                .is_none()
        );
        assert!(
            find(rv, remote_type, remote_node, "_private")
                .getter
                .is_none()
        );
        let local = find(rv, remote_type, remote_node, "localValue");
        assert_eq!(
            member_return_type(rv, local.getter.expect("local unnamed extension")),
            "double"
        );

        let available = find(rv, other_type, other_node, "available");
        assert_eq!(
            member_return_type(rv, available.getter.expect("extension not hidden")),
            "bool"
        );
        assert!(find(rv, other_type, other_node, "secret").getter.is_none());
    });
}

fn with_resolver<R>(analyzed: &mut Analyzed, f: impl FnOnce(&mut ResolverVisitor<'_>) -> R) -> R {
    let library = analyzed.library.library;
    let fragment = analyzed.library.units[0].fragment;
    let features = FeatureSet::default();
    let sink = NoopSink;
    let scopes = {
        let ctx = dartr_element::Ctx {
            world: &analyzed.driver.state.world,
            current: None,
            local: None,
            tp: &analyzed.tp,
            features: &features,
            req: &sink,
        };
        LibraryScopes::build(&ctx, library)
    };
    let unit = &mut analyzed.library.units[0];
    let ctx = dartr_element::Ctx {
        world: &analyzed.driver.state.world,
        current: None,
        local: Some(&unit.local),
        tp: &analyzed.tp,
        features: &features,
        req: &sink,
    };
    let unit_context = UnitContext {
        library,
        fragment,
        scopes: &scopes,
        options: AnalysisOptions::default(),
        features: Default::default(),
    };
    let mut resolver = ResolverVisitor::new(
        ctx,
        &mut unit.ast,
        &mut unit.tables,
        &mut unit.rt,
        &mut unit.diagnostics,
        unit_context,
    );
    f(&mut resolver)
}

fn member_return_type(rv: &ResolverVisitor<'_>, element: ElemRef) -> String {
    let ty = member::return_type(&rv.ctx, element);
    dartr_element::type_display_string_with(&rv.ctx, ty, Default::default())
}

fn resolved_member(analyzed: &Analyzed, node: dartr_ast::NodeId) -> ElemRef {
    let element = *analyzed
        .unit()
        .tables
        .element
        .get(node)
        .expect("resolved element");
    assert!(
        matches!(element, ElemRef::Member(_)),
        "extension member must retain its substitution"
    );
    element
}

fn analyzed_member_return_type(analyzed: &Analyzed, element: ElemRef) -> String {
    let ctx = analyzed.ctx(analyzed.unit());
    dartr_element::type_display_string_with(
        &ctx,
        member::return_type(&ctx, element),
        Default::default(),
    )
}

#[test]
fn generic_extension_substitutes_accessors_and_operators() {
    let source = r#"
class Box<T> {}

extension E<T> on Box<T> {
  T get value => throw 0;
  set value(T value) {}
  T operator [](int index) => throw 0;
  void operator []=(int index, T value) {}
}

void f(Box<int> box) {
  box.value;
  box[0];
}
"#;
    let Some(mut analyzed) = run(source) else {
        return;
    };
    let box_node = analyzed.node_at(NodeKind::SimpleIdentifier, "box.value", 0, 0);
    let receiver_type = *analyzed
        .unit()
        .tables
        .static_type
        .get(box_node)
        .expect("receiver type");

    with_resolver(&mut analyzed, |rv| {
        let value_name = Name::for_library(&rv.ctx, Some(rv.unit.library), "value");
        let value = find_extension(rv, receiver_type, box_node, &value_name);
        assert!(!value.is_ambiguous);
        assert_eq!(
            member_return_type(rv, value.getter.expect("value getter")),
            "int"
        );
        let setter = value.setter.expect("value setter");
        let setter_parameters = member::formal_parameters(&rv.ctx, setter);
        assert_eq!(setter_parameters.len(), 1);
        assert_eq!(
            dartr_element::type_display_string_with(
                &rv.ctx,
                member::type_(&rv.ctx, setter_parameters[0]),
                Default::default(),
            ),
            "int"
        );

        let index_name = Name::for_library(&rv.ctx, Some(rv.unit.library), "[]");
        let index = find_extension(rv, receiver_type, box_node, &index_name);
        assert_eq!(
            member_return_type(rv, index.getter.expect("[] operator")),
            "int"
        );
        assert!(index.setter.is_some(), "[]= operator");
    });
}

#[test]
fn chooses_most_specific_extension_and_reports_ambiguity() {
    let source = r#"
class A {}
class B extends A {}

extension OnA on A { int get pick => 0; }
extension OnB on B { String get pick => ''; }
extension First on B { int get clash => 0; }
extension Second on B { int get clash => 0; }

void f(B b) {
  b.pick;
  b.clash;
}
"#;
    let Some(mut analyzed) = run(source) else {
        return;
    };
    let receiver = analyzed.node_at(NodeKind::SimpleIdentifier, "b.pick", 0, 0);
    let receiver_type = *analyzed
        .unit()
        .tables
        .static_type
        .get(receiver)
        .expect("receiver type");
    let clash = analyzed.node_at(NodeKind::SimpleIdentifier, "b.clash", 0, 2);

    with_resolver(&mut analyzed, |rv| {
        let pick_name = Name::for_library(&rv.ctx, Some(rv.unit.library), "pick");
        let pick = find_extension(rv, receiver_type, receiver, &pick_name);
        assert_eq!(
            member_return_type(rv, pick.getter.expect("pick getter")),
            "String"
        );

        let clash_name = Name::for_library(&rv.ctx, Some(rv.unit.library), "clash");
        let clash_result = find_extension(rv, receiver_type, clash, &clash_name);
        assert!(clash_result.is_ambiguous);
        assert!(rv.diagnostics.iter().any(|d| {
            d.code.name == "ambiguous_extension_member_access"
                && d.offset == rv.ast.offset(clash) as usize
        }));
    });
}

#[test]
fn generic_override_records_inferred_types_and_substitutes_members() {
    let source = r#"
class Box<T> {}
extension E<T> on Box<T> { T get value => throw 0; }

void f(Box<int> box) {
  E(box).value;
}
"#;
    let Some(mut analyzed) = run(source) else {
        return;
    };
    let override_node = analyzed.node_at(NodeKind::ExtensionOverride, "E(box)", 0, 0);
    let type_arguments = *analyzed
        .unit()
        .tables
        .type_arg_types
        .get(override_node)
        .expect("override type arguments");
    let extended_type = *analyzed
        .unit()
        .tables
        .extended_type
        .get(override_node)
        .expect("override extended type");
    let ctx = analyzed.ctx(analyzed.unit());
    assert_eq!(ctx.list(type_arguments).len(), 1);
    assert_eq!(
        dartr_element::type_display_string_with(
            &ctx,
            ctx.list(type_arguments)[0],
            Default::default(),
        ),
        "int"
    );
    assert_eq!(analyzed.type_str(extended_type), "Box<int>");

    with_resolver(&mut analyzed, |rv| {
        let member = get_override_member(rv, dartr_ast::Id::from_raw(override_node), "value");
        assert_eq!(
            member_return_type(rv, member.getter.expect("override getter")),
            "int"
        );
    });
}

#[test]
fn driver_resolves_implicit_generic_extension_method_call() {
    let source = r#"
class Box<T> {}
extension E<T> on Box<T> {
  T identity(T value) => value;
}

void f(Box<int> box) {
  int result = box.identity(1);
}
"#;
    let Some(analyzed) = run(source) else {
        return;
    };
    let invocation = analyzed.node_at(NodeKind::MethodInvocation, "box.identity(1)", 0, 0);
    let method_name = analyzed.node_at(NodeKind::SimpleIdentifier, "box.identity(1)", 0, 4);
    let element = resolved_member(&analyzed, method_name);

    assert_eq!(
        analyzed.element_name(analyzed.element_of(method_name).expect("base element")),
        Some("identity".to_string())
    );
    assert_eq!(analyzed_member_return_type(&analyzed, element), "int");
    assert_eq!(
        analyzed.type_str(
            *analyzed
                .unit()
                .tables
                .static_type
                .get(invocation)
                .expect("invocation static type")
        ),
        "int"
    );
    assert_eq!(
        analyzed.type_str(
            *analyzed
                .unit()
                .tables
                .invoke_type
                .get(invocation)
                .expect("invoke type")
        ),
        "int Function(int)"
    );
    assert!(
        !analyzed
            .diagnostic_names()
            .iter()
            .any(|name| name.starts_with("undefined_method@"))
    );
}

#[test]
fn driver_resolves_explicit_generic_extension_override_method_call() {
    let source = r#"
extension E<T> on T {
  T method(T value) => value;
}

void f() {
  int result = E<int>(1).method(2);
}
"#;
    let Some(analyzed) = run(source) else {
        return;
    };
    let override_node = analyzed.node_at(NodeKind::ExtensionOverride, "E<int>(1)", 0, 0);
    let invocation = analyzed.node_at(NodeKind::MethodInvocation, "E<int>(1).method(2)", 0, 0);
    let method_name = analyzed.node_at(NodeKind::SimpleIdentifier, "E<int>(1).method(2)", 0, 10);
    let element = resolved_member(&analyzed, method_name);
    let type_arguments = *analyzed
        .unit()
        .tables
        .type_arg_types
        .get(override_node)
        .expect("override type arguments");
    let ctx = analyzed.ctx(analyzed.unit());

    assert_eq!(ctx.list(type_arguments).len(), 1);
    assert_eq!(
        dartr_element::type_display_string_with(
            &ctx,
            ctx.list(type_arguments)[0],
            Default::default(),
        ),
        "int"
    );
    assert_eq!(
        analyzed.type_str(
            *analyzed
                .unit()
                .tables
                .extended_type
                .get(override_node)
                .expect("override extended type")
        ),
        "int"
    );
    assert_eq!(analyzed_member_return_type(&analyzed, element), "int");
    assert_eq!(
        analyzed.type_str(
            *analyzed
                .unit()
                .tables
                .static_type
                .get(invocation)
                .expect("invocation static type")
        ),
        "int"
    );
    assert_eq!(
        analyzed.type_str(
            *analyzed
                .unit()
                .tables
                .invoke_type
                .get(invocation)
                .expect("invoke type")
        ),
        "int Function(int)"
    );
    assert!(
        !analyzed
            .diagnostic_names()
            .iter()
            .any(|name| name.starts_with("undefined_extension_method@"))
    );
}

#[test]
fn driver_reports_ambiguous_extension_method_call() {
    let source = r#"
class A {}
extension First on A { int collide() => 1; }
extension Second on A { int collide() => 2; }

void f(A a) {
  a.collide();
}
"#;
    let Some(analyzed) = run(source) else {
        return;
    };
    let method_name = analyzed.node_at(NodeKind::SimpleIdentifier, "a.collide()", 0, 2);
    let offset = analyzed.unit().ast.offset(method_name) as usize;

    assert!(analyzed.unit().tables.element.get(method_name).is_none());
    assert!(analyzed.unit().diagnostics.iter().any(|diagnostic| {
        diagnostic.code.name == "ambiguous_extension_member_access" && diagnostic.offset == offset
    }));
}
