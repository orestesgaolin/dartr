//! ErrorVerifier diagnostic tests of section D6, generated from
//! `pkg/analyzer/test/src/diagnostics/<code>_test.dart` (the tests with one
//! library and no other setup) by converting the inline diagnostic
//! markers or the `error(diag.x, offset, length)` lists. Only the codes
//! of `tools/difftest/error_verifier_codes.txt` are compared.

mod ev_support;
mod support;

use ev_support::assert_errors_in_code;

/// `abstract_field_initializer_test.dart` `test_abstract_field_final_initializer`.
#[test]
fn abstract_field_initializer_abstract_field_final_initializer() {
    assert_errors_in_code(
        r#"
abstract class A {
  abstract final int x = 0;
}
"#,
        &[("abstract_field_initializer", 41, 1)],
    );
}

/// `abstract_field_initializer_test.dart` `test_abstract_field_final_no_initializer`.
#[test]
fn abstract_field_initializer_abstract_field_final_no_initializer() {
    assert_errors_in_code(
        r#"
abstract class A {
  abstract final int x;
}
"#,
        &[],
    );
}

/// `abstract_field_initializer_test.dart` `test_abstract_field_initializer`.
#[test]
fn abstract_field_initializer_abstract_field_initializer() {
    assert_errors_in_code(
        r#"
abstract class A {
  abstract int x = 0;
}
"#,
        &[("abstract_field_initializer", 35, 1)],
    );
}

