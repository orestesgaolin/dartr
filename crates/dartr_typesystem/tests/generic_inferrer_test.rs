// Dart source: pkg/analyzer/test/src/dart/element/generic_inferrer_test.dart

//! Port of `generic_inferrer_test.dart` (`GenericFunctionInferenceTest`, 24
//! tests). The helpers `_assertType`, `_assertTypes`, `_inferCall`,
//! `_inferCall2` are the free functions below.
//!
//! The Dart `_inferCall` reports to a `RecordingDiagnosticListener` with a
//! `NullLiteral` at offset 0 as the error entity; here the error entity is an
//! [`InferenceErrorEntity`] for that literal (an `Expression` without a
//! static type).

#![allow(non_snake_case)]

use dartr_diagnostics::{DiagnosticReporter, RecordingDiagnosticListener, diag};
use dartr_element::{TypeId, TypeKind};
use dartr_typesystem::generic_inferrer::{
    InferenceErrorEntity, InferenceErrorEntityKind, InferenceFlags,
};
use dartr_typesystem::test_support::*;
use dartr_typesystem::type_algebra::MapSubstitution;
use dartr_typesystem::{TypeExt, element_type};

// ------------------------------------------------------------------ helpers

fn _assert_type(t: &TypeSystemTest, ty: TypeId, expected: &str) {
    assert_eq!(t.display(ty), expected);
}

fn _assert_types(t: &TypeSystemTest, actual: &[TypeId], expected: &[TypeId]) {
    let actual_str: Vec<String> = actual.iter().map(|&e| t.display(e)).collect();
    let expected_str: Vec<String> = expected.iter().map(|&e| t.display(e)).collect();
    assert_eq!(actual_str, expected_str);
}

/// `_inferCall(ft, arguments, returnType:, expectError:)`.
fn _infer_call_with(
    t: &TypeSystemTest,
    ft: TypeId,
    arguments: &[TypeId],
    return_type: TypeId,
    expect_error: bool,
) -> Vec<TypeId> {
    let ctx = t.ctx();
    let TypeKind::Function(f) = *ctx.ty(ft) else {
        panic!("FunctionTypeImpl expected");
    };

    let mut listener = RecordingDiagnosticListener::new();
    let type_arguments = {
        let mut reporter = DiagnosticReporter::new(&mut listener);

        // `NullLiteralImpl(literal: KeywordToken(Keyword.NULL, 0))`.
        let error_entity = InferenceErrorEntity {
            offset: 0,
            length: 4,
            is_invocation_in_as_expression: false,
            kind: InferenceErrorEntityKind::Expression { static_type: None },
        };

        let mut inferrer = t.type_system().setup_generic_type_inference(
            ctx.list(f.type_params),
            f.ret,
            return_type,
            Some(&mut reporter),
            Some(error_entity),
            InferenceFlags {
                generic_metadata_is_enabled: true,
                inference_using_bounds_is_enabled: true,
                strict_inference: false,
            },
            false,
            t.type_system_operations(),
            None,
            None,
        );
        inferrer.constrain_arguments2(ctx.list(f.params), arguments, None);
        inferrer.choose_final_types()
    };

    let codes: Vec<&str> = listener
        .diagnostics
        .iter()
        .map(|d| d.code.unique_name)
        .collect();
    if expect_error {
        assert_eq!(
            codes,
            vec![diag::COULD_NOT_INFER.unique_name],
            "expected exactly 1 could not infer error."
        );
    } else {
        assert!(codes.is_empty(), "did not expect any errors: {codes:?}");
    }
    type_arguments
}

fn _infer_call(t: &TypeSystemTest, ft: TypeId, arguments: &[TypeId]) -> Vec<TypeId> {
    _infer_call_with(t, ft, arguments, TypeId::UNKNOWN, false)
}

/// `_inferCall2(ft, arguments)`: `ft.instantiate(typeArguments)`.
fn _infer_call2(t: &TypeSystemTest, ft: TypeId, arguments: &[TypeId]) -> TypeId {
    let type_arguments = _infer_call(t, ft, arguments);
    t.ctx().instantiate_function_type(ft, &type_arguments)
}

