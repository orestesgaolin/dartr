//! ErrorVerifier diagnostic tests of section D7, generated from
//! `pkg/analyzer/test/src/diagnostics/<code>_test.dart` (the tests with one
//! library and no other setup) by converting the inline diagnostic
//! markers or the `error(diag.x, offset, length)` lists. Only the codes
//! of `tools/difftest/error_verifier_codes.txt` are compared.

mod ev_support;
mod support;

use ev_support::assert_errors_in_code;

/// `ambiguous_import_test.dart` `test_systemLibrary_systemLibrary`.
#[test]
fn ambiguous_import_system_library_system_library() {
    assert_errors_in_code(
        r#"
import 'dart:html';
import 'dart:io';
g(File f) {}
"#,
        &[("ambiguous_import", 41, 4)],
    );
}

/// `default_value_already_specified_in_augmentation_chain_test.dart` `test_constructor_optionalNamed`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn default_value_already_specified_in_augmentation_chain_constructor_optional_named() {
    assert_errors_in_code(
        r#"
class A {
  A({int x = 0});

  augment A({int x = 0});
}
"#,
        &[(
            "default_value_already_specified_in_augmentation_chain",
            49,
            1,
        )],
    );
}

/// `default_value_already_specified_in_augmentation_chain_test.dart` `test_constructor_optionalPositional`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn default_value_already_specified_in_augmentation_chain_constructor_optional_positional() {
    assert_errors_in_code(
        r#"
class A {
  A([int x = 0]);

  augment A([int x = 0]);
}
"#,
        &[(
            "default_value_already_specified_in_augmentation_chain",
            49,
            1,
        )],
    );
}

/// `default_value_already_specified_in_augmentation_chain_test.dart` `test_method_optionalNamed`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn default_value_already_specified_in_augmentation_chain_method_optional_named() {
    assert_errors_in_code(
        r#"
class A {
  void foo({int x = 0});

  augment void foo({int x = 0}) {}
}
"#,
        &[(
            "default_value_already_specified_in_augmentation_chain",
            63,
            1,
        )],
    );
}

/// `default_value_already_specified_in_augmentation_chain_test.dart` `test_method_optionalPositional`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn default_value_already_specified_in_augmentation_chain_method_optional_positional() {
    assert_errors_in_code(
        r#"
class A {
  void foo([int x = 0]);

  augment void foo([int x = 0]) {}
}
"#,
        &[(
            "default_value_already_specified_in_augmentation_chain",
            63,
            1,
        )],
    );
}

/// `default_value_already_specified_in_augmentation_chain_test.dart` `test_topLevelFunction_optionalPositional`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn default_value_already_specified_in_augmentation_chain_top_level_function_optional_positional() {
    assert_errors_in_code(
        r#"
void f([int x = 0]);

augment void f([int x = 0]) {}
"#,
        &[(
            "default_value_already_specified_in_augmentation_chain",
            45,
            1,
        )],
    );
}

/// `default_value_already_specified_in_augmentation_chain_test.dart` `test_topLevelFunction_optionalPositional_secondAugmentation`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn default_value_already_specified_in_augmentation_chain_top_level_function_optional_positional_second_augmentation()
 {
    assert_errors_in_code(
        r#"
void f([int x]);

augment void f([int x = 0]);

augment void f([int x = 1]) {}
"#,
        &[(
            "default_value_already_specified_in_augmentation_chain",
            71,
            1,
        )],
    );
}

/// `default_value_on_required_parameter_test.dart` `test_function_notRequired_default`.
#[test]
fn default_value_on_required_parameter_function_not_required_default() {
    assert_errors_in_code(
        r#"
void log({String message = 'no message'}) {}
"#,
        &[],
    );
}

/// `default_value_on_required_parameter_test.dart` `test_function_notRequired_noDefault`.
#[test]
fn default_value_on_required_parameter_function_not_required_no_default() {
    assert_errors_in_code(
        r#"
void log({String? message}) {}
"#,
        &[],
    );
}

/// `default_value_on_required_parameter_test.dart` `test_function_required_default`.
#[test]
fn default_value_on_required_parameter_function_required_default() {
    assert_errors_in_code(
        r#"
void log({required String? message = 'no message'}) {}
"#,
        &[("default_value_on_required_parameter", 28, 7)],
    );
}

/// `default_value_on_required_parameter_test.dart` `test_function_required_noDefault`.
#[test]
fn default_value_on_required_parameter_function_required_no_default() {
    assert_errors_in_code(
        r#"
void log({required String message}) {}
"#,
        &[],
    );
}

/// `default_value_on_required_parameter_test.dart` `test_method_abstract_required_default`.
#[test]
fn default_value_on_required_parameter_method_abstract_required_default() {
    assert_errors_in_code(
        r#"
abstract class C {
  void foo({required int? a = 0});
}
"#,
        &[("default_value_on_required_parameter", 46, 1)],
    );
}

/// `default_value_on_required_parameter_test.dart` `test_method_required_default`.
#[test]
fn default_value_on_required_parameter_method_required_default() {
    assert_errors_in_code(
        r#"
class C {
  void foo({required int? a = 0}) {}
}
"#,
        &[("default_value_on_required_parameter", 37, 1)],
    );
}

/// `extension_declares_member_of_object_test.dart` `test_instance_differentKind`.
#[test]
fn extension_declares_member_of_object_instance_different_kind() {
    assert_errors_in_code(
        r#"
extension E on String {
  void hashCode() {}
}
"#,
        &[("extension_declares_member_of_object", 32, 8)],
    );
}

/// `extension_declares_member_of_object_test.dart` `test_instance_sameKind`.
#[test]
fn extension_declares_member_of_object_instance_same_kind() {
    assert_errors_in_code(
        r#"
extension E on String {
  bool operator==(Object _) => false;
  int get hashCode => 0;
  String toString() => '';
  dynamic get runtimeType => null;
  dynamic noSuchMethod(_) => null;
}
"#,
        &[
            ("extension_declares_member_of_object", 40, 2),
            ("extension_declares_member_of_object", 73, 8),
            ("extension_declares_member_of_object", 97, 8),
            ("extension_declares_member_of_object", 129, 11),
            ("extension_declares_member_of_object", 160, 12),
        ],
    );
}

/// `extension_declares_member_of_object_test.dart` `test_static`.
#[test]
fn extension_declares_member_of_object_static() {
    assert_errors_in_code(
        r#"
extension E on String {
  static void hashCode() {}
}
"#,
        &[("extension_declares_member_of_object", 39, 8)],
    );
}