/// `abstract_field_initializer_test.dart` `test_abstract_field_no_initializer`.
#[test]
fn abstract_field_initializer_abstract_field_no_initializer() {
    assert_errors_in_code(
        r#"
abstract class A {
  abstract int x;
}
"#,
        &[],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_class_primary_assertBeforeRedirection`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assert_in_redirecting_constructor_class_primary_assert_before_redirection() {
    assert_errors_in_code(
        r#"
class A(int x) {
  A.named() : this(0);
  this : assert(x > 0), this.named();
}
"#,
        &[("primary_constructor_cannot_redirect", 65, 4)],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_class_primary_redirectionBeforeAssert`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assert_in_redirecting_constructor_class_primary_redirection_before_assert() {
    assert_errors_in_code(
        r#"
class A(int x) {
  A.named() : this(0);
  this : this.named(), assert(x > 0);
}
"#,
        &[("primary_constructor_cannot_redirect", 50, 4)],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_class_typeName_assertBeforeRedirection`.
#[test]
fn assert_in_redirecting_constructor_class_type_name_assert_before_redirection() {
    assert_errors_in_code(
        r#"
class A {
  A(int x) : assert(x > 0), this.name();
  A.name() {}
}
"#,
        &[("assert_in_redirecting_constructor", 24, 13)],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_class_typeName_justAssert`.
#[test]
fn assert_in_redirecting_constructor_class_type_name_just_assert() {
    assert_errors_in_code(
        r#"
class A {
  A(int x) : assert(x > 0);
  A.name() {}
}
"#,
        &[],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_class_typeName_justRedirection`.
#[test]
fn assert_in_redirecting_constructor_class_type_name_just_redirection() {
    assert_errors_in_code(
        r#"
class A {
  A(int x) : this.name();
  A.name() {}
}
"#,
        &[],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_class_typeName_redirectionBeforeAssert`.
#[test]
fn assert_in_redirecting_constructor_class_type_name_redirection_before_assert() {
    assert_errors_in_code(
        r#"
class A {
  A(int x) : this.name(), assert(x > 0);
  A.name() {}
}
"#,
        &[("assert_in_redirecting_constructor", 37, 13)],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_enum_primary_assertBeforeRedirection`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assert_in_redirecting_constructor_enum_primary_assert_before_redirection() {
    assert_errors_in_code(
        r#"
enum E(int x) {
  v(0);
  const E.named() : this(0);
  this : assert(x > -1), this.named();
}
"#,
        &[
            ("recursive_constant_constructor", 33, 7),
            ("primary_constructor_cannot_redirect", 79, 4),
        ],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_enum_primary_redirectionBeforeAssert`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assert_in_redirecting_constructor_enum_primary_redirection_before_assert() {
    assert_errors_in_code(
        r#"
enum E(int x) {
  v(0);
  const E.named() : this(0);
  this : this.named(), assert(x > -1);
}
"#,
        &[
            ("recursive_constant_constructor", 33, 7),
            ("primary_constructor_cannot_redirect", 63, 4),
        ],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_enum_redirectionBeforeAssert`.
#[test]
fn assert_in_redirecting_constructor_enum_redirection_before_assert() {
    assert_errors_in_code(
        r#"
enum E {
  v(42);
  const E(int x) : this.name(), assert(x > 0);
  const E.name();
}
"#,
        &[("assert_in_redirecting_constructor", 51, 13)],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_enum_typeName_assertBeforeRedirection`.
#[test]
fn assert_in_redirecting_constructor_enum_type_name_assert_before_redirection() {
    assert_errors_in_code(
        r#"
enum E {
  v(42);
  const E(int x) : assert(x > 0), this.name();
  const E.name();
}
"#,
        &[("assert_in_redirecting_constructor", 38, 13)],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_enum_typeName_justAssert`.
#[test]
fn assert_in_redirecting_constructor_enum_type_name_just_assert() {
    assert_errors_in_code(
        r#"
enum E {
  v(42);
  const E(int x) : assert(x > 0);
}
"#,
        &[],
    );
}

/// `assert_in_redirecting_constructor_test.dart` `test_enum_typeName_justRedirection`.
#[test]
fn assert_in_redirecting_constructor_enum_type_name_just_redirection() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  const E(int x) : this.name();
  const E.name();
}
"#,
        &[],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_class_instanceField_int_int`.
#[test]
fn augmentation_return_type_mismatch_class_instance_field_int_int() {
    assert_errors_in_code(
        r#"
class A {
  int? foo;
}

augment class A {
  augment abstract int? foo;
}
"#,
        &[],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_class_instanceField_int_String`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_return_type_mismatch_class_instance_field_int_string() {
    assert_errors_in_code(
        r#"
class A {
  int? foo;
}

augment class A {
  augment abstract String? foo;
}
"#,
        &[("augmentation_return_type_mismatch", 71, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_class_instanceField_multiple_oneMismatch`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_return_type_mismatch_class_instance_field_multiple_one_mismatch() {
    assert_errors_in_code(
        r#"
class A {
  String? foo;
  int? bar;
}

augment class A {
  augment abstract String? foo, bar;
}
"#,
        &[("augmentation_return_type_mismatch", 91, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_class_instanceGetter_instanceField_int_String`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_return_type_mismatch_class_instance_getter_instance_field_int_string() {
    assert_errors_in_code(
        r#"
class A {
  int? get foo => 0;
}

augment class A {
  augment abstract final String? foo;
}
"#,
        &[("augmentation_return_type_mismatch", 86, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_class_instanceGetter_int_String`.
#[test]
fn augmentation_return_type_mismatch_class_instance_getter_int_string() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}

augment class A {
  augment String get foo;
}
"#,
        &[("augmentation_return_type_mismatch", 62, 6)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_class_instanceMethod_void_int`.
#[test]
fn augmentation_return_type_mismatch_class_instance_method_void_int() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

augment class A {
  augment int foo();
}
"#,
        &[("augmentation_return_type_mismatch", 58, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_class_instanceMethod_void_void`.
#[test]
fn augmentation_return_type_mismatch_class_instance_method_void_void() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

augment class A {
  augment void foo();
}
"#,
        &[],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_class_staticField_int_int`.
#[test]
fn augmentation_return_type_mismatch_class_static_field_int_int() {
    assert_errors_in_code(
        r#"
class A {
  static int? foo;
}

augment class A {
  augment static abstract int? foo;
}
"#,
        &[],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_class_staticField_int_String`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_return_type_mismatch_class_static_field_int_string() {
    assert_errors_in_code(
        r#"
class A {
  static int? foo;
}

augment class A {
  augment static abstract String? foo;
}
"#,
        &[("augmentation_return_type_mismatch", 85, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_extension_instanceGetter_int_String`.
#[test]
fn augmentation_return_type_mismatch_extension_instance_getter_int_string() {
    assert_errors_in_code(
        r#"
extension E on int {
  int get foo => 0;
}

augment extension E {
  augment String get foo;
}
"#,
        &[("augmentation_return_type_mismatch", 77, 6)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_extension_instanceMethod_void_int`.
#[test]
fn augmentation_return_type_mismatch_extension_instance_method_void_int() {
    assert_errors_in_code(
        r#"
extension E on int {
  void foo() {}
}

augment extension E {
  augment int foo();
}
"#,
        &[("augmentation_return_type_mismatch", 73, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_extensionType_instanceGetter_int_String`.
#[test]
fn augmentation_return_type_mismatch_extension_type_instance_getter_int_string() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  int get foo => 0;
}

augment extension type A {
  augment String get foo;
}
"#,
        &[("augmentation_return_type_mismatch", 88, 6)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_extensionType_instanceMethod_void_int`.
#[test]
fn augmentation_return_type_mismatch_extension_type_instance_method_void_int() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  void foo() {}
}

augment extension type A {
  augment int foo();
}
"#,
        &[("augmentation_return_type_mismatch", 84, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_mixin_instanceGetter_int_String`.
#[test]
fn augmentation_return_type_mismatch_mixin_instance_getter_int_string() {
    assert_errors_in_code(
        r#"
mixin M {
  int get foo => 0;
}

augment mixin M {
  augment String get foo;
}
"#,
        &[("augmentation_return_type_mismatch", 62, 6)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_mixin_instanceMethod_void_int`.
#[test]
fn augmentation_return_type_mismatch_mixin_instance_method_void_int() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}

augment mixin M {
  augment int foo();
}
"#,
        &[("augmentation_return_type_mismatch", 58, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelFunction_dynamic_objectQuestion`.
#[test]
fn augmentation_return_type_mismatch_top_level_function_dynamic_object_question() {
    assert_errors_in_code(
        r#"
dynamic foo() => null;

augment Object? foo();
"#,
        &[("augmentation_return_type_mismatch", 33, 7)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelFunction_int_int_withImportPrefix`.
#[test]
fn augmentation_return_type_mismatch_top_level_function_int_int_with_import_prefix() {
    assert_errors_in_code(
        r#"
import 'dart:core';
import 'dart:core' as core;

int foo() => 0;

augment core.int foo();
"#,
        &[],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelFunction_objectQuestion_dynamic`.
#[test]
fn augmentation_return_type_mismatch_top_level_function_object_question_dynamic() {
    assert_errors_in_code(
        r#"
Object? foo() => null;

augment dynamic foo();
"#,
        &[("augmentation_return_type_mismatch", 33, 7)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelFunction_void_int`.
#[test]
fn augmentation_return_type_mismatch_top_level_function_void_int() {
    assert_errors_in_code(
        r#"
void foo() {}

augment int foo();
"#,
        &[("augmentation_return_type_mismatch", 24, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelFunction_void_int_viaTypeAlias`.
#[test]
fn augmentation_return_type_mismatch_top_level_function_void_int_via_type_alias() {
    assert_errors_in_code(
        r#"
typedef IntAlias = int;

void foo() {}

augment IntAlias foo();
"#,
        &[("augmentation_return_type_mismatch", 49, 8)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelFunction_void_int_withImportPrefix`.
#[test]
fn augmentation_return_type_mismatch_top_level_function_void_int_with_import_prefix() {
    assert_errors_in_code(
        r#"
import 'dart:core' as core;
void foo() {}

augment core.int foo();
"#,
        &[("augmentation_return_type_mismatch", 52, 8)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelFunction_void_nothing`.
#[test]
fn augmentation_return_type_mismatch_top_level_function_void_nothing() {
    assert_errors_in_code(
        r#"
void foo() {}

augment foo();
"#,
        &[],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelFunction_void_void`.
#[test]
fn augmentation_return_type_mismatch_top_level_function_void_void() {
    assert_errors_in_code(
        r#"
void foo() {}

augment void foo();
"#,
        &[],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelFunction_void_void_viaTypeAlias`.
#[test]
fn augmentation_return_type_mismatch_top_level_function_void_void_via_type_alias() {
    assert_errors_in_code(
        r#"
typedef VoidAlias = void;

void foo() {}

augment VoidAlias foo();
"#,
        &[],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelGetter_int_String`.
#[test]
fn augmentation_return_type_mismatch_top_level_getter_int_string() {
    assert_errors_in_code(
        r#"
int get foo => 0;

augment String get foo;
"#,
        &[("augmentation_return_type_mismatch", 28, 6)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelGetter_topLevelVariable_int_String`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_return_type_mismatch_top_level_getter_top_level_variable_int_string() {
    assert_errors_in_code(
        r#"
int? get foo => 0;

augment abstract final String? foo;
"#,
        &[("augmentation_return_type_mismatch", 52, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelVariable_int_int`.
#[test]
fn augmentation_return_type_mismatch_top_level_variable_int_int() {
    assert_errors_in_code(
        r#"
int? foo;

augment abstract int? foo;
"#,
        &[],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelVariable_int_String`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_return_type_mismatch_top_level_variable_int_string() {
    assert_errors_in_code(
        r#"
int? foo;

augment abstract String? foo;
"#,
        &[("augmentation_return_type_mismatch", 37, 3)],
    );
}

/// `augmentation_return_type_mismatch_test.dart` `test_topLevelVariable_multiple_oneMismatch`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_return_type_mismatch_top_level_variable_multiple_one_mismatch() {
    assert_errors_in_code(
        r#"
String? foo;
int? bar;

augment abstract String? foo, bar;
"#,
        &[("augmentation_return_type_mismatch", 55, 3)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_newHead_instance_abstract`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_new_head_instance_abstract() {
    assert_errors_in_code(
        r#"
mixin A {
  abstract int a;
}

class B with A {
  @override
  int a;
  const new(this.a);
}
"#,
        &[("const_constructor_with_mixin_with_field", 78, 3)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_newHead_instance_abstract_final`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_new_head_instance_abstract_final() {
    assert_errors_in_code(
        r#"
mixin A {
  abstract final int a;
}

class B with A {
  @override
  final int a;
  const new(this.a);
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_newHead_instance_final`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_new_head_instance_final() {
    assert_errors_in_code(
        r#"
mixin A {
  final a = 0;
}

class B extends Object with A {
  const new();
}
"#,
        &[("const_constructor_with_mixin_with_field", 69, 3)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_newHead_instance_getter`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_new_head_instance_getter() {
    assert_errors_in_code(
        r#"
mixin A {
  int get a => 7;
}

class B extends Object with A {
  const new();
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_newHead_instance_setter`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_new_head_instance_setter() {
    assert_errors_in_code(
        r#"
mixin A {
  set a(int x) {}
}

class B extends Object with A {
  const new();
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_newHead_instanceField`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_new_head_instance_field() {
    assert_errors_in_code(
        r#"
mixin A {
  var a;
}

class B extends Object with A {
  const new();
}
"#,
        &[("const_constructor_with_mixin_with_field", 63, 3)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_newHead_multipleInstanceFields`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_new_head_multiple_instance_fields() {
    assert_errors_in_code(
        r#"
mixin A {
  var a;
  var b;
}

class B extends Object with A {
  const new();
}
"#,
        &[("const_constructor_with_mixin_with_field", 72, 3)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_newHead_noFields`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_new_head_no_fields() {
    assert_errors_in_code(
        r#"
mixin M {}

class X extends Object with M {
  const new();
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_newHead_static`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_new_head_static() {
    assert_errors_in_code(
        r#"
mixin M {
  static final a = 0;
}

class X extends Object with M {
  const new();
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_typeName_instance_abstract`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_type_name_instance_abstract() {
    assert_errors_in_code(
        r#"
mixin A {
  abstract int a;
}

class B with A {
  @override
  int a;
  const B(this.a);
}
"#,
        &[("const_constructor_with_mixin_with_field", 78, 1)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_typeName_instance_abstract_final`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_type_name_instance_abstract_final() {
    assert_errors_in_code(
        r#"
mixin A {
  abstract final int a;
}

class B with A {
  @override
  final int a;
  const B(this.a);
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_typeName_instance_final`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_type_name_instance_final() {
    assert_errors_in_code(
        r#"
mixin A {
  final a = 0;
}

class B extends Object with A {
  const B();
}
"#,
        &[("const_constructor_with_mixin_with_field", 69, 1)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_typeName_instance_getter`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_type_name_instance_getter() {
    assert_errors_in_code(
        r#"
mixin A {
  int get a => 7;
}

class B extends Object with A {
  const B();
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_typeName_instance_setter`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_type_name_instance_setter() {
    assert_errors_in_code(
        r#"
mixin A {
  set a(int x) {}
}

class B extends Object with A {
  const B();
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_typeName_instanceField`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_type_name_instance_field() {
    assert_errors_in_code(
        r#"
mixin A {
  var a;
}

class B extends Object with A {
  const B();
}
"#,
        &[("const_constructor_with_mixin_with_field", 63, 1)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_typeName_multipleInstanceFields`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_type_name_multiple_instance_fields() {
    assert_errors_in_code(
        r#"
mixin A {
  var a;
  var b;
}

class B extends Object with A {
  const B();
}
"#,
        &[("const_constructor_with_mixin_with_field", 72, 1)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_typeName_noFields`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_type_name_no_fields() {
    assert_errors_in_code(
        r#"
mixin M {}

class X extends Object with M {
  const X();
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_constructor_typeName_static`.
#[test]
fn const_constructor_with_mixin_with_field_constructor_type_name_static() {
    assert_errors_in_code(
        r#"
mixin M {
  static final a = 0;
}

class X extends Object with M {
  const X();
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_primaryConstructor_instance_abstract`.
#[test]
fn const_constructor_with_mixin_with_field_primary_constructor_instance_abstract() {
    assert_errors_in_code(
        r#"
mixin A {
  abstract int a;
}

class const B(this.a) with A {
  @override
  final int a;
  set a(int x) {}
}
"#,
        &[("const_constructor_with_mixin_with_field", 38, 5)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_primaryConstructor_instance_abstract_final`.
#[test]
fn const_constructor_with_mixin_with_field_primary_constructor_instance_abstract_final() {
    assert_errors_in_code(
        r#"
mixin A {
  abstract final int a;
}

class const B(this.a) with A {
  @override
  final int a;
}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_primaryConstructor_instance_final`.
#[test]
fn const_constructor_with_mixin_with_field_primary_constructor_instance_final() {
    assert_errors_in_code(
        r#"
mixin A {
  final a = 0;
}

class const B() extends Object with A {}
"#,
        &[("const_constructor_with_mixin_with_field", 35, 5)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_primaryConstructor_instance_getter`.
#[test]
fn const_constructor_with_mixin_with_field_primary_constructor_instance_getter() {
    assert_errors_in_code(
        r#"
mixin A {
  int get a => 7;
}

class const B() extends Object with A {}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_primaryConstructor_instance_setter`.
#[test]
fn const_constructor_with_mixin_with_field_primary_constructor_instance_setter() {
    assert_errors_in_code(
        r#"
mixin A {
  set a(int x) {}
}

class const B() extends Object with A {}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_primaryConstructor_instanceField`.
#[test]
fn const_constructor_with_mixin_with_field_primary_constructor_instance_field() {
    assert_errors_in_code(
        r#"
mixin A {
  var a;
}

class const B() extends Object with A {}
"#,
        &[("const_constructor_with_mixin_with_field", 29, 5)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_primaryConstructor_multipleInstanceFields`.
#[test]
fn const_constructor_with_mixin_with_field_primary_constructor_multiple_instance_fields() {
    assert_errors_in_code(
        r#"
mixin A {
  var a;
  var b;
}

class const B() extends Object with A {}
"#,
        &[("const_constructor_with_mixin_with_field", 38, 5)],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_primaryConstructor_noFields`.
#[test]
fn const_constructor_with_mixin_with_field_primary_constructor_no_fields() {
    assert_errors_in_code(
        r#"
mixin M {}

class const X() extends Object with M {}
"#,
        &[],
    );
}

/// `const_constructor_with_mixin_with_field_test.dart` `test_primaryConstructor_static`.
#[test]
fn const_constructor_with_mixin_with_field_primary_constructor_static() {
    assert_errors_in_code(
        r#"
mixin M {
  static final a = 0;
}

class const X() extends Object with M {}
"#,
        &[],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_class_explicit_constructor_newHead`.
#[test]
fn const_constructor_with_non_const_super_class_explicit_constructor_new_head() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {
  const new(): super();
}
"#,
        &[("const_constructor_with_non_const_super", 47, 7)],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_class_explicit_constructor_typeName`.
#[test]
fn const_constructor_with_non_const_super_class_explicit_constructor_type_name() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {
  const B(): super();
}
"#,
        &[("const_constructor_with_non_const_super", 45, 7)],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_class_explicit_primaryConstructor_hasBody`.
#[test]
fn const_constructor_with_non_const_super_class_explicit_primary_constructor_has_body() {
    assert_errors_in_code(
        r#"
class A {}
class const B() extends A {
  this : super();
}
"#,
        &[("const_constructor_with_non_const_super", 49, 7)],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_class_implicit_constructor_newHead`.
#[test]
fn const_constructor_with_non_const_super_class_implicit_constructor_new_head() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {
  const new();
}
"#,
        &[("const_constructor_with_non_const_super", 40, 3)],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_class_implicit_constructor_typeName`.
#[test]
fn const_constructor_with_non_const_super_class_implicit_constructor_type_name() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {
  const B();
}
"#,
        &[("const_constructor_with_non_const_super", 40, 1)],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_class_implicit_primaryConstructor_hasBody`.
#[test]
fn const_constructor_with_non_const_super_class_implicit_primary_constructor_has_body() {
    assert_errors_in_code(
        r#"
class A {}
class const B() extends A {
  this;
}
"#,
        &[("const_constructor_with_non_const_super", 18, 5)],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_class_implicit_primaryConstructor_noBody`.
#[test]
fn const_constructor_with_non_const_super_class_implicit_primary_constructor_no_body() {
    assert_errors_in_code(
        r#"
class A {}
class const B() extends A;
"#,
        &[("const_constructor_with_non_const_super", 18, 5)],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_class_redirectConst_superConst`.
#[test]
fn const_constructor_with_non_const_super_class_redirect_const_super_const() {
    assert_errors_in_code(
        r#"
class A {
  factory A() = A._;
  const A._();
}

class B extends A {
  const B.foo() : this.bar();
  const B.bar() : super._();
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_class_redirectConst_superNotConst`.
#[test]
fn const_constructor_with_non_const_super_class_redirect_const_super_not_const() {
    assert_errors_in_code(
        r#"
class A {
  factory A() = A._;
  A._();
}

class B extends A {
  const B.foo() : this.bar();
  const B.bar() : super._();
}
"#,
        &[("const_constructor_with_non_const_super", 112, 9)],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_enum`.
#[test]
fn const_constructor_with_non_const_super_enum() {
    assert_errors_in_code(
        r#"
enum E {
  v
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_const_super_test.dart` `test_enum_hasConstructor`.
#[test]
fn const_constructor_with_non_const_super_enum_has_constructor() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  const E(int a);
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constFactory_named_hasNonFinal_redirect`.
#[test]
fn const_constructor_with_non_final_field_const_factory_named_has_non_final_redirect() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  const factory A.named() = B;
}

class B implements A {
  const B();
  int get x => 0;
  void set x(_) {}
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constFactory_unnamed_hasNonFinal_redirect`.
#[test]
fn const_constructor_with_non_final_field_const_factory_unnamed_has_non_final_redirect() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  const factory A() = B;
}

class B implements A {
  const B();
  int get x => 0;
  void set x(_) {}
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constructor_newHead_unnamed_hasAbstract`.
#[test]
fn const_constructor_with_non_final_field_constructor_new_head_unnamed_has_abstract() {
    assert_errors_in_code(
        r#"
abstract class A {
  abstract int x;
  const new();
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constructor_newHead_unnamed_hasFinal`.
#[test]
fn const_constructor_with_non_final_field_constructor_new_head_unnamed_has_final() {
    assert_errors_in_code(
        r#"
class A {
  final int x = 0;
  const new();
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constructor_newHead_unnamed_hasNonFinal`.
#[test]
fn const_constructor_with_non_final_field_constructor_new_head_unnamed_has_non_final() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  const new();
}
"#,
        &[("const_constructor_with_non_final_field", 32, 3)],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constructor_typeName_named_hasAbstract`.
#[test]
fn const_constructor_with_non_final_field_constructor_type_name_named_has_abstract() {
    assert_errors_in_code(
        r#"
abstract class A {
  abstract int x;
  const A.named();
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constructor_typeName_named_hasFinal`.
#[test]
fn const_constructor_with_non_final_field_constructor_type_name_named_has_final() {
    assert_errors_in_code(
        r#"
class A {
  final int x = 0;
  const A.named();
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constructor_typeName_named_hasNonFinal`.
#[test]
fn const_constructor_with_non_final_field_constructor_type_name_named_has_non_final() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  const A.named();
}
"#,
        &[("const_constructor_with_non_final_field", 32, 7)],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constructor_typeName_unnamed_hasAbstract`.
#[test]
fn const_constructor_with_non_final_field_constructor_type_name_unnamed_has_abstract() {
    assert_errors_in_code(
        r#"
abstract class A {
  abstract int x;
  const A();
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constructor_typeName_unnamed_hasFinal`.
#[test]
fn const_constructor_with_non_final_field_constructor_type_name_unnamed_has_final() {
    assert_errors_in_code(
        r#"
class A {
  final int x = 0;
  const A();
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_constructor_typeName_unnamed_hasNonFinal`.
#[test]
fn const_constructor_with_non_final_field_constructor_type_name_unnamed_has_non_final() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  const A();
}
"#,
        &[("const_constructor_with_non_final_field", 32, 1)],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_primaryConstructor_named_hasNonFinal`.
#[test]
fn const_constructor_with_non_final_field_primary_constructor_named_has_non_final() {
    assert_errors_in_code(
        r#"
class const A.named() {
  int x = 0;
}
"#,
        &[("const_constructor_with_non_final_field", 7, 13)],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_primaryConstructor_unnamed_hasAbstract`.
#[test]
fn const_constructor_with_non_final_field_primary_constructor_unnamed_has_abstract() {
    assert_errors_in_code(
        r#"
abstract class const A() {
  abstract int x;
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_primaryConstructor_unnamed_hasFinal`.
#[test]
fn const_constructor_with_non_final_field_primary_constructor_unnamed_has_final() {
    assert_errors_in_code(
        r#"
class const A() {
  final int x = 0;
}
"#,
        &[],
    );
}

/// `const_constructor_with_non_final_field_test.dart` `test_primaryConstructor_unnamed_hasNonFinal`.
#[test]
fn const_constructor_with_non_final_field_primary_constructor_unnamed_has_non_final() {
    assert_errors_in_code(
        r#"
class const A() {
  int x = 0;
}
"#,
        &[("const_constructor_with_non_final_field", 7, 5)],
    );
}

/// `const_instance_field_test.dart` `test_class`.
#[test]
fn const_instance_field_class() {
    assert_errors_in_code(
        r#"
class C {
  const int f = 0;
}
"#,
        &[("const_instance_field", 13, 5)],
    );
}

/// `const_instance_field_test.dart` `test_mixin`.
#[test]
fn const_instance_field_mixin() {
    assert_errors_in_code(
        r#"
mixin C {
  const int f = 0;
}
"#,
        &[("const_instance_field", 13, 5)],
    );
}

/// `const_with_non_const_test.dart` `test_inConstContext`.
#[test]
fn const_with_non_const_in_const_context() {
    assert_errors_in_code(
        r#"
class A {
  const A(x);
}
class B {
}
main() {
  const A(B());
}
"#,
        &[("const_with_non_const", 58, 3)],
    );
}

/// `const_with_non_const_test.dart` `test_mixinApplication_constSuperConstructor`.
#[test]
fn const_with_non_const_mixin_application_const_super_constructor() {
    assert_errors_in_code(
        r#"
mixin M {}
class A {
  const A();
}
class B = A with M;
const b = const B();
"#,
        &[],
    );
}

/// `const_with_non_const_test.dart` `test_mixinApplication_constSuperConstructor_field`.
#[test]
fn const_with_non_const_mixin_application_const_super_constructor_field() {
    assert_errors_in_code(
        r#"
mixin M {
  int i = 0;
}
class A {
  const A();
}
class B = A with M;
var b = const B();
"#,
        &[("const_with_non_const", 79, 5)],
    );
}

/// `const_with_non_const_test.dart` `test_mixinApplication_constSuperConstructor_getter`.
#[test]
fn const_with_non_const_mixin_application_const_super_constructor_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  int get i => 0;
}
class A {
  const A();
}
class B = A with M;
var b = const B();
"#,
        &[],
    );
}

/// `const_with_non_const_test.dart` `test_mixinApplication_constSuperConstructor_setter`.
#[test]
fn const_with_non_const_mixin_application_const_super_constructor_setter() {
    assert_errors_in_code(
        r#"
mixin M {
  set(int i) {}
}
class A {
  const A();
}
class B = A with M;
var b = const B();
"#,
        &[],
    );
}

/// `const_with_non_const_test.dart` `test_nonConst_factory`.
#[test]
fn const_with_non_const_non_const_factory() {
    assert_errors_in_code(
        r#"
class A {
  factory A(int a) => throw 0;
}

void f() {
  const A(0);
}
"#,
        &[("const_with_non_const", 58, 5)],
    );
}

/// `const_with_non_const_test.dart` `test_nonConst_generative`.
#[test]
fn const_with_non_const_non_const_generative() {
    assert_errors_in_code(
        r#"
class A {
  A(int a);
}

void f() {
  const A(0);
}
"#,
        &[("const_with_non_const", 39, 5)],
    );
}

/// `const_with_undefined_constructor_test.dart` `test_class_named`.
#[test]
fn const_with_undefined_constructor_class_named() {
    assert_errors_in_code(
        r#"
class A {
  const A();
}
f() {
  return const A.noSuchConstructor();
}
"#,
        &[("const_with_undefined_constructor", 49, 17)],
    );
}

/// `const_with_undefined_constructor_test.dart` `test_class_named_prefixed`.
#[test]
fn const_with_undefined_constructor_class_named_prefixed() {
    assert_errors_in_code(
        r#"
import 'dart:async' as a;
f() {
  return const a.Future.noSuchConstructor();
}
"#,
        &[("const_with_undefined_constructor", 57, 17)],
    );
}

/// `const_with_undefined_constructor_test.dart` `test_class_nonFunctionTypedef`.
#[test]
fn const_with_undefined_constructor_class_non_function_typedef() {
    assert_errors_in_code(
        r#"
class A {
  const A.name();
}
typedef B = A;
f() {
  return const B();
}
"#,
        &[("const_with_undefined_constructor_default", 67, 1)],
    );
}

/// `const_with_undefined_constructor_test.dart` `test_class_unnamed`.
#[test]
fn const_with_undefined_constructor_class_unnamed() {
    assert_errors_in_code(
        r#"
class A {
  const A.name();
}
f() {
  return const A();
}
"#,
        &[("const_with_undefined_constructor_default", 52, 1)],
    );
}

/// `const_with_undefined_constructor_test.dart` `test_enum_notConstructor_constant`.
#[test]
fn const_with_undefined_constructor_enum_not_constructor_constant() {
    assert_errors_in_code(
        r#"
void f() {
  const E.v();
}

enum E {
  v
}
"#,
        &[("const_with_undefined_constructor", 22, 1)],
    );
}

/// `const_with_undefined_constructor_test.dart` `test_enum_notConstructor_method`.
#[test]
fn const_with_undefined_constructor_enum_not_constructor_method() {
    assert_errors_in_code(
        r#"
void f() {
  const E.foo();
}

enum E {
  v;
  
  void foo() {}
}
"#,
        &[("const_with_undefined_constructor", 22, 3)],
    );
}

/// `const_with_undefined_constructor_test.dart` `test_enum_unresolved`.
#[test]
fn const_with_undefined_constructor_enum_unresolved() {
    assert_errors_in_code(
        r#"
void f() {
  const E.foo();
}

enum E {
  v
}
"#,
        &[("const_with_undefined_constructor", 22, 3)],
    );
}

/// `default_value_in_redirecting_factory_constructor_test.dart` `test_default_value`.
#[test]
fn default_value_in_redirecting_factory_constructor_default_value() {
    assert_errors_in_code(
        r#"
class A {
  factory A([int x = 0]) = B;
}

class B implements A {
  B([int x = 1]) {}
}
"#,
        &[("default_value_in_redirecting_factory_constructor", 28, 1)],
    );
}

/// `experiment_not_enabled_test.dart` `test_constructor_tearoffs_disabled_grammar`.
#[test]
fn experiment_not_enabled_constructor_tearoffs_disabled_grammar() {
    assert_errors_in_code(
        r#"
// @dart = 2.12
class Foo<X> {
  const Foo.bar();
  int get baz => 0;
}
main() {
  Foo<int>.bar.baz();
}
"#,
        &[
            ("experiment_not_enabled", 87, 5),
            ("undefined_method", 97, 3),
        ],
    );
}

/// `experiment_not_enabled_test.dart` `test_dotShorthands_disabled`.
#[test]
fn experiment_not_enabled_dot_shorthands_disabled() {
    assert_errors_in_code(
        r#"
// @dart = 3.8
void main() {
  Object c = .hash(1, 2);
  print(c);
}
"#,
        &[("experiment_not_enabled", 43, 1)],
    );
}

/// `experiment_not_enabled_test.dart` `test_nonFunctionTypeAliases_disabled`.
#[test]
fn experiment_not_enabled_non_function_type_aliases_disabled() {
    assert_errors_in_code(
        r#"
// @dart = 2.12
typedef A = int;
"#,
        &[("experiment_not_enabled", 27, 1)],
    );
}

/// `experiment_not_enabled_test.dart` `test_nonFunctionTypeAliases_disabled_nullable`.
#[test]
fn experiment_not_enabled_non_function_type_aliases_disabled_nullable() {
    assert_errors_in_code(
        r#"
// @dart = 2.12
typedef A = int?;
"#,
        &[("experiment_not_enabled", 27, 1)],
    );
}

/// `experiment_not_enabled_test.dart` `test_privateNamedParameters_disabled`.
#[test]
fn experiment_not_enabled_private_named_parameters_disabled() {
    assert_errors_in_code(
        r#"
// @dart = 3.8
class C {
  int? _x;
  C({this._x});
}
"#,
        &[("unused_field", 33, 2), ("experiment_not_enabled", 47, 2)],
    );
}

/// `extension_type_constructor_with_super_formal_parameter_test.dart` `test_named`.
#[test]
fn extension_type_constructor_with_super_formal_parameter_named() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named(this.it, {super.foo});
}
"#,
        &[(
            "extension_type_constructor_with_super_formal_parameter",
            48,
            5,
        )],
    );
}

/// `extension_type_constructor_with_super_formal_parameter_test.dart` `test_positional`.
#[test]
fn extension_type_constructor_with_super_formal_parameter_positional() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named(this.it, super.foo);
}
"#,
        &[(
            "extension_type_constructor_with_super_formal_parameter",
            47,
            5,
        )],
    );
}

/// `extension_type_constructor_with_super_invocation_test.dart` `test_named`.
#[test]
fn extension_type_constructor_with_super_invocation_named() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named() : it = 0, super.named();
}
"#,
        &[("extension_type_constructor_with_super_invocation", 50, 5)],
    );
}

/// `extension_type_constructor_with_super_invocation_test.dart` `test_notLast`.
#[test]
fn extension_type_constructor_with_super_invocation_not_last() {
    assert_errors_in_code(
        r#"
extension type const E._(int it) {
  const E(int it) : super._(it), assert(it >= 0);
}
"#,
        &[
            ("final_not_initialized_constructor", 44, 1),
            ("extension_type_constructor_with_super_invocation", 56, 5),
        ],
    );
}

/// `extension_type_constructor_with_super_invocation_test.dart` `test_unnamed`.
#[test]
fn extension_type_constructor_with_super_invocation_unnamed() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named() : it = 0, super();
}
"#,
        &[("extension_type_constructor_with_super_invocation", 50, 5)],
    );
}

/// `field_initializer_factory_constructor_test.dart` `test_class_fieldFormalParameter`.
#[test]
fn field_initializer_factory_constructor_class_field_formal_parameter() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  factory A(this.x) => throw 0;
}
"#,
        &[("field_initializer_factory_constructor", 36, 6)],
    );
}

/// `field_initializer_factory_constructor_test.dart` `test_class_fieldFormalParameter_functionTyped`.
#[test]
fn field_initializer_factory_constructor_class_field_formal_parameter_function_typed() {
    assert_errors_in_code(
        r#"
class A {
  int Function()? x;
  factory A(int this.x());
}
"#,
        &[("field_initializer_factory_constructor", 44, 12)],
    );
}

/// `field_initializer_factory_constructor_test.dart` `test_class_fieldFormalParameter_functionTyped_language305`.
#[test]
fn field_initializer_factory_constructor_class_field_formal_parameter_function_typed_language305() {
    assert_errors_in_code(
        r#"
// @dart = 3.5
class A {
  int Function()? x;
  factory A(int this.x());
}
"#,
        &[
            ("field_initializer_factory_constructor", 59, 12),
            ("missing_function_body", 72, 1),
        ],
    );
}

/// `field_initializer_factory_constructor_test.dart` `test_enum_fieldFormalParameter`.
#[test]
fn field_initializer_factory_constructor_enum_field_formal_parameter() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  final int x = 0;
  const E();
  factory E._(this.x) => throw 0;
}

void f() {
  E._(0);
}
"#,
        &[("field_initializer_factory_constructor", 61, 6)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_closure`.
#[test]
fn field_initializer_outside_constructor_closure() {
    assert_errors_in_code(
        r#"
class A {
  dynamic field = ({this.field}) {};
}
"#,
        &[("field_initializer_outside_constructor", 31, 4)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_defaultParameter`.
#[test]
fn field_initializer_outside_constructor_default_parameter() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  m([this.x = 0]) {}
}
"#,
        &[("field_initializer_outside_constructor", 29, 4)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_functionTypedFieldFormalParameter`.
#[test]
fn field_initializer_outside_constructor_function_typed_field_formal_parameter() {
    assert_errors_in_code(
        r#"
class A {
  int Function()? x;
  m(int this.x()) {}
}
"#,
        &[("field_initializer_outside_constructor", 40, 4)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_inFunctionTypedParameter`.
#[test]
fn field_initializer_outside_constructor_in_function_typed_parameter() {
    assert_errors_in_code(
        r#"
class A {
  int? x;
  A(int p(this.x));
}
"#,
        &[("field_initializer_outside_constructor", 31, 4)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_localFunction_optionalNamed`.
#[test]
fn field_initializer_outside_constructor_local_function_optional_named() {
    assert_errors_in_code(
        r#"
void f() {
  void foo({this.x}) {}
  foo(x: 0);
}
"#,
        &[("field_initializer_outside_constructor", 24, 4)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_localFunction_optionalPositional`.
#[test]
fn field_initializer_outside_constructor_local_function_optional_positional() {
    assert_errors_in_code(
        r#"
void f() {
  void foo([this.x]) {}
  foo(0);
}
"#,
        &[("field_initializer_outside_constructor", 24, 4)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_localFunction_requiredNamed`.
#[test]
fn field_initializer_outside_constructor_local_function_required_named() {
    assert_errors_in_code(
        r#"
void f() {
  void foo({required this.x}) {}
  foo(x: 0);
}
"#,
        &[("field_initializer_outside_constructor", 33, 4)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_localFunction_requiredPositional`.
#[test]
fn field_initializer_outside_constructor_local_function_required_positional() {
    assert_errors_in_code(
        r#"
void f() {
  void foo(this.x) {}
  foo(0);
}
"#,
        &[("field_initializer_outside_constructor", 23, 4)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_method`.
#[test]
fn field_initializer_outside_constructor_method() {
    assert_errors_in_code(
        r#"
class A {
  int? x;
  m(this.x) {}
}
"#,
        &[("field_initializer_outside_constructor", 25, 4)],
    );
}

/// `field_initializer_outside_constructor_test.dart` `test_topLevelFunction`.
#[test]
fn field_initializer_outside_constructor_top_level_function() {
    assert_errors_in_code(
        r#"
f(this.x(y)) {}
"#,
        &[("field_initializer_outside_constructor", 3, 4)],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_class_primary_afterRedirection`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn field_initializer_redirecting_constructor_class_primary_after_redirection() {
    assert_errors_in_code(
        r#"
class A() {
  int x;
  A.named() : this();
  this : this.named(), x = 0;
}
"#,
        &[("primary_constructor_cannot_redirect", 53, 4)],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_class_primary_beforeRedirection`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn field_initializer_redirecting_constructor_class_primary_before_redirection() {
    assert_errors_in_code(
        r#"
class A() {
  int x;
  A.named() : this();
  this : x = 0, this.named();
}
"#,
        &[("primary_constructor_cannot_redirect", 60, 4)],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_class_typeName_afterRedirection`.
#[test]
fn field_initializer_redirecting_constructor_class_type_name_after_redirection() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  A.named() {}
  A() : this.named(), x = 42;
}
"#,
        &[("field_initializer_redirecting_constructor", 61, 6)],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_class_typeName_beforeRedirection`.
#[test]
fn field_initializer_redirecting_constructor_class_type_name_before_redirection() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  A.named() {}
  A() : x = 42, this.named();
}
"#,
        &[("field_initializer_redirecting_constructor", 47, 6)],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_class_typeName_redirectionOnly`.
#[test]
fn field_initializer_redirecting_constructor_class_type_name_redirection_only() {
    assert_errors_in_code(
        r#"
class A {
  int x = 0;
  A.named() {}
  A(this.x) : this.named();
}
"#,
        &[("field_initializer_redirecting_constructor", 43, 6)],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_enum_primary_afterRedirection`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn field_initializer_redirecting_constructor_enum_primary_after_redirection() {
    assert_errors_in_code(
        r#"
enum E() {
  v;
  final int x;
  const E.named() : this();
  this : this.named(), x = 0;
}
"#,
        &[
            ("recursive_constant_constructor", 40, 7),
            ("primary_constructor_cannot_redirect", 69, 4),
        ],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_enum_primary_beforeRedirection`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn field_initializer_redirecting_constructor_enum_primary_before_redirection() {
    assert_errors_in_code(
        r#"
enum E() {
  v;
  final int x;
  const E.named() : this();
  this : x = 0, this.named();
}
"#,
        &[
            ("recursive_constant_constructor", 40, 7),
            ("primary_constructor_cannot_redirect", 76, 4),
        ],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_enum_typeName_afterRedirection`.
#[test]
fn field_initializer_redirecting_constructor_enum_type_name_after_redirection() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  final int x;
  const E.named() : x = 0;
  const E() : this.named(), x = 42;
}
"#,
        &[("field_initializer_redirecting_constructor", 85, 6)],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_enum_typeName_beforeRedirection`.
#[test]
fn field_initializer_redirecting_constructor_enum_type_name_before_redirection() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  final int x;
  const E.named() : x = 0;
  const E() : x = 42, this.named();
}
"#,
        &[("field_initializer_redirecting_constructor", 71, 6)],
    );
}

/// `field_initializer_redirecting_constructor_test.dart` `test_enum_typeName_redirectionOnly`.
#[test]
fn field_initializer_redirecting_constructor_enum_type_name_redirection_only() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  final int x;
  const E.named() : x = 0;
  const E(this.x) : this.named();
}
"#,
        &[("field_initializer_redirecting_constructor", 70, 6)],
    );
}

/// `field_initializing_formal_not_assignable_test.dart` `test_class_dynamic`.
#[test]
fn field_initializing_formal_not_assignable_class_dynamic() {
    assert_errors_in_code(
        r#"
class A {
  int x;
  A(dynamic this.x) {}
}
"#,
        &[("field_initializing_formal_not_assignable", 24, 14)],
    );
}

/// `field_initializing_formal_not_assignable_test.dart` `test_class_unrelated`.
#[test]
fn field_initializing_formal_not_assignable_class_unrelated() {
    assert_errors_in_code(
        r#"
class A {
  int x;
  A(String this.x) {}
}
"#,
        &[("field_initializing_formal_not_assignable", 24, 13)],
    );
}

/// `field_initializing_formal_not_assignable_test.dart` `test_enum_dynamic`.
#[test]
fn field_initializing_formal_not_assignable_enum_dynamic() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  final int x;
  const E(dynamic this.x);
}
"#,
        &[("field_initializing_formal_not_assignable", 43, 14)],
    );
}

/// `field_initializing_formal_not_assignable_test.dart` `test_enum_unrelated`.
#[test]
fn field_initializing_formal_not_assignable_enum_unrelated() {
    assert_errors_in_code(
        r#"
enum E {
  v('');
  final int x;
  const E(String this.x);
}
"#,
        &[
            ("const_constructor_param_type_mismatch", 14, 2),
            ("field_initializing_formal_not_assignable", 44, 13),
        ],
    );
}

/// `initializer_for_non_existent_field_test.dart` `test_const`.
#[test]
fn initializer_for_non_existent_field_const() {
    assert_errors_in_code(
        r#"
class A {
  const A() : x = 'foo';
}
A a = const A();
"#,
        &[("initializer_for_non_existent_field", 25, 9)],
    );
}

/// `initializer_for_non_existent_field_test.dart` `test_getter`.
#[test]
fn initializer_for_non_existent_field_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get x => 0;
  A() : x = 0;
}
"#,
        &[("initializer_for_non_existent_field", 37, 5)],
    );
}

/// `initializer_for_non_existent_field_test.dart` `test_initializer`.
#[test]
fn initializer_for_non_existent_field_initializer() {
    assert_errors_in_code(
        r#"
class A {
  A() : x = 0 {}
}
"#,
        &[("initializer_for_non_existent_field", 19, 5)],
    );
}

/// `initializer_for_static_field_test.dart` `test_class_primaryConstructor_fieldFormalParameter`.
#[test]
fn initializer_for_static_field_class_primary_constructor_field_formal_parameter() {
    assert_errors_in_code(
        r#"
class A(this.x) {
  static int? x;
}
"#,
        &[("initializer_for_static_field", 9, 4)],
    );
}

/// `initializer_for_static_field_test.dart` `test_class_secondaryConstructor_fieldFormalParameter`.
#[test]
fn initializer_for_static_field_class_secondary_constructor_field_formal_parameter() {
    assert_errors_in_code(
        r#"
class A {
  static int? x;
  A([this.x = 0]) {}
}
"#,
        &[("initializer_for_static_field", 33, 4)],
    );
}

/// `initializer_for_static_field_test.dart` `test_class_secondaryConstructor_initializerList`.
#[test]
fn initializer_for_static_field_class_secondary_constructor_initializer_list() {
    assert_errors_in_code(
        r#"
class A {
  static int x = 1;
  A() : x = 0 {}
}
"#,
        &[("initializer_for_static_field", 39, 5)],
    );
}

/// `initializer_for_static_field_test.dart` `test_enum_primaryConstructor_fieldFormalParameter`.
#[test]
fn initializer_for_static_field_enum_primary_constructor_field_formal_parameter() {
    assert_errors_in_code(
        r#"
enum E(this.x) {
  v(0);

  static int? x;
}
"#,
        &[("initializer_for_static_field", 8, 4)],
    );
}

/// `initializer_for_static_field_test.dart` `test_enum_secondaryConstructor_fieldFormalParameter`.
#[test]
fn initializer_for_static_field_enum_secondary_constructor_field_formal_parameter() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  static int x = 0;
  const E(this.x);
}
"#,
        &[("initializer_for_static_field", 48, 4)],
    );
}

/// `initializer_for_static_field_test.dart` `test_enum_secondaryConstructor_initializerList`.
#[test]
fn initializer_for_static_field_enum_secondary_constructor_initializer_list() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static int x = 1;
  const E() : x = 0;
}
"#,
        &[("initializer_for_static_field", 49, 5)],
    );
}

/// `initializer_for_static_field_test.dart` `test_extensionType_primaryConstructor_fieldFormalParameter_notReportedHere`.
#[test]
fn initializer_for_static_field_extension_type_primary_constructor_field_formal_parameter_not_reported_here()
 {
    assert_errors_in_code(
        r#"
extension type E(this.x) {
  static int? x;
}
"#,
        &[("expected_representation_field", 18, 4)],
    );
}

/// `initializer_for_static_field_test.dart` `test_extensionType_secondaryConstructor_fieldFormalParameter`.
#[test]
fn initializer_for_static_field_extension_type_secondary_constructor_field_formal_parameter() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  static int x = 0;
  E.named(this.x) : this.it = 0;
}
"#,
        &[("initializer_for_static_field", 58, 4)],
    );
}

/// `initializer_for_static_field_test.dart` `test_extensionType_secondaryConstructor_initializerList`.
#[test]
fn initializer_for_static_field_extension_type_secondary_constructor_initializer_list() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  static int x = 1;
  E.named() : x = 0, this.it = 0;
}
"#,
        &[("initializer_for_static_field", 62, 5)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_class_primary_fieldExists`.
#[test]
fn initializing_formal_for_non_existent_field_class_primary_field_exists() {
    assert_errors_in_code(
        r#"
class C(this.x) {
  final int x;
}
"#,
        &[],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_class_primary_fieldMissing`.
#[test]
fn initializing_formal_for_non_existent_field_class_primary_field_missing() {
    assert_errors_in_code(
        r#"
class C(this.x) {}
"#,
        &[("initializing_formal_for_non_existent_field", 9, 6)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_class_secondary_fieldExists`.
#[test]
fn initializing_formal_for_non_existent_field_class_secondary_field_exists() {
    assert_errors_in_code(
        r#"
class C {
  final int x;
  C(this.x);
}
"#,
        &[],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_class_secondary_fieldExists_augmentationAfterWildcard`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn initializing_formal_for_non_existent_field_class_secondary_field_exists_augmentation_after_wildcard()
 {
    assert_errors_in_code(
        r#"
class C {
  int? x;
  C(int? _);
}

augment class C {
  augment C(this.x);
}
"#,
        &[],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_class_secondary_fieldMissing`.
#[test]
fn initializing_formal_for_non_existent_field_class_secondary_field_missing() {
    assert_errors_in_code(
        r#"
class A {
  A(this.x) {}
}
"#,
        &[("initializing_formal_for_non_existent_field", 15, 6)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_class_secondary_fieldMissing_getter`.
#[test]
fn initializing_formal_for_non_existent_field_class_secondary_field_missing_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get x => 1;
  A(this.x) {}
}
"#,
        &[("initializing_formal_for_non_existent_field", 33, 6)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_class_secondary_notInEnclosingClass`.
#[test]
fn initializing_formal_for_non_existent_field_class_secondary_not_in_enclosing_class() {
    assert_errors_in_code(
        r#"
class A {
  int x = 1;
}
class B extends A {
  B(this.x) {}
}
"#,
        &[("initializing_formal_for_non_existent_field", 50, 6)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_class_secondary_optionalPositional_fieldMissing`.
#[test]
fn initializing_formal_for_non_existent_field_class_secondary_optional_positional_field_missing() {
    assert_errors_in_code(
        r#"
class A {
  A([this.x]) {}
}
"#,
        &[("initializing_formal_for_non_existent_field", 16, 6)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_enum_primary_fieldExists`.
#[test]
fn initializing_formal_for_non_existent_field_enum_primary_field_exists() {
    assert_errors_in_code(
        r#"
enum E(this.x) {
  v(0);

  final int x;
}
"#,
        &[],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_enum_primary_fieldMissing`.
#[test]
fn initializing_formal_for_non_existent_field_enum_primary_field_missing() {
    assert_errors_in_code(
        r#"
enum E(this.x) {
  v(0);
}
"#,
        &[("initializing_formal_for_non_existent_field", 8, 6)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_enum_secondary_fieldExists`.
#[test]
fn initializing_formal_for_non_existent_field_enum_secondary_field_exists() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  final int x;
  const E(this.x);
}
"#,
        &[],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_enum_secondary_fieldMissing`.
#[test]
fn initializing_formal_for_non_existent_field_enum_secondary_field_missing() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  const E(this.x);
}
"#,
        &[("initializing_formal_for_non_existent_field", 28, 6)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_enum_secondary_fieldMissing_getter`.
#[test]
fn initializing_formal_for_non_existent_field_enum_secondary_field_missing_getter() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  const E(this.x);
  int get x => 1;
}
"#,
        &[("initializing_formal_for_non_existent_field", 28, 6)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_enum_secondary_optionalPositional_fieldMissing`.
#[test]
fn initializing_formal_for_non_existent_field_enum_secondary_optional_positional_field_missing() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  const E([this.x]);
}
"#,
        &[
            ("initializing_formal_for_non_existent_field", 26, 6),
            ("unused_element_parameter", 31, 1),
        ],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_extensionType_primary_fieldMissing_notReportedHere`.
#[test]
fn initializing_formal_for_non_existent_field_extension_type_primary_field_missing_not_reported_here()
 {
    assert_errors_in_code(
        r#"
extension type E(this.x) {}
"#,
        &[("expected_representation_field", 18, 4)],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_extensionType_secondary_fieldExists`.
#[test]
fn initializing_formal_for_non_existent_field_extension_type_secondary_field_exists() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named(this.it);
}
"#,
        &[],
    );
}

/// `initializing_formal_for_non_existent_field_test.dart` `test_extensionType_secondary_fieldMissing`.
#[test]
fn initializing_formal_for_non_existent_field_extension_type_secondary_field_missing() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named(this.x) : this.it = 0;
}
"#,
        &[("initializing_formal_for_non_existent_field", 38, 6)],
    );
}

/// `instantiate_abstract_class_test.dart` `test_const_generic`.
#[test]
fn instantiate_abstract_class_const_generic() {
    assert_errors_in_code(
        r#"
abstract class A<E> {
  const A();
}
void f() {
  var a = const A<int>();
}"#,
        &[
            ("unused_local_variable", 55, 1),
            ("instantiate_abstract_class", 65, 6),
        ],
    );
}

/// `instantiate_abstract_class_test.dart` `test_const_simple`.
#[test]
fn instantiate_abstract_class_const_simple() {
    assert_errors_in_code(
        r#"
abstract class A {
  const A();
}
void f() {
  A a = const A();
}"#,
        &[
            ("unused_local_variable", 50, 1),
            ("instantiate_abstract_class", 60, 1),
        ],
    );
}

/// `instantiate_abstract_class_test.dart` `test_new_generic`.
#[test]
fn instantiate_abstract_class_new_generic() {
    assert_errors_in_code(
        r#"
abstract class A<E> {}
void f() {
  new A<int>();
}
"#,
        &[("instantiate_abstract_class", 41, 6)],
    );
}

/// `instantiate_abstract_class_test.dart` `test_new_interfaceTypeTypedef`.
#[test]
fn instantiate_abstract_class_new_interface_type_typedef() {
    assert_errors_in_code(
        r#"
abstract class A {}
typedef B = A;
void f() {
  new B();
}
"#,
        &[("instantiate_abstract_class", 53, 1)],
    );
}

/// `instantiate_abstract_class_test.dart` `test_new_nonGeneric`.
#[test]
fn instantiate_abstract_class_new_non_generic() {
    assert_errors_in_code(
        r#"
abstract class A {}
void f() {
  new A();
}
"#,
        &[("instantiate_abstract_class", 38, 1)],
    );
}

/// `instantiate_abstract_class_test.dart` `test_noKeyword_generic`.
#[test]
fn instantiate_abstract_class_no_keyword_generic() {
    assert_errors_in_code(
        r#"
abstract class A<E> {}
void f() {
  A<int>();
}
"#,
        &[("instantiate_abstract_class", 37, 6)],
    );
}

/// `instantiate_abstract_class_test.dart` `test_noKeyword_interfaceTypeTypedef`.
#[test]
fn instantiate_abstract_class_no_keyword_interface_type_typedef() {
    assert_errors_in_code(
        r#"
abstract class A {}
typedef B = A;
void f() {
  B();
}
"#,
        &[("instantiate_abstract_class", 49, 1)],
    );
}

/// `instantiate_abstract_class_test.dart` `test_noKeyword_nonGeneric`.
#[test]
fn instantiate_abstract_class_no_keyword_non_generic() {
    assert_errors_in_code(
        r#"
abstract class A {}
void f() {
  A();
}
"#,
        &[("instantiate_abstract_class", 34, 1)],
    );
}

/// `invalid_modifier_on_constructor_test.dart` `test_async`.
#[test]
fn invalid_modifier_on_constructor_async() {
    assert_errors_in_code(
        r#"
class A {
  A() async {}
}
"#,
        &[("invalid_modifier_on_constructor", 17, 5)],
    );
}

/// `invalid_modifier_on_constructor_test.dart` `test_asyncStar`.
#[test]
fn invalid_modifier_on_constructor_async_star() {
    assert_errors_in_code(
        r#"
class A {
  A() async* {}
}
"#,
        &[("invalid_modifier_on_constructor", 17, 5)],
    );
}

/// `invalid_modifier_on_constructor_test.dart` `test_syncStar`.
#[test]
fn invalid_modifier_on_constructor_sync_star() {
    assert_errors_in_code(
        r#"
class A {
  A() sync* {}
}
"#,
        &[("invalid_modifier_on_constructor", 17, 4)],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_factory_named`.
#[test]
fn invalid_reference_to_generative_enum_constructor_factory_named() {
    assert_errors_in_code(
        r#"
enum E {
  v();

  factory E.named() => v;
}

void f() {
  E.named;
  E.named();
}
"#,
        &[],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_factory_unnamed`.
#[test]
fn invalid_reference_to_generative_enum_constructor_factory_unnamed() {
    assert_errors_in_code(
        r#"
enum E {
  v.named();

  const E.named();
  factory E() => v;
}

void f() {
  E.new;
  E();
}
"#,
        &[],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_named_constructorReference`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_named_constructor_reference() {
    assert_errors_in_code(
        r#"
enum E {
  v.named();

  const E.named();
}

void f() {
  E.named;
}
"#,
        &[("invalid_reference_to_generative_enum_constructor", 59, 7)],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_named_instanceCreation_implicitNew`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_named_instance_creation_implicit_new()
 {
    assert_errors_in_code(
        r#"
enum E {
  v.named();

  const E.named();
}

void f() {
  E.named();
}
"#,
        &[("invalid_reference_to_generative_enum_constructor", 59, 7)],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_named_redirectingConstructorInvocation`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_named_redirecting_constructor_invocation()
 {
    assert_errors_in_code(
        r#"
enum E {
  v;

  const E() : this.named();
  const E.named();
}
"#,
        &[],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_named_redirectingFactory`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_named_redirecting_factory() {
    assert_errors_in_code(
        r#"
enum E {
  v;

  const factory E() = E.named;
  const E.named();
}
"#,
        &[
            ("enum_constant_invokes_factory_constructor", 12, 1),
            ("invalid_reference_to_generative_enum_constructor", 38, 7),
        ],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_unnamed_constructorReference`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_unnamed_constructor_reference() {
    assert_errors_in_code(
        r#"
enum E {
  v
}

void f() {
  E.new;
}
"#,
        &[("invalid_reference_to_generative_enum_constructor", 30, 5)],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_unnamed_instanceCreation_explicitConst`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_unnamed_instance_creation_explicit_const()
 {
    assert_errors_in_code(
        r#"
enum E {
  v
}

void f() {
  const E();
}
"#,
        &[("invalid_reference_to_generative_enum_constructor", 36, 1)],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_unnamed_instanceCreation_explicitNew`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_unnamed_instance_creation_explicit_new()
 {
    assert_errors_in_code(
        r#"
enum E {
  v
}

void f() {
  new E();
}
"#,
        &[("invalid_reference_to_generative_enum_constructor", 34, 1)],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_unnamed_instanceCreation_implicitNew`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_unnamed_instance_creation_implicit_new()
 {
    assert_errors_in_code(
        r#"
enum E {
  v
}

void f() {
  E();
}
"#,
        &[("invalid_reference_to_generative_enum_constructor", 30, 1)],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_unnamed_redirectingConstructorInvocation`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_unnamed_redirecting_constructor_invocation()
 {
    assert_errors_in_code(
        r#"
enum E {
  v1,
  v2.named();

  const E();
  const E.named() : this();
}
"#,
        &[],
    );
}

/// `invalid_reference_to_generative_enum_constructor_test.dart` `test_generative_unnamed_redirectingFactory`.
#[test]
fn invalid_reference_to_generative_enum_constructor_generative_unnamed_redirecting_factory() {
    assert_errors_in_code(
        r#"
enum E {
  v;

  const factory E.named() = E;
  const E();
}
"#,
        &[("invalid_reference_to_generative_enum_constructor", 44, 1)],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_class_constructor_external`.
#[test]
fn invalid_super_formal_parameter_location_class_constructor_external() {
    assert_errors_in_code(
        r#"
class A {
  external A(super.a);
}
"#,
        &[("invalid_super_formal_parameter_location", 24, 5)],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_class_constructor_redirecting`.
#[test]
fn invalid_super_formal_parameter_location_class_constructor_redirecting() {
    assert_errors_in_code(
        r#"
class A {
  A(super.a) : this._();
  A._();
}
"#,
        &[("invalid_super_formal_parameter_location", 15, 5)],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_class_method`.
#[test]
fn invalid_super_formal_parameter_location_class_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo(super.a) {}
}
"#,
        &[("invalid_super_formal_parameter_location", 22, 5)],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_extension_method`.
#[test]
fn invalid_super_formal_parameter_location_extension_method() {
    assert_errors_in_code(
        r#"
extension E on int {
  void foo(super.a) {}
}
"#,
        &[("invalid_super_formal_parameter_location", 33, 5)],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_local_function`.
#[test]
fn invalid_super_formal_parameter_location_local_function() {
    assert_errors_in_code(
        r#"
void f() {
  // ignore:unused_element
  void g(super.a) {}
}
"#,
        &[("invalid_super_formal_parameter_location", 48, 5)],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_mixin_method`.
#[test]
fn invalid_super_formal_parameter_location_mixin_method() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo(super.a) {}
}
"#,
        &[("invalid_super_formal_parameter_location", 22, 5)],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_unit_function`.
#[test]
fn invalid_super_formal_parameter_location_unit_function() {
    assert_errors_in_code(
        r#"
void f(super.a) {}
"#,
        &[("invalid_super_formal_parameter_location", 8, 5)],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_valid_optionalNamed`.
#[test]
fn invalid_super_formal_parameter_location_valid_optional_named() {
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}

class B extends A {
  B({super.a});
}
"#,
        &[],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_valid_optionalPositional`.
#[test]
fn invalid_super_formal_parameter_location_valid_optional_positional() {
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}

class B extends A {
  B([super.a]);
}
"#,
        &[],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_valid_requiredNamed`.
#[test]
fn invalid_super_formal_parameter_location_valid_required_named() {
    assert_errors_in_code(
        r#"
class A {
  A({required int a});
}

class B extends A {
  B({required super.a});
}
"#,
        &[],
    );
}

/// `invalid_super_formal_parameter_location_test.dart` `test_valid_requiredPositional`.
#[test]
fn invalid_super_formal_parameter_location_valid_required_positional() {
    assert_errors_in_code(
        r#"
class A {
  A(int a);
}

class B extends A {
  B(super.a);
}
"#,
        &[],
    );
}

/// `mixin_instantiate_test.dart` `test_namedConstructor`.
#[test]
fn mixin_instantiate_named_constructor() {
    assert_errors_in_code(
        r#"
mixin M {
  M.named() {}
}

void f() {
  new M.named();
}
"#,
        &[
            ("mixin_declares_constructor", 13, 1),
            ("mixin_instantiate", 46, 1),
        ],
    );
}

/// `mixin_instantiate_test.dart` `test_unnamedConstructor`.
#[test]
fn mixin_instantiate_unnamed_constructor() {
    assert_errors_in_code(
        r#"
mixin M {}

void f() {
  new M();
}
"#,
        &[("mixin_instantiate", 30, 1)],
    );
}

/// `multiple_redirecting_constructor_invocations_test.dart` `test_class_primary`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn multiple_redirecting_constructor_invocations_class_primary() {
    assert_errors_in_code(
        r#"
class A {}

class B() extends A {
  B.foo() : this();
  B.bar() : this();
  this : this.foo(), this.bar();
}
"#,
        &[
            ("primary_constructor_cannot_redirect", 84, 4),
            ("primary_constructor_cannot_redirect", 96, 4),
        ],
    );
}

/// `multiple_redirecting_constructor_invocations_test.dart` `test_class_typeName_twoNamed`.
#[test]
fn multiple_redirecting_constructor_invocations_class_type_name_two_named() {
    assert_errors_in_code(
        r#"
class A {
  A() : this.foo(), this.bar();
  A.foo() {}
  A.bar() {}
}
"#,
        &[("multiple_redirecting_constructor_invocations", 31, 10)],
    );
}

/// `multiple_redirecting_constructor_invocations_test.dart` `test_enum_primary`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn multiple_redirecting_constructor_invocations_enum_primary() {
    assert_errors_in_code(
        r#"
enum E() {
  v;
  const E.foo() : this();
  const E.bar() : this();
  this : this.foo(), this.bar();
}
"#,
        &[
            ("recursive_constant_constructor", 25, 5),
            ("recursive_constant_constructor", 51, 5),
            ("primary_constructor_cannot_redirect", 78, 4),
            ("primary_constructor_cannot_redirect", 90, 4),
        ],
    );
}

/// `multiple_redirecting_constructor_invocations_test.dart` `test_enum_typeName_twoNamed`.
#[test]
fn multiple_redirecting_constructor_invocations_enum_type_name_two_named() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  const E() : this.foo(), this.bar();
  const E.foo();
  const E.bar();
}
"#,
        &[("multiple_redirecting_constructor_invocations", 41, 10)],
    );
}

/// `multiple_super_initializers_test.dart` `test_primary_twoSuperInitializers`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn multiple_super_initializers_primary_two_super_initializers() {
    assert_errors_in_code(
        r#"
class A {}
class B() extends A {
  this : super(), super();
}
"#,
        &[("multiple_super_initializers", 52, 5)],
    );
}

/// `multiple_super_initializers_test.dart` `test_typeName_oneSuperInitializer`.
#[test]
fn multiple_super_initializers_type_name_one_super_initializer() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {
  B() : super() {}
}
"#,
        &[],
    );
}

/// `multiple_super_initializers_test.dart` `test_typeName_twoSuperInitializers`.
#[test]
fn multiple_super_initializers_type_name_two_super_initializers() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {
  B() : super(), super() {}
}
"#,
        &[("multiple_super_initializers", 49, 7)],
    );
}

/// `new_with_undefined_constructor_test.dart` `test_default`.
#[test]
fn new_with_undefined_constructor_default() {
    assert_errors_in_code(
        r#"
class A {
  A.name() {}
}
f() {
  new A();
}
"#,
        &[("new_with_undefined_constructor_default", 39, 1)],
    );
}

/// `new_with_undefined_constructor_test.dart` `test_default_noKeyword`.
#[test]
fn new_with_undefined_constructor_default_no_keyword() {
    assert_errors_in_code(
        r#"
class A {
  A.name() {}
}
f() {
  A();
}
"#,
        &[("new_with_undefined_constructor_default", 35, 1)],
    );
}

/// `new_with_undefined_constructor_test.dart` `test_default_unnamedViaNew`.
#[test]
fn new_with_undefined_constructor_default_unnamed_via_new() {
    assert_errors_in_code(
        r#"
class A {
  A.name() {}
}
f() {
  A.new();
}
"#,
        &[("new_with_undefined_constructor_default", 37, 3)],
    );
}

/// `new_with_undefined_constructor_test.dart` `test_defaultViaNew`.
#[test]
fn new_with_undefined_constructor_default_via_new() {
    assert_errors_in_code(
        r#"
class A {
  A.new() {}
}
f() {
  A();
}
"#,
        &[],
    );
}

/// `new_with_undefined_constructor_test.dart` `test_defined_named`.
#[test]
fn new_with_undefined_constructor_defined_named() {
    assert_errors_in_code(
        r#"
class A {
  A.name() {}
}
f() {
  new A.name();
}
"#,
        &[],
    );
}

/// `new_with_undefined_constructor_test.dart` `test_defined_unnamed`.
#[test]
fn new_with_undefined_constructor_defined_unnamed() {
    assert_errors_in_code(
        r#"
class A {
  A() {}
}
f() {
  new A();
}
"#,
        &[],
    );
}

/// `new_with_undefined_constructor_test.dart` `test_named`.
#[test]
fn new_with_undefined_constructor_named() {
    assert_errors_in_code(
        r#"
class A {
  A() {}
}
f() {
  new A.name();
}
"#,
        &[("new_with_undefined_constructor", 36, 4)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_implicit_subclass_explicit_constructor_newHead`.
#[test]
fn no_default_super_constructor_super_implicit_subclass_explicit_constructor_new_head() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {
  new named();
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_implicit_subclass_explicit_constructor_typeName`.
#[test]
fn no_default_super_constructor_super_implicit_subclass_explicit_constructor_type_name() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {
  B.named();
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_implicit_subclass_explicit_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_implicit_subclass_explicit_primary_constructor_has_body() {
    assert_errors_in_code(
        r#"
class A {}
class B() extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_implicit_subclass_explicit_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_implicit_subclass_explicit_primary_constructor_no_body() {
    assert_errors_in_code(
        r#"
class A {}
class B() extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_implicit_subclass_implicit`.
#[test]
fn no_default_super_constructor_super_implicit_subclass_implicit() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_noParameters`.
#[test]
fn no_default_super_constructor_super_no_parameters() {
    assert_errors_in_code(
        r#"
class A {
  A();
}
class B extends A {
  B();
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalNamed_subclass_explicit_constructor_newHead`.
#[test]
fn no_default_super_constructor_super_optional_named_subclass_explicit_constructor_new_head() {
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}
class B extends A {
  new named();
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalNamed_subclass_explicit_constructor_typeName`.
#[test]
fn no_default_super_constructor_super_optional_named_subclass_explicit_constructor_type_name() {
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}
class B extends A {
  B.named();
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalNamed_subclass_explicit_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_optional_named_subclass_explicit_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}
class B() extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalNamed_subclass_explicit_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_optional_named_subclass_explicit_primary_constructor_no_body()
{
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}
class B() extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalNamed_subclass_implicit`.
#[test]
fn no_default_super_constructor_super_optional_named_subclass_implicit() {
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}
class B extends A {}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalNamed_subclass_superParameter_constructor`.
#[test]
fn no_default_super_constructor_super_optional_named_subclass_super_parameter_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}
class B extends A {
  B({super.a});
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalNamed_subclass_superParameter_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_optional_named_subclass_super_parameter_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}
class B({super.a}) extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalNamed_subclass_superParameter_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_optional_named_subclass_super_parameter_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({int? a});
}
class B({super.a}) extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalPositional_subclass_explicit_constructor_newHead`.
#[test]
fn no_default_super_constructor_super_optional_positional_subclass_explicit_constructor_new_head() {
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}
class B extends A {
  new named();
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalPositional_subclass_explicit_constructor_typeName`.
#[test]
fn no_default_super_constructor_super_optional_positional_subclass_explicit_constructor_type_name()
{
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}
class B extends A {
  B.named();
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalPositional_subclass_explicit_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_optional_positional_subclass_explicit_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}
class B() extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalPositional_subclass_explicit_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_optional_positional_subclass_explicit_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}
class B() extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalPositional_subclass_implicit`.
#[test]
fn no_default_super_constructor_super_optional_positional_subclass_implicit() {
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}
class B extends A {}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalPositional_subclass_superParameter_constructor`.
#[test]
fn no_default_super_constructor_super_optional_positional_subclass_super_parameter_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}
class B extends A {
  B(super.a);
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalPositional_subclass_superParameter_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_optional_positional_subclass_super_parameter_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}
class B([super.a]) extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_optionalPositional_subclass_superParameter_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_optional_positional_subclass_super_parameter_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A([int? a]);
}
class B([super.a]) extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_explicit_constructor_newHead`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_explicit_constructor_new_head() {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B extends A {
  new named();
}
"#,
        &[("implicit_super_initializer_missing_arguments", 59, 9)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_explicit_constructor_typeName`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_explicit_constructor_type_name() {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B extends A {
  B.named();
}
"#,
        &[("implicit_super_initializer_missing_arguments", 59, 7)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_explicit_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_explicit_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B() extends A {
  this;
}
"#,
        &[("implicit_super_initializer_missing_arguments", 61, 4)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_explicit_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_explicit_primary_constructor_no_body()
{
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B() extends A;
"#,
        &[("implicit_super_initializer_missing_arguments", 43, 1)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_implicit`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_implicit() {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B extends A {}
"#,
        &[("no_default_super_constructor", 43, 1)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_constructor`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B extends A {
  B({required super.a});
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_oneLeft_constructor`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_one_left_constructor()
{
    assert_errors_in_code(
        r#"
class A {
  A({required int? a, required int? b});
}
class B extends A {
  B({required super.a});
}
"#,
        &[("implicit_super_initializer_missing_arguments", 76, 1)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_oneLeft_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_one_left_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a, required int? b});
}
class B({required super.a}) extends A {
  this;
}
"#,
        &[("implicit_super_initializer_missing_arguments", 96, 4)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_oneLeft_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_one_left_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a, required int? b});
}
class B({required super.a}) extends A;
"#,
        &[("implicit_super_initializer_missing_arguments", 60, 1)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_optionalNamed_hasDefault_constructor`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_optional_named_has_default_constructor()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B extends A {
  B({super.a = 0});
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_optionalNamed_hasDefault_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_optional_named_has_default_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B({super.a = 0}) extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_optionalNamed_hasDefault_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_optional_named_has_default_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B({super.a = 0}) extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_optionalNamed_noDefault_constructor`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_optional_named_no_default_constructor()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B extends A {
  B({super.a});
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_optionalNamed_noDefault_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_optional_named_no_default_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B({super.a}) extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_optionalNamed_noDefault_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_optional_named_no_default_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B({super.a}) extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B({required super.a}) extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredNamed_subclass_superParameter_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_required_named_subclass_super_parameter_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A({required int? a});
}
class B({required super.a}) extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_explicit_constructor_newHead`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_explicit_constructor_new_head() {
    assert_errors_in_code(
        r#"
class A {
  A(int p);
}
class B extends A {
  new named();
}
"#,
        &[("implicit_super_initializer_missing_arguments", 47, 9)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_explicit_constructor_typeName`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_explicit_constructor_type_name()
{
    assert_errors_in_code(
        r#"
class A {
  A(int p);
}
class B extends A {
  B.named();
}
"#,
        &[("implicit_super_initializer_missing_arguments", 47, 7)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_explicit_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_explicit_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int p);
}
class B() extends A {
  this;
}
"#,
        &[("implicit_super_initializer_missing_arguments", 49, 4)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_explicit_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_explicit_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int p);
}
class B() extends A;
"#,
        &[("implicit_super_initializer_missing_arguments", 31, 1)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_external`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_external() {
    assert_errors_in_code(
        r#"
class A {
  A(int p);
}
class B extends A {
  external B();
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_implicit`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_implicit() {
    assert_errors_in_code(
        r#"
class A {
  A(int p);
}
class B extends A {}
"#,
        &[("no_default_super_constructor", 31, 1)],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_superParameter_optionalPositional_withDefault_constructor`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_super_parameter_optional_positional_with_default_constructor()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int? a);
}
class B extends A {
  B([super.a = 0]);
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_superParameter_optionalPositional_withDefault_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_super_parameter_optional_positional_with_default_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int? a);
}
class B([super.a = 0]) extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_superParameter_optionalPositional_withDefault_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_super_parameter_optional_positional_with_default_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int? a);
}
class B([super.a = 0]) extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_superParameter_optionalPositional_withoutDefault_constructor`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_super_parameter_optional_positional_without_default_constructor()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int? a);
}
class B extends A {
  B([super.a]);
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_superParameter_optionalPositional_withoutDefault_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_super_parameter_optional_positional_without_default_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int? a);
}
class B([super.a]) extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_superParameter_optionalPositional_withoutDefault_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_super_parameter_optional_positional_without_default_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int? a);
}
class B([super.a]) extends A;
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_superParameter_requiredPositional_constructor`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_super_parameter_required_positional_constructor()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int? a);
}
class B extends A {
  B(super.a);
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_superParameter_requiredPositional_primaryConstructor_hasBody`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_super_parameter_required_positional_primary_constructor_has_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int? a);
}
class B(super.a) extends A {
  this;
}
"#,
        &[],
    );
}

/// `no_default_super_constructor_test.dart` `test_super_requiredPositional_subclass_superParameter_requiredPositional_primaryConstructor_noBody`.
#[test]
fn no_default_super_constructor_super_required_positional_subclass_super_parameter_required_positional_primary_constructor_no_body()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int? a);
}
class B(super.a) extends A;
"#,
        &[],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_factoryHead_unnamed`.
#[test]
fn non_const_generative_enum_constructor_factory_head_unnamed() {
    assert_errors_in_code(
        r#"
enum E {
  v.named();
  const E.named();
  factory () => v;
}
"#,
        &[],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_generative_const_newHead_named`.
#[test]
fn non_const_generative_enum_constructor_generative_const_new_head_named() {
    assert_errors_in_code(
        r#"
enum E {
  v.named();
  const new named();
}
"#,
        &[],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_generative_const_newHead_unnamed`.
#[test]
fn non_const_generative_enum_constructor_generative_const_new_head_unnamed() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  const new ();
}
"#,
        &[],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_generative_const_typeName`.
#[test]
fn non_const_generative_enum_constructor_generative_const_type_name() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  const E();
}
"#,
        &[],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_generative_nonConst_newHead_unnamed`.
#[test]
fn non_const_generative_enum_constructor_generative_non_const_new_head_unnamed() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  new ();
}
"#,
        &[],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_generative_nonConst_typeName_named`.
#[test]
fn non_const_generative_enum_constructor_generative_non_const_type_name_named() {
    assert_errors_in_code(
        r#"
enum E {
  v.named();
  E.named();
}
"#,
        &[],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_generative_nonConst_typeName_named_language310`.
#[test]
fn non_const_generative_enum_constructor_generative_non_const_type_name_named_language310() {
    assert_errors_in_code(
        r#"
// @dart = 3.10
enum E {
  v.named();
  E.named();
}
"#,
        &[("non_const_generative_enum_constructor", 41, 7)],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_generative_nonConst_typeName_unnamed`.
#[test]
fn non_const_generative_enum_constructor_generative_non_const_type_name_unnamed() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  E();
}
"#,
        &[],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_generative_nonConst_typeName_unnamed_language310`.
#[test]
fn non_const_generative_enum_constructor_generative_non_const_type_name_unnamed_language310() {
    assert_errors_in_code(
        r#"
// @dart = 3.10
enum E {
  v;
  E();
}
"#,
        &[("non_const_generative_enum_constructor", 33, 1)],
    );
}

/// `non_const_generative_enum_constructor_test.dart` `test_typeName_factory_named`.
#[test]
fn non_const_generative_enum_constructor_type_name_factory_named() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  factory E.foo() => v;
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_optionalNamed_typeInt_final`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_optional_named_type_int_final() {
    assert_errors_in_code(
        r#"
enum E({final int foo = 0}) {
  v(foo: 0);
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_optionalNamed_typeInt_var`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_optional_named_type_int_var() {
    assert_errors_in_code(
        r#"
enum E({var int foo = 0}) {
  v(foo: 0);
}
"#,
        &[("non_final_field_in_enum", 17, 3)],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_optionalPositional_typeInt_final`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_optional_positional_type_int_final() {
    assert_errors_in_code(
        r#"
enum E([final int foo = 0]) {
  v(0);
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_optionalPositional_typeInt_var`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_optional_positional_type_int_var() {
    assert_errors_in_code(
        r#"
enum E([var int foo = 0]) {
  v(0);
}
"#,
        &[("non_final_field_in_enum", 17, 3)],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_requiredNamed_typeInt_final`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_required_named_type_int_final() {
    assert_errors_in_code(
        r#"
enum E({required final int foo}) {
  v(foo: 0);
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_requiredNamed_typeInt_var`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_required_named_type_int_var() {
    assert_errors_in_code(
        r#"
enum E({required var int foo}) {
  v(foo: 0);
}
"#,
        &[("non_final_field_in_enum", 26, 3)],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_requiredPositional_functionTyped_final`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_required_positional_function_typed_final() {
    assert_errors_in_code(
        r#"
enum E(final void foo()?) {
  v(null);
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_requiredPositional_functionTyped_var`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_required_positional_function_typed_var() {
    assert_errors_in_code(
        r#"
enum E(var void foo()?) {
  v(null);
}
"#,
        &[("non_final_field_in_enum", 17, 3)],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_requiredPositional_typeInt_final`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_required_positional_type_int_final() {
    assert_errors_in_code(
        r#"
enum E(final int foo) {
  v(0);
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_declaringFormalParameter_requiredPositional_typeInt_var`.
#[test]
fn non_final_field_in_enum_declaring_formal_parameter_required_positional_type_int_var() {
    assert_errors_in_code(
        r#"
enum E(var int foo) {
  v(0);
}
"#,
        &[("non_final_field_in_enum", 16, 3)],
    );
}

/// `non_final_field_in_enum_test.dart` `test_fieldDeclaration_instance`.
#[test]
fn non_final_field_in_enum_field_declaration_instance() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  int foo = 0;
}
"#,
        &[("non_final_field_in_enum", 21, 3)],
    );
}

/// `non_final_field_in_enum_test.dart` `test_fieldDeclaration_instance_covariant`.
#[test]
fn non_final_field_in_enum_field_declaration_instance_covariant() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  covariant int foo = 0;
}
"#,
        &[("non_final_field_in_enum", 31, 3)],
    );
}

/// `non_final_field_in_enum_test.dart` `test_fieldDeclaration_instance_external`.
#[test]
fn non_final_field_in_enum_field_declaration_instance_external() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  external int foo;
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_fieldDeclaration_instance_external_final`.
#[test]
fn non_final_field_in_enum_field_declaration_instance_external_final() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  external final int foo;
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_fieldDeclaration_instance_final`.
#[test]
fn non_final_field_in_enum_field_declaration_instance_final() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  final int foo = 0;
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_fieldDeclaration_instance_late`.
#[test]
fn non_final_field_in_enum_field_declaration_instance_late() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  late int foo = 0;
}
"#,
        &[("non_final_field_in_enum", 26, 3)],
    );
}

/// `non_final_field_in_enum_test.dart` `test_fieldDeclaration_static`.
#[test]
fn non_final_field_in_enum_field_declaration_static() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static int foo = 0;
}
"#,
        &[],
    );
}

/// `non_final_field_in_enum_test.dart` `test_fieldDeclaration_static_final`.
#[test]
fn non_final_field_in_enum_field_declaration_static_final() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static final int foo = 0;
}
"#,
        &[],
    );
}

/// `non_generative_constructor_test.dart` `test_factory_explicit_constructor`.
#[test]
fn non_generative_constructor_factory_explicit_constructor() {
    assert_errors_in_code(
        r#"
class A {
  factory A.named() => throw 0;
  A.generative();
}
class B extends A {
  B() : super.named();
}
"#,
        &[("non_generative_constructor", 91, 13)],
    );
}

/// `non_generative_constructor_test.dart` `test_factory_explicit_primaryConstructor_hasBody`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn non_generative_constructor_factory_explicit_primary_constructor_has_body() {
    assert_errors_in_code(
        r#"
class A {
  factory A.named() => throw 0;
  A.generative();
}
class B() extends A {
  this : super.named();
}
"#,
        &[("non_generative_constructor", 94, 13)],
    );
}

/// `non_generative_constructor_test.dart` `test_factory_implicit_constructor_newHead`.
#[test]
fn non_generative_constructor_factory_implicit_constructor_new_head() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw 0;
  A.named();
}
class B extends A {
  new foo();
}
"#,
        &[("non_generative_constructor", 74, 7)],
    );
}

/// `non_generative_constructor_test.dart` `test_factory_implicit_constructor_typeName`.
#[test]
fn non_generative_constructor_factory_implicit_constructor_type_name() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw 0;
  A.named();
}
class B extends A {
  B.foo();
}
"#,
        &[("non_generative_constructor", 74, 5)],
    );
}

/// `non_generative_constructor_test.dart` `test_factory_implicit_constructor_typeName_external`.
#[test]
fn non_generative_constructor_factory_implicit_constructor_type_name_external() {
    assert_errors_in_code(
        r#"
class A {
  A.named() {}
  factory A() => throw 0;
}
class B extends A {
  external B();
}
"#,
        &[],
    );
}

/// `non_generative_constructor_test.dart` `test_factory_implicit_primaryConstructor_hasBody`.
#[test]
fn non_generative_constructor_factory_implicit_primary_constructor_has_body() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw 0;
  A.named();
}
class B() extends A {
  this;
}
"#,
        &[("non_generative_constructor", 76, 4)],
    );
}

/// `non_generative_constructor_test.dart` `test_generative_constructor`.
#[test]
fn non_generative_constructor_generative_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.named() {}
  factory A() => throw 0;
}
class B extends A {
  B() : super.named();
}
"#,
        &[],
    );
}

/// `non_generative_constructor_test.dart` `test_generative_primaryConstructor`.
#[test]
fn non_generative_constructor_generative_primary_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.named() {}
  factory A() => throw 0;
}
class B() extends A {
  this : super.named();
}
"#,
        &[],
    );
}

/// `non_generative_constructor_test.dart` `test_generative_primaryContructor`.
#[test]
fn non_generative_constructor_generative_primary_contructor() {
    assert_errors_in_code(
        r#"
class A {
  A.named() {}
  factory A() => throw 0;
}
class B() extends A {
  this : super.named();
}
"#,
        &[],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_factory`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_factory() {
    assert_errors_in_code(
        r#"
class C(int x) {
  factory C.named() => C(0);
}
"#,
        &[],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_generative_augmentationOfPrimary`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_generative_augmentation_of_primary() {
    assert_errors_in_code(
        r#"
class C({int p = 0});

augment class C {
  augment C({int p});
}
"#,
        &[],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_generative_augmentationRedirectingToPrimary`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_generative_augmentation_redirecting_to_primary()
 {
    assert_errors_in_code(
        r#"
class C(int x) {
  C.named(int x);
}

augment class C {
  augment C.named(int x) : this(x);
}
"#,
        &[],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_generative_augmentationRedirectingToPrimary_sameClass`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_generative_augmentation_redirecting_to_primary_same_class()
 {
    assert_errors_in_code(
        r#"
class C(int x) {
  C.named(int x);
  augment C.named(int x) : this(x);
}
"#,
        &[],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_generative_augmentationRedirectingToUnresolved`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_generative_augmentation_redirecting_to_unresolved()
 {
    assert_errors_in_code(
        r#"
class C(int x) {
  C.named();
}

augment class C {
  augment C.named() : this.missing();
}
"#,
        &[("redirect_generative_to_missing_constructor", 74, 14)],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_generative_nonRedirecting`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_generative_non_redirecting() {
    assert_errors_in_code(
        r#"
class C(int x) {
  C.named();
}
"#,
        &[("non_redirecting_generative_constructor_with_primary", 20, 7)],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_generative_redirectingToNonPrimary`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_generative_redirecting_to_non_primary()
{
    assert_errors_in_code(
        r#"
class C(int x) {
  C.named1() : this.named2();
  C.named2();
}
"#,
        &[("non_redirecting_generative_constructor_with_primary", 50, 8)],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_generative_redirectingToPrimary`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_generative_redirecting_to_primary() {
    assert_errors_in_code(
        r#"
class C(int x) {
  C.named() : this(0);
}
"#,
        &[],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_generative_redirectingToUnresolved`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_generative_redirecting_to_unresolved()
{
    assert_errors_in_code(
        r#"
class C(int x) {
  C.named() : this.missing();
}
"#,
        &[("redirect_generative_to_missing_constructor", 32, 14)],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_class_noPrimaryConstructor`.
#[test]
fn non_redirecting_generative_constructor_with_primary_class_no_primary_constructor() {
    assert_errors_in_code(
        r#"
class C {
  C.named();
}
"#,
        &[],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_enum_factory`.
#[test]
fn non_redirecting_generative_constructor_with_primary_enum_factory() {
    assert_errors_in_code(
        r#"
enum E(int x) {
  v(0);
  factory E.named() => E.v;
}
"#,
        &[],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_enum_generative_nonRedirecting`.
#[test]
fn non_redirecting_generative_constructor_with_primary_enum_generative_non_redirecting() {
    assert_errors_in_code(
        r#"
enum E(int x) {
  v(0);
  const E.named();
}
"#,
        &[
            ("non_redirecting_generative_constructor_with_primary", 33, 7),
            ("unused_element", 35, 5),
        ],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_enum_generative_redirectingToPrimary`.
#[test]
fn non_redirecting_generative_constructor_with_primary_enum_generative_redirecting_to_primary() {
    assert_errors_in_code(
        r#"
enum E(int x) {
  v(0);
  const E.named(int x) : this(x);
}
"#,
        &[("unused_element", 35, 5)],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_enum_noPrimaryConstructor`.
#[test]
fn non_redirecting_generative_constructor_with_primary_enum_no_primary_constructor() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  const E();
}
"#,
        &[],
    );
}

/// `non_redirecting_generative_constructor_with_primary_test.dart` `test_extensionType_generative_nonRedirecting`.
#[test]
fn non_redirecting_generative_constructor_with_primary_extension_type_generative_non_redirecting() {
    assert_errors_in_code(
        r#"
extension type E(int x) {
  E.named(this.x);
}
"#,
        &[],
    );
}

/// `private_optional_parameter_test.dart` `test_class_constructorDeclaration_fieldFormal`.
#[test]
fn private_optional_parameter_class_constructor_declaration_field_formal() {
    assert_errors_in_code(
        r#"
class A {
  int? _p;
  A({this._p = 0});
}
"#,
        &[("unused_field", 18, 2)],
    );
}

/// `private_optional_parameter_test.dart` `test_class_constructorDeclaration_nonFieldParameter`.
#[test]
fn private_optional_parameter_class_constructor_declaration_non_field_parameter() {
    assert_errors_in_code(
        r#"
class C {
  C({int? _notField});
}
"#,
        &[("private_named_non_field_parameter", 21, 9)],
    );
}

/// `private_optional_parameter_test.dart` `test_class_constructorDeclaration_noPublicName_nonIdentifier`.
#[test]
fn private_optional_parameter_class_constructor_declaration_no_public_name_non_identifier() {
    assert_errors_in_code(
        r#"
class C {
  int? _123;
  C({this._123}) {}
}
"#,
        &[
            ("unused_field", 18, 4),
            ("private_named_parameter_without_public_name", 34, 4),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_constructorDeclaration_noPublicName_preFeature`.
#[test]
fn private_optional_parameter_class_constructor_declaration_no_public_name_pre_feature() {
    assert_errors_in_code(
        r#"
// @dart=3.10
class C {
  int? _123;
  C({this._123}) {}
}
"#,
        &[("unused_field", 32, 4), ("experiment_not_enabled", 48, 4)],
    );
}

/// `private_optional_parameter_test.dart` `test_class_constructorDeclaration_noPublicName_reservedWord`.
#[test]
fn private_optional_parameter_class_constructor_declaration_no_public_name_reserved_word() {
    assert_errors_in_code(
        r#"
class C {
  int? _for;
  C({this._for}) {}
}
"#,
        &[
            ("unused_field", 18, 4),
            ("private_named_parameter_without_public_name", 34, 4),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_constructorDeclaration_noPublicName_stillPrivate`.
#[test]
fn private_optional_parameter_class_constructor_declaration_no_public_name_still_private() {
    assert_errors_in_code(
        r#"
class C {
  int? __extraPrivate;
  C({this.__extraPrivate}) {}
}
"#,
        &[
            ("unused_field", 18, 14),
            ("private_named_parameter_without_public_name", 44, 14),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_constructorDeclaration_noPublicName_wildcard`.
#[test]
fn private_optional_parameter_class_constructor_declaration_no_public_name_wildcard() {
    assert_errors_in_code(
        r#"
class C {
  int? _;
  C({this._}) {}
}
"#,
        &[
            ("unused_field", 18, 1),
            ("private_named_parameter_without_public_name", 31, 1),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_method`.
#[test]
fn private_optional_parameter_class_method() {
    assert_errors_in_code(
        r#"
class A {
  void f({int? _p}) {}
}
"#,
        &[("private_named_non_field_parameter", 26, 2)],
    );
}

/// `private_optional_parameter_test.dart` `test_class_method_noPublicName`.
#[test]
fn private_optional_parameter_class_method_no_public_name() {
    assert_errors_in_code(
        r#"
class A {
  void f({int? _123}) {}
}
"#,
        &[("private_named_non_field_parameter", 26, 4)],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_declaringFormalParameter_optionalNamed`.
#[test]
fn private_optional_parameter_class_primary_constructor_declaring_formal_parameter_optional_named()
{
    assert_errors_in_code(
        r#"
class A({final int _p = 0}) {}
"#,
        &[("unused_field_from_primary_constructor", 20, 2)],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_declaringFormalParameter_optionalNamed_noPublicName_nonIdentifier`.
#[test]
fn private_optional_parameter_class_primary_constructor_declaring_formal_parameter_optional_named_no_public_name_non_identifier()
 {
    assert_errors_in_code(
        r#"
class C({final int? _123}) {}
"#,
        &[
            ("private_named_parameter_without_public_name", 21, 4),
            ("unused_field_from_primary_constructor", 21, 4),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_declaringFormalParameter_optionalNamed_noPublicName_reservedWord`.
#[test]
fn private_optional_parameter_class_primary_constructor_declaring_formal_parameter_optional_named_no_public_name_reserved_word()
 {
    assert_errors_in_code(
        r#"
class C({final int? _for}) {}
"#,
        &[
            ("private_named_parameter_without_public_name", 21, 4),
            ("unused_field_from_primary_constructor", 21, 4),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_declaringFormalParameter_optionalNamed_noPublicName_stillPrivate`.
#[test]
fn private_optional_parameter_class_primary_constructor_declaring_formal_parameter_optional_named_no_public_name_still_private()
 {
    assert_errors_in_code(
        r#"
class C({final int? __extraPrivate}) {}
"#,
        &[
            ("private_named_parameter_without_public_name", 21, 14),
            ("unused_field_from_primary_constructor", 21, 14),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_declaringFormalParameter_optionalNamed_noPublicName_wildcard`.
#[test]
fn private_optional_parameter_class_primary_constructor_declaring_formal_parameter_optional_named_no_public_name_wildcard()
 {
    assert_errors_in_code(
        r#"
class C({final int? _}) {}
"#,
        &[
            ("private_named_parameter_without_public_name", 21, 1),
            ("unused_field_from_primary_constructor", 21, 1),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_formalParameter_optionalNamed_fieldFormal`.
#[test]
fn private_optional_parameter_class_primary_constructor_formal_parameter_optional_named_field_formal()
 {
    assert_errors_in_code(
        r#"
class A({this._p = 0}) {
  int? _p;
}
"#,
        &[("unused_field", 33, 2)],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_formalParameter_optionalNamed_fieldFormal_noPublicName_nonIdentifier`.
#[test]
fn private_optional_parameter_class_primary_constructor_formal_parameter_optional_named_field_formal_no_public_name_non_identifier()
 {
    assert_errors_in_code(
        r#"
class C({this._123}) {
  int? _123;
}
"#,
        &[
            ("private_named_parameter_without_public_name", 15, 4),
            ("unused_field", 31, 4),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_formalParameter_optionalNamed_fieldFormal_noPublicName_reservedWord`.
#[test]
fn private_optional_parameter_class_primary_constructor_formal_parameter_optional_named_field_formal_no_public_name_reserved_word()
 {
    assert_errors_in_code(
        r#"
class C({this._for}) {
  int? _for;
}
"#,
        &[
            ("private_named_parameter_without_public_name", 15, 4),
            ("unused_field", 31, 4),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_formalParameter_optionalNamed_fieldFormal_noPublicName_stillPrivate`.
#[test]
fn private_optional_parameter_class_primary_constructor_formal_parameter_optional_named_field_formal_no_public_name_still_private()
 {
    assert_errors_in_code(
        r#"
class C({this.__extraPrivate}) {
  int? __extraPrivate;
}
"#,
        &[
            ("private_named_parameter_without_public_name", 15, 14),
            ("unused_field", 41, 14),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_formalParameter_optionalNamed_fieldFormal_noPublicName_wildcard`.
#[test]
fn private_optional_parameter_class_primary_constructor_formal_parameter_optional_named_field_formal_no_public_name_wildcard()
 {
    assert_errors_in_code(
        r#"
class C({this._}) {
  int? _;
}
"#,
        &[
            ("private_named_parameter_without_public_name", 15, 1),
            ("unused_field", 28, 1),
        ],
    );
}

/// `private_optional_parameter_test.dart` `test_class_primaryConstructor_formalParameter_optionalNamed_nonFieldFormal`.
#[test]
fn private_optional_parameter_class_primary_constructor_formal_parameter_optional_named_non_field_formal()
 {
    assert_errors_in_code(
        r#"
class C({int? _notField}) {}
"#,
        &[("private_named_non_field_parameter", 15, 9)],
    );
}

/// `private_optional_parameter_test.dart` `test_extensionType_method`.
#[test]
fn private_optional_parameter_extension_type_method() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  void f({int? _p}) {}
}
"#,
        &[("private_named_non_field_parameter", 43, 2)],
    );
}

/// `private_optional_parameter_test.dart` `test_extensionType_primaryConstructor_requiredNamed`.
#[test]
fn private_optional_parameter_extension_type_primary_constructor_required_named() {
    assert_errors_in_code(
        r#"
extension type E({required int _it});
"#,
        &[],
    );
}

/// `private_optional_parameter_test.dart` `test_extensionType_primaryConstructor_requiredNamed_noPublicName_nonIdentifier`.
#[test]
fn private_optional_parameter_extension_type_primary_constructor_required_named_no_public_name_non_identifier()
 {
    assert_errors_in_code(
        r#"
extension type E({required int _123});
"#,
        &[("private_named_parameter_without_public_name", 32, 4)],
    );
}

/// `private_optional_parameter_test.dart` `test_topLevel_function`.
#[test]
fn private_optional_parameter_top_level_function() {
    assert_errors_in_code(
        r#"
void f({int? _p}) {}
"#,
        &[("private_named_non_field_parameter", 14, 2)],
    );
}

/// `private_optional_parameter_test.dart` `test_topLevel_function_withDefaultValue`.
#[test]
fn private_optional_parameter_top_level_function_with_default_value() {
    assert_errors_in_code(
        r#"
void f({int _p = 0}) {}
"#,
        &[("private_named_non_field_parameter", 13, 2)],
    );
}

/// `recursive_constructor_redirect_test.dart` `test_directSelfReference`.
#[test]
fn recursive_constructor_redirect_direct_self_reference() {
    assert_errors_in_code(
        r#"
class A {
  A() : this();
}
"#,
        &[("recursive_constructor_redirect", 19, 6)],
    );
}

/// `recursive_constructor_redirect_test.dart` `test_recursive`.
#[test]
fn recursive_constructor_redirect_recursive() {
    assert_errors_in_code(
        r#"
class A {
  A.a() : this.b();
  A.b() : this.a();
}
"#,
        &[
            ("recursive_constructor_redirect", 21, 8),
            ("recursive_constructor_redirect", 41, 8),
        ],
    );
}

/// `recursive_constructor_redirect_test.dart` `test_valid_redirect`.
#[test]
fn recursive_constructor_redirect_valid_redirect() {
    assert_errors_in_code(
        r#"
class A {
  A.a() : this.b();
  A.b() : this.c();
  A.c() {}
}
"#,
        &[],
    );
}

/// `redirect_generative_to_missing_constructor_test.dart` `test_class_primary_missing`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn redirect_generative_to_missing_constructor_class_primary_missing() {
    assert_errors_in_code(
        r#"
class A() {
  this : this.noSuchConstructor();
}
"#,
        &[("primary_constructor_cannot_redirect", 22, 4)],
    );
}

/// `redirect_generative_to_missing_constructor_test.dart` `test_class_typeName_missing`.
#[test]
fn redirect_generative_to_missing_constructor_class_type_name_missing() {
    assert_errors_in_code(
        r#"
class A {
  A() : this.noSuchConstructor();
}
"#,
        &[("redirect_generative_to_missing_constructor", 19, 24)],
    );
}

/// `redirect_generative_to_missing_constructor_test.dart` `test_enum_primary_missing`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn redirect_generative_to_missing_constructor_enum_primary_missing() {
    assert_errors_in_code(
        r#"
enum E() {
  v;
  this : this.noSuchConstructor();
}
"#,
        &[("primary_constructor_cannot_redirect", 26, 4)],
    );
}

/// `redirect_generative_to_missing_constructor_test.dart` `test_enum_typeName_missing`.
#[test]
fn redirect_generative_to_missing_constructor_enum_type_name_missing() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  const E() : this.noSuchConstructor();
}
"#,
        &[("redirect_generative_to_missing_constructor", 29, 24)],
    );
}

/// `redirect_generative_to_non_generative_constructor_test.dart` `test_primary_toFactory`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn redirect_generative_to_non_generative_constructor_primary_to_factory() {
    assert_errors_in_code(
        r#"
class A() {
  this : this.x();
  factory A.x() => throw 0;
}
"#,
        &[("primary_constructor_cannot_redirect", 22, 4)],
    );
}

/// `redirect_generative_to_non_generative_constructor_test.dart` `test_typeName_toFactory`.
#[test]
fn redirect_generative_to_non_generative_constructor_type_name_to_factory() {
    assert_errors_in_code(
        r#"
class A {
  A() : this.x();
  factory A.x() => throw 0;
}
"#,
        &[("redirect_generative_to_non_generative_constructor", 19, 8)],
    );
}

/// `redirect_to_abstract_class_constructor_test.dart` `test_abstractRedirectsToSelf`.
#[test]
fn redirect_to_abstract_class_constructor_abstract_redirects_to_self() {
    assert_errors_in_code(
        r#"
abstract class A {
  factory A() = A._;
  A._();
}
"#,
        &[("redirect_to_abstract_class_constructor", 36, 3)],
    );
}

/// `redirect_to_abstract_class_constructor_test.dart` `test_redirectsToAbstractSubclass`.
#[test]
fn redirect_to_abstract_class_constructor_redirects_to_abstract_subclass() {
    assert_errors_in_code(
        r#"
class A {
  factory A.named() = B;
  A();
}

abstract class B extends A {}
"#,
        &[("redirect_to_abstract_class_constructor", 33, 1)],
    );
}

/// `redirect_to_abstract_class_constructor_test.dart` `test_redirectsToSubclass`.
#[test]
fn redirect_to_abstract_class_constructor_redirects_to_subclass() {
    assert_errors_in_code(
        r#"
class A {
  factory A.named() = B;
  A();
}

class B extends A {}
"#,
        &[],
    );
}

/// `redirect_to_abstract_class_constructor_test.dart` `test_redirectsToSubclass_asTypedef`.
#[test]
fn redirect_to_abstract_class_constructor_redirects_to_subclass_as_typedef() {
    assert_errors_in_code(
        r#"
class A {
  factory A.named() = C;
  A();
}

class B extends A {}
typedef C = B;
"#,
        &[],
    );
}

/// `redirect_to_invalid_function_type_test.dart` `test_redirectToInvalidFunctionType`.
#[test]
fn redirect_to_invalid_function_type_redirect_to_invalid_function_type() {
    assert_errors_in_code(
        r#"
class A implements B {
  A(int p) {}
}
class B {
  factory B() = A;
}"#,
        &[("redirect_to_invalid_function_type", 66, 1)],
    );
}

/// `redirect_to_invalid_function_type_test.dart` `test_valid_redirect`.
#[test]
fn redirect_to_invalid_function_type_valid_redirect() {
    assert_errors_in_code(
        r#"
class A implements B {
  A(int p) {}
}
class B {
  factory B(int p) = A;
}
"#,
        &[],
    );
}

/// `redirect_to_invalid_return_type_test.dart` `test_redirectToInvalidReturnType`.
#[test]
fn redirect_to_invalid_return_type_redirect_to_invalid_return_type() {
    assert_errors_in_code(
        r#"
class A {
  A() {}
}
class B {
  factory B() = A;
}"#,
        &[("redirect_to_invalid_return_type", 48, 1)],
    );
}

/// `redirect_to_missing_constructor_test.dart` `test_named`.
#[test]
fn redirect_to_missing_constructor_named() {
    assert_errors_in_code(
        r#"
class A implements B{
  A() {}
}
class B {
  factory B() = A.name;
}"#,
        &[("redirect_to_missing_constructor", 60, 6)],
    );
}

/// `redirect_to_missing_constructor_test.dart` `test_unnamed`.
#[test]
fn redirect_to_missing_constructor_unnamed() {
    assert_errors_in_code(
        r#"
class A implements B{
  A.name() {}
}
class B {
  factory B() = A;
}"#,
        &[("redirect_to_missing_constructor", 65, 1)],
    );
}

/// `redirect_to_non_const_constructor_test.dart` `test_constRedirector_cannotResolveRedirectee`.
#[test]
fn redirect_to_non_const_constructor_const_redirector_cannot_resolve_redirectee() {
    assert_errors_in_code(
        r#"
class A {
  const factory A.b() = A.a;
}
"#,
        &[("redirect_to_missing_constructor", 35, 3)],
    );
}

/// `redirect_to_non_const_constructor_test.dart` `test_constRedirector_constRedirectee`.
#[test]
fn redirect_to_non_const_constructor_const_redirector_const_redirectee() {
    assert_errors_in_code(
        r#"
class A {
  const A.a();
  const factory A.b() = A.a;
}
"#,
        &[],
    );
}

/// `redirect_to_non_const_constructor_test.dart` `test_constRedirector_constRedirectee_generic`.
#[test]
fn redirect_to_non_const_constructor_const_redirector_const_redirectee_generic() {
    assert_errors_in_code(
        r#"
class A<T> {
  const A(T value) : this._(value);
  const A._(T value) : value = value;
  final T value;
}

void main(){
  const A<int>(1);
}
"#,
        &[],
    );
}

/// `redirect_to_non_const_constructor_test.dart` `test_constRedirector_constRedirectee_viaInitializer`.
#[test]
fn redirect_to_non_const_constructor_const_redirector_const_redirectee_via_initializer() {
    assert_errors_in_code(
        r#"
class A {
  const A.a();
  const A.b() : this.a();
}
"#,
        &[],
    );
}

/// `redirect_to_non_const_constructor_test.dart` `test_constRedirector_nonConstRedirectee`.
#[test]
fn redirect_to_non_const_constructor_const_redirector_non_const_redirectee() {
    assert_errors_in_code(
        r#"
class A {
  A.a();
  const factory A.b() = A.a;
}
"#,
        &[("redirect_to_non_const_constructor", 44, 3)],
    );
}

/// `redirect_to_non_const_constructor_test.dart` `test_constRedirector_nonConstRedirectee_viaInitializer`.
#[test]
fn redirect_to_non_const_constructor_const_redirector_non_const_redirectee_via_initializer() {
    assert_errors_in_code(
        r#"
class A {
  A.a();
  const A.b() : this.a();
}
"#,
        &[("redirect_to_non_const_constructor", 41, 1)],
    );
}

/// `redirect_to_non_const_constructor_test.dart` `test_constRedirector_nonConstRedirectee_viaInitializer_unnamed`.
#[test]
fn redirect_to_non_const_constructor_const_redirector_non_const_redirectee_via_initializer_unnamed()
{
    assert_errors_in_code(
        r#"
class A {
  A();
  const A.named() : this();
}
"#,
        &[("redirect_to_non_const_constructor", 38, 4)],
    );
}

/// `redirect_to_non_const_constructor_test.dart` `test_constRedirector_viaInitializer_cannotResolveRedirectee`.
#[test]
fn redirect_to_non_const_constructor_const_redirector_via_initializer_cannot_resolve_redirectee() {
    assert_errors_in_code(
        r#"
class A {
  const A.b() : this.a();
}
"#,
        &[("redirect_generative_to_missing_constructor", 27, 8)],
    );
}

/// `redirect_to_non_const_constructor_test.dart` `test_redirect_to_const`.
#[test]
fn redirect_to_non_const_constructor_redirect_to_const() {
    assert_errors_in_code(
        r#"
class A {
  const A.a();
  const factory A.b() = A.a;
}
"#,
        &[],
    );
}

/// `return_in_generative_constructor_test.dart` `test_blockBody`.
#[test]
#[ignore = "reported by ReturnTypeVerifier (error/*, branch wd-errors)"]
fn return_in_generative_constructor_block_body() {
    assert_errors_in_code(
        r#"
class A {
  A() { return 0; }
}
"#,
        &[("return_in_generative_constructor", 26, 1)],
    );
}

/// `return_in_generative_constructor_test.dart` `test_expressionFunctionBody`.
#[test]
fn return_in_generative_constructor_expression_function_body() {
    assert_errors_in_code(
        r#"
class A {
  A() => A();
}
"#,
        &[("return_in_generative_constructor", 17, 7)],
    );
}

/// `return_in_generative_constructor_test.dart` `test_return_without_value`.
#[test]
fn return_in_generative_constructor_return_without_value() {
    assert_errors_in_code(
        r#"
class A {
  A() { return; }
}
"#,
        &[],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_generic_requiredPositional_explicit_notSubtype`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_generic_required_positional_explicit_not_subtype()
 {
    assert_errors_in_code(
        r#"
class A<T> {
  A(T a);
}

class B extends A<int> {
  B(num super.a);
}
"#,
        &[(
            "super_formal_parameter_type_is_not_subtype_of_associated",
            66,
            1,
        )],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_generic_requiredPositional_explicit_same`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_generic_required_positional_explicit_same()
 {
    assert_errors_in_code(
        r#"
class A<T> {
  A(T a);
}

class B extends A<num> {
  B(num super.a);
}
"#,
        &[],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_generic_requiredPositional_explicit_subtype`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_generic_required_positional_explicit_subtype()
 {
    assert_errors_in_code(
        r#"
class A<T> {
  A(T a);
}

class B extends A<num> {
  B(int super.a);
}
"#,
        &[],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_requiredNamed_explicit_notSubtype`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_required_named_explicit_not_subtype() {
    assert_errors_in_code(
        r#"
class A {
  A({required int a});
}

class B extends A {
  B({required num super.a});
}
"#,
        &[(
            "super_formal_parameter_type_is_not_subtype_of_associated",
            81,
            1,
        )],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_requiredNamed_explicit_same`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_required_named_explicit_same() {
    assert_errors_in_code(
        r#"
class A {
  A({required num a});
}

class B extends A {
  B({required num super.a});
}
"#,
        &[],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_requiredNamed_explicit_subtype`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_required_named_explicit_subtype() {
    assert_errors_in_code(
        r#"
class A {
  A({required num a});
}

class B extends A {
  B({required int super.a});
}
"#,
        &[],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_requiredNamed_inherited`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_required_named_inherited() {
    assert_errors_in_code(
        r#"
class A {
  A({required int a});
}

class B extends A {
  B({required super.a});
}
"#,
        &[],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_requiredPositional_explicit_notSubtype`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_required_positional_explicit_not_subtype()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int a);
}

class B extends A {
  B(num super.a);
}
"#,
        &[(
            "super_formal_parameter_type_is_not_subtype_of_associated",
            60,
            1,
        )],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_requiredPositional_explicit_notSubtype_dynamic`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_required_positional_explicit_not_subtype_dynamic()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int a);
}

class B extends A {
  B(dynamic super.a);
}
"#,
        &[(
            "super_formal_parameter_type_is_not_subtype_of_associated",
            64,
            1,
        )],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_requiredPositional_explicit_same`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_required_positional_explicit_same() {
    assert_errors_in_code(
        r#"
class A {
  A(num a);
}

class B extends A {
  B(num super.a);
}
"#,
        &[],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_requiredPositional_explicit_subtype`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_required_positional_explicit_subtype() {
    assert_errors_in_code(
        r#"
class A {
  A(num a);
}

class B extends A {
  B(int super.a);
}
"#,
        &[],
    );
}

/// `super_formal_parameter_type_is_not_subtype_of_associated_test.dart` `test_requiredPositional_inherited`.
#[test]
fn super_formal_parameter_type_is_not_subtype_of_associated_required_positional_inherited() {
    assert_errors_in_code(
        r#"
class A {
  A(int a);
}

class B extends A {
  B(super.a);
}
"#,
        &[],
    );
}

/// `super_formal_parameter_without_associated_named_test.dart` `test_explicit_optional`.
#[test]
fn super_formal_parameter_without_associated_named_explicit_optional() {
    assert_errors_in_code(
        r#"
class A {}

class B extends A {
  B({super.a}) : super();
}
"#,
        &[("super_formal_parameter_without_associated_named", 44, 1)],
    );
}

/// `super_formal_parameter_without_associated_named_test.dart` `test_explicit_required`.
#[test]
fn super_formal_parameter_without_associated_named_explicit_required() {
    assert_errors_in_code(
        r#"
class A {}

class B extends A {
  B({required super.a}) : super();
}
"#,
        &[("super_formal_parameter_without_associated_named", 53, 1)],
    );
}

/// `super_formal_parameter_without_associated_named_test.dart` `test_implicit_optional`.
#[test]
fn super_formal_parameter_without_associated_named_implicit_optional() {
    assert_errors_in_code(
        r#"
class A {}

class B extends A {
  B({super.a});
}
"#,
        &[("super_formal_parameter_without_associated_named", 44, 1)],
    );
}

/// `super_formal_parameter_without_associated_named_test.dart` `test_implicit_required`.
#[test]
fn super_formal_parameter_without_associated_named_implicit_required() {
    assert_errors_in_code(
        r#"
class A {}

class B extends A {
  B({required super.a});
}
"#,
        &[("super_formal_parameter_without_associated_named", 53, 1)],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_class_primary_optionalPositional_01`.
#[test]
fn super_formal_parameter_without_associated_positional_class_primary_optional_positional_01() {
    assert_errors_in_code(
        r#"
class A {}

class B([super.a]) extends A {}
"#,
        &[(
            "super_formal_parameter_without_associated_positional",
            28,
            1,
        )],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_class_primary_requiredPositional_01`.
#[test]
fn super_formal_parameter_without_associated_positional_class_primary_required_positional_01() {
    assert_errors_in_code(
        r#"
class A {}

class B(super.a) extends A {}
"#,
        &[(
            "super_formal_parameter_without_associated_positional",
            27,
            1,
        )],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_class_secondary_optionalPositional_01`.
#[test]
fn super_formal_parameter_without_associated_positional_class_secondary_optional_positional_01() {
    assert_errors_in_code(
        r#"
class A {}

class B extends A {
  B([super.a]);
}
"#,
        &[(
            "super_formal_parameter_without_associated_positional",
            44,
            1,
        )],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_class_secondary_requiredPositional_01`.
#[test]
fn super_formal_parameter_without_associated_positional_class_secondary_required_positional_01() {
    assert_errors_in_code(
        r#"
class A {}

class B extends A {
  B(super.a);
}
"#,
        &[(
            "super_formal_parameter_without_associated_positional",
            43,
            1,
        )],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_class_secondary_requiredPositional_12`.
#[test]
fn super_formal_parameter_without_associated_positional_class_secondary_required_positional_12() {
    assert_errors_in_code(
        r#"
class A {
  A(int a);
}

class B extends A {
  B(super.a, super.b);
}
"#,
        &[(
            "super_formal_parameter_without_associated_positional",
            65,
            1,
        )],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_enum_primary_requiredPositional_01`.
#[test]
fn super_formal_parameter_without_associated_positional_enum_primary_required_positional_01() {
    assert_errors_in_code(
        r#"
enum E(super.a) {
  v(0);
}
"#,
        &[(
            "super_formal_parameter_without_associated_positional",
            14,
            1,
        )],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_enum_secondary_requiredPositional_01`.
#[test]
fn super_formal_parameter_without_associated_positional_enum_secondary_required_positional_01() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  const E(super.x);
}
"#,
        &[(
            "super_formal_parameter_without_associated_positional",
            34,
            1,
        )],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_recovery_hasSuperClass_noSuperConstructor_primary`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn super_formal_parameter_without_associated_positional_recovery_has_super_class_no_super_constructor_primary()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int x);
}

class B(super.a) extends A {
  this : super.named();
}
"#,
        &[("undefined_constructor_in_initializer", 64, 13)],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_recovery_hasSuperClass_noSuperConstructor_secondary`.
#[test]
fn super_formal_parameter_without_associated_positional_recovery_has_super_class_no_super_constructor_secondary()
 {
    assert_errors_in_code(
        r#"
class A {
  A(int x);
}

class B extends A {
  B(super.x) : super.named();
}
"#,
        &[("undefined_constructor_in_initializer", 61, 13)],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_recovery_noSuperClass_primary`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn super_formal_parameter_without_associated_positional_recovery_no_super_class_primary() {
    assert_errors_in_code(
        r#"
class B(super.a) extends A {
  this : super.named();
}
"#,
        &[
            ("extends_non_class", 26, 1),
            ("undefined_constructor_in_initializer", 39, 13),
        ],
    );
}

/// `super_formal_parameter_without_associated_positional_test.dart` `test_recovery_noSuperClass_secondary`.
#[test]
fn super_formal_parameter_without_associated_positional_recovery_no_super_class_secondary() {
    assert_errors_in_code(
        r#"
class B extends A {
  B(super.x) : super.named();
}
"#,
        &[
            ("extends_non_class", 17, 1),
            ("undefined_constructor_in_initializer", 36, 13),
        ],
    );
}

/// `super_in_enum_constructor_test.dart` `test_primary_one`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn super_in_enum_constructor_primary_one() {
    assert_errors_in_code(
        r#"
enum E() {
  v;
  this : super();
}
"#,
        &[("super_in_enum_constructor", 26, 5)],
    );
}

/// `super_in_enum_constructor_test.dart` `test_typeName_hasRedirect`.
#[test]
fn super_in_enum_constructor_type_name_has_redirect() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  const E.named();
  const E() : this.named(), super();
}
"#,
        &[("super_in_enum_constructor", 62, 5)],
    );
}

/// `super_in_enum_constructor_test.dart` `test_typeName_one`.
#[test]
fn super_in_enum_constructor_type_name_one() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  const E() : super();
}
"#,
        &[("super_in_enum_constructor", 29, 5)],
    );
}

/// `super_in_enum_constructor_test.dart` `test_typeName_two`.
#[test]
fn super_in_enum_constructor_type_name_two() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  const E() : super(), super();
}
"#,
        &[
            ("super_in_enum_constructor", 29, 5),
            ("super_in_enum_constructor", 38, 5),
        ],
    );
}

/// `super_in_redirecting_constructor_test.dart` `test_class_primary_redirectBeforeSuper`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn super_in_redirecting_constructor_class_primary_redirect_before_super() {
    assert_errors_in_code(
        r#"
class A() {
  A.named() : this();
  this : this.named(), super();
}
"#,
        &[("primary_constructor_cannot_redirect", 44, 4)],
    );
}

/// `super_in_redirecting_constructor_test.dart` `test_class_primary_superBeforeRedirect`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn super_in_redirecting_constructor_class_primary_super_before_redirect() {
    assert_errors_in_code(
        r#"
class A() {
  A.named() : this();
  this : super(), this.named();
}
"#,
        &[("primary_constructor_cannot_redirect", 53, 4)],
    );
}

/// `super_in_redirecting_constructor_test.dart` `test_typeName_redirectionSuper`.
#[test]
fn super_in_redirecting_constructor_type_name_redirection_super() {
    assert_errors_in_code(
        r#"
class A {
  A() : this.name(), super();
  A.name() {}
}
"#,
        &[("super_in_redirecting_constructor", 32, 7)],
    );
}

/// `super_in_redirecting_constructor_test.dart` `test_typeName_superRedirection`.
#[test]
fn super_in_redirecting_constructor_type_name_super_redirection() {
    assert_errors_in_code(
        r#"
class A {
  A() : super(), this.name();
  A.name() {}
}
"#,
        &[("super_in_redirecting_constructor", 19, 7)],
    );
}

/// `super_invocation_not_last_test.dart` `test_primary_superBeforeAssert`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn super_invocation_not_last_primary_super_before_assert() {
    assert_errors_in_code(
        r#"
class A(int x) {
  this : super(), assert(x > 0);
}
"#,
        &[("super_invocation_not_last", 27, 5)],
    );
}

/// `super_invocation_not_last_test.dart` `test_primary_superBeforeField`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn super_invocation_not_last_primary_super_before_field() {
    assert_errors_in_code(
        r#"
class A() {
  int x;
  this : super(), x = 0;
}
"#,
        &[("super_invocation_not_last", 31, 5)],
    );
}

/// `super_invocation_not_last_test.dart` `test_primary_superIsLast`.
#[test]
#[ignore = "primary constructor bodies are not resolved (field names of `this :` initializers have no element; branch w-primary)"]
fn super_invocation_not_last_primary_super_is_last() {
    assert_errors_in_code(
        r#"
class A() {
  int x;
  this : x = 0, super();
}
"#,
        &[],
    );
}

/// `super_invocation_not_last_test.dart` `test_typeName_superBeforeAssert`.
#[test]
fn super_invocation_not_last_type_name_super_before_assert() {
    assert_errors_in_code(
        r#"
class A {
  A(int? x) : super(), assert(x != null);
}
"#,
        &[("super_invocation_not_last", 25, 5)],
    );
}

/// `super_invocation_not_last_test.dart` `test_typeName_superBeforeField`.
#[test]
fn super_invocation_not_last_type_name_super_before_field() {
    assert_errors_in_code(
        r#"
class A {
  final int x;
  A() : super(), x = 1;
}
"#,
        &[("super_invocation_not_last", 34, 5)],
    );
}

/// `super_invocation_not_last_test.dart` `test_typeName_superIsLast`.
#[test]
fn super_invocation_not_last_type_name_super_is_last() {
    assert_errors_in_code(
        r#"
class A {
  final int x;
  A() : x = 1, super();
}
"#,
        &[],
    );
}

/// `undefined_constructor_in_initializer_test.dart` `test_explicit_named_defined_constructor`.
#[test]
fn undefined_constructor_in_initializer_explicit_named_defined_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.named() {}
}
class B extends A {
  B() : super.named();
}
"#,
        &[],
    );
}

/// `undefined_constructor_in_initializer_test.dart` `test_explicit_named_defined_primaryConstructorBody`.
#[test]
fn undefined_constructor_in_initializer_explicit_named_defined_primary_constructor_body() {
    assert_errors_in_code(
        r#"
class A {
  A.named() {}
}
class B() extends A {
  this : super.named();
}
"#,
        &[],
    );
}

/// `undefined_constructor_in_initializer_test.dart` `test_explicit_named_notDefined_constructor`.
#[test]
fn undefined_constructor_in_initializer_explicit_named_not_defined_constructor() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {
  B() : super.named();
}
"#,
        &[("undefined_constructor_in_initializer", 40, 13)],
    );
}

/// `undefined_constructor_in_initializer_test.dart` `test_explicit_named_notDefined_primateConstructorBody`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn undefined_constructor_in_initializer_explicit_named_not_defined_primate_constructor_body() {
    assert_errors_in_code(
        r#"
class A {}
class B() extends A {
  this : super.named();
}
"#,
        &[("undefined_constructor_in_initializer", 43, 13)],
    );
}

/// `undefined_constructor_in_initializer_test.dart` `test_explicit_unnamed_defined_constructor`.
#[test]
fn undefined_constructor_in_initializer_explicit_unnamed_defined_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A() {}
}
class B extends A {
  B() : super();
}
"#,
        &[],
    );
}

/// `undefined_constructor_in_initializer_test.dart` `test_explicit_unnamed_defined_primaryConstructorBody`.
#[test]
fn undefined_constructor_in_initializer_explicit_unnamed_defined_primary_constructor_body() {
    assert_errors_in_code(
        r#"
class A {
  A() {}
}
class B() extends A {
  this : super();
}
"#,
        &[],
    );
}

/// `undefined_constructor_in_initializer_test.dart` `test_explicit_unnamed_notDefined_constructor`.
#[test]
fn undefined_constructor_in_initializer_explicit_unnamed_not_defined_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.named() {}
}
class B extends A {
  B() : super();
}
"#,
        &[("undefined_constructor_in_initializer", 56, 7)],
    );
}

/// `undefined_constructor_in_initializer_test.dart` `test_explicit_unnamed_notDefined_primaryConstructorBody`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn undefined_constructor_in_initializer_explicit_unnamed_not_defined_primary_constructor_body() {
    assert_errors_in_code(
        r#"
class A {
  A.named() {}
}
class B() extends A {
  this : super();
}
"#,
        &[("undefined_constructor_in_initializer", 59, 7)],
    );
}
