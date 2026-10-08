// Dart source: pkg/_fe_analyzer_shared/test/mini_types.dart (class TypeSystem)

//! Rust-only tests of [`TypeSystem`]. `mini_types_test.dart` has no tests of
//! `TypeSystem`; in Dart it is covered only indirectly (flow analysis and
//! type inference tests). The expectations are the subtyping rules of the
//! Dart language specification.

use dartr_mini_ast::mini_types::*;

fn ty(type_str: &str) -> Type {
    Type::new(type_str)
}

fn set_up() -> RegistryGuard {
    let guard = TypeRegistry::init_for_test();
    TypeRegistry::add_type_parameter("T");
    TypeRegistry::add_interface_type_name("Function");
    TypeRegistry::add_interface_type_name("Record");
    guard
}

/// Asserts `t0 <: t1` for each `(t0, t1, expected)`.
fn check_subtypes(cases: &[(&str, &str, bool)]) {
    let ts = TypeSystem::new();
    let failures: Vec<String> = cases
        .iter()
        .filter(|(t0, t1, expected)| ts.is_subtype(ty(t0), ty(t1)) != *expected)
        .map(|(t0, t1, expected)| format!("{t0} <: {t1} should be {expected}"))
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn is_subtype_interface_types() {
    let _g = set_up();
    check_subtypes(&[
        ("int", "int", true),
        ("int", "num", true),
        ("num", "int", false),
        ("int", "Object", true),
        ("int", "String", false),
        ("List<int>", "Iterable<num>", true),
        ("List<num>", "Iterable<int>", false),
        ("List<int>", "List<num>", true),
        ("Iterable<int>", "List<int>", false),
    ]);
}

#[test]
fn is_subtype_top_and_bottom_types() {
    let _g = set_up();
    check_subtypes(&[
        ("int", "dynamic", true),
        ("int?", "void", true),
        ("int?", "Object?", true),
        ("dynamic", "Object?", true),
        ("dynamic", "Object", false),
        ("void", "int", false),
        ("Never", "int", true),
        ("Never?", "int", false),
        ("Never?", "int?", true),
        ("error", "int", true),
        ("int", "error", true),
        ("_", "int", true),
    ]);
}

#[test]
fn is_subtype_nullability() {
    let _g = set_up();
    check_subtypes(&[
        ("int", "int?", true),
        ("int?", "int", false),
        ("int?", "num?", true),
        ("Null", "int?", true),
        ("Null", "int", false),
        ("Null", "Object", false),
        ("Null", "T", false),
        ("Null", "T?", true),
        ("int?", "Object", false),
    ]);
}

#[test]
fn is_subtype_future_or() {
    let _g = set_up();
    check_subtypes(&[
        ("int", "FutureOr<int>", true),
        ("Future<int>", "FutureOr<num>", true),
        ("FutureOr<int>", "Object", true),
        ("FutureOr<int?>", "Object", false),
        ("FutureOr<int>", "FutureOr<num>", true),
        ("FutureOr<num>", "FutureOr<int>", false),
        ("Null", "FutureOr<int?>", true),
        ("Null", "FutureOr<int>", false),
        ("String", "FutureOr<int>", false),
    ]);
}

#[test]
fn is_subtype_type_parameters() {
    let _g = set_up();
    check_subtypes(&[
        ("T", "T", true),
        ("T", "Object?", true),
        ("T", "Object", false),
        ("T&int", "int", true),
        ("T&int", "T", true),
        ("T&int", "Object", true),
        ("T", "T&int", false),
        ("T&int", "T&num", true),
        ("T&num", "T&int", false),
    ]);
}

#[test]
fn is_subtype_function_types() {
    let _g = set_up();
    check_subtypes(&[
        ("int Function(num)", "num Function(int)", true),
        ("num Function(int)", "int Function(num)", false),
        ("void Function([int])", "void Function()", true),
        ("void Function()", "void Function([int])", false),
        ("void Function(int, [int])", "void Function(int)", true),
        (
            "void Function({int x, int y})",
            "void Function({int y})",
            true,
        ),
        (
            "void Function({int y})",
            "void Function({int x, int y})",
            false,
        ),
        (
            "void Function({required int x})",
            "void Function({int x})",
            true,
        ),
        // Note: the next two results differ from the language specification.
        // They are the results of the Dart `TypeSystem.isSubtype` of
        // mini_types.dart (`isNamedFunctionSubtype` returns `false` when T1
        // has no more named parameters but T0 has, and it checks `required`
        // in the opposite direction). The port keeps the Dart behavior.
        ("void Function({int x})", "void Function()", false),
        (
            "void Function({int x})",
            "void Function({required int x})",
            false,
        ),
        ("void Function()", "Function", true),
        ("void Function()", "Object", true),
    ]);
}

#[test]
fn is_subtype_record_types() {
    let _g = set_up();
    check_subtypes(&[
        ("(int, {String s})", "(num, {String s})", true),
        ("(num, {String s})", "(int, {String s})", false),
        ("(int,)", "(int, int)", false),
        ("({int a})", "({int b})", false),
        ("(int,)", "Record", true),
    ]);
}

#[test]
fn factor() {
    let _g = set_up();
    let ts = TypeSystem::new();
    let factor = |t: &str, s: &str| ts.factor(ty(t), ty(s)).to_string();
    assert_eq!(factor("int", "num"), "Never");
    // Note: mini types do not normalize `Never?` to `Null`.
    assert_eq!(factor("int?", "int"), "Never?");
    assert_eq!(factor("int?", "Null"), "int");
    assert_eq!(factor("num", "int"), "num");
    assert_eq!(factor("FutureOr<int>", "Future<int>"), "int");
    assert_eq!(factor("FutureOr<int>", "int"), "Future<int>");
}

#[test]
fn derived_future_type() {
    let _g = set_up();
    let mut ts = TypeSystem::new();
    TypeRegistry::add_interface_type_name("MyFuture");
    ts.add_super_interfaces("MyFuture", |args| {
        vec![
            PrimaryType::new(TypeRegistry::future(), args.to_vec()).into_type(),
            Type::new("Object"),
        ]
    });
    let derived = |t: &str| ts.derived_future_type(ty(t)).map(|f| f.to_string());
    assert_eq!(derived("Future<int>").as_deref(), Some("Future<int>"));
    assert_eq!(derived("MyFuture<int>").as_deref(), Some("Future<int>"));
    assert_eq!(derived("MyFuture<int>?").as_deref(), Some("Future<int>?"));
    assert_eq!(derived("FutureOr<int>").as_deref(), Some("FutureOr<int>"));
    assert_eq!(derived("int"), None);
    let t = TypeRegistry::lookup("T");
    let TypeNameInfo::TypeParameter(t) = t else {
        panic!("T is a type parameter")
    };
    t.set_explicit_bound(Some(ty("Future<String>")));
    assert_eq!(derived("T").as_deref(), Some("Future<String>"));
}

#[test]
fn shared_getters() {
    let _g = set_up();
    assert_eq!(
        ty("void Function(String, [num])").required_positional_parameter_count(),
        1
    );
    let f = ty("int Function<T>(String, num, {required bool b})");
    assert_eq!(
        f.shared_type_kind(),
        dartr_flow::shared_type::SharedTypeKind::Function
    );
    assert_eq!(f.required_positional_parameter_count(), 2);
    assert_eq!(f.positional_parameter_types_shared().len(), 2);
    let named = f.sorted_named_parameters_shared();
    assert_eq!(named.len(), 1);
    assert!(named[0].is_required);
    assert_eq!(named[0].name_shared, Name::new("b"));
    assert_eq!(f.return_type_shared(), ty("int"));
    assert_eq!(f.type_parameters_shared()[0].name(), "T");

    let r = ty("(int, {String s})");
    assert_eq!(
        r.shared_type_kind(),
        dartr_flow::shared_type::SharedTypeKind::Record
    );
    assert_eq!(r.positional_types_shared(), vec![ty("int")]);
    assert_eq!(r.sorted_named_types_shared()[0].type_shared, ty("String"));

    assert_eq!(ty("int").as_question_type(true), ty("int?"));
    assert_eq!(ty("dynamic").as_question_type(true), ty("dynamic"));
    assert_eq!(ty("Null").as_question_type(true), ty("Null"));
    assert_eq!(ty("Never").as_question_type(true).to_string(), "Never?");
}

#[test]
#[should_panic(expected = "init() already called")]
fn init_twice_panics() {
    let _g = set_up();
    TypeRegistry::init();
}

#[test]
fn guard_uninitializes_after_panic() {
    let result = std::panic::catch_unwind(|| {
        let _g = set_up();
        panic!("test failure");
    });
    assert!(result.is_err());
    assert!(!TypeRegistry::is_initialized());
    // A second init on the same thread works.
    let _g = set_up();
    assert!(TypeRegistry::is_initialized());
}
