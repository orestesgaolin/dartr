// Dart source: pkg/analyzer/test/src/dart/element/function_type_test.dart

//! Port of `function_type_test.dart`.
//!
//! Dart `expect(f1, f2)` / `equals` on types is Dart `==`, which is
//! `TypeSystem::dart_eq` here. Dart `same` is `TypeId ==`.
//!
//! The `test_hash_*` tests check `FunctionTypeImpl.hashCode`. The Rust port
//! has no Dart `hashCode`: types are hashed by their `TypeId` (identity), and
//! no map is keyed by Dart `==`. The helpers below use the hash of the
//! `TypeId`. "Sometimes differ" holds for it (different structures have
//! different ids); "always equal" cannot hold, because the Dart hash ignores
//! properties (positional parameter names and types, type formal names) that
//! are part of the identity. These tests are `#[ignore]`d.

use std::hash::{DefaultHasher, Hash, Hasher};

use dartr_element::{FnParam, TypeId, TypeKind};
use dartr_typesystem::TypeExt;
use dartr_typesystem::test_support::*;
use dartr_typesystem::type_ext::{is_named, is_optional_positional, is_required_positional};
use indexmap::IndexMap;

/// `FunctionTypeTest`: the test world and `_classTypes`.
struct FunctionTypeTest {
    t: TypeSystemTest,
    class_types: IndexMap<String, TypeId>,
}

impl FunctionTypeTest {
    fn new() -> FunctionTypeTest {
        FunctionTypeTest {
            t: TypeSystemTest::new(),
            class_types: IndexMap::new(),
        }
    }

    fn parse_function_type(&self, input: &str) -> TypeId {
        self.t.parse_function_type(input)
    }

    /// `_classType(name)`.
    fn class_type(&mut self, name: &str) -> TypeId {
        if let Some(&t) = self.class_types.get(name) {
            return t;
        }
        self.t.build_test_library(LibrarySpec {
            classes: vec![ClassSpec::new(&format!("class {name}"))],
            ..LibrarySpec::test()
        });
        let element = self.t.class_element(name);
        let this_type = self.t.ctx().interface_this_type(element.upcast());
        self.class_types.insert(name.to_string(), this_type);
        this_type
    }

    /// `_typeStr(type)`.
    fn type_str(&self, t: TypeId) -> String {
        self.t.display(t)
    }

    fn params(&self, f: TypeId) -> &[FnParam] {
        let ctx = self.t.ctx();
        let TypeKind::Function(data) = *ctx.ty(f) else {
            panic!("not a function type");
        };
        ctx.list(data.params)
    }

    fn param_name(&self, p: &FnParam) -> String {
        p.name
            .map(|n| self.t.ctx().name_str(n).to_string())
            .unwrap_or_default()
    }

    /// `basicChecks(f, ...)`.
    fn basic_checks(&self, f: TypeId, c: Checks) {
        let ctx = self.t.ctx();
        let TypeKind::Function(data) = *ctx.ty(f) else {
            panic!("not a function type");
        };
        let params = self.params(f);
        // DartType properties
        assert_eq!(self.t.display(f), c.display_name, "displayName");
        // FunctionType properties
        let named_parameter_types: Vec<(String, String)> = params
            .iter()
            .filter(|p| is_named(p.kind))
            .map(|p| (self.param_name(p), self.type_str(p.ty)))
            .collect();
        let expected: Vec<(String, String)> = c
            .named_parameter_types
            .iter()
            .map(|(n, t)| (n.to_string(), t.to_string()))
            .collect();
        assert_eq!(named_parameter_types, expected, "namedParameterTypes");
        let names = |filter: fn(dartr_element::ParameterKind) -> bool| -> Vec<String> {
            params
                .iter()
                .filter(|p| filter(p.kind))
                .map(|p| self.param_name(p))
                .collect()
        };
        let types = |filter: fn(dartr_element::ParameterKind) -> bool| -> Vec<String> {
            params
                .iter()
                .filter(|p| filter(p.kind))
                .map(|p| self.type_str(p.ty))
                .collect()
        };
        assert_eq!(
            names(is_required_positional),
            c.normal_parameter_names,
            "normalParameterNames"
        );
        assert_eq!(
            types(is_required_positional),
            c.normal_parameter_types,
            "normalParameterTypes"
        );
        assert_eq!(
            names(is_optional_positional),
            c.optional_parameter_names,
            "optionalParameterNames"
        );
        assert_eq!(
            types(is_optional_positional),
            c.optional_parameter_types,
            "optionalParameterTypes"
        );
        assert_eq!(params.len(), c.parameters, "parameters");
        assert_eq!(self.type_str(data.ret), c.return_type, "returnType");
        // Dart: `typeFormals` is `isEmpty` or a list of `same(...)` matchers.
        assert_eq!(
            ctx.list(data.type_params),
            c.type_formals.as_slice(),
            "typeFormals"
        );
    }
}

