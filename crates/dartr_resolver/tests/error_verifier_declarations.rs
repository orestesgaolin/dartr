//! ErrorVerifier section D4 (class, enum, mixin, extension, extension type
//! and type alias declarations, directives): ports of representative tests
//! of `pkg/analyzer/test/src/diagnostics/<code>_test.dart`. The expected
//! offsets are the offsets of the Dart tests (computed from the marked
//! text, the comment lines of the Dart tests removed).

mod ev_support;
mod support;

use ev_support::{assert_errors_in_code, assert_errors_in_files};

/// The offset of the [nth] (0-based) occurrence of [needle] in [source].
fn at(source: &str, needle: &str, nth: usize) -> usize {
    source
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("{needle:?} #{nth} not in source"))
        .0
}

// class_used_as_mixin_test.dart

#[test]
fn class_used_as_mixin_inside() {
    let code = "class Foo {}\nclass Bar with Foo {}\n";
    assert_errors_in_code(code, &[("class_used_as_mixin", at(code, "Foo", 1), 3)]);
}

#[test]
fn class_used_as_mixin_outside() {
    let code = "import 'foo.dart';\nclass Bar with Foo {}\n";
    assert_errors_in_files(
        &[("main.dart", code), ("foo.dart", "class Foo {}\n")],
        &[("class_used_as_mixin", at(code, "Foo", 0), 3)],
    );
}

#[test]
fn class_used_as_mixin_inside_mixin_class() {
    assert_errors_in_code("mixin class Foo {}\nclass Bar with Foo {}\n", &[]);
}

// mixin_inherits_from_not_object_test.dart

#[test]
fn mixin_inherits_from_not_object_class_extends() {
    let code = "class A {}\nmixin class B extends A {}\nclass C extends Object with B {}\n";
    assert_errors_in_code(
        code,
        &[
            (
                "mixin_class_declaration_extends_not_object",
                at(code, "A", 1),
                1,
            ),
            ("mixin_inherits_from_not_object", at(code, "B", 1), 1),
        ],
    );
}

// conflicting_static_and_instance_test.dart

#[test]
fn conflicting_static_and_instance_static_getter_inherited_method() {
    // The conflict inside one class is reported by the
    // MemberDuplicateDefinitionVerifier (error/*), not by ErrorVerifier.
    let code =
        "class A {\n  void foo() {}\n}\nclass B extends A {\n  static int get foo => 0;\n}\n";
    assert_errors_in_code(
        code,
        &[("conflicting_static_and_instance", at(code, "foo", 1), 3)],
    );
}

#[test]
fn conflicting_static_and_instance_in_superclass() {
    let code =
        "class A {\n  int get foo => 0;\n}\nclass B extends A {\n  static void foo() {}\n}\n";
    assert_errors_in_code(
        code,
        &[("conflicting_static_and_instance", at(code, "foo", 1), 3)],
    );
}

// conflicting_method_and_field_test.dart, conflicting_field_and_method_test.dart

#[test]
fn conflicting_method_and_field_inherited_getter() {
    let code = "class A {\n  int get x => 0;\n}\nclass B extends A {\n  x() {}\n}\n";
    assert_errors_in_code(
        code,
        &[("conflicting_method_and_field", at(code, "x()", 0), 1)],
    );
}

#[test]
fn conflicting_field_and_method_inherited_method() {
    let code = "class A {\n  x() {}\n}\nclass B extends A {\n  int get x => 0;\n}\n";
    assert_errors_in_code(
        code,
        &[("conflicting_field_and_method", at(code, "x =>", 0), 1)],
    );
}

// type_alias_cannot_reference_itself_test.dart

#[test]
fn type_alias_cannot_reference_itself_function_with_it() {
    let code = "typedef T = void Function(T);\n";
    assert_errors_in_code(code, &[("type_alias_cannot_reference_itself", 8, 1)]);
}

#[test]
fn type_alias_cannot_reference_itself_question() {
    let code = "typedef T = T?;\n";
    assert_errors_in_code(code, &[("type_alias_cannot_reference_itself", 8, 1)]);
}

// enum_without_constants_test.dart, enum_with_name_values_test.dart,
// enum_constant_same_name_as_enclosing_test.dart