/// `interfaceType.getMethod(name)!.type`: the type of the method [name] of
/// the class of [interface_type], substituted with its type arguments.
fn get_method_type(t: &TypeSystemTest, interface_type: TypeId, name: &str) -> TypeId {
    let ctx = t.ctx();
    let element = ctx.interface_element(interface_type).unwrap();
    let method = t.method(element, name);
    let raw = element_type::executable_type(&ctx, method.upcast());
    MapSubstitution::from_interface_type(&ctx, interface_type).substitute_type(&ctx, raw)
}

fn classes(headers: &[ClassSpec]) -> LibrarySpec {
    LibrarySpec {
        classes: headers.to_vec(),
        ..LibrarySpec::test()
    }
}

// -------------------------------------------------------------------- tests

#[test]
fn test_boundedByAnotherTypeParameter() {
    let t = TypeSystemTest::new();
    // <TFrom, TTo extends Iterable<TFrom>>(TFrom) -> TTo
    let cast = t.parse_function_type("TTo Function<TFrom, TTo extends Iterable<TFrom>>(TFrom)");
    _assert_types(
        &t,
        &_infer_call(&t, cast, &[t.parse_type("String")]),
        &[t.parse_type("String"), t.parse_type("Iterable<String>")],
    );
}

#[test]
fn test_boundedByOuterClass() {
    // Regression test for https://github.com/dart-lang/sdk/issues/25740.
    let mut t = TypeSystemTest::new();
    t.build_test_library(classes(&[
        ClassSpec::new("class A"),
        ClassSpec::new("class B extends A"),
        ClassSpec::new("class C<T extends A>").methods(&["S m<S extends T>(S _)"]),
    ]));

    // class B extends A {}
    let type_B = t.parse_interface_type("B");

    // class C<T extends A> { S m<S extends T>(S); }
    // C<Object> cOfObject;
    let c_of_object = t.parse_interface_type("C<Object>");
    // C<A> cOfA;
    let c_of_A = t.parse_interface_type("C<A>");
    // C<B> cOfB;
    let c_of_B = t.parse_interface_type("C<B>");
    // B b;
    // cOfB.m(b); // infer <B>
    _assert_type(
        &t,
        _infer_call2(&t, get_method_type(&t, c_of_B, "m"), &[type_B]),
        "B Function(B)",
    );
    // cOfA.m(b); // infer <B>
    _assert_type(
        &t,
        _infer_call2(&t, get_method_type(&t, c_of_A, "m"), &[type_B]),
        "B Function(B)",
    );
    // cOfObject.m(b); // infer <B>
    _assert_type(
        &t,
        _infer_call2(&t, get_method_type(&t, c_of_object, "m"), &[type_B]),
        "B Function(B)",
    );
}

#[test]
fn test_boundedByOuterClassSubstituted() {
    // Regression test for https://github.com/dart-lang/sdk/issues/25740.
    let mut t = TypeSystemTest::new();
    t.build_test_library(classes(&[
        ClassSpec::new("class A"),
        ClassSpec::new("class B extends A"),
        ClassSpec::new("class C<T extends A>").methods(&["S m<S extends Iterable<T>>(S _)"]),
    ]));

    // class C<T extends A> { S m<S extends Iterable<T>>(S); }
    // C<Object> cOfObject;
    let c_of_object = t.parse_interface_type("C<Object>");
    // C<A> cOfA;
    let c_of_A = t.parse_interface_type("C<A>");
    // C<B> cOfB;
    let c_of_B = t.parse_interface_type("C<B>");
    // List<B> b;
    let list_of_B = t.parse_type("List<B>");
    // cOfB.m(b); // infer <B>
    _assert_type(
        &t,
        _infer_call2(&t, get_method_type(&t, c_of_B, "m"), &[list_of_B]),
        "List<B> Function(List<B>)",
    );
    // cOfA.m(b); // infer <B>
    _assert_type(
        &t,
        _infer_call2(&t, get_method_type(&t, c_of_A, "m"), &[list_of_B]),
        "List<B> Function(List<B>)",
    );
    // cOfObject.m(b); // infer <B>
    _assert_type(
        &t,
        _infer_call2(&t, get_method_type(&t, c_of_object, "m"), &[list_of_B]),
        "List<B> Function(List<B>)",
    );
}