/// The named arguments of `basicChecks` with their defaults.
struct Checks {
    display_name: &'static str,
    return_type: &'static str,
    named_parameter_types: Vec<(&'static str, &'static str)>,
    normal_parameter_names: Vec<&'static str>,
    normal_parameter_types: Vec<&'static str>,
    optional_parameter_names: Vec<&'static str>,
    optional_parameter_types: Vec<&'static str>,
    /// `hasLength(n)` (`isEmpty` is 0).
    parameters: usize,
    type_formals: Vec<dartr_element::EId<dartr_element::TypeParameterElement>>,
}

impl Default for Checks {
    fn default() -> Checks {
        Checks {
            display_name: "dynamic Function()",
            return_type: "dynamic",
            named_parameter_types: Vec::new(),
            normal_parameter_names: Vec::new(),
            normal_parameter_types: Vec::new(),
            optional_parameter_names: Vec::new(),
            optional_parameter_types: Vec::new(),
            parameters: 0,
            type_formals: Vec::new(),
        }
    }
}

/// The hash of a type in Rust: the hash of its `TypeId`.
fn hash_code(t: TypeId) -> u64 {
    let mut h = DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// `_testHashesAlwaysEqual`.
fn test_hashes_always_equal(mut generate: impl FnMut(i32) -> TypeId) {
    let mut x = generate(0);
    for i in 1..10 {
        let y = generate(i);
        assert_eq!(hash_code(x), hash_code(y));
        x = y;
    }
}

/// `_testHashesSometimesDiffer`.
fn test_hashes_sometimes_differ(mut generate: impl FnMut(i32) -> TypeId) {
    let mut x = generate(0);
    for i in 1..10 {
        let y = generate(i);
        if hash_code(x) != hash_code(y) {
            return;
        }
        x = y;
    }
    panic!("Hashes never differed");
}

/// `_testHashesSometimesDifferPairwise`.
fn test_hashes_sometimes_differ_pairwise(mut generate: impl FnMut(i32) -> (TypeId, TypeId)) {
    for i in 1..10 {
        let (x, y) = generate(i);
        if hash_code(x) != hash_code(y) {
            return;
        }
    }
    panic!("Hashes never differed");
}

fn assert_not_equal(f: &FunctionTypeTest, f1: TypeId, f2: TypeId) {
    // Dart: isNot(equals(f2))
    assert!(
        !f.t.type_system().dart_eq(f1, f2),
        "{} == {}",
        f.t.display(f1),
        f.t.display(f2)
    );
}

#[test]
fn equality_left_required_right_positional() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function(int a)");
    let f2 = f.parse_function_type("void Function([int a])");
    assert_not_equal(&f, f1, f2);
}

#[test]
fn equality_named_parameters_different_name() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function({int a})");
    let f2 = f.parse_function_type("void Function({int b})");
    assert_not_equal(&f, f1, f2);
}

#[test]
fn equality_named_parameters_different_type() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function({int a})");
    let f2 = f.parse_function_type("void Function({double a})");
    assert_not_equal(&f, f1, f2);
}

#[test]
fn equality_named_parameters_equal() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function({int a, double b})");
    let f2 = f.parse_function_type("void Function({int a, double b})");
    // Dart: ==
    assert!(f.t.type_system().dart_eq(f1, f2));
}

#[test]
fn equality_named_parameters_extra_left() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function({int a, double b})");
    let f2 = f.parse_function_type("void Function({int a})");
    assert_not_equal(&f, f1, f2);
}

#[test]
fn equality_named_parameters_extra_right() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function({int a})");
    let f2 = f.parse_function_type("void Function({int a, double b})");
    assert_not_equal(&f, f1, f2);
}

#[test]
fn equality_named_parameters_required_left() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function({required int a})");
    let f2 = f.parse_function_type("void Function({int a})");
    assert_not_equal(&f, f1, f2);
}

#[test]
fn equality_named_parameters_required_right() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function({int a})");
    let f2 = f.parse_function_type("void Function({required int a})");
    assert_not_equal(&f, f1, f2);
}

#[test]
fn equality_required_parameters_extra_left() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function(int a, double b)");
    let f2 = f.parse_function_type("void Function(int a)");
    assert_not_equal(&f, f1, f2);
}