/// `extension_type_declares_member_of_object_test.dart` `test_instance_differentKind`.
#[test]
fn extension_type_declares_member_of_object_instance_different_kind() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  int get hashCode => 0;
}
"#,
        &[("extension_type_declares_member_of_object", 38, 8)],
    );
}

/// `extension_type_declares_member_of_object_test.dart` `test_instance_sameKind`.
#[test]
fn extension_type_declares_member_of_object_instance_same_kind() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  bool operator==(Object _) => false;
  int get hashCode => 0;
  String toString() => '';
  dynamic get runtimeType => null;
  dynamic noSuchMethod(_) => null;
}
"#,
        &[
            ("extension_type_declares_member_of_object", 43, 2),
            ("extension_type_declares_member_of_object", 76, 8),
            ("extension_type_declares_member_of_object", 100, 8),
            ("extension_type_declares_member_of_object", 132, 11),
            ("extension_type_declares_member_of_object", 163, 12),
        ],
    );
}

/// `extension_type_declares_member_of_object_test.dart` `test_representation`.
#[test]
fn extension_type_declares_member_of_object_representation() {
    assert_errors_in_code(
        r#"
extension type E0(Object? hashCode) {}
extension type E1(Object? noSuchMethod) {}
extension type E2(Object? runtimeType) {}
extension type E3(Object? toString) {}
"#,
        &[
            ("extension_type_declares_member_of_object", 27, 8),
            ("extension_type_declares_member_of_object", 66, 12),
            ("extension_type_declares_member_of_object", 109, 11),
            ("extension_type_declares_member_of_object", 151, 8),
        ],
    );
}

/// `extension_type_declares_member_of_object_test.dart` `test_static_getter`.
#[test]
fn extension_type_declares_member_of_object_static_getter() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  static int get hashCode => 0;
  static int get noSuchMethod => 0;
  static int get runtimeType => 0;
  static int get toString => 0;
}
"#,
        &[
            ("extension_type_declares_member_of_object", 45, 8),
            ("extension_type_declares_member_of_object", 77, 12),
            ("extension_type_declares_member_of_object", 113, 11),
            ("extension_type_declares_member_of_object", 148, 8),
        ],
    );
}

/// `extension_type_declares_member_of_object_test.dart` `test_static_method`.
#[test]
fn extension_type_declares_member_of_object_static_method() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  static int hashCode() => 0;
  static dynamic noSuchMethod(Invocation i) => null;
  static Type runtimeType() => int;
  static String toString() => '';
}
"#,
        &[
            ("extension_type_declares_member_of_object", 41, 8),
            ("extension_type_declares_member_of_object", 75, 12),
            ("extension_type_declares_member_of_object", 125, 11),
            ("extension_type_declares_member_of_object", 163, 8),
        ],
    );
}

/// `extension_type_declares_member_of_object_test.dart` `test_static_setter`.
#[test]
fn extension_type_declares_member_of_object_static_setter() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  static set hashCode(int _) {}
  static set noSuchMethod(int _) {}
  static set runtimeType(int _) {}
  static set toString(int _) {}
}
"#,
        &[
            ("extension_type_declares_member_of_object", 41, 8),
            ("extension_type_declares_member_of_object", 73, 12),
            ("extension_type_declares_member_of_object", 109, 11),
            ("extension_type_declares_member_of_object", 144, 8),
        ],
    );
}

/// `generic_function_type_cannot_be_bound_test.dart` `test_class`.
#[test]
fn generic_function_type_cannot_be_bound_class() {
    assert_errors_in_code(
        r#"
class C<T extends S Function<S>(S)> {
}
"#,
        &[],
    );
}

/// `generic_function_type_cannot_be_bound_test.dart` `test_genericFunction`.
#[test]
fn generic_function_type_cannot_be_bound_generic_function() {
    assert_errors_in_code(
        r#"
late T Function<T extends S Function<S>(S)>(T) fun;
"#,
        &[],
    );
}

/// `generic_function_type_cannot_be_bound_test.dart` `test_genericFunctionTypedef`.
#[test]
fn generic_function_type_cannot_be_bound_generic_function_typedef() {
    assert_errors_in_code(
        r#"
typedef foo = T Function<T extends S Function<S>(S)>(T t);
"#,
        &[],
    );
}

/// `generic_function_type_cannot_be_bound_test.dart` `test_parameterOfFunction`.
#[test]
fn generic_function_type_cannot_be_bound_parameter_of_function() {
    assert_errors_in_code(
        r#"
class C<T extends void Function(S Function<S>(S))> {}
"#,
        &[],
    );
}