#[test]
fn test_boundedRecursively() {
    let mut t = TypeSystemTest::new();
    t.build_test_library(classes(&[
        ClassSpec::new("class Cloneable<T extends Cloneable<T>>"),
        ClassSpec::new("class B extends Cloneable<B>"),
    ]));

    // class Cloneable<T extends Cloneable<T>>

    // class B extends A<B> {}
    let type_B = t.parse_interface_type("B");

    // (S, S) -> S
    let clone = t.parse_function_type("S Function<S extends Cloneable<S>>(S, S)");
    _assert_types(&t, &_infer_call(&t, clone, &[type_B, type_B]), &[type_B]);

    // Something invalid...
    _assert_types(
        &t,
        &_infer_call_with(
            &t,
            clone,
            &[t.parse_type("String"), t.parse_type("num")],
            TypeId::UNKNOWN,
            true,
        ),
        &[t.parse_interface_type("Cloneable<Object?>")],
    );
}

#[test]
fn test_buildTestLibrary_topLevelFunctionHeader() {
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["T f<T>(T value)"]),
        ..LibrarySpec::test()
    });

    let f = t.top_level_function("f");
    _assert_type(
        &t,
        element_type::executable_type(&t.ctx(), f.upcast()),
        "T Function<T>(T)",
    );
}

#[test]
fn test_buildTestLibrary_topLevelFunctionHeader_namedParameters() {
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void f({int optional, required int required})"]),
        ..LibrarySpec::test()
    });

    let f = t.top_level_function("f");
    _assert_type(
        &t,
        element_type::executable_type(&t.ctx(), f.upcast()),
        "void Function({int optional, required int required})",
    );
}

/// https://github.com/dart-lang/language/issues/1182#issuecomment-702272641
#[test]
fn test_demoteType() {
    let t = TypeSystemTest::new();
    // <T>(T x) -> void
    let raw_type = t.parse_function_type("void Function<T>(T)");

    t.with_type_parameter_scope("S", |scope| {
        let S = scope.type_parameter("S");
        let S_and_int = scope.parse_type_parameter_type("S & int");

        let inferred_types = _infer_call(&t, raw_type, &[S_and_int]);
        let TypeKind::TypeParameter {
            param,
            promoted_bound,
            ..
        } = *t.ctx().ty(inferred_types[0])
        else {
            panic!("TypeParameterTypeImpl expected");
        };
        assert_eq!(param, S);
        assert_eq!(promoted_bound, None);
    });
}

#[test]
fn test_genericCastFunction() {
    let t = TypeSystemTest::new();
    // <TFrom, TTo>(TFrom) -> TTo
    let cast = t.parse_function_type("TTo Function<TFrom, TTo>(TFrom)");
    _assert_types(
        &t,
        &_infer_call(&t, cast, &[t.parse_type("int")]),
        &[t.parse_type("int"), t.parse_type("dynamic")],
    );
}

#[test]
fn test_genericCastFunctionWithUpperBound() {
    let t = TypeSystemTest::new();
    // <TFrom, TTo extends TFrom>(TFrom) -> TTo
    let cast = t.parse_function_type("TTo Function<TFrom, TTo extends TFrom>(TFrom)");
    _assert_types(
        &t,
        &_infer_call(&t, cast, &[t.parse_type("int")]),
        &[t.parse_type("int"), t.parse_type("int")],
    );
}

#[test]
fn test_parameter_contravariantUseUpperBound() {
    let t = TypeSystemTest::new();
    // <T>(T x, void Function(T) y) -> T
    // Generates constraints int <: T <: num.
    // Since T is contravariant, choose num.
    let num_function = t.parse_function_type("void Function(num)");
    let function = t.parse_function_type("T Function<in T>(T, void Function(T))");

    _assert_types(
        &t,
        &_infer_call(&t, function, &[t.parse_type("int"), num_function]),
        &[t.parse_type("num")],
    );
}

#[test]
fn test_parameter_covariantUseLowerBound() {
    let t = TypeSystemTest::new();
    // <T>(T x, void Function(T) y) -> T
    // Generates constraints int <: T <: num.
    // Since T is covariant, choose int.
    let num_function = t.parse_function_type("void Function(num)");
    let function = t.parse_function_type("T Function<out T>(T, void Function(T))");

    _assert_types(
        &t,
        &_infer_call(&t, function, &[t.parse_type("int"), num_function]),
        &[t.parse_type("int")],
    );
}

#[test]
fn test_parametersToFunctionParam() {
    let t = TypeSystemTest::new();
    // <T>(f(T t)) -> T
    let cast = t.parse_function_type("T Function<T>(dynamic Function(T))");
    _assert_types(
        &t,
        &_infer_call(&t, cast, &[t.parse_function_type("dynamic Function(num)")]),
        &[t.parse_type("num")],
    );
}