#[test]
fn equality_required_parameters_extra_right() {
    let f = FunctionTypeTest::new();
    let f1 = f.parse_function_type("void Function(int a)");
    let f2 = f.parse_function_type("void Function(int a, double b)");
    assert_not_equal(&f, f1, f2);
}

#[test]
fn hash_named_parameter_optionality() {
    let f = FunctionTypeTest::new();
    test_hashes_sometimes_differ_pairwise(|i| {
        (
            f.parse_function_type(&format!("void Function({{int p{i}}})")),
            f.parse_function_type(&format!("void Function({{required int p{i}}})")),
        )
    });
}

#[test]
fn hash_nullability_suffix() {
    let mut f = FunctionTypeTest::new();
    test_hashes_sometimes_differ_pairwise(|i| {
        f.class_type(&format!("C{i}"));
        (
            f.parse_function_type(&format!("void Function(C{i} x)")),
            f.parse_function_type(&format!("void Function(C{i} x)?")),
        )
    });
}

#[test]
fn hash_optional_named_parameter_name() {
    let f = FunctionTypeTest::new();
    test_hashes_sometimes_differ(|i| {
        f.parse_function_type(&format!("void Function({{int p{i}}})"))
    });
}

#[test]
fn hash_optional_named_parameter_type() {
    let mut f = FunctionTypeTest::new();
    test_hashes_sometimes_differ(|i| {
        f.class_type(&format!("C{i}"));
        f.parse_function_type(&format!("void Function({{C{i} x}})"))
    });
}

#[test]
#[ignore = "Dart hashCode is not ported: Rust hashes types by TypeId, which includes positional parameter names"]
fn hash_optional_positional_parameter_name() {
    let f = FunctionTypeTest::new();
    // Optional parameter names are irrelevant
    test_hashes_always_equal(|i| f.parse_function_type(&format!("void Function([int p{i}])")));
}

#[test]
#[ignore = "Dart hashCode is not ported: Rust hashes types by TypeId, which includes positional parameter types"]
fn hash_optional_positional_parameter_type() {
    let mut f = FunctionTypeTest::new();
    test_hashes_always_equal(|i| {
        f.class_type(&format!("C{i}"));
        f.parse_function_type(&format!("void Function([C{i} x])"))
    });
}

#[test]
fn hash_positional_parameter_optionality() {
    let f = FunctionTypeTest::new();
    test_hashes_sometimes_differ_pairwise(|i| {
        (
            f.parse_function_type(&format!("void Function(int p{i})")),
            f.parse_function_type(&format!("void Function([int p{i}])")),
        )
    });
}

#[test]
fn hash_required_named_parameter_name() {
    let f = FunctionTypeTest::new();
    test_hashes_sometimes_differ(|i| {
        f.parse_function_type(&format!("void Function({{required int p{i}}})"))
    });
}

#[test]
fn hash_required_named_parameter_type() {
    let mut f = FunctionTypeTest::new();
    test_hashes_sometimes_differ(|i| {
        f.class_type(&format!("C{i}"));
        f.parse_function_type(&format!("void Function({{required C{i} x}})"))
    });
}

#[test]
#[ignore = "Dart hashCode is not ported: Rust hashes types by TypeId, which includes positional parameter names"]
fn hash_required_positional_parameter_name() {
    let f = FunctionTypeTest::new();
    // Required parameter names are irrelevant
    test_hashes_always_equal(|i| f.parse_function_type(&format!("void Function(int p{i})")));
}

#[test]
#[ignore = "Dart hashCode is not ported: Rust hashes types by TypeId, which includes positional parameter types"]
fn hash_required_positional_parameter_type() {
    let mut f = FunctionTypeTest::new();
    test_hashes_always_equal(|i| {
        f.class_type(&format!("C{i}"));
        f.parse_function_type(&format!("void Function(C{i} x)"))
    });
}

#[test]
fn hash_return_type() {
    let mut f = FunctionTypeTest::new();
    test_hashes_sometimes_differ(|i| {
        f.class_type(&format!("C{i}"));
        f.parse_function_type(&format!("C{i} Function()"))
    });
}

#[test]
#[ignore = "Dart hashCode is not ported: Rust hashes types by TypeId, which includes the type formal elements"]
fn hash_type_formal_names() {
    let f = FunctionTypeTest::new();
    test_hashes_always_equal(|i| {
        f.parse_function_type(&format!("void Function<T{i}, U{i}>(T{i} x, T{i} y)"))
    });
}

#[test]
fn new_sorts_named_parameters() {
    let f = FunctionTypeTest::new();
    let ty = f.parse_function_type("void Function(int a, {int c, int b})");
    let parameters = f.params(ty);
    assert_eq!(parameters.len(), 3);
    assert_eq!(f.param_name(&parameters[0]), "a");
    assert_eq!(f.param_name(&parameters[1]), "b");
    assert_eq!(f.param_name(&parameters[2]), "c");
}