/// `generic_function_type_cannot_be_bound_test.dart` `test_typedef`.
#[test]
fn generic_function_type_cannot_be_bound_typedef() {
    assert_errors_in_code(
        r#"
typedef T foo<T extends S Function<S>(S)>(T t);
"#,
        &[],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_class_primaryConstructor`.
#[test]
fn invalid_use_of_covariant_class_primary_constructor() {
    assert_errors_in_code(
        r#"
class A(covariant var int a);
"#,
        &[],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_functionExpression`.
#[test]
fn invalid_use_of_covariant_function_expression() {
    assert_errors_in_code(
        r#"
Function f = (covariant int x) {};
"#,
        &[("invalid_use_of_covariant", 15, 9)],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_functionType_inFunctionTypedParameterOfInstanceMethod`.
#[test]
fn invalid_use_of_covariant_function_type_in_function_typed_parameter_of_instance_method() {
    assert_errors_in_code(
        r#"
class C {
  void m(void p(covariant int)) {}
}
"#,
        &[("invalid_use_of_covariant", 27, 9)],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_functionType_inParameterOfInstanceMethod`.
#[test]
fn invalid_use_of_covariant_function_type_in_parameter_of_instance_method() {
    assert_errors_in_code(
        r#"
class C {
  void m(void Function(covariant int) p) {}
}
"#,
        &[("invalid_use_of_covariant", 34, 9)],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_functionType_inTypeAlias`.
#[test]
fn invalid_use_of_covariant_function_type_in_type_alias() {
    assert_errors_in_code(
        r#"
typedef F = void Function(covariant int);
"#,
        &[("invalid_use_of_covariant", 27, 9)],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_functionType_inTypeArgument`.
#[test]
fn invalid_use_of_covariant_function_type_in_type_argument() {
    assert_errors_in_code(
        r#"
List<void Function(covariant int)> a = [];
}
"#,
        &[
            ("invalid_use_of_covariant", 20, 9),
            ("expected_executable", 44, 1),
        ],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_functionType_inTypeParameterBound`.
#[test]
fn invalid_use_of_covariant_function_type_in_type_parameter_bound() {
    assert_errors_in_code(
        r#"
void foo<T extends void Function(covariant int)>() {}
}
"#,
        &[
            ("invalid_use_of_covariant", 34, 9),
            ("expected_executable", 55, 1),
        ],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_localFunction`.
#[test]
fn invalid_use_of_covariant_local_function() {
    assert_errors_in_code(
        r#"
void foo() {
  void f(covariant int x) {}
}
"#,
        &[
            ("unused_element", 21, 1),
            ("invalid_use_of_covariant", 23, 9),
        ],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_staticFunction`.
#[test]
fn invalid_use_of_covariant_static_function() {
    assert_errors_in_code(
        r#"
class C {
  static void m(covariant int x) {}
}
"#,
        &[("extraneous_modifier", 27, 9)],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_staticFunction_onMixin`.
#[test]
fn invalid_use_of_covariant_static_function_on_mixin() {
    assert_errors_in_code(
        r#"
mixin M {
  static void m(covariant int x) {}
}
"#,
        &[("extraneous_modifier", 27, 9)],
    );
}

/// `invalid_use_of_covariant_test.dart` `test_topLevelFunction`.
#[test]
fn invalid_use_of_covariant_top_level_function() {
    assert_errors_in_code(
        r#"
void f(covariant int x) {}
"#,
        &[("extraneous_modifier", 8, 9)],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalOptional_listOfInt`.
#[test]
fn main_first_positional_parameter_type_positional_optional_list_of_int() {
    assert_errors_in_code(
        r#"
void main([List<int> args = const []]) {}
"#,
        &[("main_first_positional_parameter_type", 12, 9)],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalRequired_dynamic`.
#[test]
fn main_first_positional_parameter_type_positional_required_dynamic() {
    assert_errors_in_code(
        r#"
void main(dynamic args) {}
"#,
        &[],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalRequired_functionTypedFormal`.
#[test]
fn main_first_positional_parameter_type_positional_required_function_typed_formal() {
    assert_errors_in_code(
        r#"
void main(void args()) {}
"#,
        &[("main_first_positional_parameter_type", 11, 4)],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalRequired_iterableOfString`.
#[test]
fn main_first_positional_parameter_type_positional_required_iterable_of_string() {
    assert_errors_in_code(
        r#"
void main(Iterable<String> args) {}
"#,
        &[],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalRequired_listOfInt`.
#[test]
fn main_first_positional_parameter_type_positional_required_list_of_int() {
    assert_errors_in_code(
        r#"
void main(List<int> args) {}
"#,
        &[("main_first_positional_parameter_type", 11, 9)],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalRequired_listOfString`.
#[test]
fn main_first_positional_parameter_type_positional_required_list_of_string() {
    assert_errors_in_code(
        r#"
void main(List<String> args) {}
"#,
        &[],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalRequired_listOfStringQuestion`.
#[test]
fn main_first_positional_parameter_type_positional_required_list_of_string_question() {
    assert_errors_in_code(
        r#"
void main(List<String?> args) {}
"#,
        &[],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalRequired_listQuestionOfString`.
#[test]
fn main_first_positional_parameter_type_positional_required_list_question_of_string() {
    assert_errors_in_code(
        r#"
void main(List<String>? args) {}
"#,
        &[],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalRequired_object`.
#[test]
fn main_first_positional_parameter_type_positional_required_object() {
    assert_errors_in_code(
        r#"
void main(Object args) {}
"#,
        &[],
    );
}

/// `main_first_positional_parameter_type_test.dart` `test_positionalRequired_objectQuestion`.
#[test]
fn main_first_positional_parameter_type_positional_required_object_question() {
    assert_errors_in_code(
        r#"
void main(Object? args) {}
"#,
        &[],
    );
}

/// `main_has_required_named_parameters_test.dart` `test_namedOptional`.
#[test]
fn main_has_required_named_parameters_named_optional() {
    assert_errors_in_code(
        r#"
void main({int a = 0}) {}
"#,
        &[],
    );
}

/// `main_has_required_named_parameters_test.dart` `test_namedRequired`.
#[test]
fn main_has_required_named_parameters_named_required() {
    assert_errors_in_code(
        r#"
void main({required List<String> a}) {}
"#,
        &[("main_has_required_named_parameters", 6, 4)],
    );
}

/// `main_has_too_many_required_positional_parameters_test.dart` `test_namedOptional_1`.
#[test]
fn main_has_too_many_required_positional_parameters_named_optional_1() {
    assert_errors_in_code(
        r#"
void main({int a = 0}) {}
"#,
        &[],
    );
}

/// `main_has_too_many_required_positional_parameters_test.dart` `test_positionalOptional_1`.
#[test]
fn main_has_too_many_required_positional_parameters_positional_optional_1() {
    assert_errors_in_code(
        r#"
void f([int a = 0]) {}
"#,
        &[],
    );
}

/// `main_has_too_many_required_positional_parameters_test.dart` `test_positionalRequired_0`.
#[test]
fn main_has_too_many_required_positional_parameters_positional_required_0() {
    assert_errors_in_code(
        r#"
void main() {}
"#,
        &[],
    );
}

/// `main_has_too_many_required_positional_parameters_test.dart` `test_positionalRequired_1`.
#[test]
fn main_has_too_many_required_positional_parameters_positional_required_1() {
    assert_errors_in_code(
        r#"
void main(args) {}
"#,
        &[],
    );
}

/// `main_has_too_many_required_positional_parameters_test.dart` `test_positionalRequired_2`.
#[test]
fn main_has_too_many_required_positional_parameters_positional_required_2() {
    assert_errors_in_code(
        r#"
void main(args, int a) {}
"#,
        &[],
    );
}

/// `main_has_too_many_required_positional_parameters_test.dart` `test_positionalRequired_2_positionalOptional_1`.
#[test]
fn main_has_too_many_required_positional_parameters_positional_required_2_positional_optional_1() {
    assert_errors_in_code(
        r#"
void main(args, int a, [int b = 0]) {}
"#,
        &[],
    );
}

/// `main_has_too_many_required_positional_parameters_test.dart` `test_positionalRequired_3`.
#[test]
fn main_has_too_many_required_positional_parameters_positional_required_3() {
    assert_errors_in_code(
        r#"
void main(args, int a, int b) {}
"#,
        &[("main_has_too_many_required_positional_parameters", 6, 4)],
    );
}

/// `main_has_too_many_required_positional_parameters_test.dart` `test_positionalRequired_3_namedOptional_1`.
#[test]
fn main_has_too_many_required_positional_parameters_positional_required_3_named_optional_1() {
    assert_errors_in_code(
        r#"
void main(args, int a, int b, {int c = 0}) {}
"#,
        &[("main_has_too_many_required_positional_parameters", 6, 4)],
    );
}

/// `main_has_too_many_required_positional_parameters_test.dart` `test_positionalRequired_3_namedRequired_1`.
#[test]
fn main_has_too_many_required_positional_parameters_positional_required_3_named_required_1() {
    assert_errors_in_code(
        r#"
void main(args, int a, int b, {required int c}) {}
"#,
        &[
            ("main_has_too_many_required_positional_parameters", 6, 4),
            ("main_has_required_named_parameters", 6, 4),
        ],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_closure_nonNullable_named_optional_default`.
#[test]
fn missing_default_value_for_parameter_closure_non_nullable_named_optional_default() {
    assert_errors_in_code(
        r#"
var f = ({int a = 0}) {};
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_closure_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_closure_non_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
var f = ({int a}) {};
"#,
        &[("missing_default_value_for_parameter", 15, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_closure_nonNullable_positional_optional_default`.
#[test]
fn missing_default_value_for_parameter_closure_non_nullable_positional_optional_default() {
    assert_errors_in_code(
        r#"
var f = ([int a = 0]) {};
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_closure_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_closure_non_nullable_positional_optional_no_default() {
    assert_errors_in_code(
        r#"
var f = ([int a]) {};
"#,
        &[("missing_default_value_for_parameter", 15, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_externalFactory_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_external_factory_non_nullable_named_optional_no_default()
 {
    assert_errors_in_code(
        r#"
class C {
  external factory C({int a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_externalFactory_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_external_factory_non_nullable_positional_optional_no_default()
 {
    assert_errors_in_code(
        r#"
class C {
  external factory C([int a]);
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_externalFactory_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_external_factory_nullable_named_optional_no_default()
 {
    assert_errors_in_code(
        r#"
class C {
  external factory C({int? a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_factory_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_factory_non_nullable_named_optional_no_default()
{
    assert_errors_in_code(
        r#"
class C {
  factory C({int a}) => C._();
  C._();
}
"#,
        &[("missing_default_value_for_parameter", 28, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_factory_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_factory_non_nullable_positional_optional_no_default()
 {
    assert_errors_in_code(
        r#"
class C {
  factory C([int a]) => C._();
  C._();
}
"#,
        &[("missing_default_value_for_parameter", 28, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_factory_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_factory_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  factory C({int? a}) => C._();
  C._();
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_named_optional_no_default()
 {
    assert_errors_in_code(
        r#"
class C {
  C({int a});
}
"#,
        &[("missing_default_value_for_parameter", 20, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_named_optional_super_hasDefault_explicit`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_named_optional_super_has_default_explicit()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int a});
}
class B extends A{
  B({super.a = 0});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_named_optional_super_hasDefault_fromSuper`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_named_optional_super_has_default_from_super()
 {
    assert_errors_in_code(
        r#"
class A {
  A({int a = 0});
}
class B extends A{
  B({super.a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_named_optional_super_hasDefault_fromSuper_extensionType`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_named_optional_super_has_default_from_super_extension_type()
 {
    assert_errors_in_code(
        r#"
extension type const E(int it) {}

class A {
  A({E a = const E(0)});
}

class B extends A {
  B({super.a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_named_optional_super_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_named_optional_super_no_default()
 {
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}
class B extends A{
  B({int super.a});
}
"#,
        &[("missing_default_value_for_parameter", 62, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_named_optional_super_noDefault_fromSuper`.
#[test]
#[ignore = "not at parity yet (open point of the D4-D7 port)"]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_named_optional_super_no_default_from_super()
 {
    assert_errors_in_code(
        r#"
class A {
  A({num a = 1.2});
}
class B extends A{
  B({int super.a});
}
"#,
        &[("missing_default_value_for_parameter", 67, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_positional_optional_no_default()
 {
    assert_errors_in_code(
        r#"
class C {
  C([int a]);
}
"#,
        &[("missing_default_value_for_parameter", 20, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_positional_optional_super_hasDefault_explicit`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_positional_optional_super_has_default_explicit()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int a);
}
class B extends A{
  B([super.a = 0]);
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_positional_optional_super_hasDefault_fromSuper`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_positional_optional_super_has_default_from_super()
 {
    assert_errors_in_code(
        r#"
class A {
  A([int a = 0]);
}
class B extends A{
  B([super.a]);
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_positional_optional_super_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_positional_optional_super_no_default()
 {
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}
class B extends A{
  B([int super.a]);
}
"#,
        &[("missing_default_value_for_parameter", 62, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nonNullable_positional_optional_super_noDefault_fromSuper`.
#[test]
#[ignore = "not at parity yet (open point of the D4-D7 port)"]
fn missing_default_value_for_parameter_constructor_generative_non_nullable_positional_optional_super_no_default_from_super()
 {
    assert_errors_in_code(
        r#"
class A {
  A([num a = 1.2]);
}
class B extends A{
  B([int super.a]);
}
"#,
        &[("missing_default_value_for_parameter", 67, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  C({int? a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_nullable_named_optional_noDefault_fieldFormal`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_nullable_named_optional_no_default_field_formal()
 {
    assert_errors_in_code(
        r#"
class C {
  int? f;
  C({this.f});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_super_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_super_non_nullable_named_optional_no_default()
 {
    assert_errors_in_code(
        r#"
class A {
  final int a;
  A({this.a = 0});
}

class B extends A {
  B({required super.a});
}

class C extends B {
  C({super.a});
}
"#,
        &[("missing_default_value_for_parameter", 127, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_generative_super_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_constructor_generative_super_nullable_named_optional_no_default()
 {
    assert_errors_in_code(
        r#"
class A {
  final int? a;
  A({this.a});
}

class B extends A {
  B({required super.a});
}

class C extends B {
  C({super.a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_redirectingFactory_nonNullable_named_optional`.
#[test]
fn missing_default_value_for_parameter_constructor_redirecting_factory_non_nullable_named_optional()
{
    assert_errors_in_code(
        r#"
class A {
  factory A({int a}) = B;
}

class B implements A {
  B({int a = 0});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_redirectingFactory_nonNullable_positional_optional`.
#[test]
fn missing_default_value_for_parameter_constructor_redirecting_factory_non_nullable_positional_optional()
 {
    assert_errors_in_code(
        r#"
class A {
  factory A([int a]) = B;
}

class B implements A {
  B([int a = 0]);
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_constructor_redirectingFactory_nullable_named_optional`.
#[test]
fn missing_default_value_for_parameter_constructor_redirecting_factory_nullable_named_optional() {
    assert_errors_in_code(
        r#"
class A {
  factory A({int? a}) = B;
}

class B implements A {
  B({int? a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_fieldFormalParameter_functionTyped_named_optional`.
#[test]
fn missing_default_value_for_parameter_field_formal_parameter_function_typed_named_optional() {
    assert_errors_in_code(
        r#"
class A {
  dynamic f;
  A(void this.f({int a, int? b}));
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_fieldFormalParameter_functionTyped_positional_optional`.
#[test]
fn missing_default_value_for_parameter_field_formal_parameter_function_typed_positional_optional() {
    assert_errors_in_code(
        r#"
class A {
  dynamic f;
  A(void this.f([int a, int? b]));
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_external_nonNullable_named_optional_default`.
#[test]
fn missing_default_value_for_parameter_function_external_non_nullable_named_optional_default() {
    assert_errors_in_code(
        r#"
external void f({int a = 0});
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_external_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_external_non_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
external void f({int a});
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_external_nonNullable_named_required_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_external_non_nullable_named_required_no_default() {
    assert_errors_in_code(
        r#"
external void f({required int a});
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_external_nonNullable_positional_optional_default`.
#[test]
fn missing_default_value_for_parameter_function_external_non_nullable_positional_optional_default()
{
    assert_errors_in_code(
        r#"
external void f([int a = 0]);
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_external_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_external_non_nullable_positional_optional_no_default()
 {
    assert_errors_in_code(
        r#"
external void f([int a]);
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_external_nonNullable_positional_required_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_external_non_nullable_positional_required_no_default()
 {
    assert_errors_in_code(
        r#"
external void f(int a);
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_external_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_external_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
external void f({int? a});
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_external_nullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_external_nullable_positional_optional_no_default() {
    assert_errors_in_code(
        r#"
external void f([int? a]);
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_native_nonNullable_named_optional_default`.
#[test]
fn missing_default_value_for_parameter_function_native_non_nullable_named_optional_default() {
    assert_errors_in_code(
        r#"
void f({int a = 0}) native;
"#,
        &[("native_function_body_in_non_sdk_code", 21, 7)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_native_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_native_non_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
void f({int a}) native;
"#,
        &[("native_function_body_in_non_sdk_code", 17, 7)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_native_nonNullable_positional_optional_default`.
#[test]
fn missing_default_value_for_parameter_function_native_non_nullable_positional_optional_default() {
    assert_errors_in_code(
        r#"
void f([int a = 0]) native;
"#,
        &[("native_function_body_in_non_sdk_code", 21, 7)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_native_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_native_non_nullable_positional_optional_no_default()
{
    assert_errors_in_code(
        r#"
void f([int a]) native;
"#,
        &[("native_function_body_in_non_sdk_code", 17, 7)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_named_optional_default`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_named_optional_default() {
    assert_errors_in_code(
        r#"
void f({int a = 0}) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
void f({int a}) {}
"#,
        &[("missing_default_value_for_parameter", 13, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_named_required`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_named_required() {
    assert_errors_in_code(
        r#"
void f({required int a}) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_positional_optional_default`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_positional_optional_default() {
    assert_errors_in_code(
        r#"
void f([int a = 0]) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_positional_optional_default_augmentation`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_positional_optional_default_augmentation()
 {
    assert_errors_in_code(
        r#"
void f([int a]);

augment void f([int a = 0]) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_positional_optional_default_introduction`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_positional_optional_default_introduction()
 {
    assert_errors_in_code(
        r#"
void f([int a = 0]);

augment void f([int a]) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_positional_optional_default_middleAugmentation`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_positional_optional_default_middle_augmentation()
 {
    assert_errors_in_code(
        r#"
void f([int a]);

augment void f([int a = 0]);

augment void f([int a]) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_positional_optional_no_default() {
    assert_errors_in_code(
        r#"
void f([int a]) {}
"#,
        &[("missing_default_value_for_parameter", 13, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_positional_optional_noDefault_augmentation`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_positional_optional_no_default_augmentation()
 {
    assert_errors_in_code(
        r#"
void f([int a]);

augment void f([int a]) {}
"#,
        &[
            ("missing_default_value_for_parameter", 13, 1),
            ("missing_default_value_for_parameter", 39, 1),
        ],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nonNullable_positional_required`.
#[test]
fn missing_default_value_for_parameter_function_non_nullable_positional_required() {
    assert_errors_in_code(
        r#"
void f(int a) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nullable_named_optional_default`.
#[test]
fn missing_default_value_for_parameter_function_nullable_named_optional_default() {
    assert_errors_in_code(
        r#"
void f({int? a = 0}) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
void f({int? a}) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nullable_named_required`.
#[test]
fn missing_default_value_for_parameter_function_nullable_named_required() {
    assert_errors_in_code(
        r#"
void f({required int? a}) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nullable_positional_optional_default`.
#[test]
fn missing_default_value_for_parameter_function_nullable_positional_optional_default() {
    assert_errors_in_code(
        r#"
void f([int? a = 0]) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_function_nullable_positional_optional_no_default() {
    assert_errors_in_code(
        r#"
void f([int? a]) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_function_nullable_positional_required`.
#[test]
fn missing_default_value_for_parameter_function_nullable_positional_required() {
    assert_errors_in_code(
        r#"
void f(int? a) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_functionTypeAlias_named_optional`.
#[test]
fn missing_default_value_for_parameter_function_type_alias_named_optional() {
    assert_errors_in_code(
        r#"
typedef void F({int a, int? b});
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_functionTypeAlias_positional_optional`.
#[test]
fn missing_default_value_for_parameter_function_type_alias_positional_optional() {
    assert_errors_in_code(
        r#"
typedef void F([int a, int? b]);
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_functionTypedFormalParameter_named_optional`.
#[test]
fn missing_default_value_for_parameter_function_typed_formal_parameter_named_optional() {
    assert_errors_in_code(
        r#"
void f(void p({int a, int? b})) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_functionTypedFormalParameter_positional_optional`.
#[test]
fn missing_default_value_for_parameter_function_typed_formal_parameter_positional_optional() {
    assert_errors_in_code(
        r#"
void f(void p([int a, int? b])) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_genericFunctionType_named_optional`.
#[test]
fn missing_default_value_for_parameter_generic_function_type_named_optional() {
    assert_errors_in_code(
        r#"
void f(void Function({int a, int? b}) p) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_genericFunctionType_positional_optional`.
#[test]
fn missing_default_value_for_parameter_generic_function_type_positional_optional() {
    assert_errors_in_code(
        r#"
void f(void Function([int a, int? b]) p) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_genericFunctionType_positional_optional2`.
#[test]
fn missing_default_value_for_parameter_generic_function_type_positional_optional2() {
    assert_errors_in_code(
        r#"
void f(void Function([int, int?]) p) {}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_abstract_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_abstract_non_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
abstract class C {
  void foo({int a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_abstract_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_abstract_non_nullable_positional_optional_no_default()
{
    assert_errors_in_code(
        r#"
abstract class C {
  void foo([int a]);
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_abstract_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_abstract_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
abstract class C {
  void foo({int? a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_abstract_potentiallyNonNullable_named_optional`.
#[test]
fn missing_default_value_for_parameter_method_abstract_potentially_non_nullable_named_optional() {
    assert_errors_in_code(
        r#"
abstract class A<T> {
  void foo({T a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_abstract_potentiallyNonNullable_positional_optional`.
#[test]
fn missing_default_value_for_parameter_method_abstract_potentially_non_nullable_positional_optional()
 {
    assert_errors_in_code(
        r#"
abstract class A<T extends Object?> {
  void foo([T a]);
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_external_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_external_non_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  external void foo({int a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_external_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_external_non_nullable_positional_optional_no_default()
{
    assert_errors_in_code(
        r#"
class C {
  external void foo([int a]);
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_external_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_external_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  external void foo({int? a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_external_potentiallyNonNullable_named_optional`.
#[test]
fn missing_default_value_for_parameter_method_external_potentially_non_nullable_named_optional() {
    assert_errors_in_code(
        r#"
class A<T> {
  external void foo({T a});
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_external_potentiallyNonNullable_positional_optional`.
#[test]
fn missing_default_value_for_parameter_method_external_potentially_non_nullable_positional_optional()
 {
    assert_errors_in_code(
        r#"
class A<T extends Object?> {
  external void foo([T a]);
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_native_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_native_non_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  void foo({int a}) native;
}
"#,
        &[("native_function_body_in_non_sdk_code", 31, 7)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_native_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_native_non_nullable_positional_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  void foo([int a]) native;
}
"#,
        &[("native_function_body_in_non_sdk_code", 31, 7)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_native_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_native_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  void foo({int? a}) native;
}
"#,
        &[("native_function_body_in_non_sdk_code", 32, 7)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_native_potentiallyNonNullable_named_optional`.
#[test]
fn missing_default_value_for_parameter_method_native_potentially_non_nullable_named_optional() {
    assert_errors_in_code(
        r#"
class A<T> {
  void foo({T a}) native;
}
"#,
        &[("native_function_body_in_non_sdk_code", 32, 7)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_native_potentiallyNonNullable_positional_optional`.
#[test]
fn missing_default_value_for_parameter_method_native_potentially_non_nullable_positional_optional()
{
    assert_errors_in_code(
        r#"
class A<T extends Object?> {
  void foo([T a]) native;
}
"#,
        &[("native_function_body_in_non_sdk_code", 48, 7)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_nonNullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_non_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  void foo({int a}) {}
}
"#,
        &[("missing_default_value_for_parameter", 27, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_nonNullable_positional_optional_default_augmentation`.
#[test]
fn missing_default_value_for_parameter_method_non_nullable_positional_optional_default_augmentation()
 {
    assert_errors_in_code(
        r#"
class C {
  void foo([int a]);

  augment void foo([int a = 0]) {}
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_nonNullable_positional_optional_default_introduction`.
#[test]
fn missing_default_value_for_parameter_method_non_nullable_positional_optional_default_introduction()
 {
    assert_errors_in_code(
        r#"
class C {
  void foo([int a = 0]);

  augment void foo([int a]) {}
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_nonNullable_positional_optional_default_middleAugmentation`.
#[test]
fn missing_default_value_for_parameter_method_non_nullable_positional_optional_default_middle_augmentation()
 {
    assert_errors_in_code(
        r#"
class C {
  void foo([int a]);

  augment void foo([int a = 0]);

  augment void foo([int a]) {}
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_nonNullable_positional_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_non_nullable_positional_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  void foo([int a]) {}
}
"#,
        &[("missing_default_value_for_parameter", 27, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_nonNullable_positional_optional_noDefault_augmentation`.
#[test]
fn missing_default_value_for_parameter_method_non_nullable_positional_optional_no_default_augmentation()
 {
    assert_errors_in_code(
        r#"
class C {
  void foo([int a]);

  augment void foo([int a]) {}
}
"#,
        &[("missing_default_value_for_parameter", 57, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_nullable_named_optional_noDefault`.
#[test]
fn missing_default_value_for_parameter_method_nullable_named_optional_no_default() {
    assert_errors_in_code(
        r#"
class C {
  void foo({int? a}) {}
}
"#,
        &[],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_potentiallyNonNullable_named_optional`.
#[test]
fn missing_default_value_for_parameter_method_potentially_non_nullable_named_optional() {
    assert_errors_in_code(
        r#"
class A<T extends Object?> {
  void foo({T a}) {}
}
"#,
        &[("missing_default_value_for_parameter", 44, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_method_potentiallyNonNullable_positional_optional`.
#[test]
fn missing_default_value_for_parameter_method_potentially_non_nullable_positional_optional() {
    assert_errors_in_code(
        r#"
class A<T extends Object?> {
  void foo([T a]) {}
}
"#,
        &[("missing_default_value_for_parameter", 44, 1)],
    );
}

/// `missing_default_value_for_parameter_test.dart` `test_super_forward_wildcards`.
#[test]
fn missing_default_value_for_parameter_super_forward_wildcards() {
    assert_errors_in_code(
        r#"
class A {
  final int x, y;
  A(this.x, [this.y = 0]);
}

class C extends A {
  final int c;
  C(this.c, super._, [super._]);
}
"#,
        &[],
    );
}

/// `non_void_return_for_operator_test.dart` `test_indexSetter`.
#[test]
fn non_void_return_for_operator_index_setter() {
    assert_errors_in_code(
        r#"
class A {
  int operator []=(a, b) { return a; }
}"#,
        &[("non_void_return_for_operator", 13, 3)],
    );
}

/// `non_void_return_for_operator_test.dart` `test_no_return`.
#[test]
fn non_void_return_for_operator_no_return() {
    assert_errors_in_code(
        r#"
class A {
  operator []=(a, b) {}
}
"#,
        &[],
    );
}

/// `non_void_return_for_operator_test.dart` `test_void`.
#[test]
fn non_void_return_for_operator_void() {
    assert_errors_in_code(
        r#"
class A {
  void operator []=(a, b) {}
}
"#,
        &[],
    );
}

/// `non_void_return_for_setter_test.dart` `test_function`.
#[test]
fn non_void_return_for_setter_function() {
    assert_errors_in_code(
        r#"
int set x(int v) {
  return 42;
}"#,
        &[("non_void_return_for_setter", 1, 3)],
    );
}

/// `non_void_return_for_setter_test.dart` `test_function_no_return`.
#[test]
fn non_void_return_for_setter_function_no_return() {
    assert_errors_in_code(
        r#"
set x(v) {}
"#,
        &[],
    );
}

/// `non_void_return_for_setter_test.dart` `test_function_void`.
#[test]
fn non_void_return_for_setter_function_void() {
    assert_errors_in_code(
        r#"
void set x(v) {}
"#,
        &[],
    );
}

/// `non_void_return_for_setter_test.dart` `test_method_no_return`.
#[test]
fn non_void_return_for_setter_method_no_return() {
    assert_errors_in_code(
        r#"
class A {
  set x(v) {}
}
"#,
        &[],
    );
}

/// `non_void_return_for_setter_test.dart` `test_method_void`.
#[test]
fn non_void_return_for_setter_method_void() {
    assert_errors_in_code(
        r#"
class A {
  void set x(v) {}
}
"#,
        &[],
    );
}

/// `optional_parameter_in_operator_test.dart` `test_optionalNamed`.
#[test]
fn optional_parameter_in_operator_optional_named() {
    assert_errors_in_code(
        r#"
class A {
  int operator +({Object? other}) => 0;
}
"#,
        &[("optional_parameter_in_operator", 29, 13)],
    );
}

/// `optional_parameter_in_operator_test.dart` `test_optionalPositional`.
#[test]
fn optional_parameter_in_operator_optional_positional() {
    assert_errors_in_code(
        r#"
class A {
  int operator +([Object? other]) => 0;
}
"#,
        &[("optional_parameter_in_operator", 29, 13)],
    );
}

/// `optional_parameter_in_operator_test.dart` `test_requiredNamed`.
#[test]
fn optional_parameter_in_operator_required_named() {
    assert_errors_in_code(
        r#"
class A {
  int operator +({required Object other}) => 0;
}
"#,
        &[("optional_parameter_in_operator", 29, 21)],
    );
}

/// `optional_parameter_in_operator_test.dart` `test_requiredPositional`.
#[test]
fn optional_parameter_in_operator_required_positional() {
    assert_errors_in_code(
        r#"
class A {
  int operator +(Object other) => 0;
}
"#,
        &[],
    );
}

/// `type_parameter_referenced_by_static_test.dart` `test_class_field`.
#[test]
fn type_parameter_referenced_by_static_class_field() {
    assert_errors_in_code(
        r#"
class A<T> {
  static T? foo;
}
"#,
        &[("type_parameter_referenced_by_static", 23, 1)],
    );
}

/// `type_parameter_referenced_by_static_test.dart` `test_class_getter`.
#[test]
fn type_parameter_referenced_by_static_class_getter() {
    assert_errors_in_code(
        r#"
class A<T> {
  static T? get foo => null;
}
"#,
        &[("type_parameter_referenced_by_static", 23, 1)],
    );
}

/// `type_parameter_referenced_by_static_test.dart` `test_class_method_parameter`.
#[test]
fn type_parameter_referenced_by_static_class_method_parameter() {
    assert_errors_in_code(
        r#"
class A<T> {
  static foo(T a) {}
}
"#,
        &[("type_parameter_referenced_by_static", 27, 1)],
    );
}

/// `type_parameter_referenced_by_static_test.dart` `test_class_setter`.
#[test]
fn type_parameter_referenced_by_static_class_setter() {
    assert_errors_in_code(
        r#"
class A<T> {
  static set foo(T _) {}
}
"#,
        &[("type_parameter_referenced_by_static", 31, 1)],
    );
}

/// `type_parameter_referenced_by_static_test.dart` `test_extension_field`.
#[test]
fn type_parameter_referenced_by_static_extension_field() {
    assert_errors_in_code(
        r#"
extension E<T> on int {
  static T? foo;
}
"#,
        &[("type_parameter_referenced_by_static", 34, 1)],
    );
}

/// `type_parameter_referenced_by_static_test.dart` `test_extension_method_return`.
#[test]
fn type_parameter_referenced_by_static_extension_method_return() {
    assert_errors_in_code(
        r#"
extension E<T> on int {
  static T foo() => throw 0;
}
"#,
        &[("type_parameter_referenced_by_static", 34, 1)],
    );
}

/// `type_parameter_referenced_by_static_test.dart` `test_mixin_field`.
#[test]
fn type_parameter_referenced_by_static_mixin_field() {
    assert_errors_in_code(
        r#"
mixin A<T> {
  static T? foo;
}
"#,
        &[("type_parameter_referenced_by_static", 23, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_ampersand_none`.
#[test]
fn wrong_number_of_parameters_for_operator_ampersand_none() {
    assert_errors_in_code(
        r#"
class A {
  operator &() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_ampersand_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_ampersand_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator &(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_ampersand_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_ampersand_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator &(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_ampersand_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_ampersand_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator &(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_ampersand_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_ampersand_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator &(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_caret_none`.
#[test]
fn wrong_number_of_parameters_for_operator_caret_none() {
    assert_errors_in_code(
        r#"
class A {
  operator ^() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_caret_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_caret_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator ^(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_caret_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_caret_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator ^(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_caret_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_caret_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator ^(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_caret_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_caret_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator ^(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greater_none`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_none() {
    assert_errors_in_code(
        r#"
class A {
  operator >() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greater_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator >(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greater_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator >(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greater_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator >(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greater_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator >(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greaterEqual_none`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_equal_none() {
    assert_errors_in_code(
        r#"
class A {
  operator >=() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greaterEqual_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_equal_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator >=(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greaterEqual_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_equal_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator >=(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greaterEqual_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_equal_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator >=(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_greaterEqual_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_greater_equal_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator >=(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_index_none`.
#[test]
fn wrong_number_of_parameters_for_operator_index_none() {
    assert_errors_in_code(
        r#"
class A {
  operator []() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_index_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_index_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator [](a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_index_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_index_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator [](a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_index_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_index_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator [](a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_index_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_index_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator [](a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_indexEq_none`.
#[test]
fn wrong_number_of_parameters_for_operator_index_eq_none() {
    assert_errors_in_code(
        r#"
class A {
  operator []=() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 3)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_indexEq_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_index_eq_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator []=(a) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 3)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_indexEq_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_index_eq_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator []=(a, b) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_indexEq_rP_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_index_eq_r_p_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator []=(a, b, {c}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 3)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_indexEq_rP_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_index_eq_r_p_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator []=(a, b, c) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 3)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_indexEq_rP_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_index_eq_r_p_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator []=(a, b, [c]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 3)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_less_none`.
#[test]
fn wrong_number_of_parameters_for_operator_less_none() {
    assert_errors_in_code(
        r#"
class A {
  operator <() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_less_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_less_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator <(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_less_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_less_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator <(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_less_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_less_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator <(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_less_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_less_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator <(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_lessEqual_none`.
#[test]
fn wrong_number_of_parameters_for_operator_less_equal_none() {
    assert_errors_in_code(
        r#"
class A {
  operator <=() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_lessEqual_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_less_equal_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator <=(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_lessEqual_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_less_equal_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator <=(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_lessEqual_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_less_equal_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator <=(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_lessEqual_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_less_equal_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator <=(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_minus_none`.
#[test]
fn wrong_number_of_parameters_for_operator_minus_none() {
    assert_errors_in_code(
        r#"
class A {
  operator -() {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_minus_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_minus_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator -(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_minus_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_minus_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator -(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_percent_none`.
#[test]
fn wrong_number_of_parameters_for_operator_percent_none() {
    assert_errors_in_code(
        r#"
class A {
  operator %() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_percent_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_percent_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator %(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_percent_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_percent_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator %(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_percent_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_percent_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator %(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_percent_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_percent_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator %(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_pipe_none`.
#[test]
fn wrong_number_of_parameters_for_operator_pipe_none() {
    assert_errors_in_code(
        r#"
class A {
  operator |() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_pipe_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_pipe_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator |(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_pipe_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_pipe_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator |(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_pipe_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_pipe_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator |(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_pipe_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_pipe_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator |(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_plus_none`.
#[test]
fn wrong_number_of_parameters_for_operator_plus_none() {
    assert_errors_in_code(
        r#"
class A {
  operator +() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_plus_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_plus_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator +(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_plus_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_plus_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator +(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_plus_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_plus_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator +(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_plus_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_plus_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator +(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftLeft_none`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_left_none() {
    assert_errors_in_code(
        r#"
class A {
  operator <<() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftLeft_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_left_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator <<(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftLeft_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_left_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator <<(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftLeft_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_left_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator <<(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftLeft_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_left_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator <<(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftRight_none`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_right_none() {
    assert_errors_in_code(
        r#"
class A {
  operator >>() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftRight_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_right_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator >>(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftRight_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_right_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator >>(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftRight_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_right_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator >>(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_shiftRight_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_shift_right_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator >>(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_slash_none`.
#[test]
fn wrong_number_of_parameters_for_operator_slash_none() {
    assert_errors_in_code(
        r#"
class A {
  operator /() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_slash_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_slash_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator /(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_slash_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_slash_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator /(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_slash_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_slash_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator /(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_slash_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_slash_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator /(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_star_none`.
#[test]
fn wrong_number_of_parameters_for_operator_star_none() {
    assert_errors_in_code(
        r#"
class A {
  operator *() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_star_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_star_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator *(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_star_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_star_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator *(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_star_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_star_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator *(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_star_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_star_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator *(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tilde_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_tilde_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator ~(a) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tilde_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_tilde_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator ~(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tilde_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_tilde_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator ~(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tilde_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_tilde_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator ~(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 1)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tildeSlash_none`.
#[test]
fn wrong_number_of_parameters_for_operator_tilde_slash_none() {
    assert_errors_in_code(
        r#"
class A {
  operator ~/() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tildeSlash_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_tilde_slash_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator ~/(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tildeSlash_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_tilde_slash_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator ~/(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tildeSlash_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_tilde_slash_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator ~/(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tildeSlash_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_tilde_slash_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator ~/(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 2)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tripleShiftRight_none`.
#[test]
fn wrong_number_of_parameters_for_operator_triple_shift_right_none() {
    assert_errors_in_code(
        r#"
class A {
  operator >>>() {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 3)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tripleShiftRight_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_triple_shift_right_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator >>>(a) {}
}
"#,
        &[],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tripleShiftRight_rP_rn`.
#[test]
fn wrong_number_of_parameters_for_operator_triple_shift_right_r_p_rn() {
    assert_errors_in_code(
        r#"
class A {
  operator >>>(a, {b}) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 3)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tripleShiftRight_rP_rP`.
#[test]
fn wrong_number_of_parameters_for_operator_triple_shift_right_r_p_r_p() {
    assert_errors_in_code(
        r#"
class A {
  operator >>>(a, b) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 3)],
    );
}

/// `wrong_number_of_parameters_for_operator_test.dart` `test_tripleShiftRight_rP_rp`.
#[test]
fn wrong_number_of_parameters_for_operator_triple_shift_right_r_p_rp() {
    assert_errors_in_code(
        r#"
class A {
  operator >>>(a, [b]) {}
}
"#,
        &[("wrong_number_of_parameters_for_operator", 22, 3)],
    );
}