#[test]
fn test_parametersUseLeastUpperBound() {
    let t = TypeSystemTest::new();
    // <T>(T x, T y) -> T
    let cast = t.parse_function_type("T Function<T>(T, T)");
    _assert_types(
        &t,
        &_infer_call(&t, cast, &[t.parse_type("int"), t.parse_type("double")]),
        &[t.parse_type("num")],
    );
}

#[test]
fn test_parameterTypeUsesUpperBound() {
    let t = TypeSystemTest::new();
    // <T extends num>(T) -> dynamic
    let f = t.parse_function_type("dynamic Function<T extends num>(T)");
    _assert_types(
        &t,
        &_infer_call(&t, f, &[t.parse_type("int")]),
        &[t.parse_type("int")],
    );
}

#[test]
fn test_returnFunctionWithGenericParameter() {
    let t = TypeSystemTest::new();
    // <T>(T -> T) -> (T -> void)
    let f = t.parse_function_type("void Function(T) Function<T>(T Function(T))");
    _assert_types(
        &t,
        &_infer_call(&t, f, &[t.parse_function_type("int Function(num)")]),
        &[t.parse_type("int")],
    );
}

#[test]
fn test_returnFunctionWithGenericParameterAndContext() {
    let t = TypeSystemTest::new();
    // <T>(T -> T) -> (T -> Null)
    let f = t.parse_function_type("Null Function(T) Function<T>(T Function(T))");
    _assert_types(
        &t,
        &_infer_call_with(
            &t,
            f,
            &[],
            t.parse_function_type("int? Function(num)"),
            false,
        ),
        &[t.parse_type("num")],
    );
}

#[test]
fn test_returnFunctionWithGenericParameterAndReturn() {
    let t = TypeSystemTest::new();
    // <T>(T -> T) -> (T -> T)
    let f = t.parse_function_type("T Function(T) Function<T>(T Function(T))");
    _assert_types(
        &t,
        &_infer_call(&t, f, &[t.parse_function_type("int Function(num)")]),
        &[t.parse_type("int")],
    );
}

#[test]
fn test_returnFunctionWithGenericReturn() {
    let t = TypeSystemTest::new();
    // <T>(T -> T) -> (() -> T)
    let f = t.parse_function_type("T Function() Function<T>(T Function(T))");
    _assert_types(
        &t,
        &_infer_call(&t, f, &[t.parse_function_type("int Function(num)")]),
        &[t.parse_type("int")],
    );
}

#[test]
fn test_returnTypeFromContext() {
    let t = TypeSystemTest::new();
    // <T>() -> T
    let f = t.parse_function_type("T Function<T>()");
    _assert_types(
        &t,
        &_infer_call_with(&t, f, &[], t.parse_type("String"), false),
        &[t.parse_type("String")],
    );
}

#[test]
fn test_returnTypeWithBoundFromContext() {
    let t = TypeSystemTest::new();
    // <T extends num>() -> T
    let f = t.parse_function_type("T Function<T extends num>()");
    _assert_types(
        &t,
        &_infer_call_with(&t, f, &[], t.parse_type("double"), false),
        &[t.parse_type("double")],
    );
}

#[test]
fn test_returnTypeWithBoundFromInvalidContext() {
    let t = TypeSystemTest::new();
    // <T extends num>() -> T
    let f = t.parse_function_type("T Function<T extends num>()");
    _assert_types(
        &t,
        &_infer_call_with(&t, f, &[], t.parse_type("String"), false),
        &[t.parse_type("Never")],
    );
}

#[test]
fn test_unifyParametersToFunctionParam() {
    let t = TypeSystemTest::new();
    // <T>(f(T t), g(T t)) -> T
    let cast = t.parse_function_type("T Function<T>(dynamic Function(T), dynamic Function(T))");
    _assert_types(
        &t,
        &_infer_call(
            &t,
            cast,
            &[
                t.parse_function_type("dynamic Function(int)"),
                t.parse_function_type("dynamic Function(double)"),
            ],
        ),
        &[t.parse_type("Never")],
    );
}

#[test]
fn test_unusedReturnTypeIsDynamic() {
    let t = TypeSystemTest::new();
    // <T>() -> T
    let f = t.parse_function_type("T Function<T>()");
    _assert_types(&t, &_infer_call(&t, f, &[]), &[t.parse_type("dynamic")]);
}