#[test]
fn synthetic() {
    let f = FunctionTypeTest::new();
    let ty = f.parse_function_type("dynamic Function()");
    f.basic_checks(ty, Checks::default());
}

#[test]
fn synthetic_instantiate() {
    let f = FunctionTypeTest::new();
    let ctx = f.t.ctx();
    // T Function<T>(T x)
    let ty = f.parse_function_type("T Function<T>(T x)");
    let instantiated = ctx.instantiate_function_type(ty, &[f.t.tp.object_type()]);
    f.basic_checks(
        instantiated,
        Checks {
            display_name: "Object Function(Object)",
            return_type: "Object",
            normal_parameter_names: vec!["x"],
            normal_parameter_types: vec!["Object"],
            parameters: 1,
            ..Checks::default()
        },
    );
}

#[test]
#[should_panic(expected = "argumentTypes.length != typeFormals.length")]
fn synthetic_instantiate_argument_length_mismatch() {
    let f = FunctionTypeTest::new();
    // dynamic Function<T>()
    let ty = f.parse_function_type("dynamic Function<T>()");
    // Dart: throwsA(TypeMatcher<ArgumentError>()); Rust panics.
    f.t.ctx().instantiate_function_type(ty, &[]);
}

#[test]
fn synthetic_instantiate_no_type_formals() {
    let f = FunctionTypeTest::new();
    let ty = f.parse_function_type("dynamic Function()");
    // Dart: same(f)
    assert_eq!(f.t.ctx().instantiate_function_type(ty, &[]), ty);
}

#[test]
fn synthetic_named_parameter() {
    let f = FunctionTypeTest::new();
    let ty = f.parse_function_type("dynamic Function({Object x})");
    f.basic_checks(
        ty,
        Checks {
            display_name: "dynamic Function({Object x})",
            named_parameter_types: vec![("x", "Object")],
            parameters: 1,
            ..Checks::default()
        },
    );
    let p = f.params(ty)[0];
    assert!(is_named(p.kind));
    assert_eq!(f.param_name(&p), "x");
    assert_eq!(f.type_str(p.ty), "Object");
}

#[test]
fn synthetic_normal_parameter() {
    let f = FunctionTypeTest::new();
    let ty = f.parse_function_type("dynamic Function(Object x)");
    f.basic_checks(
        ty,
        Checks {
            display_name: "dynamic Function(Object)",
            normal_parameter_names: vec!["x"],
            normal_parameter_types: vec!["Object"],
            parameters: 1,
            ..Checks::default()
        },
    );
    let p = f.params(ty)[0];
    assert!(is_required_positional(p.kind));
    assert_eq!(f.param_name(&p), "x");
    assert_eq!(f.type_str(p.ty), "Object");
}

#[test]
fn synthetic_optional_parameter() {
    let f = FunctionTypeTest::new();
    let ty = f.parse_function_type("dynamic Function([Object x])");
    f.basic_checks(
        ty,
        Checks {
            display_name: "dynamic Function([Object])",
            optional_parameter_names: vec!["x"],
            optional_parameter_types: vec!["Object"],
            parameters: 1,
            ..Checks::default()
        },
    );
    let p = f.params(ty)[0];
    assert!(is_optional_positional(p.kind));
    assert_eq!(f.param_name(&p), "x");
    assert_eq!(f.type_str(p.ty), "Object");
}

#[test]
fn synthetic_return_type() {
    let f = FunctionTypeTest::new();
    let ty = f.parse_function_type("Object Function()");
    f.basic_checks(
        ty,
        Checks {
            display_name: "Object Function()",
            return_type: "Object",
            ..Checks::default()
        },
    );
}

#[test]
fn synthetic_type_formals() {
    let f = FunctionTypeTest::new();
    let ctx = f.t.ctx();
    let ty = f.parse_function_type("T Function<T>()");
    let TypeKind::Function(data) = *ctx.ty(ty) else {
        panic!("not a function type");
    };
    let type_params = ctx.list(data.type_params);
    assert_eq!(type_params.len(), 1);
    let t = type_params[0];
    f.basic_checks(
        ty,
        Checks {
            display_name: "T Function<T>()",
            return_type: "T",
            type_formals: vec![t],
            ..Checks::default()
        },
    );
    let TypeKind::TypeParameter { param, .. } = *ctx.ty(data.ret) else {
        panic!("not a type parameter type");
    };
    // Dart: same(t)
    assert_eq!(param, t);
}