#[test]
fn enum_without_constants() {
    assert_errors_in_code("enum E {}\n", &[("enum_without_constants", 5, 1)]);
}

#[test]
fn enum_constant_same_name_as_enclosing() {
    let code = "enum E {\n  E\n}\n";
    assert_errors_in_code(code, &[("enum_constant_same_name_as_enclosing", 11, 1)]);
}

// implements_repeated_test.dart

#[test]
fn implements_repeated_4times() {
    let code = "class A {} class C{}\nclass B implements A, A, A, A {}\n";
    let first = at(code, "A, A", 0);
    assert_errors_in_code(
        code,
        &[
            ("implements_repeated", first + 3, 1),
            ("implements_repeated", first + 6, 1),
            ("implements_repeated", first + 9, 1),
        ],
    );
}

// implements_super_class_test.dart

#[test]
fn implements_super_class() {
    let code = "class A {}\nclass B extends A implements A {}\n";
    assert_errors_in_code(code, &[("implements_super_class", at(code, "A {}", 1), 1)]);
}

// built_in_identifier_as_type_name_test.dart,
// built_in_identifier_as_typedef_name_test.dart

#[test]
fn built_in_identifier_as_type_name_class() {
    let code = "class as {}\n";
    assert_errors_in_code(code, &[("built_in_identifier_in_declaration", 6, 2)]);
}

#[test]
fn built_in_identifier_as_typedef_name() {
    let code = "typedef void as();\n";
    assert_errors_in_code(code, &[("built_in_identifier_in_declaration", 13, 2)]);
}

// mixin_application_not_implemented_interface_test.dart

#[test]
fn mixin_application_not_implemented_interface() {
    let code = "class A {}\nmixin M on A {}\nclass X = Object with M;\n";
    assert_errors_in_code(
        code,
        &[(
            "mixin_application_not_implemented_interface",
            at(code, "M;", 0),
            1,
        )],
    );
}

// mixin_application_no_concrete_super_invoked_member_test.dart

#[test]
fn mixin_application_no_concrete_super_invoked_member() {
    let code = "abstract class A {\n  void foo();\n}\nmixin M on A {\n  void bar() {\n    super.foo();\n  }\n}\nabstract class X extends A with M {}\n";
    assert_errors_in_code(
        code,
        &[(
            "mixin_application_no_concrete_super_invoked_member",
            at(code, "M {}", 0),
            1,
        )],
    );
}

// conflicting_generic_interfaces_test.dart

#[test]
fn conflicting_generic_interfaces() {
    let code = "class I<T> {}\nclass A implements I<int> {}\nclass B implements I<String> {}\nclass C extends A implements B {}\n";
    assert_errors_in_code(
        code,
        &[(
            "conflicting_generic_interfaces",
            at(code, "C extends", 0),
            1,
        )],
    );
}

// conflicting_type_variable_and_member_test.dart,
// conflicting_type_variable_and_container_test.dart

#[test]
fn conflicting_type_variable_and_member_class_method() {
    let code = "class A<T> {\n  T() {}\n}\n";
    assert_errors_in_code(code, &[("conflicting_type_variable_and_member", 8, 1)]);
}

#[test]
fn conflicting_type_variable_and_class() {
    let code = "class T<T> {}\n";
    assert_errors_in_code(code, &[("conflicting_type_variable_and_container", 8, 1)]);
}

// not_initialized_non_nullable_variable_test.dart,
// not_initialized_non_nullable_instance_field_test.dart,
// final_not_initialized_test.dart, const_not_initialized_test.dart

#[test]
fn not_initialized_fields() {
    let code = "class A {\n  static int a;\n  int b;\n  final int c;\n  static const int d;\n  int? e;\n}\n";
    assert_errors_in_code(
        code,
        &[
            (
                "not_initialized_non_nullable_variable",
                at(code, "a;", 0),
                1,
            ),
            (
                "not_initialized_non_nullable_instance_field",
                at(code, "b;", 0),
                1,
            ),
            ("final_not_initialized", at(code, "c;", 0), 1),
            ("const_not_initialized", at(code, "d;", 0), 1),
        ],
    );
}