#[test]
fn test_unusedReturnTypeWithUpperBound() {
    let t = TypeSystemTest::new();
    // <T extends num>() -> T
    let f = t.parse_function_type("T Function<T extends num>()");
    _assert_types(&t, &_infer_call(&t, f, &[]), &[t.parse_type("num")]);
}

// ------------------------------------------------------------ not in Dart

/// Infers a call of the top-level function [name] of the test library with
/// [arguments] in the context `_` (an expression statement) and returns the
/// messages of the reported diagnostics.
fn _infer_top_level_call_messages(
    t: &TypeSystemTest,
    name: &str,
    arguments: &[TypeId],
) -> Vec<String> {
    let ctx = t.ctx();
    let ft = element_type::executable_type(&ctx, t.top_level_function(name).upcast());
    let TypeKind::Function(f) = *ctx.ty(ft) else {
        unreachable!()
    };

    let mut listener = RecordingDiagnosticListener::new();
    {
        let mut reporter = DiagnosticReporter::new(&mut listener);
        let mut inferrer = t.type_system().setup_generic_type_inference(
            ctx.list(f.type_params),
            f.ret,
            TypeId::UNKNOWN,
            Some(&mut reporter),
            Some(InferenceErrorEntity::other(0, 1)),
            InferenceFlags {
                generic_metadata_is_enabled: true,
                inference_using_bounds_is_enabled: true,
                strict_inference: false,
            },
            false,
            t.type_system_operations(),
            None,
            None,
        );
        inferrer.constrain_arguments2(ctx.list(f.params), arguments, None);
        inferrer.choose_final_types();
    }
    listener
        .diagnostics
        .into_iter()
        .map(|d| d.message)
        .collect()
}

/// Not in the Dart test file: the text of `couldNotInfer` (`_formatError`)
/// for an `extends` origin. The expected message is the output of
/// `dart analyze --format=machine` (SDK 3.13.3) for:
///
/// ```dart
/// class Cloneable<T extends Cloneable<T>> {}
/// S clone<S extends Cloneable<S>>(S a, S b) => a;
/// void f(String s, num n) { clone(s, n); }
/// ```
#[test]
fn couldNotInfer_message_extendsOrigin_matches_dart_analyze() {
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        classes: vec![ClassSpec::new("class Cloneable<T extends Cloneable<T>>")],
        functions: strs(&["S clone<S extends Cloneable<S>>(S a, S b)"]),
        ..LibrarySpec::test()
    });
    let messages =
        _infer_top_level_call_messages(&t, "clone", &[t.parse_type("String"), t.parse_type("num")]);
    assert_eq!(
        messages,
        vec![
            "Couldn't infer type parameter 'S'.\n\n\
             Tried to infer 'Cloneable<Object?>' for 'S' which doesn't work:\n  \
             Type parameter 'S' is declared to extend 'Cloneable<S>' producing 'Cloneable<Cloneable<Object?>>'.\n\n\
             Consider passing explicit type argument(s) to the generic.\n\n"
        ]
    );
}

/// Not in the Dart test file: `_formatError` with satisfied and unsatisfied
/// argument origins (padding of the lines). The expected message is the
/// output of `dart analyze --format=machine` (SDK 3.13.3) for:
///
/// ```dart
/// void h<T>(List<T> a, void Function(T) b) {}
/// void g(List<String> l, void Function(int) fn) { h(l, fn); }
/// ```
#[test]
fn couldNotInfer_message_argumentOrigins_matches_dart_analyze() {
    let mut t = TypeSystemTest::new();
    t.build_test_library(LibrarySpec {
        functions: strs(&["void h<T>(List<T> a, void Function(T) b)"]),
        ..LibrarySpec::test()
    });
    let messages = _infer_top_level_call_messages(
        &t,
        "h",
        &[
            t.parse_type("List<String>"),
            t.parse_function_type("void Function(int)"),
        ],
    );
    assert_eq!(
        messages,
        vec![
            "Couldn't infer type parameter 'T'.\n\n\
             Tried to infer 'String' for 'T' which doesn't work:\n  \
             Parameter 'b' declared as     'void Function(T)'\n                \
             but argument is 'void Function(int)'.\n\
             The type 'String' was inferred from:\n  \
             Parameter 'a' declared as     'List<T>'\n                \
             but argument is 'List<String>'.\n\n\
             Consider passing explicit type argument(s) to the generic.\n\n"
        ]
    );
}