#[test]
fn not_initialized_instance_field_with_constructor() {
    // The ConstructorFieldsVerifier reports for classes with constructors.
    assert_errors_in_code("class A {\n  int b;\n  A(this.b);\n}\n", &[]);
}

// extension_type_declares_member_of_object_test.dart,
// expected_representation_field_test.dart

#[test]
fn extension_type_representation_member_of_object() {
    let code = "extension type E(int hashCode) {}\n";
    assert_errors_in_code(
        code,
        &[(
            "extension_type_declares_member_of_object",
            at(code, "hashCode", 0),
            8,
        )],
    );
}

#[test]
fn extension_type_multiple_representation_fields() {
    let code = "extension type E(int a, String b) {}\n";
    assert_errors_in_code(
        code,
        &[("multiple_representation_fields", at(code, ", String", 0), 1)],
    );
}

// invalid_use_of_type_outside_library (final, base, interface, sealed)

#[test]
fn final_class_extended_outside_of_library() {
    let code = "import 'foo.dart';\nclass B extends A {}\n";
    assert_errors_in_files(
        &[("main.dart", code), ("foo.dart", "final class A {}\n")],
        &[(
            "invalid_use_of_type_outside_library",
            at(code, "A {}", 0),
            1,
        )],
    );
}

#[test]
fn base_class_implemented_outside_of_library() {
    let code = "import 'foo.dart';\nbase class B implements A {}\n";
    assert_errors_in_files(
        &[("main.dart", code), ("foo.dart", "base class A {}\n")],
        &[(
            "invalid_use_of_type_outside_library",
            at(code, "A {}", 0),
            1,
        )],
    );
}

#[test]
fn interface_class_extended_outside_of_library() {
    let code = "import 'foo.dart';\nclass B extends A {}\n";
    assert_errors_in_files(
        &[("main.dart", code), ("foo.dart", "interface class A {}\n")],
        &[(
            "invalid_use_of_type_outside_library",
            at(code, "A {}", 0),
            1,
        )],
    );
}

#[test]
fn sealed_class_subtype_outside_of_library() {
    let code = "import 'foo.dart';\nclass B extends A {}\n";
    assert_errors_in_files(
        &[("main.dart", code), ("foo.dart", "sealed class A {}\n")],
        &[(
            "invalid_use_of_type_outside_library",
            at(code, "A {}", 0),
            1,
        )],
    );
}

#[test]
fn final_class_inside_library() {
    assert_errors_in_code("final class A {}\nfinal class B extends A {}\n", &[]);
}

// multiple_combinators_test.dart (a hint in Dart)

#[test]
fn multiple_combinators() {
    let code = "import 'dart:math' show max hide min;\n";
    let start = at(code, "show", 0);
    assert_errors_in_code(
        code,
        &[("multiple_combinators", start, at(code, ";", 0) - start)],
    );
}

// native_clause_in_non_sdk_code_test.dart

#[test]
fn native_clause_in_non_sdk_code() {
    let code = "class A native 'string' {}\n";
    assert_errors_in_code(code, &[("native_clause_in_non_sdk_code", 8, 15)]);
}

// main_is_not_function_test.dart

#[test]
fn main_is_not_function_class() {
    assert_errors_in_code("class main {}\n", &[("main_is_not_function", 6, 4)]);
}

// mixin_class_declares_non_trivial_generative_constructor_test.dart

#[test]
fn mixin_class_declares_non_trivial_generative_constructor() {
    let code = "mixin class A {\n  A(int x);\n}\n";
    assert_errors_in_code(
        code,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            at(code, "A(", 0),
            1,
        )],
    );
}

// no_default_super_constructor_test.dart (implicit)

#[test]
fn no_default_super_constructor_implicit() {
    let code = "class A {\n  A(int p);\n}\nclass B extends A {}\n";
    assert_errors_in_code(
        code,
        &[("no_default_super_constructor", at(code, "B", 0), 1)],
    );
}

// mixins_super_class_test.dart

#[test]
fn mixin_class_with_valid_hierarchy_has_no_errors() {
    assert_errors_in_code(
        "mixin M {}\nabstract class A {}\nclass B extends A with M implements Comparable<B> {\n  int compareTo(B other) => 0;\n}\n",
        &[],
    );
}
