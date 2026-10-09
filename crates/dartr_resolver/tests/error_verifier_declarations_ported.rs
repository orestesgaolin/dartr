//! ErrorVerifier diagnostic tests of section D4, generated from
//! `pkg/analyzer/test/src/diagnostics/<code>_test.dart` (the tests with one
//! library and no other setup) by converting the inline diagnostic
//! markers or the `error(diag.x, offset, length)` lists. Only the codes
//! of `tools/difftest/error_verifier_codes.txt` are compared.

mod ev_support;
mod support;

use ev_support::assert_errors_in_code;

/// `augmentation_extends_clause_already_present_test.dart` `test_alreadyPresent`.
#[test]
fn augmentation_extends_clause_already_present_already_present() {
    assert_errors_in_code(
        r#"
class A {}

class B extends A {}
augment class B extends A {}
"#,
        &[("augmentation_extends_clause_already_present", 50, 7)],
    );
}

/// `augmentation_extends_clause_already_present_test.dart` `test_alreadyPresent2`.
#[test]
fn augmentation_extends_clause_already_present_already_present2() {
    assert_errors_in_code(
        r#"
class A {}

class B extends A {}
augment class B {}
augment class B extends A {}
"#,
        &[("augmentation_extends_clause_already_present", 69, 7)],
    );
}

/// `augmentation_extends_clause_already_present_test.dart` `test_notPresent`.
#[test]
fn augmentation_extends_clause_already_present_not_present() {
    assert_errors_in_code(
        r#"
class A {}

class B {}
augment class B extends A {}
"#,
        &[],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_abstract_abstractBase`.
#[test]
fn augmentation_modifier_extra_class_abstract_abstract_base() {
    assert_errors_in_code(
        r#"
abstract class A {}
augment abstract base class A {}
"#,
        &[("augmentation_modifier_extra", 38, 4)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_abstractBase_abstractBase`.
#[test]
fn augmentation_modifier_extra_class_abstract_base_abstract_base() {
    assert_errors_in_code(
        r#"
abstract base class A {}
augment abstract base class A {}
"#,
        &[],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_base_abstractBase`.
#[test]
fn augmentation_modifier_extra_class_base_abstract_base() {
    assert_errors_in_code(
        r#"
base class A {}
augment abstract base class A {}
"#,
        &[("augmentation_modifier_extra", 25, 8)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_constructor_secondary_nothing_const`.
#[test]
fn augmentation_modifier_extra_class_constructor_secondary_nothing_const() {
    assert_errors_in_code(
        r#"
class A {
  A();
  augment const A();
}
"#,
        &[("augmentation_modifier_extra", 28, 5)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_constructor_secondary_nothing_factory`.
#[test]
fn augmentation_modifier_extra_class_constructor_secondary_nothing_factory() {
    assert_errors_in_code(
        r#"
class A {
  A();
  augment factory A();
}
"#,
        &[("augmentation_modifier_extra", 28, 7)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_nothing_abstract`.
#[test]
fn augmentation_modifier_extra_class_nothing_abstract() {
    assert_errors_in_code(
        r#"
class A {}
augment abstract class A {}
"#,
        &[("augmentation_modifier_extra", 20, 8)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_nothing_abstractBase`.
#[test]
fn augmentation_modifier_extra_class_nothing_abstract_base() {
    assert_errors_in_code(
        r#"
class A {}
augment abstract base class A {}
"#,
        &[
            ("augmentation_modifier_extra", 20, 8),
            ("augmentation_modifier_extra", 29, 4),
        ],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_nothing_base`.
#[test]
fn augmentation_modifier_extra_class_nothing_base() {
    assert_errors_in_code(
        r#"
class A {}
augment base class A {}
"#,
        &[("augmentation_modifier_extra", 20, 4)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_nothing_final`.
#[test]
fn augmentation_modifier_extra_class_nothing_final() {
    assert_errors_in_code(
        r#"
class A {}
augment final class A {}
"#,
        &[("augmentation_modifier_extra", 20, 5)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_nothing_interface`.
#[test]
fn augmentation_modifier_extra_class_nothing_interface() {
    assert_errors_in_code(
        r#"
class A {}
augment interface class A {}
"#,
        &[("augmentation_modifier_extra", 20, 9)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_nothing_mixin`.
#[test]
fn augmentation_modifier_extra_class_nothing_mixin() {
    assert_errors_in_code(
        r#"
class A {}
augment mixin class A {}
"#,
        &[("augmentation_modifier_extra", 20, 5)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_nothing_nothing_abstract`.
#[test]
fn augmentation_modifier_extra_class_nothing_nothing_abstract() {
    assert_errors_in_code(
        r#"
class A {}
augment class A {}
augment abstract class A {}
"#,
        &[("augmentation_modifier_extra", 39, 8)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_class_nothing_sealed`.
#[test]
fn augmentation_modifier_extra_class_nothing_sealed() {
    assert_errors_in_code(
        r#"
class A {}
augment sealed class A {}
"#,
        &[("augmentation_modifier_extra", 20, 6)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_enum_constructor_primary_const_factory`.
#[test]
fn augmentation_modifier_extra_enum_constructor_primary_const_factory() {
    assert_errors_in_code(
        r#"
enum E.named() {
  v.named();
}

augment enum E {
  ;
  augment factory E.named();
}
"#,
        &[
            ("augmentation_modifier_missing", 57, 7),
            ("augmentation_modifier_extra", 65, 7),
        ],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_extensionType_constructor_primary_nothing_factory`.
#[test]
fn augmentation_modifier_extra_extension_type_constructor_primary_nothing_factory() {
    assert_errors_in_code(
        r#"
extension type E(int it);

augment extension type E {
  augment factory E(int it);
}
"#,
        &[("augmentation_modifier_extra", 65, 7)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_mixin_base_base`.
#[test]
fn augmentation_modifier_extra_mixin_base_base() {
    assert_errors_in_code(
        r#"
base mixin A {}
augment base mixin A {}
"#,
        &[],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_mixin_nothing_base`.
#[test]
fn augmentation_modifier_extra_mixin_nothing_base() {
    assert_errors_in_code(
        r#"
mixin A {}
augment base mixin A {}
"#,
        &[("augmentation_modifier_extra", 20, 4)],
    );
}

/// `augmentation_modifier_extra_test.dart` `test_mixin_nothing_nothing_base`.
#[test]
fn augmentation_modifier_extra_mixin_nothing_nothing_base() {
    assert_errors_in_code(
        r#"
mixin A {}
augment mixin A {}
augment base mixin A {}
"#,
        &[("augmentation_modifier_extra", 39, 4)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_abstract_abstract_nothing`.
#[test]
fn augmentation_modifier_missing_class_abstract_abstract_nothing() {
    assert_errors_in_code(
        r#"
abstract class A {}
augment abstract class A {}
augment class A {}
"#,
        &[("augmentation_modifier_missing", 49, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_abstract_nothing`.
#[test]
fn augmentation_modifier_missing_class_abstract_nothing() {
    assert_errors_in_code(
        r#"
abstract class A {}
augment class A {}
"#,
        &[("augmentation_modifier_missing", 21, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_abstractBase_abstract`.
#[test]
fn augmentation_modifier_missing_class_abstract_base_abstract() {
    assert_errors_in_code(
        r#"
abstract base class A {}
augment abstract class A {}
"#,
        &[("augmentation_modifier_missing", 26, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_abstractBase_abstractBase`.
#[test]
fn augmentation_modifier_missing_class_abstract_base_abstract_base() {
    assert_errors_in_code(
        r#"
abstract base class A {}
augment abstract base class A {}
"#,
        &[],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_abstractBase_base`.
#[test]
fn augmentation_modifier_missing_class_abstract_base_base() {
    assert_errors_in_code(
        r#"
abstract base class A {}
augment base class A {}
"#,
        &[("augmentation_modifier_missing", 26, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_abstractBase_nothing`.
#[test]
fn augmentation_modifier_missing_class_abstract_base_nothing() {
    assert_errors_in_code(
        r#"
abstract base class A {}
augment class A {}
"#,
        &[
            ("augmentation_modifier_missing", 26, 7),
            ("augmentation_modifier_missing", 26, 7),
        ],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_base_nothing`.
#[test]
fn augmentation_modifier_missing_class_base_nothing() {
    assert_errors_in_code(
        r#"
base class A {}
augment class A {}
"#,
        &[("augmentation_modifier_missing", 17, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_constructor_secondary_const_nothing`.
#[test]
fn augmentation_modifier_missing_class_constructor_secondary_const_nothing() {
    assert_errors_in_code(
        r#"
class A {
  const A();
  augment A();
}
"#,
        &[("augmentation_modifier_missing", 26, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_constructor_secondary_factory_nothing`.
#[test]
fn augmentation_modifier_missing_class_constructor_secondary_factory_nothing() {
    assert_errors_in_code(
        r#"
class A {
  A._();
  factory A() = A._;
  augment A();
}
"#,
        &[("augmentation_modifier_missing", 43, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_final_nothing`.
#[test]
fn augmentation_modifier_missing_class_final_nothing() {
    assert_errors_in_code(
        r#"
final class A {}
augment class A {}
"#,
        &[("augmentation_modifier_missing", 18, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_interface_nothing`.
#[test]
fn augmentation_modifier_missing_class_interface_nothing() {
    assert_errors_in_code(
        r#"
interface class A {}
augment class A {}
"#,
        &[("augmentation_modifier_missing", 22, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_mixin_nothing`.
#[test]
fn augmentation_modifier_missing_class_mixin_nothing() {
    assert_errors_in_code(
        r#"
mixin class A {}
augment class A {}
"#,
        &[("augmentation_modifier_missing", 18, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_class_sealed_nothing`.
#[test]
fn augmentation_modifier_missing_class_sealed_nothing() {
    assert_errors_in_code(
        r#"
sealed class A {}
augment class A {}
"#,
        &[("augmentation_modifier_missing", 19, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_enum_constructor_secondary_factory_nothing`.
#[test]
fn augmentation_modifier_missing_enum_constructor_secondary_factory_nothing() {
    assert_errors_in_code(
        r#"
enum E {
  v;

  factory E.named() => v;
}

augment enum E {
  ;
  augment E.named();
}
"#,
        &[("augmentation_modifier_missing", 68, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_extensionType_constructor_primary_const_nothing`.
#[test]
fn augmentation_modifier_missing_extension_type_constructor_primary_const_nothing() {
    assert_errors_in_code(
        r#"
extension type const E(int it);

augment extension type E {
  augment E(int it);
}
"#,
        &[("augmentation_modifier_missing", 63, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_mixin_base_base`.
#[test]
fn augmentation_modifier_missing_mixin_base_base() {
    assert_errors_in_code(
        r#"
base mixin A {}
augment base mixin A {}
"#,
        &[],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_mixin_base_base_nothing`.
#[test]
fn augmentation_modifier_missing_mixin_base_base_nothing() {
    assert_errors_in_code(
        r#"
base mixin A {}
augment base mixin A {}
augment mixin A {}
"#,
        &[("augmentation_modifier_missing", 41, 7)],
    );
}

/// `augmentation_modifier_missing_test.dart` `test_mixin_base_nothing`.
#[test]
fn augmentation_modifier_missing_mixin_base_nothing() {
    assert_errors_in_code(
        r#"
base mixin A {}
augment mixin A {}
"#,
        &[("augmentation_modifier_missing", 17, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_augments_enum`.
#[test]
fn augmentation_of_different_declaration_kind_class_augments_enum() {
    assert_errors_in_code(
        r#"
enum A {v}
augment class A {}
"#,
        &[("augmentation_of_different_declaration_kind", 12, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_augments_extension`.
#[test]
fn augmentation_of_different_declaration_kind_class_augments_extension() {
    assert_errors_in_code(
        r#"
extension A on int {}
augment class A {}
"#,
        &[("augmentation_of_different_declaration_kind", 23, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_augments_extensionType`.
#[test]
fn augmentation_of_different_declaration_kind_class_augments_extension_type() {
    assert_errors_in_code(
        r#"
extension type A(int it) {}
augment class A {}
"#,
        &[("augmentation_of_different_declaration_kind", 29, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_augments_function`.
#[test]
fn augmentation_of_different_declaration_kind_class_augments_function() {
    assert_errors_in_code(
        r#"
void foo() {}
augment class foo {}
"#,
        &[("augmentation_of_different_declaration_kind", 15, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_augments_getter`.
#[test]
fn augmentation_of_different_declaration_kind_class_augments_getter() {
    assert_errors_in_code(
        r#"
int get foo => 0;
augment class foo {}
"#,
        &[("augmentation_of_different_declaration_kind", 19, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_augments_mixin`.
#[test]
fn augmentation_of_different_declaration_kind_class_augments_mixin() {
    assert_errors_in_code(
        r#"
mixin A {}
augment class A {}
"#,
        &[("augmentation_of_different_declaration_kind", 12, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_augments_setter`.
#[test]
fn augmentation_of_different_declaration_kind_class_augments_setter() {
    assert_errors_in_code(
        r#"
set foo(int _) {}
augment class foo {}
"#,
        &[("augmentation_of_different_declaration_kind", 19, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_augments_variable`.
#[test]
fn augmentation_of_different_declaration_kind_class_augments_variable() {
    assert_errors_in_code(
        r#"
int foo = 0;
augment class foo {}
"#,
        &[("augmentation_of_different_declaration_kind", 14, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_constructor_augments_constructor`.
#[test]
fn augmentation_of_different_declaration_kind_class_constructor_augments_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.foo();
}
augment class A {
  augment A.foo();
}
"#,
        &[],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_constructor_augments_staticField`.
#[test]
fn augmentation_of_different_declaration_kind_class_constructor_augments_static_field() {
    assert_errors_in_code(
        r#"
class A {
  static int foo = 0;
}
augment class A {
  augment A.foo();
}
"#,
        &[("augmentation_of_different_declaration_kind", 55, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_constructor_augments_staticMethod`.
#[test]
fn augmentation_of_different_declaration_kind_class_constructor_augments_static_method() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment A.foo();
}
"#,
        &[("augmentation_of_different_declaration_kind", 56, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceField_augments_instanceField`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_field_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}
augment class A {
  augment abstract int foo;
}
"#,
        &[],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceField_augments_instanceMethod`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_field_augments_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
augment class A {
  augment int foo = 0;
}
"#,
        &[("augmentation_of_different_declaration_kind", 61, 3)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceGetter_augments_instanceField`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_getter_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}
augment class A {
  augment int get foo;
}
"#,
        &[],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceGetter_augments_instanceMethod`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_getter_augments_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
augment class A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_of_different_declaration_kind", 49, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceMethod_augments_instanceField`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_method_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}
augment class A {
  augment void foo() {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 48, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceMethod_augments_instanceGetter`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_method_augments_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
augment class A {
  augment void foo() {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 53, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceMethod_augments_instanceMethod`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_method_augments_instance_method() {
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

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceMethod_augments_instanceSetter`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_method_augments_instance_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(int _) {}
}
augment class A {
  augment void foo() {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 53, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceSetter_augments_instanceField`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_setter_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}
augment class A {
  augment set foo(int _);
}
"#,
        &[],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_instanceSetter_augments_instanceMethod`.
#[test]
fn augmentation_of_different_declaration_kind_class_instance_setter_augments_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
augment class A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 49, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticField_augments_constructor`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_field_augments_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.foo();
}
augment class A {
  augment static int foo = 0;
}
"#,
        &[("augmentation_of_different_declaration_kind", 63, 3)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticField_augments_staticMethod`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_field_augments_static_method() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment static int foo = 0;
}
"#,
        &[("augmentation_of_different_declaration_kind", 75, 3)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticGetter_augments_constructor`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_getter_augments_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.foo();
}
augment class A {
  augment static int get foo => 0;
}
"#,
        &[("augmentation_of_different_declaration_kind", 44, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticGetter_augments_staticMethod`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_getter_augments_static_method() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment static int get foo => 0;
}
"#,
        &[("augmentation_of_different_declaration_kind", 56, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticMethod_augments_constructor`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_method_augments_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.foo();
}
augment class A {
  augment static void foo() {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 44, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticMethod_augments_staticField`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_method_augments_static_field() {
    assert_errors_in_code(
        r#"
class A {
  static int foo = 0;
}
augment class A {
  augment static void foo() {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 55, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticMethod_augments_staticGetter`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_method_augments_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  static int get foo => 0;
}
augment class A {
  augment static void foo() {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 60, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticMethod_augments_staticSetter`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_method_augments_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  static set foo(int _) {}
}
augment class A {
  augment static void foo() {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 60, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticSetter_augments_constructor`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_setter_augments_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.foo();
}
augment class A {
  augment static set foo(int _) {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 44, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_class_staticSetter_augments_staticMethod`.
#[test]
fn augmentation_of_different_declaration_kind_class_static_setter_augments_static_method() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment static set foo(int _) {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 56, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_enum_augments_class`.
#[test]
fn augmentation_of_different_declaration_kind_enum_augments_class() {
    assert_errors_in_code(
        r#"
class A {}
augment enum A {}
"#,
        &[("augmentation_of_different_declaration_kind", 12, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_enum_constant_augments_instanceMethod`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_of_different_declaration_kind_enum_constant_augments_instance_method() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  void foo() {}
}
augment enum A {
  augment foo(),
}
"#,
        &[
            ("constant_variable_augmentation", 60, 3),
            ("conflicting_static_and_instance", 60, 3),
        ],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_enum_staticMethod_augments_constant`.
#[test]
fn augmentation_of_different_declaration_kind_enum_static_method_augments_constant() {
    assert_errors_in_code(
        r#"
enum A {
  foo
}
augment enum A {;
  augment static void foo() {}
}
"#,
        &[("augmentation_of_different_declaration_kind", 38, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_extension_augments_class`.
#[test]
fn augmentation_of_different_declaration_kind_extension_augments_class() {
    assert_errors_in_code(
        r#"
class A {}
augment extension A {}
"#,
        &[("augmentation_of_different_declaration_kind", 12, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_extensionType_augments_class`.
#[test]
fn augmentation_of_different_declaration_kind_extension_type_augments_class() {
    assert_errors_in_code(
        r#"
class A {}
augment extension type A {}
"#,
        &[("augmentation_of_different_declaration_kind", 12, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_function_augments_class`.
#[test]
fn augmentation_of_different_declaration_kind_function_augments_class() {
    assert_errors_in_code(
        r#"
class A {}
augment void A() {}
"#,
        &[("augmentation_of_different_declaration_kind", 12, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_mixin_augments_class`.
#[test]
fn augmentation_of_different_declaration_kind_mixin_augments_class() {
    assert_errors_in_code(
        r#"
class A {}
augment mixin A {}
"#,
        &[("augmentation_of_different_declaration_kind", 12, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_typedef_augments_class`.
#[test]
fn augmentation_of_different_declaration_kind_typedef_augments_class() {
    assert_errors_in_code(
        r#"
class A {}
augment typedef A = int;
"#,
        &[("typedef_augmentation", 12, 7)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_variable_augments_class`.
#[test]
fn augmentation_of_different_declaration_kind_variable_augments_class() {
    assert_errors_in_code(
        r#"
class A {}
augment int A = 0;
"#,
        &[("augmentation_of_different_declaration_kind", 24, 1)],
    );
}

/// `augmentation_of_different_declaration_kind_test.dart` `test_variable_augments_function`.
#[test]
fn augmentation_of_different_declaration_kind_variable_augments_function() {
    assert_errors_in_code(
        r#"
void foo() {}
augment int foo = 0;
"#,
        &[("augmentation_of_different_declaration_kind", 27, 3)],
    );
}

/// `augmentation_of_mixin_application_class_test.dart` `test_class`.
#[test]
fn augmentation_of_mixin_application_class_class() {
    assert_errors_in_code(
        r#"
class A {}
mixin M {}
class C = A with M;
augment class C {}
"#,
        &[("augmentation_of_mixin_application_class", 43, 7)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_dynamic_objectQuestion`.
#[test]
fn augmentation_type_parameter_bound_class_dynamic_object_question() {
    assert_errors_in_code(
        r#"
class A<T extends dynamic> {}
augment class A<T extends Object?> {}
"#,
        &[("augmentation_type_parameter_bound", 57, 7)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_method_nothing_num`.
#[test]
fn augmentation_type_parameter_bound_class_method_nothing_num() {
    assert_errors_in_code(
        r#"
class A {
  void foo<T>() {}
  augment void foo<T extends num>();
}
"#,
        &[("augmentation_type_parameter_bound", 59, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_method_num_int`.
#[test]
fn augmentation_type_parameter_bound_class_method_num_int() {
    assert_errors_in_code(
        r#"
class A {
  void foo<T extends num>() {}
  augment void foo<T extends int>();
}
"#,
        &[("augmentation_type_parameter_bound", 71, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_method_num_nothing`.
#[test]
fn augmentation_type_parameter_bound_class_method_num_nothing() {
    assert_errors_in_code(
        r#"
class A {
  void foo<T extends num>() {}
  augment void foo<T>();
}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_nothing_num`.
#[test]
fn augmentation_type_parameter_bound_class_nothing_num() {
    assert_errors_in_code(
        r#"
class A<T> {}
augment class A<T extends num> {}
"#,
        &[("augmentation_type_parameter_bound", 41, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_num_int`.
#[test]
fn augmentation_type_parameter_bound_class_num_int() {
    assert_errors_in_code(
        r#"
class A<T extends num> {}
augment class A<T extends int> {}
"#,
        &[("augmentation_type_parameter_bound", 53, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_num_nothing`.
#[test]
fn augmentation_type_parameter_bound_class_num_nothing() {
    assert_errors_in_code(
        r#"
class A<T extends num> {}
augment class A<T> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_num_num`.
#[test]
fn augmentation_type_parameter_bound_class_num_num() {
    assert_errors_in_code(
        r#"
class A<T extends num> {}
augment class A<T extends num> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_num_num_viaTypeAlias`.
#[test]
fn augmentation_type_parameter_bound_class_num_num_via_type_alias() {
    assert_errors_in_code(
        r#"
typedef N = num;

class A<T extends num> {}
augment class A<T extends N> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_num_num_withImportPrefix`.
#[test]
fn augmentation_type_parameter_bound_class_num_num_with_import_prefix() {
    assert_errors_in_code(
        r#"
import 'dart:core';
import 'dart:core' as core;

class A<T extends num> {}
augment class A<T extends core.num> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_num_Object`.
#[test]
fn augmentation_type_parameter_bound_class_num_object() {
    assert_errors_in_code(
        r#"
class A<T extends num> {}
augment class A<T extends Object> {}
"#,
        &[("augmentation_type_parameter_bound", 53, 6)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_class_objectQuestion_dynamic`.
#[test]
fn augmentation_type_parameter_bound_class_object_question_dynamic() {
    assert_errors_in_code(
        r#"
class A<T extends Object?> {}
augment class A<T extends dynamic> {}
"#,
        &[("augmentation_type_parameter_bound", 57, 7)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_enum_nothing_num`.
#[test]
fn augmentation_type_parameter_bound_enum_nothing_num() {
    assert_errors_in_code(
        r#"
enum A<T> {v}
augment enum A<T extends num> {}
"#,
        &[("augmentation_type_parameter_bound", 40, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_enum_num_int`.
#[test]
fn augmentation_type_parameter_bound_enum_num_int() {
    assert_errors_in_code(
        r#"
enum A<T extends num> {v}
augment enum A<T extends int> {}
"#,
        &[("augmentation_type_parameter_bound", 52, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_enum_num_nothing`.
#[test]
fn augmentation_type_parameter_bound_enum_num_nothing() {
    assert_errors_in_code(
        r#"
enum A<T extends num> {v}
augment enum A<T> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_enum_num_num`.
#[test]
fn augmentation_type_parameter_bound_enum_num_num() {
    assert_errors_in_code(
        r#"
enum A<T extends num> {v}
augment enum A<T extends num> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_extension_nothing_num`.
#[test]
fn augmentation_type_parameter_bound_extension_nothing_num() {
    assert_errors_in_code(
        r#"
extension A<T> on int {}
augment extension A<T extends num> {}
"#,
        &[("augmentation_type_parameter_bound", 56, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_extension_num_int`.
#[test]
fn augmentation_type_parameter_bound_extension_num_int() {
    assert_errors_in_code(
        r#"
extension A<T extends num> on int {}
augment extension A<T extends int> {}
"#,
        &[("augmentation_type_parameter_bound", 68, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_extension_num_nothing`.
#[test]
fn augmentation_type_parameter_bound_extension_num_nothing() {
    assert_errors_in_code(
        r#"
extension A<T extends num> on int {}
augment extension A<T> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_extension_num_num`.
#[test]
fn augmentation_type_parameter_bound_extension_num_num() {
    assert_errors_in_code(
        r#"
extension A<T extends num> on int {}
augment extension A<T extends num> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_extensionType_nothing_num`.
#[test]
fn augmentation_type_parameter_bound_extension_type_nothing_num() {
    assert_errors_in_code(
        r#"
extension type A<T>(int it) {}
augment extension type A<T extends num> {}
"#,
        &[("augmentation_type_parameter_bound", 67, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_extensionType_num_int`.
#[test]
fn augmentation_type_parameter_bound_extension_type_num_int() {
    assert_errors_in_code(
        r#"
extension type A<T extends num>(int it) {}
augment extension type A<T extends int> {}
"#,
        &[("augmentation_type_parameter_bound", 79, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_extensionType_num_nothing`.
#[test]
fn augmentation_type_parameter_bound_extension_type_num_nothing() {
    assert_errors_in_code(
        r#"
extension type A<T extends num>(int it) {}
augment extension type A<T> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_extensionType_num_num`.
#[test]
fn augmentation_type_parameter_bound_extension_type_num_num() {
    assert_errors_in_code(
        r#"
extension type A<T extends num>(int it) {}
augment extension type A<T extends num> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_mixin_nothing_num`.
#[test]
fn augmentation_type_parameter_bound_mixin_nothing_num() {
    assert_errors_in_code(
        r#"
mixin A<T> {}
augment mixin A<T extends num> {}
"#,
        &[("augmentation_type_parameter_bound", 41, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_mixin_num_int`.
#[test]
fn augmentation_type_parameter_bound_mixin_num_int() {
    assert_errors_in_code(
        r#"
mixin A<T extends num> {}
augment mixin A<T extends int> {}
"#,
        &[("augmentation_type_parameter_bound", 53, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_mixin_num_nothing`.
#[test]
fn augmentation_type_parameter_bound_mixin_num_nothing() {
    assert_errors_in_code(
        r#"
mixin A<T extends num> {}
augment mixin A<T> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_mixin_num_num`.
#[test]
fn augmentation_type_parameter_bound_mixin_num_num() {
    assert_errors_in_code(
        r#"
mixin A<T extends num> {}
augment mixin A<T extends num> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_topLevelFunction_nothing_num`.
#[test]
fn augmentation_type_parameter_bound_top_level_function_nothing_num() {
    assert_errors_in_code(
        r#"
void foo<T>() {}
augment void foo<T extends num>();
"#,
        &[("augmentation_type_parameter_bound", 45, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_topLevelFunction_num_int`.
#[test]
fn augmentation_type_parameter_bound_top_level_function_num_int() {
    assert_errors_in_code(
        r#"
void foo<T extends num>() {}
augment void foo<T extends int>();
"#,
        &[("augmentation_type_parameter_bound", 57, 3)],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_topLevelFunction_num_nothing`.
#[test]
fn augmentation_type_parameter_bound_top_level_function_num_nothing() {
    assert_errors_in_code(
        r#"
void foo<T extends num>() {}
augment void foo<T>();
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_topLevelFunction_num_num_viaTypeAlias`.
#[test]
fn augmentation_type_parameter_bound_top_level_function_num_num_via_type_alias() {
    assert_errors_in_code(
        r#"
typedef N = num;

void foo<T extends num>() {}
augment void foo<T extends N>();
"#,
        &[],
    );
}

/// `augmentation_type_parameter_bound_test.dart` `test_topLevelFunction_num_num_withImportPrefix`.
#[test]
fn augmentation_type_parameter_bound_top_level_function_num_num_with_import_prefix() {
    assert_errors_in_code(
        r#"
import 'dart:core';
import 'dart:core' as core;

void foo<T extends num>() {}
augment void foo<T extends core.num>();
"#,
        &[],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_0_1`.
#[test]
fn augmentation_type_parameter_count_class_0_1() {
    assert_errors_in_code(
        r#"
class A {}
augment class A<T> {}
"#,
        &[("augmentation_type_parameter_count", 28, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_1_0`.
#[test]
fn augmentation_type_parameter_count_class_1_0() {
    assert_errors_in_code(
        r#"
class A<T> {}
augment class A {}
"#,
        &[("augmentation_type_parameter_count", 29, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_1_1`.
#[test]
fn augmentation_type_parameter_count_class_1_1() {
    assert_errors_in_code(
        r#"
class A<T> {}
augment class A<T> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_1_2`.
#[test]
fn augmentation_type_parameter_count_class_1_2() {
    assert_errors_in_code(
        r#"
class A<T> {}
augment class A<T, U> {}

void f(A<int> a) {}
"#,
        &[("augmentation_type_parameter_count", 34, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_1_3`.
#[test]
fn augmentation_type_parameter_count_class_1_3() {
    assert_errors_in_code(
        r#"
class A<T> {}
augment class A<T, U, V> {}
"#,
        &[("augmentation_type_parameter_count", 34, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_2_1`.
#[test]
fn augmentation_type_parameter_count_class_2_1() {
    assert_errors_in_code(
        r#"
class A<T, U> {}
augment class A<T> {}
"#,
        &[("augmentation_type_parameter_count", 35, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_method_0_1`.
#[test]
fn augmentation_type_parameter_count_class_method_0_1() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
augment class A {
  augment void foo<T>();
}
"#,
        &[("augmentation_type_parameter_count", 66, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_method_1_1`.
#[test]
fn augmentation_type_parameter_count_class_method_1_1() {
    assert_errors_in_code(
        r#"
class A {
  void foo<T>() {}
}
augment class A {
  augment void foo<T>();
}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_method_1_2`.
#[test]
fn augmentation_type_parameter_count_class_method_1_2() {
    assert_errors_in_code(
        r#"
class A {
  void foo<T>() {}
}
augment class A {
  augment void foo<T, U>();
}

void f(A a) {
  a.foo<int>();
}
"#,
        &[("augmentation_type_parameter_count", 72, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_method_2_1`.
#[test]
fn augmentation_type_parameter_count_class_method_2_1() {
    assert_errors_in_code(
        r#"
class A {
  void foo<T, U>() {}
}
augment class A {
  augment void foo<T>();
}
"#,
        &[("augmentation_type_parameter_count", 73, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_staticMethod_0_1`.
#[test]
fn augmentation_type_parameter_count_class_static_method_0_1() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment static void foo<T>();
}
"#,
        &[("augmentation_type_parameter_count", 80, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_staticMethod_1_2`.
#[test]
fn augmentation_type_parameter_count_class_static_method_1_2() {
    assert_errors_in_code(
        r#"
class A {
  static void foo<T>() {}
}
augment class A {
  augment static void foo<T, U>();
}

void f() {
  A.foo<int>();
}
"#,
        &[("augmentation_type_parameter_count", 86, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_class_staticMethod_2_1`.
#[test]
fn augmentation_type_parameter_count_class_static_method_2_1() {
    assert_errors_in_code(
        r#"
class A {
  static void foo<T, U>() {}
}
augment class A {
  augment static void foo<T>();
}
"#,
        &[("augmentation_type_parameter_count", 87, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_enum_0_1`.
#[test]
fn augmentation_type_parameter_count_enum_0_1() {
    assert_errors_in_code(
        r#"
enum A {v}
augment enum A<T> {}
"#,
        &[("augmentation_type_parameter_count", 27, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_enum_1_0`.
#[test]
fn augmentation_type_parameter_count_enum_1_0() {
    assert_errors_in_code(
        r#"
enum A<T> {v}
augment enum A {}
"#,
        &[("augmentation_type_parameter_count", 28, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_enum_1_1`.
#[test]
fn augmentation_type_parameter_count_enum_1_1() {
    assert_errors_in_code(
        r#"
enum A<T> {v}
augment enum A <T>{}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_enum_1_2`.
#[test]
fn augmentation_type_parameter_count_enum_1_2() {
    assert_errors_in_code(
        r#"
enum A<T> {v}
augment enum A<T, U> {}
"#,
        &[("augmentation_type_parameter_count", 33, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_enum_2_1`.
#[test]
fn augmentation_type_parameter_count_enum_2_1() {
    assert_errors_in_code(
        r#"
enum A<T, U> {v}
augment enum A<T> {}
"#,
        &[("augmentation_type_parameter_count", 34, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extension_0_1`.
#[test]
fn augmentation_type_parameter_count_extension_0_1() {
    assert_errors_in_code(
        r#"
extension A on int {}
augment extension A<T> {}
"#,
        &[("augmentation_type_parameter_count", 43, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extension_1_0`.
#[test]
fn augmentation_type_parameter_count_extension_1_0() {
    assert_errors_in_code(
        r#"
extension A<T> on int {}
augment extension A {}
"#,
        &[("augmentation_type_parameter_count", 44, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extension_1_1`.
#[test]
fn augmentation_type_parameter_count_extension_1_1() {
    assert_errors_in_code(
        r#"
extension A<T> on int {}
augment extension A<T> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extension_1_2`.
#[test]
fn augmentation_type_parameter_count_extension_1_2() {
    assert_errors_in_code(
        r#"
extension A<T> on int {}
augment extension A<T, U> {}
"#,
        &[("augmentation_type_parameter_count", 49, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extension_2_1`.
#[test]
fn augmentation_type_parameter_count_extension_2_1() {
    assert_errors_in_code(
        r#"
extension A<T, U> on int {}
augment extension A<T> {}
"#,
        &[("augmentation_type_parameter_count", 50, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extensionType_0_1`.
#[test]
fn augmentation_type_parameter_count_extension_type_0_1() {
    assert_errors_in_code(
        r#"
extension type A(int it) {}
augment extension type A<T> {}
"#,
        &[("augmentation_type_parameter_count", 54, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extensionType_1_0`.
#[test]
fn augmentation_type_parameter_count_extension_type_1_0() {
    assert_errors_in_code(
        r#"
extension type A<T>(int it) {}
augment extension type A {}
"#,
        &[("augmentation_type_parameter_count", 55, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extensionType_1_1`.
#[test]
fn augmentation_type_parameter_count_extension_type_1_1() {
    assert_errors_in_code(
        r#"
extension type A<T>(int it) {}
augment extension type A<T> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extensionType_1_2`.
#[test]
fn augmentation_type_parameter_count_extension_type_1_2() {
    assert_errors_in_code(
        r#"
extension type A<T>(int it) {}
augment extension type A<T, U> {}
"#,
        &[("augmentation_type_parameter_count", 60, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_extensionType_2_1`.
#[test]
fn augmentation_type_parameter_count_extension_type_2_1() {
    assert_errors_in_code(
        r#"
extension type A<T, U>(int it) {}
augment extension type A<T> {}
"#,
        &[("augmentation_type_parameter_count", 61, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_mixin_0_1`.
#[test]
fn augmentation_type_parameter_count_mixin_0_1() {
    assert_errors_in_code(
        r#"
mixin A {}
augment mixin A<T> {}
"#,
        &[("augmentation_type_parameter_count", 28, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_mixin_1_0`.
#[test]
fn augmentation_type_parameter_count_mixin_1_0() {
    assert_errors_in_code(
        r#"
mixin A<T> {}
augment mixin A {}
"#,
        &[("augmentation_type_parameter_count", 29, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_mixin_1_1`.
#[test]
fn augmentation_type_parameter_count_mixin_1_1() {
    assert_errors_in_code(
        r#"
mixin A<T> {}
augment mixin A<T> {}
"#,
        &[],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_mixin_1_2`.
#[test]
fn augmentation_type_parameter_count_mixin_1_2() {
    assert_errors_in_code(
        r#"
mixin A<T> {}
augment mixin A<T, U> {}
"#,
        &[("augmentation_type_parameter_count", 34, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_mixin_2_1`.
#[test]
fn augmentation_type_parameter_count_mixin_2_1() {
    assert_errors_in_code(
        r#"
mixin A<T, U> {}
augment mixin A<T> {}
"#,
        &[("augmentation_type_parameter_count", 35, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_topLevelFunction_0_1`.
#[test]
fn augmentation_type_parameter_count_top_level_function_0_1() {
    assert_errors_in_code(
        r#"
void f() {}
augment void f<T>();
"#,
        &[("augmentation_type_parameter_count", 28, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_topLevelFunction_1_1`.
#[test]
fn augmentation_type_parameter_count_top_level_function_1_1() {
    assert_errors_in_code(
        r#"
void f<T>() {}
augment void f<T>();
"#,
        &[],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_topLevelFunction_1_2`.
#[test]
fn augmentation_type_parameter_count_top_level_function_1_2() {
    assert_errors_in_code(
        r#"
void f<T>() {}
augment void f<T, U>();

void g() {
  f<int>();
}
"#,
        &[("augmentation_type_parameter_count", 34, 1)],
    );
}

/// `augmentation_type_parameter_count_test.dart` `test_topLevelFunction_2_1`.
#[test]
fn augmentation_type_parameter_count_top_level_function_2_1() {
    assert_errors_in_code(
        r#"
void f<T, U>() {}
augment void f<T>();
"#,
        &[("augmentation_type_parameter_count", 35, 1)],
    );
}

/// `augmentation_type_parameter_name_test.dart` `test_class_method_T_U`.
#[test]
fn augmentation_type_parameter_name_class_method_t_u() {
    assert_errors_in_code(
        r#"
class A {
  void foo<T>() {}
  augment void foo<U>();
}
"#,
        &[("augmentation_type_parameter_name", 49, 1)],
    );
}

/// `augmentation_type_parameter_name_test.dart` `test_class_T_U`.
#[test]
fn augmentation_type_parameter_name_class_t_u() {
    assert_errors_in_code(
        r#"
class A<T> {}
augment class A<U> {}
"#,
        &[("augmentation_type_parameter_name", 31, 1)],
    );
}

/// `augmentation_type_parameter_name_test.dart` `test_class_TU_UT`.
#[test]
fn augmentation_type_parameter_name_class_t_u_u_t() {
    assert_errors_in_code(
        r#"
class A<T, U> {}
augment class A<U, T> {}
"#,
        &[
            ("augmentation_type_parameter_name", 34, 1),
            ("augmentation_type_parameter_name", 37, 1),
        ],
    );
}

/// `augmentation_type_parameter_name_test.dart` `test_enum_T_U`.
#[test]
fn augmentation_type_parameter_name_enum_t_u() {
    assert_errors_in_code(
        r#"
enum A<T> {v}
augment enum A<U> {}
"#,
        &[("augmentation_type_parameter_name", 30, 1)],
    );
}

/// `augmentation_type_parameter_name_test.dart` `test_extension_T_U`.
#[test]
fn augmentation_type_parameter_name_extension_t_u() {
    assert_errors_in_code(
        r#"
extension A<T> on int {}
augment extension A<U> {}
"#,
        &[("augmentation_type_parameter_name", 46, 1)],
    );
}

/// `augmentation_type_parameter_name_test.dart` `test_extensionType_T_U`.
#[test]
fn augmentation_type_parameter_name_extension_type_t_u() {
    assert_errors_in_code(
        r#"
extension type A<T>(int it) {}
augment extension type A<U> {}
"#,
        &[("augmentation_type_parameter_name", 57, 1)],
    );
}

/// `augmentation_type_parameter_name_test.dart` `test_mixin_T_U`.
#[test]
fn augmentation_type_parameter_name_mixin_t_u() {
    assert_errors_in_code(
        r#"
mixin A<T> {}
augment mixin A<U> {}
"#,
        &[("augmentation_type_parameter_name", 31, 1)],
    );
}

/// `augmentation_type_parameter_name_test.dart` `test_topLevelFunction_T_U`.
#[test]
fn augmentation_type_parameter_name_top_level_function_t_u() {
    assert_errors_in_code(
        r#"
void foo<T>() {}
augment void foo<U>();
"#,
        &[("augmentation_type_parameter_name", 35, 1)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class`.
#[test]
fn augmentation_without_declaration_class() {
    assert_errors_in_code(
        r#"
augment class A {}
"#,
        &[("augmentation_without_declaration", 1, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_augments_class`.
#[test]
fn augmentation_without_declaration_class_augments_class() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {}
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_constructor`.
#[test]
fn augmentation_without_declaration_class_constructor() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {
  augment A.named();
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_constructor_augments_instanceField`.
#[test]
fn augmentation_without_declaration_class_constructor_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}

augment class A {
  augment A.foo();
}
"#,
        &[("augmentation_without_declaration", 49, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_constructor_augments_instanceMethod`.
#[test]
fn augmentation_without_declaration_class_constructor_augments_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

augment class A {
  augment A.foo();
}
"#,
        &[("augmentation_without_declaration", 50, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField`.
#[test]
fn augmentation_without_declaration_class_instance_field() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {
  augment int foo = 0;
}
"#,
        &[("augmentation_without_declaration", 45, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_constructor`.
#[test]
fn augmentation_without_declaration_class_instance_field_augments_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.foo();
}

augment class A {
  augment int foo = 0;
}
"#,
        &[("augmentation_without_declaration", 57, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_instanceField`.
#[test]
fn augmentation_without_declaration_class_instance_field_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}

augment class A {
  augment abstract int foo;
}
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_instanceField_final`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_instance_field_augments_instance_field_final() {
    assert_errors_in_code(
        r#"
class A {
  final int foo = 0;
}

augment class A {
  augment abstract int foo;
}
"#,
        &[("augmentation_without_declaration", 76, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_instanceGetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_instance_field_augments_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}

augment class A {
  augment abstract int foo;
}
"#,
        &[("augmentation_without_declaration", 75, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_instanceSetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_instance_field_augments_instance_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(int _) {}
}

augment class A {
  augment int foo = 0;
}
"#,
        &[
            ("augmentation_without_declaration", 66, 3),
            ("declaration_already_complete", 66, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_staticField`.
#[test]
fn augmentation_without_declaration_class_instance_field_augments_static_field() {
    assert_errors_in_code(
        r#"
class A {
  static int foo = 0;
}
augment class A {
  augment int foo = 0;
}
"#,
        &[("augmentation_without_declaration", 67, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_staticField_noBody`.
#[test]
fn augmentation_without_declaration_class_instance_field_augments_static_field_no_body() {
    assert_errors_in_code(
        r#"
class A {
  static int foo = 0;
}
augment class A {
  augment abstract int foo;
}
"#,
        &[("augmentation_without_declaration", 76, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_staticGetter`.
#[test]
fn augmentation_without_declaration_class_instance_field_augments_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  static int get foo => 0;
}
augment class A {
  augment int foo = 0;
}
"#,
        &[("augmentation_without_declaration", 72, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_staticMethod`.
#[test]
fn augmentation_without_declaration_class_instance_field_augments_static_method() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment int foo = 0;
}
"#,
        &[("augmentation_without_declaration", 68, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_augments_staticSetter`.
#[test]
fn augmentation_without_declaration_class_instance_field_augments_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  static set foo(int _) {}
}
augment class A {
  augment int foo = 0;
}
"#,
        &[("augmentation_without_declaration", 72, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_final_augments_instanceField_final`.
#[test]
fn augmentation_without_declaration_class_instance_field_final_augments_instance_field_final() {
    assert_errors_in_code(
        r#"
class A {
  final int foo = 0;
}

augment class A {
  augment abstract final int foo;
}
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_final_augments_instanceGetter`.
#[test]
fn augmentation_without_declaration_class_instance_field_final_augments_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}

augment class A {
  augment abstract final int foo;
}
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceField_final_augments_instanceSetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_instance_field_final_augments_instance_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(int _) {}
}

augment class A {
  augment abstract final int foo;
}
"#,
        &[("augmentation_without_declaration", 81, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceGetter`.
#[test]
fn augmentation_without_declaration_class_instance_getter() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceGetter_augments_constructor`.
#[test]
fn augmentation_without_declaration_class_instance_getter_augments_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.foo();
}

augment class A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 45, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceGetter_augments_staticField`.
#[test]
fn augmentation_without_declaration_class_instance_getter_augments_static_field() {
    assert_errors_in_code(
        r#"
class A {
  static int foo = 0;
}
augment class A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 55, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceGetter_augments_staticGetter`.
#[test]
fn augmentation_without_declaration_class_instance_getter_augments_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  static int get foo => 0;
}
augment class A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 60, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceGetter_augments_staticGetter_noBody`.
#[test]
fn augmentation_without_declaration_class_instance_getter_augments_static_getter_no_body() {
    assert_errors_in_code(
        r#"
class A {
  static int get foo => 0;
}
augment class A {
  augment int get foo;
}
"#,
        &[("augmentation_without_declaration", 60, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceGetter_augments_staticMethod`.
#[test]
fn augmentation_without_declaration_class_instance_getter_augments_static_method() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 56, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceGetter_augments_staticSetter`.
#[test]
fn augmentation_without_declaration_class_instance_getter_augments_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  static set foo(int _) {}
}
augment class A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 60, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceMethod`.
#[test]
fn augmentation_without_declaration_class_instance_method() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceMethod_augments_constructor`.
#[test]
fn augmentation_without_declaration_class_instance_method_augments_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.foo();
}

augment class A {
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 45, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceMethod_augments_instanceMethod`.
#[test]
fn augmentation_without_declaration_class_instance_method_augments_instance_method() {
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

/// `augmentation_without_declaration_test.dart` `test_class_instanceMethod_augments_staticField`.
#[test]
fn augmentation_without_declaration_class_instance_method_augments_static_field() {
    assert_errors_in_code(
        r#"
class A {
  static int foo = 0;
}
augment class A {
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 55, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceMethod_augments_staticGetter`.
#[test]
fn augmentation_without_declaration_class_instance_method_augments_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  static int get foo => 0;
}
augment class A {
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 60, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceMethod_augments_staticMethod`.
#[test]
fn augmentation_without_declaration_class_instance_method_augments_static_method() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 56, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceMethod_augments_staticMethod_noBody`.
#[test]
fn augmentation_without_declaration_class_instance_method_augments_static_method_no_body() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment void foo();
}
"#,
        &[("augmentation_without_declaration", 56, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceMethod_augments_staticSetter`.
#[test]
fn augmentation_without_declaration_class_instance_method_augments_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  static set foo(int _) {}
}
augment class A {
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 60, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceSetter`.
#[test]
fn augmentation_without_declaration_class_instance_setter() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceSetter_augments_constructor`.
#[test]
fn augmentation_without_declaration_class_instance_setter_augments_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A.foo();
}

augment class A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 45, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceSetter_augments_staticField`.
#[test]
fn augmentation_without_declaration_class_instance_setter_augments_static_field() {
    assert_errors_in_code(
        r#"
class A {
  static int foo = 0;
}
augment class A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 55, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceSetter_augments_staticGetter`.
#[test]
fn augmentation_without_declaration_class_instance_setter_augments_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  static int get foo => 0;
}
augment class A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 60, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceSetter_augments_staticMethod`.
#[test]
fn augmentation_without_declaration_class_instance_setter_augments_static_method() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}
augment class A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 56, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceSetter_augments_staticSetter`.
#[test]
fn augmentation_without_declaration_class_instance_setter_augments_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  static set foo(int _) {}
}
augment class A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 60, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_instanceSetter_augments_staticSetter_noBody`.
#[test]
fn augmentation_without_declaration_class_instance_setter_augments_static_setter_no_body() {
    assert_errors_in_code(
        r#"
class A {
  static set foo(int _) {}
}
augment class A {
  augment set foo(int _);
}
"#,
        &[("augmentation_without_declaration", 60, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField`.
#[test]
fn augmentation_without_declaration_class_static_field() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {
  augment static int foo = 0;
}
"#,
        &[("augmentation_without_declaration", 52, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_augments_instanceField`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_field_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}
augment class A {
  augment static int foo = 0;
}
"#,
        &[
            ("augmentation_without_declaration", 67, 3),
            ("conflicting_static_and_instance", 67, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_augments_instanceGetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_field_augments_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
augment class A {
  augment static int foo = 0;
}
"#,
        &[
            ("augmentation_without_declaration", 72, 3),
            ("conflicting_static_and_instance", 72, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_augments_instanceMethod`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_field_augments_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
augment class A {
  augment static int foo = 0;
}
"#,
        &[
            ("augmentation_without_declaration", 68, 3),
            ("conflicting_static_and_instance", 68, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_augments_instanceSetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_field_augments_instance_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(int _) {}
}
augment class A {
  augment static int foo = 0;
}
"#,
        &[
            ("augmentation_without_declaration", 72, 3),
            ("conflicting_static_and_instance", 72, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_augments_staticField`.
#[test]
fn augmentation_without_declaration_class_static_field_augments_static_field() {
    assert_errors_in_code(
        r#"
class A {
  static int foo = 0;
}

augment class A {
  augment static abstract int foo;
}
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_augments_staticField_final`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_field_augments_static_field_final() {
    assert_errors_in_code(
        r#"
class A {
  static final int foo = 0;
}

augment class A {
  augment static abstract int foo;
}
"#,
        &[("augmentation_without_declaration", 90, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_augments_staticGetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_field_augments_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  static int get foo => 0;
}

augment class A {
  augment static abstract int foo;
}
"#,
        &[("augmentation_without_declaration", 89, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_augments_staticSetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_field_augments_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  static set foo(int _) {}
}

augment class A {
  augment static abstract int foo;
}
"#,
        &[("augmentation_without_declaration", 89, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_final_augments_staticField_final`.
#[test]
fn augmentation_without_declaration_class_static_field_final_augments_static_field_final() {
    assert_errors_in_code(
        r#"
class A {
  static final int foo = 0;
}

augment class A {
  augment static abstract final int foo;
}
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_final_augments_staticGetter`.
#[test]
fn augmentation_without_declaration_class_static_field_final_augments_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  static int get foo => 0;
}

augment class A {
  augment static abstract final int foo;
}
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticField_final_augments_staticSetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_field_final_augments_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  static set foo(int _) {}
}

augment class A {
  augment static abstract final int foo;
}
"#,
        &[("augmentation_without_declaration", 95, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticGetter`.
#[test]
fn augmentation_without_declaration_class_static_getter() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {
  augment static int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticGetter_augments_instanceField`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_getter_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}
augment class A {
  augment static int get foo => 0;
}
"#,
        &[
            ("augmentation_without_declaration", 48, 7),
            ("conflicting_static_and_instance", 71, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticGetter_augments_instanceGetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_getter_augments_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
augment class A {
  augment static int get foo => 0;
}
"#,
        &[
            ("augmentation_without_declaration", 53, 7),
            ("conflicting_static_and_instance", 76, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticGetter_augments_instanceMethod`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_getter_augments_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
augment class A {
  augment static int get foo => 0;
}
"#,
        &[
            ("augmentation_without_declaration", 49, 7),
            ("conflicting_static_and_instance", 72, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticGetter_augments_instanceSetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_getter_augments_instance_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(int _) {}
}
augment class A {
  augment static int get foo => 0;
}
"#,
        &[
            ("augmentation_without_declaration", 53, 7),
            ("conflicting_static_and_instance", 76, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticMethod`.
#[test]
fn augmentation_without_declaration_class_static_method() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {
  augment static void foo() {}
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticMethod_augments_instanceField`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_method_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}
augment class A {
  augment static void foo() {}
}
"#,
        &[
            ("augmentation_without_declaration", 48, 7),
            ("conflicting_static_and_instance", 68, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticMethod_augments_instanceGetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_method_augments_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
augment class A {
  augment static void foo() {}
}
"#,
        &[
            ("augmentation_without_declaration", 53, 7),
            ("conflicting_static_and_instance", 73, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticMethod_augments_instanceMethod`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_method_augments_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
augment class A {
  augment static void foo() {}
}
"#,
        &[
            ("augmentation_without_declaration", 49, 7),
            ("conflicting_static_and_instance", 69, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticMethod_augments_instanceSetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_method_augments_instance_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(int _) {}
}
augment class A {
  augment static void foo() {}
}
"#,
        &[
            ("augmentation_without_declaration", 53, 7),
            ("conflicting_static_and_instance", 73, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticSetter`.
#[test]
fn augmentation_without_declaration_class_static_setter() {
    assert_errors_in_code(
        r#"
class A {}

augment class A {
  augment static set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticSetter_augments_instanceField`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_setter_augments_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}
augment class A {
  augment static set foo(int _) {}
}
"#,
        &[
            ("augmentation_without_declaration", 48, 7),
            ("conflicting_static_and_instance", 67, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticSetter_augments_instanceGetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_setter_augments_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
augment class A {
  augment static set foo(int _) {}
}
"#,
        &[
            ("augmentation_without_declaration", 53, 7),
            ("conflicting_static_and_instance", 72, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticSetter_augments_instanceMethod`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_setter_augments_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
augment class A {
  augment static set foo(int _) {}
}
"#,
        &[
            ("augmentation_without_declaration", 49, 7),
            ("conflicting_static_and_instance", 68, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_class_staticSetter_augments_instanceSetter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_class_static_setter_augments_instance_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(int _) {}
}
augment class A {
  augment static set foo(int _) {}
}
"#,
        &[
            ("augmentation_without_declaration", 53, 7),
            ("conflicting_static_and_instance", 72, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum`.
#[test]
fn augmentation_without_declaration_enum() {
    assert_errors_in_code(
        r#"
augment enum A {}
"#,
        &[("augmentation_without_declaration", 1, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum_constructor`.
#[test]
fn augmentation_without_declaration_enum_constructor() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  const A();
}

augment enum A {;
  augment const A.named();
}
"#,
        &[
            ("augmentation_without_declaration", 51, 7),
            ("unused_element", 67, 5),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum_instanceField`.
#[test]
fn augmentation_without_declaration_enum_instance_field() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  const A();
}

augment enum A {;
  augment final int foo = 0;
}
"#,
        &[("augmentation_without_declaration", 69, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum_instanceField_augments_staticField_noBody`.
#[test]
fn augmentation_without_declaration_enum_instance_field_augments_static_field_no_body() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  const A();
  static int foo = 0;
}

augment enum A {;
  augment abstract int foo;
}
"#,
        &[
            ("augmentation_without_declaration", 94, 3),
            ("non_final_field_in_enum", 94, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum_instanceGetter`.
#[test]
fn augmentation_without_declaration_enum_instance_getter() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  const A();
}

augment enum A {;
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 51, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum_instanceGetter_augments_staticGetter_noBody`.
#[test]
fn augmentation_without_declaration_enum_instance_getter_augments_static_getter_no_body() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  const A();
  static int get foo => 0;
}

augment enum A {;
  augment int get foo;
}
"#,
        &[("augmentation_without_declaration", 78, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum_instanceMethod`.
#[test]
fn augmentation_without_declaration_enum_instance_method() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  const A();
}

augment enum A {;
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 51, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum_instanceMethod_augments_staticMethod_noBody`.
#[test]
fn augmentation_without_declaration_enum_instance_method_augments_static_method_no_body() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  const A();
  static void foo() {}
}

augment enum A {;
  augment void foo();
}
"#,
        &[("augmentation_without_declaration", 74, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum_instanceSetter`.
#[test]
fn augmentation_without_declaration_enum_instance_setter() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  const A();
}

augment enum A {;
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 51, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_enum_instanceSetter_augments_staticSetter_noBody`.
#[test]
fn augmentation_without_declaration_enum_instance_setter_augments_static_setter_no_body() {
    assert_errors_in_code(
        r#"
enum A {
  v;
  const A();
  static set foo(int _) {}
}

augment enum A {;
  augment set foo(int _);
}
"#,
        &[("augmentation_without_declaration", 78, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extension`.
#[test]
fn augmentation_without_declaration_extension() {
    assert_errors_in_code(
        r#"
augment extension A {}
"#,
        &[("augmentation_without_declaration", 1, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extension_instanceGetter`.
#[test]
fn augmentation_without_declaration_extension_instance_getter() {
    assert_errors_in_code(
        r#"
extension A on int {}

augment extension A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 48, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extension_instanceMethod`.
#[test]
fn augmentation_without_declaration_extension_instance_method() {
    assert_errors_in_code(
        r#"
extension A on int {}

augment extension A {
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 48, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extension_instanceSetter`.
#[test]
fn augmentation_without_declaration_extension_instance_setter() {
    assert_errors_in_code(
        r#"
extension A on int {}

augment extension A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 48, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extensionType`.
#[test]
fn augmentation_without_declaration_extension_type() {
    assert_errors_in_code(
        r#"
augment extension type A {}
"#,
        &[("augmentation_without_declaration", 1, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extensionType_constructor`.
#[test]
fn augmentation_without_declaration_extension_type_constructor() {
    assert_errors_in_code(
        r#"
extension type A(int it) {}

augment extension type A {
  augment A.named() : this(0);
}
"#,
        &[("augmentation_without_declaration", 59, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extensionType_hasPrimaryConstructor`.
#[test]
fn augmentation_without_declaration_extension_type_has_primary_constructor() {
    assert_errors_in_code(
        r#"
augment extension type A(int it) {}
"#,
        &[
            ("augmentation_without_declaration", 1, 7),
            (
                "extension_type_augmentation_specifies_representation_field",
                25,
                1,
            ),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extensionType_instanceGetter`.
#[test]
fn augmentation_without_declaration_extension_type_instance_getter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {}

augment extension type A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 59, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extensionType_instanceMethod`.
#[test]
fn augmentation_without_declaration_extension_type_instance_method() {
    assert_errors_in_code(
        r#"
extension type A(int it) {}

augment extension type A {
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 59, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_extensionType_instanceSetter`.
#[test]
fn augmentation_without_declaration_extension_type_instance_setter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {}

augment extension type A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 59, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_mixin`.
#[test]
fn augmentation_without_declaration_mixin() {
    assert_errors_in_code(
        r#"
augment mixin A {}
"#,
        &[("augmentation_without_declaration", 1, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_mixin_instanceField`.
#[test]
fn augmentation_without_declaration_mixin_instance_field() {
    assert_errors_in_code(
        r#"
mixin A {}

augment mixin A {
  augment int foo = 0;
}
"#,
        &[("augmentation_without_declaration", 45, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_mixin_instanceGetter`.
#[test]
fn augmentation_without_declaration_mixin_instance_getter() {
    assert_errors_in_code(
        r#"
mixin A {}

augment mixin A {
  augment int get foo => 0;
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_mixin_instanceMethod`.
#[test]
fn augmentation_without_declaration_mixin_instance_method() {
    assert_errors_in_code(
        r#"
mixin A {}

augment mixin A {
  augment void foo() {}
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_mixin_instanceMethod_augments_instanceMethod`.
#[test]
fn augmentation_without_declaration_mixin_instance_method_augments_instance_method() {
    assert_errors_in_code(
        r#"
mixin A {
  void foo() {}
}

augment mixin A {
  augment void foo();
}
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_mixin_instanceSetter`.
#[test]
fn augmentation_without_declaration_mixin_instance_setter() {
    assert_errors_in_code(
        r#"
mixin A {}

augment mixin A {
  augment set foo(int _) {}
}
"#,
        &[("augmentation_without_declaration", 33, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_function`.
#[test]
fn augmentation_without_declaration_top_level_function() {
    assert_errors_in_code(
        r#"
augment void foo() {}
"#,
        &[("augmentation_without_declaration", 1, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_function_augments_function`.
#[test]
fn augmentation_without_declaration_top_level_function_augments_function() {
    assert_errors_in_code(
        r#"
void foo() {}

augment void foo();
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_getter`.
#[test]
fn augmentation_without_declaration_top_level_getter() {
    assert_errors_in_code(
        r#"
augment int get foo => 0;
"#,
        &[("augmentation_without_declaration", 1, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_setter`.
#[test]
fn augmentation_without_declaration_top_level_setter() {
    assert_errors_in_code(
        r#"
augment set foo(int _) {}
"#,
        &[("augmentation_without_declaration", 1, 7)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable`.
#[test]
fn augmentation_without_declaration_top_level_variable() {
    assert_errors_in_code(
        r#"
augment int foo = 0;
"#,
        &[("augmentation_without_declaration", 13, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable_augments_getter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_top_level_variable_augments_getter() {
    assert_errors_in_code(
        r#"
int? get foo => 0;

augment abstract int? foo;
"#,
        &[("augmentation_without_declaration", 43, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable_augments_setter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_top_level_variable_augments_setter() {
    assert_errors_in_code(
        r#"
set foo(int? _) {}

augment abstract int? foo;
"#,
        &[("augmentation_without_declaration", 43, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable_augments_variable`.
#[test]
fn augmentation_without_declaration_top_level_variable_augments_variable() {
    assert_errors_in_code(
        r#"
int? foo = 0;

augment abstract int? foo;
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable_augments_variable_final`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_top_level_variable_augments_variable_final() {
    assert_errors_in_code(
        r#"
final int? foo = 0;

augment abstract int? foo;
"#,
        &[("augmentation_without_declaration", 44, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable_final_augments_getter`.
#[test]
fn augmentation_without_declaration_top_level_variable_final_augments_getter() {
    assert_errors_in_code(
        r#"
int? get foo => 0;

augment abstract final int? foo;
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable_final_augments_setter`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn augmentation_without_declaration_top_level_variable_final_augments_setter() {
    assert_errors_in_code(
        r#"
set foo(int? _) {}

augment abstract final int? foo;
"#,
        &[("augmentation_without_declaration", 49, 3)],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable_final_augments_variable_final`.
#[test]
fn augmentation_without_declaration_top_level_variable_final_augments_variable_final() {
    assert_errors_in_code(
        r#"
final int? foo = 0;

augment abstract final int? foo;
"#,
        &[],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable_multiple`.
#[test]
fn augmentation_without_declaration_top_level_variable_multiple() {
    assert_errors_in_code(
        r#"
augment int foo = 0, bar = 0;
"#,
        &[
            ("augmentation_without_declaration", 13, 3),
            ("augmentation_without_declaration", 22, 3),
        ],
    );
}

/// `augmentation_without_declaration_test.dart` `test_topLevel_variable_multiple_oneMissing`.
#[test]
fn augmentation_without_declaration_top_level_variable_multiple_one_missing() {
    assert_errors_in_code(
        r#"
int? bar = 0;

augment abstract int? foo, bar;
"#,
        &[("augmentation_without_declaration", 38, 3)],
    );
}

/// `class_used_as_mixin_declares_generative_constructor_test.dart` `test_withClause_class_language219_factory`.
#[test]
fn class_used_as_mixin_declares_generative_constructor_with_clause_class_language219_factory() {
    assert_errors_in_code(
        r#"
// @dart = 2.19
class A {
  factory A() => throw 0;
}
class B extends Object with A {}
"#,
        &[],
    );
}

/// `class_used_as_mixin_declares_generative_constructor_test.dart` `test_withClause_class_language219_generative_named`.
#[test]
fn class_used_as_mixin_declares_generative_constructor_with_clause_class_language219_generative_named()
 {
    assert_errors_in_code(
        r#"
// @dart = 2.19
class A {
  A.named();
}
class B extends Object with A {}
"#,
        &[("class_used_as_mixin_declares_generative_constructor", 70, 1)],
    );
}

/// `class_used_as_mixin_declares_generative_constructor_test.dart` `test_withClause_class_language219_generative_unnamed`.
#[test]
fn class_used_as_mixin_declares_generative_constructor_with_clause_class_language219_generative_unnamed()
 {
    assert_errors_in_code(
        r#"
// @dart = 2.19
class A {
  A();
}
class B extends Object with A {}
"#,
        &[("class_used_as_mixin_declares_generative_constructor", 64, 1)],
    );
}

/// `class_used_as_mixin_declares_generative_constructor_test.dart` `test_withClause_classTypeAlias_language219_generative_unnamed`.
#[test]
fn class_used_as_mixin_declares_generative_constructor_with_clause_class_type_alias_language219_generative_unnamed()
 {
    assert_errors_in_code(
        r#"
// @dart = 2.19
class A {
  A();
}
class B = Object with A;
"#,
        &[("class_used_as_mixin_declares_generative_constructor", 58, 1)],
    );
}

/// `class_used_as_mixin_declares_generative_constructor_test.dart` `test_withClause_enum_language219_generative_unnamed`.
#[test]
fn class_used_as_mixin_declares_generative_constructor_with_clause_enum_language219_generative_unnamed()
 {
    assert_errors_in_code(
        r#"
// @dart = 2.19
class A {
  A();
}

enum E with A {
  v
}
"#,
        &[("class_used_as_mixin_declares_generative_constructor", 49, 1)],
    );
}

/// `class_used_as_mixin_test.dart` `test_coreLib`.
#[test]
fn class_used_as_mixin_core_lib() {
    assert_errors_in_code(
        r#"
class Bar with Comparable<int> {
  int compareTo(int x) => 0;
}
"#,
        &[("class_used_as_mixin", 16, 15)],
    );
}

/// `class_used_as_mixin_test.dart` `test_coreLib_dartCoreEnum`.
#[test]
fn class_used_as_mixin_core_lib_dart_core_enum() {
    assert_errors_in_code(
        r#"
abstract class A with Enum {}
abstract class B = Object with Enum;
"#,
        &[
            ("class_used_as_mixin", 23, 4),
            ("class_used_as_mixin", 62, 4),
        ],
    );
}

/// `class_used_as_mixin_test.dart` `test_coreLib_dartCoreEnum_language219`.
#[test]
fn class_used_as_mixin_core_lib_dart_core_enum_language219() {
    assert_errors_in_code(
        r#"
// @dart = 2.19
abstract class A with Enum {}
abstract class B = Object with Enum;
"#,
        &[],
    );
}

/// `class_used_as_mixin_test.dart` `test_coreLib_language219`.
#[test]
fn class_used_as_mixin_core_lib_language219() {
    assert_errors_in_code(
        r#"
// @dart = 2.19
class Bar with Comparable<int> {
  int compareTo(int x) => 0;
}
"#,
        &[],
    );
}

/// `class_used_as_mixin_test.dart` `test_inside`.
#[test]
fn class_used_as_mixin_inside() {
    assert_errors_in_code(
        r#"
class Foo {}
class Bar with Foo {}
"#,
        &[("class_used_as_mixin", 29, 3)],
    );
}

/// `class_used_as_mixin_test.dart` `test_inside_class_hasGenerativeConstructor`.
#[test]
fn class_used_as_mixin_inside_class_has_generative_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A() {}
}
class B extends Object with A {}
"#,
        &[("class_used_as_mixin", 50, 1)],
    );
}

/// `class_used_as_mixin_test.dart` `test_inside_classTypeAlias_hasGenerativeConstructor`.
#[test]
fn class_used_as_mixin_inside_class_type_alias_has_generative_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A() {}
}
class B = Object with A;
"#,
        &[("class_used_as_mixin", 44, 1)],
    );
}

/// `class_used_as_mixin_test.dart` `test_inside_enum_hasGenerativeConstructor`.
#[test]
fn class_used_as_mixin_inside_enum_has_generative_constructor() {
    assert_errors_in_code(
        r#"
class A {
  A() {}
}

enum E with A {
  v
}
"#,
        &[("class_used_as_mixin", 35, 1)],
    );
}

/// `class_used_as_mixin_test.dart` `test_inside_language219`.
#[test]
fn class_used_as_mixin_inside_language219() {
    assert_errors_in_code(
        r#"
// @dart = 2.19
class Foo {}
class Bar with Foo {}
"#,
        &[],
    );
}

/// `class_used_as_mixin_test.dart` `test_inside_mixinClass`.
#[test]
fn class_used_as_mixin_inside_mixin_class() {
    assert_errors_in_code(
        r#"
mixin class Foo {}
class Bar with Foo {}
"#,
        &[],
    );
}

/// `conflicting_field_and_method_test.dart` `test_class_inSuper_field`.
#[test]
fn conflicting_field_and_method_class_in_super_field() {
    assert_errors_in_code(
        r#"
class A {
  foo() {}
}
class B extends A {
  int foo = 0;
}
"#,
        &[("conflicting_field_and_method", 50, 3)],
    );
}

/// `conflicting_field_and_method_test.dart` `test_class_inSuper_getter`.
#[test]
fn conflicting_field_and_method_class_in_super_getter() {
    assert_errors_in_code(
        r#"
class A {
  foo() {}
}
class B extends A {
  get foo => 0;
}
"#,
        &[("conflicting_field_and_method", 50, 3)],
    );
}

/// `conflicting_field_and_method_test.dart` `test_class_inSuper_getter_withAugmentation_inAugmentation`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_field_and_method_class_in_super_getter_with_augmentation_in_augmentation() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

class B extends A {}

augment class B {
  int get foo => 0;
}
"#,
        &[],
    );
}

/// `conflicting_field_and_method_test.dart` `test_class_inSuper_getter_withAugmentation_inDeclaration`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_field_and_method_class_in_super_getter_with_augmentation_in_declaration() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

class B {
  int get foo => 0;
}

augment class B extends A {}
"#,
        &[],
    );
}

/// `conflicting_field_and_method_test.dart` `test_class_inSuper_setter`.
#[test]
fn conflicting_field_and_method_class_in_super_setter() {
    assert_errors_in_code(
        r#"
class A {
  foo() {}
}
class B extends A {
  set foo(_) {}
}
"#,
        &[("conflicting_field_and_method", 50, 3)],
    );
}

/// `conflicting_field_and_method_test.dart` `test_enum_inMixin_field`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_field_and_method_enum_in_mixin_field() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}

enum E with M {
  v;
  final int foo = 0;
}
"#,
        &[("conflicting_field_and_method", 63, 3)],
    );
}

/// `conflicting_field_and_method_test.dart` `test_enum_inMixin_getter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_field_and_method_enum_in_mixin_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}

enum E with M {
  v;
  int get foo => 0;
}
"#,
        &[("conflicting_field_and_method", 61, 3)],
    );
}

/// `conflicting_field_and_method_test.dart` `test_enum_inMixin_getter_withAugmentation_inAugmentation`.
#[test]
#[ignore = "augmented enum: the analyzer reports nothing (its `getInherited` finds no mixin member when the enum has an augmentation), dartr reports; inheritance or element model difference, not the verifier port"]
fn conflicting_field_and_method_enum_in_mixin_getter_with_augmentation_in_augmentation() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}

enum E with M {v}

augment enum E {;
  int get foo => 0;
}
"#,
        &[],
    );
}

/// `conflicting_field_and_method_test.dart` `test_enum_inMixin_getter_withAugmentation_inDeclaration`.
#[test]
#[ignore = "augmented enum: the analyzer reports nothing (its `getInherited` finds no mixin member when the enum has an augmentation), dartr reports; inheritance or element model difference, not the verifier port"]
fn conflicting_field_and_method_enum_in_mixin_getter_with_augmentation_in_declaration() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}

enum E {
  v;
  int get foo => 0;
}

augment enum E with M {}
"#,
        &[],
    );
}

/// `conflicting_field_and_method_test.dart` `test_enum_inMixin_setter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_field_and_method_enum_in_mixin_setter() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}

enum E with M {
  v;
  set foo(int _) {}
}
"#,
        &[("conflicting_field_and_method", 57, 3)],
    );
}

/// `conflicting_field_and_method_test.dart` `test_extensionType_getter`.
#[test]
fn conflicting_field_and_method_extension_type_getter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  void foo() {}
}

extension type B(int it) implements A {
  int get foo => 0;
}
"#,
        &[],
    );
}

/// `conflicting_field_and_method_test.dart` `test_extensionType_setter`.
#[test]
fn conflicting_field_and_method_extension_type_setter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  void foo() {}
}

extension type B(int it) implements A {
  set foo(int _) {}
}
"#,
        &[],
    );
}

/// `conflicting_field_and_method_test.dart` `test_mixin_inSuper_getter_withAugmentation_inAugmentation`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_field_and_method_mixin_in_super_getter_with_augmentation_in_augmentation() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

mixin B on A {}

augment mixin B {
  int get foo => 0;
}
"#,
        &[],
    );
}

/// `conflicting_field_and_method_test.dart` `test_mixin_inSuper_getter_withAugmentation_inDeclaration`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_field_and_method_mixin_in_super_getter_with_augmentation_in_declaration() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

mixin B {
  int get foo => 0;
}

augment mixin B on A {}
"#,
        &[],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_class_extends_augmentation_implements`.
#[test]
fn conflicting_generic_interfaces_class_extends_augmentation_implements() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<int> {}
class B extends A {}
augment class B implements I<String> {}
"#,
        &[("conflicting_generic_interfaces", 50, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_class_extends_implements`.
#[test]
fn conflicting_generic_interfaces_class_extends_implements() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<int> {}
class B implements I<String> {}
class C extends A implements B {}
"#,
        &[("conflicting_generic_interfaces", 82, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_class_extends_implements_never`.
#[test]
fn conflicting_generic_interfaces_class_extends_implements_never() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<Never> {}
class B implements I<Never> {}
class C extends A implements B {}
"#,
        &[],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_class_extends_implements_nullability`.
#[test]
fn conflicting_generic_interfaces_class_extends_implements_nullability() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<int> {}
class B implements I<int?> {}
class C extends A implements B {}
"#,
        &[("conflicting_generic_interfaces", 80, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_class_extends_implements_object_objectQuestion`.
#[test]
fn conflicting_generic_interfaces_class_extends_implements_object_object_question() {
    assert_errors_in_code(
        r#"
class A<T> {}
class B implements A<Object> {}
class C implements A<Object?> {}
class D extends B implements C {}
"#,
        &[("conflicting_generic_interfaces", 86, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_class_extends_with`.
#[test]
fn conflicting_generic_interfaces_class_extends_with() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<int> {}
mixin B implements I<String> {}
class C extends A with B {}
"#,
        &[("conflicting_generic_interfaces", 82, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_class_topMerge`.
#[test]
fn conflicting_generic_interfaces_class_top_merge() {
    assert_errors_in_code(
        r#"
import 'dart:async';

class A<T> {}

class B extends A<FutureOr<Object>> {}

class C extends B implements A<Object> {}
"#,
        &[],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_classTypeAlias_extends_nonFunctionTypedef_with`.
#[test]
fn conflicting_generic_interfaces_class_type_alias_extends_non_function_typedef_with() {
    assert_errors_in_code(
        r#"
class I<T> {}
typedef A = I<int>;
mixin M implements I<String> {}
class C = A with M;
"#,
        &[("conflicting_generic_interfaces", 73, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_classTypeAlias_extends_nonFunctionTypedef_with_ok`.
#[test]
fn conflicting_generic_interfaces_class_type_alias_extends_non_function_typedef_with_ok() {
    assert_errors_in_code(
        r#"
class I<T> {}
typedef A = I<String>;
mixin M implements I<String> {}
class C = A with M;
"#,
        &[],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_classTypeAlias_extends_with`.
#[test]
fn conflicting_generic_interfaces_class_type_alias_extends_with() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<int> {}
mixin M implements I<String> {}
class C = A with M;
"#,
        &[("conflicting_generic_interfaces", 82, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_enum_implements`.
#[test]
fn conflicting_generic_interfaces_enum_implements() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<int> {}
class B implements I<String> {}
enum E implements A, B {
  v
}
"#,
        &[("conflicting_generic_interfaces", 81, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_enum_with`.
#[test]
fn conflicting_generic_interfaces_enum_with() {
    assert_errors_in_code(
        r#"
class I<T> {}
mixin M1 implements I<int> {}
mixin M2 implements I<String> {}
enum E with M1, M2 {
  v
}
"#,
        &[("conflicting_generic_interfaces", 83, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_extensionType`.
#[test]
fn conflicting_generic_interfaces_extension_type() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<int> {}
class B implements I<num> {}
extension type C(Never it) implements A, B {}
"#,
        &[
            ("conflicting_generic_interfaces", 88, 1),
            ("extension_type_representation_type_bottom", 90, 5),
        ],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_mixin_on_implements`.
#[test]
fn conflicting_generic_interfaces_mixin_on_implements() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<int> {}
class B implements I<String> {}
mixin M on A implements B {}
"#,
        &[("conflicting_generic_interfaces", 82, 1)],
    );
}

/// `conflicting_generic_interfaces_test.dart` `test_noConflict`.
#[test]
fn conflicting_generic_interfaces_no_conflict() {
    assert_errors_in_code(
        r#"
class I<T> {}
class A implements I<int> {}
class B implements I<int> {}
class C extends A implements B {}
"#,
        &[],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_class_declaresSetter`.
#[test]
fn conflicting_inherited_method_and_setter_class_declares_setter() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

class B {
  set foo(int _) {}
}

abstract class C implements A, B {
  set foo(int _) {}
}
"#,
        &[("conflicting_field_and_method", 104, 3)],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_class_interface2`.
#[test]
fn conflicting_inherited_method_and_setter_class_interface2() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

class B {
  set foo(int _) {}
}

abstract class C implements A, B {}
"#,
        &[("conflicting_inherited_method_and_setter", 78, 1)],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_class_mixin_interface`.
#[test]
fn conflicting_inherited_method_and_setter_class_mixin_interface() {
    assert_errors_in_code(
        r#"
mixin A {
  void foo() {}
}

class B {
  set foo(int _) {}
}

abstract class C with A implements B {}
"#,
        &[("conflicting_inherited_method_and_setter", 78, 1)],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_class_superclass_interface`.
#[test]
fn conflicting_inherited_method_and_setter_class_superclass_interface() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

class B {
  set foo(int _) {}
}

abstract class C extends A implements B {}
"#,
        &[("conflicting_inherited_method_and_setter", 78, 1)],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_class_superclass_mixin`.
#[test]
fn conflicting_inherited_method_and_setter_class_superclass_mixin() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

mixin B {
  set foo(int _) {}
}

abstract class C extends A with B {}
"#,
        &[("conflicting_inherited_method_and_setter", 78, 1)],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedGetterSetter_noConflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_getter_setter_no_conflict() {
    assert_errors_in_code(
        r#"
extension type A(Object? it) {
  int get foo => 0;
}

extension type B(Object? it) {
  set foo(int _) {}
}

extension type C(Object? it) implements A, B {}
"#,
        &[],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethod_diamond_noConflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_diamond_no_conflict() {
    assert_errors_in_code(
        r#"
extension type Base(Object? it) {
  void foo() {}
}

extension type Left(Object? it) implements Base {}

extension type Right(Object? it) implements Base {}

extension type C(Object? it) implements Left, Right {}
"#,
        &[],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_conflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_conflict() {
    assert_errors_in_code(
        r#"
extension type A(Object? it) {
  void foo() {}
}

extension type B(Object? it) {
  set foo(int _) {}
}

extension type C(Object? it) implements A, B {}
"#,
        &[("conflicting_inherited_method_and_setter", 120, 1)],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_declaredGetter_noConflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_declared_getter_no_conflict()
 {
    assert_errors_in_code(
        r#"
class A {}

extension type A1(A it) {
  void foo() {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A1, B {
  int get foo => 0;
}
"#,
        &[],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_declaredMethod_noConflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_declared_method_no_conflict()
 {
    assert_errors_in_code(
        r#"
class A {}

extension type A1(A it) {
  void foo() {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A1, B {
  void foo() {}
}
"#,
        &[],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_declaredSetter_noConflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_declared_setter_no_conflict()
 {
    assert_errors_in_code(
        r#"
class A {}

extension type A1(A it) {
  void foo() {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A1, B {
  set foo(int _) {}
}
"#,
        &[],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_declaredStaticMethod_conflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_declared_static_method_conflict()
 {
    assert_errors_in_code(
        r#"
extension type A(Object? it) {
  void foo() {}
}

extension type B(Object? it) {
  set foo(int _) {}
}

extension type C(Object? it) implements A, B {
  static void foo() {}
}
"#,
        &[
            ("conflicting_inherited_method_and_setter", 120, 1),
            ("conflicting_static_and_instance", 166, 3),
        ],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_declaredUnrelated_conflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_declared_unrelated_conflict()
 {
    assert_errors_in_code(
        r#"
extension type A(Object? it) {
  void foo() {}
}

extension type B(Object? it) {
  set foo(int _) {}
}

extension type C(Object? it) implements A, B {
  void bar() {}
}
"#,
        &[("conflicting_inherited_method_and_setter", 120, 1)],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_fromClass_declaredGetter_noConflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_from_class_declared_getter_no_conflict()
 {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A, B {
  int get foo => 0;
}
"#,
        &[],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_fromClass_declaredMethod_noConflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_from_class_declared_method_no_conflict()
 {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A, B {
  void foo() {}
}
"#,
        &[],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_fromClass_declaredSetter_noConflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_from_class_declared_setter_no_conflict()
 {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A, B {
  set foo(int _) {}
}
"#,
        &[],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_implicitSetter_conflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_implicit_setter_conflict()
 {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}

abstract class I {
  void foo();
}

extension type E(Object it) implements A, I {}
"#,
        &[
            ("conflicting_inherited_method_and_setter", 80, 1),
            ("extension_type_implements_not_supertype", 104, 1),
            ("extension_type_implements_not_supertype", 107, 1),
        ],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_indirect_conflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_indirect_conflict()
 {
    assert_errors_in_code(
        r#"
extension type BaseMethod(Object? it) {
  void foo() {}
}

extension type BaseSetter(Object? it) {
  set foo(int _) {}
}

extension type Left(Object? it) implements BaseMethod {}

extension type Right(Object? it) implements BaseSetter {}

extension type C(Object? it) implements Left, Right {}
"#,
        &[("conflicting_inherited_method_and_setter", 255, 1)],
    );
}

/// `conflicting_inherited_method_and_setter_test.dart` `test_extensionType_inheritedMethodSetter_multiple_conflict`.
#[test]
fn conflicting_inherited_method_and_setter_extension_type_inherited_method_setter_multiple_conflict()
 {
    assert_errors_in_code(
        r#"
extension type A(Object? it) {
  void foo() {}
}

extension type B(Object? it) {
  set foo(int _) {}
}

extension type C(Object? it) {
  void bar() {}
}

extension type D(Object? it) implements A, B, C {}
"#,
        &[("conflicting_inherited_method_and_setter", 170, 1)],
    );
}

/// `conflicting_method_and_field_test.dart` `test_class_inSuper_field`.
#[test]
fn conflicting_method_and_field_class_in_super_field() {
    assert_errors_in_code(
        r#"
class A {
  int foo = 0;
}
class B extends A {
  foo() {}
}
"#,
        &[("conflicting_method_and_field", 50, 3)],
    );
}

/// `conflicting_method_and_field_test.dart` `test_class_inSuper_getter`.
#[test]
fn conflicting_method_and_field_class_in_super_getter() {
    assert_errors_in_code(
        r#"
class A {
  get foo => 0;
}
class B extends A {
  foo() {}
}
"#,
        &[("conflicting_method_and_field", 51, 3)],
    );
}

/// `conflicting_method_and_field_test.dart` `test_class_inSuper_getter_hasAugmentation_inAugmentation`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_method_and_field_class_in_super_getter_has_augmentation_in_augmentation() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}

class B extends A {}

augment class B {
  void foo() {}
}
"#,
        &[],
    );
}

/// `conflicting_method_and_field_test.dart` `test_class_inSuper_getter_hasAugmentation_inDeclaration`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_method_and_field_class_in_super_getter_has_augmentation_in_declaration() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}

class B extends A {
  void foo() {}
}

augment class B {}
"#,
        &[],
    );
}

/// `conflicting_method_and_field_test.dart` `test_class_inSuper_setter`.
#[test]
fn conflicting_method_and_field_class_in_super_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
class B extends A {
  foo() {}
}
"#,
        &[("conflicting_method_and_field", 51, 3)],
    );
}

/// `conflicting_method_and_field_test.dart` `test_enum_inMixin_field`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_method_and_field_enum_in_mixin_field() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}

enum E with M {
  v;
  final int foo = 0;
}
"#,
        &[("conflicting_field_and_method", 63, 3)],
    );
}

/// `conflicting_method_and_field_test.dart` `test_enum_inMixin_getter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_method_and_field_enum_in_mixin_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  int get foo => 0;
}

enum E with M {
  v;
  void foo() {}
}
"#,
        &[("conflicting_method_and_field", 62, 3)],
    );
}

/// `conflicting_method_and_field_test.dart` `test_enum_inMixin_getter_hasAugmentation_inAugmentation`.
#[test]
#[ignore = "augmented enum: the analyzer reports nothing (its `getInherited` finds no mixin member when the enum has an augmentation), dartr reports; inheritance or element model difference, not the verifier port"]
fn conflicting_method_and_field_enum_in_mixin_getter_has_augmentation_in_augmentation() {
    assert_errors_in_code(
        r#"
mixin M {
  int get foo => 0;
}

enum E with M {v}

augment enum E {;
  void foo() {}
}
"#,
        &[],
    );
}

/// `conflicting_method_and_field_test.dart` `test_enum_inMixin_getter_hasAugmentation_inDeclaration`.
#[test]
#[ignore = "augmented enum: the analyzer reports nothing (its `getInherited` finds no mixin member when the enum has an augmentation), dartr reports; inheritance or element model difference, not the verifier port"]
fn conflicting_method_and_field_enum_in_mixin_getter_has_augmentation_in_declaration() {
    assert_errors_in_code(
        r#"
mixin M {
  int get foo => 0;
}

enum E with M {
  v;
  void foo() {}
}

augment enum E {}
"#,
        &[],
    );
}

/// `conflicting_method_and_field_test.dart` `test_enum_inMixin_setter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_method_and_field_enum_in_mixin_setter() {
    assert_errors_in_code(
        r#"
mixin M {
  set foo(int _) {}
}

enum E with M {
  v;
  void foo() {}
}
"#,
        &[("conflicting_method_and_field", 62, 3)],
    );
}

/// `conflicting_method_and_field_test.dart` `test_extensionType_field_external`.
#[test]
fn conflicting_method_and_field_extension_type_field_external() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  external int foo;
}

extension type B(int it) implements A {
  void foo() {}
}
"#,
        &[],
    );
}

/// `conflicting_method_and_field_test.dart` `test_extensionType_getter`.
#[test]
fn conflicting_method_and_field_extension_type_getter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  int get foo => 0;
}

extension type B(int it) implements A {
  void foo() {}
}
"#,
        &[],
    );
}

/// `conflicting_method_and_field_test.dart` `test_extensionType_setter`.
#[test]
fn conflicting_method_and_field_extension_type_setter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  set foo(int _) {}
}

extension type B(int it) implements A {
  void foo() {}
}
"#,
        &[],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_instanceMethod_staticMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_instance_method_static_method() {
    assert_errors_in_code(
        r#"
class C {
  void foo() {}
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 41, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_instanceMethod_staticMethodInAugmentation`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_static_and_instance_in_class_instance_method_static_method_in_augmentation() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

augment class A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 62, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticGetter_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_static_getter_instance_getter() {
    assert_errors_in_code(
        r#"
class C {
  static int get foo => 0;
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 28, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticGetter_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_static_getter_instance_method() {
    assert_errors_in_code(
        r#"
class C {
  static int get foo => 0;
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 28, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticGetter_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_static_getter_instance_setter() {
    assert_errors_in_code(
        r#"
class C {
  static int get foo => 0;
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 28, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticMethod_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_static_method_instance_getter() {
    assert_errors_in_code(
        r#"
class C {
  static void foo() {}
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 25, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticMethod_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_static_method_instance_method() {
    assert_errors_in_code(
        r#"
class C {
  static void foo() {}
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 25, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticMethod_instanceMethodInAugmentation`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_static_and_instance_in_class_static_method_instance_method_in_augmentation() {
    assert_errors_in_code(
        r#"
class A {
  static void foo() {}
}

augment class A {
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 25, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticMethod_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_static_method_instance_setter() {
    assert_errors_in_code(
        r#"
class C {
  static void foo() {}
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 25, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticSetter_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_static_setter_instance_getter() {
    assert_errors_in_code(
        r#"
class C {
  static set foo(_) {}
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 24, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticSetter_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_static_setter_instance_method() {
    assert_errors_in_code(
        r#"
class C {
  static set foo(_) {}
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 24, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inClass_staticSetter_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_class_static_setter_instance_setter() {
    assert_errors_in_code(
        r#"
class C {
  static set foo(_) {}
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 24, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inInterface_instanceGetter_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_interface_instance_getter_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
abstract class B implements A {
  static int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 82, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inInterface_instanceGetter_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_interface_instance_getter_static_method() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
abstract class B implements A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 79, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inInterface_instanceMethod_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_interface_instance_method_static_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
abstract class B implements A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 75, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inInterface_instanceMethod_staticSetter`.
#[test]
fn conflicting_static_and_instance_in_interface_instance_method_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
abstract class B implements A {
  static set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 74, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inInterface_instanceSetter_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_interface_instance_setter_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
abstract class B implements A {
  static int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 78, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inInterface_instanceSetter_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_interface_instance_setter_static_method() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
abstract class B implements A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 75, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inInterface_instanceSetter_staticSetter`.
#[test]
fn conflicting_static_and_instance_in_interface_instance_setter_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
abstract class B implements A {
  static set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 74, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_instanceGetter_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_mixin_instance_getter_static_getter() {
    assert_errors_in_code(
        r#"
mixin A {
  int get foo => 0;
}
class B extends Object with A {
  static int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 82, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_instanceGetter_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_mixin_instance_getter_static_method() {
    assert_errors_in_code(
        r#"
mixin A {
  int get foo => 0;
}
class B extends Object with A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 79, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_instanceMethod_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_mixin_instance_method_static_method() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}
class B extends Object with M {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 75, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_instanceMethod_staticMethodInAugmentation`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_static_and_instance_in_mixin_instance_method_static_method_in_augmentation() {
    assert_errors_in_code(
        r#"
mixin A {
  void foo() {}
}

augment mixin A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 62, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_instanceMethod_staticSetter`.
#[test]
fn conflicting_static_and_instance_in_mixin_instance_method_static_setter() {
    assert_errors_in_code(
        r#"
mixin A {
  void foo() {}
}
class B extends Object with A {
  static set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 74, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_instanceSetter_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_mixin_instance_setter_static_getter() {
    assert_errors_in_code(
        r#"
mixin A {
  set foo(_) {}
}
class B extends Object with A {
  static int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 78, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_instanceSetter_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_mixin_instance_setter_static_method() {
    assert_errors_in_code(
        r#"
mixin A {
  set foo(_) {}
}
class B extends Object with A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 75, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_instanceSetter_staticSetter`.
#[test]
fn conflicting_static_and_instance_in_mixin_instance_setter_static_setter() {
    assert_errors_in_code(
        r#"
mixin A {
  set foo(_) {}
}
class B extends Object with A {
  static set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 74, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticMethod_instanceMethodInAugmentation`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn conflicting_static_and_instance_in_mixin_static_method_instance_method_in_augmentation() {
    assert_errors_in_code(
        r#"
mixin A {
  static void foo() {}
}

augment mixin A {
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 25, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_implicitObject_staticMethod_instanceGetter`.
#[test]
fn conflicting_static_and_instance_in_super_implicit_object_static_method_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  static String runtimeType() => 'x';
}
"#,
        &[("conflicting_static_and_instance", 27, 11)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_implicitObject_staticMethod_instanceMethod`.
#[test]
fn conflicting_static_and_instance_in_super_implicit_object_static_method_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  static String toString() => 'x';
}
"#,
        &[("conflicting_static_and_instance", 27, 8)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_instanceGetter_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_super_instance_getter_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
class B extends A {
  static int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 70, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_instanceGetter_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_super_instance_getter_static_method() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
class B extends A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 67, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_instanceMethod_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_super_instance_method_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
class B extends A {
  static int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 66, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_instanceMethod_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_super_instance_method_static_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
class B extends A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 63, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_instanceMethod_staticSetter`.
#[test]
fn conflicting_static_and_instance_in_super_instance_method_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
class B extends A {
  static set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 62, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_instanceSetter_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_super_instance_setter_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
class B extends A {
  static int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 66, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_instanceSetter_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_super_instance_setter_static_method() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
class B extends A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 63, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inSuper_instanceSetter_staticSetter`.
#[test]
fn conflicting_static_and_instance_in_super_instance_setter_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
class B extends A {
  static set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 62, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_constant_hashCode`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_constant_hash_code() {
    assert_errors_in_code(
        r#"
enum E {
  a, hashCode, b
}
"#,
        &[("conflicting_static_and_instance", 15, 8)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_constant_index`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_constant_index() {
    assert_errors_in_code(
        r#"
enum E {
  a, index, b
}
"#,
        &[("conflicting_static_and_instance", 15, 5)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_constant_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_constant_instance_setter() {
    assert_errors_in_code(
        r#"
enum E {
  foo;
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 12, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_constant_noSuchMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_constant_no_such_method() {
    assert_errors_in_code(
        r#"
enum E {
  a, noSuchMethod, b
}
"#,
        &[("conflicting_static_and_instance", 15, 12)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_constant_runtimeType`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_constant_runtime_type() {
    assert_errors_in_code(
        r#"
enum E {
  a, runtimeType, b
}
"#,
        &[("conflicting_static_and_instance", 15, 11)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_constant_staticMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_constant_static_method() {
    assert_errors_in_code(
        r#"
enum E {
  foo;
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 12, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_constant_toString`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_constant_to_string() {
    assert_errors_in_code(
        r#"
enum E {
  a, toString, b
}
"#,
        &[("conflicting_static_and_instance", 15, 8)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_field_dartCoreEnum`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_field_dart_core_enum() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static final int hashCode = 0;
}
"#,
        &[("conflicting_static_and_instance", 34, 8)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_field_mixin_getter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_field_mixin_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  int get foo => 0;
}

enum E with M {
  v;
  static final int foo = 0;
}
"#,
        &[("conflicting_static_and_instance", 74, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_field_mixin_method`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_field_mixin_method() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}

enum E with M {
  v;
  static final int foo = 0;
}
"#,
        &[("conflicting_static_and_instance", 70, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_field_mixin_setter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_field_mixin_setter() {
    assert_errors_in_code(
        r#"
mixin M {
  set foo(int _) {}
}

enum E with M {
  v;
  static final int foo = 0;
}
"#,
        &[("conflicting_static_and_instance", 74, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_field_this_constant`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_field_this_constant() {
    assert_errors_in_code(
        r#"
enum E {
  foo;
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 12, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_field_this_getter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_field_this_getter() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static final int foo = 0;
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 34, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_field_this_method`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_field_this_method() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static final int foo = 0;
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 34, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_field_this_setter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_field_this_setter() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static final int foo = 0;
  set foo(int _) {}
}
"#,
        &[("conflicting_static_and_instance", 34, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_method_dartCoreEnum`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_method_dart_core_enum() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static int hashCode() => 0;
}
"#,
        &[("conflicting_static_and_instance", 28, 8)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_method_mixin_getter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_method_mixin_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  int get foo => 0;
}

enum E with M {
  v;
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 69, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_method_mixin_method`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_method_mixin_method() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo() {}
}

enum E with M {
  v;
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 65, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_method_mixin_setter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_method_mixin_setter() {
    assert_errors_in_code(
        r#"
mixin M {
  set foo(int _) {}
}

enum E with M {
  v;
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 69, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_staticGetter_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_static_getter_instance_setter() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static int get foo => 0;
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 32, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_staticMethod_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_static_method_instance_getter() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static void foo() {}
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 29, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_staticMethod_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_static_method_instance_method() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static void foo() {}
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 29, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_staticMethod_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_static_method_instance_setter() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static void foo() {}
  set foo(int _) {}
}
"#,
        &[("conflicting_static_and_instance", 29, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_staticSetter_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_static_setter_instance_getter() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static set foo(_) {}
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 28, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inExtensionType_staticGetter_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_extension_type_static_getter_instance_getter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  static int get foo => 0;
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 45, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inExtensionType_staticGetter_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_extension_type_static_getter_instance_method() {
    assert_errors_in_code(
        r#"
extension type A(int t) {
  static int get foo => 0;
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 44, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inExtensionType_staticGetter_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_extension_type_static_getter_instance_setter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  static int get foo => 0;
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 45, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inExtensionType_staticMethod_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_extension_type_static_method_instance_getter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  static void foo() {}
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 42, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inExtensionType_staticMethod_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_extension_type_static_method_instance_method() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  static void foo() {}
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 42, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inExtensionType_staticMethod_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_extension_type_static_method_instance_setter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  static void foo() {}
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 42, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inExtensionType_staticSetter_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_extension_type_static_setter_instance_getter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  static set foo(_) {}
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 41, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inExtensionType_staticSetter_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_extension_type_static_setter_instance_method() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  static set foo(_) {}
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 41, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inExtensionType_staticSetter_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_extension_type_static_setter_instance_setter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  static set foo(_) {}
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 41, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inInterface_instanceMethod_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_interface_instance_method_static_getter() {
    assert_errors_in_code(
        r#"
extension type A(int it) {
  int get foo => 0;
}

extension type B(int it) implements A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 105, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_dartCoreEnum_index_field`.
#[test]
fn conflicting_static_and_instance_dart_core_enum_index_field() {
    assert_errors_in_code(
        r#"
mixin M on Enum {
  static int index = 0;
}
"#,
        &[("conflicting_static_and_instance", 32, 5)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_dartCoreEnum_index_getter`.
#[test]
fn conflicting_static_and_instance_dart_core_enum_index_getter() {
    assert_errors_in_code(
        r#"
mixin M on Enum {
  static int get index => 0;
}
"#,
        &[("conflicting_static_and_instance", 36, 5)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_dartCoreEnum_index_method`.
#[test]
fn conflicting_static_and_instance_dart_core_enum_index_method() {
    assert_errors_in_code(
        r#"
mixin M on Enum {
  static int index() => 0;
}
"#,
        &[("conflicting_static_and_instance", 32, 5)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_dartCoreEnum_index_setter`.
#[test]
fn conflicting_static_and_instance_dart_core_enum_index_setter() {
    assert_errors_in_code(
        r#"
mixin M on Enum {
  static set index(int _) {}
}
"#,
        &[("conflicting_static_and_instance", 32, 5)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inConstraint_implicitObject_staticMethod_instanceGetter`.
#[test]
fn conflicting_static_and_instance_in_constraint_implicit_object_static_method_instance_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  static String runtimeType() => 'x';
}
"#,
        &[("conflicting_static_and_instance", 27, 11)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inConstraint_implicitObject_staticMethod_instanceMethod`.
#[test]
fn conflicting_static_and_instance_in_constraint_implicit_object_static_method_instance_method() {
    assert_errors_in_code(
        r#"
mixin M {
  static String toString() => 'x';
}
"#,
        &[("conflicting_static_and_instance", 27, 8)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inConstraint_instanceGetter_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_constraint_instance_getter_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
mixin M on A {
  static int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 65, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inConstraint_instanceGetter_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_constraint_instance_getter_static_method() {
    assert_errors_in_code(
        r#"
class A {
  int get foo => 0;
}
mixin M on A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 62, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inConstraint_instanceMethod_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_constraint_instance_method_static_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
mixin M on A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 58, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inConstraint_instanceMethod_staticSetter`.
#[test]
fn conflicting_static_and_instance_in_constraint_instance_method_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}
mixin M on A {
  static set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 57, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inConstraint_instanceSetter_staticGetter`.
#[test]
fn conflicting_static_and_instance_in_constraint_instance_setter_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
mixin M on A {
  static int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 61, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inConstraint_instanceSetter_staticMethod`.
#[test]
fn conflicting_static_and_instance_in_constraint_instance_setter_static_method() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
mixin M on A {
  static void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 58, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inConstraint_instanceSetter_staticSetter`.
#[test]
fn conflicting_static_and_instance_in_constraint_instance_setter_static_setter() {
    assert_errors_in_code(
        r#"
class A {
  set foo(_) {}
}
mixin M on A {
  static set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 57, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticGetter_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_mixin_static_getter_instance_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  static int get foo => 0;
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 28, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticGetter_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_mixin_static_getter_instance_method() {
    assert_errors_in_code(
        r#"
mixin M {
  static int get foo => 0;
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 28, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticGetter_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_mixin_static_getter_instance_setter() {
    assert_errors_in_code(
        r#"
mixin M {
  static int get foo => 0;
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 28, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticMethod_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_mixin_static_method_instance_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  static void foo() {}
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 25, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticMethod_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_mixin_static_method_instance_method() {
    assert_errors_in_code(
        r#"
mixin M {
  static void foo() {}
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 25, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticMethod_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_mixin_static_method_instance_setter() {
    assert_errors_in_code(
        r#"
mixin M {
  static void foo() {}
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 25, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticSetter_instanceGetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_mixin_static_setter_instance_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  static set foo(_) {}
  int get foo => 0;
}
"#,
        &[("conflicting_static_and_instance", 24, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticSetter_instanceMethod`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_mixin_static_setter_instance_method() {
    assert_errors_in_code(
        r#"
mixin M {
  static set foo(_) {}
  void foo() {}
}
"#,
        &[("conflicting_static_and_instance", 24, 3)],
    );
}

/// `conflicting_static_and_instance_test.dart` `test_inMixin_staticSetter_instanceSetter`.
#[test]
#[ignore = "reported by MemberDuplicateDefinitionVerifier (error/*, branch wd-errors)"]
fn conflicting_static_and_instance_in_mixin_static_setter_instance_setter() {
    assert_errors_in_code(
        r#"
mixin M {
  static set foo(_) {}
  set foo(_) {}
}
"#,
        &[("conflicting_static_and_instance", 24, 3)],
    );
}

/// `conflicting_type_variable_and_container_test.dart` `test_conflict_on_class`.
#[test]
fn conflicting_type_variable_and_container_conflict_on_class() {
    assert_errors_in_code(
        r#"
class T<T> {}
"#,
        &[("conflicting_type_variable_and_container", 9, 1)],
    );
}

/// `conflicting_type_variable_and_container_test.dart` `test_conflict`.
#[test]
fn conflicting_type_variable_and_container_conflict() {
    assert_errors_in_code(
        r#"
enum E<E> {
  v
}
"#,
        &[("conflicting_type_variable_and_container", 8, 1)],
    );
}

/// `conflicting_type_variable_and_container_test.dart` `test_conflict_on_mixin`.
#[test]
fn conflicting_type_variable_and_container_conflict_on_mixin() {
    assert_errors_in_code(
        r#"
mixin T<T> {}
"#,
        &[("conflicting_type_variable_and_container", 9, 1)],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_constructor`.
#[test]
fn conflicting_type_variable_and_member_constructor() {
    assert_errors_in_code(
        r#"
class A<T> {
  A.T();
}
"#,
        &[("conflicting_type_variable_and_member", 9, 1)],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_field`.
#[test]
fn conflicting_type_variable_and_member_field() {
    assert_errors_in_code(
        r#"
class A<T> {
  var T;
}
"#,
        &[("conflicting_type_variable_and_member", 9, 1)],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_getter`.
#[test]
fn conflicting_type_variable_and_member_getter() {
    assert_errors_in_code(
        r#"
class A<T> {
  get T => null;
}
"#,
        &[("conflicting_type_variable_and_member", 9, 1)],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_method`.
#[test]
fn conflicting_type_variable_and_member_method() {
    assert_errors_in_code(
        r#"
class A<T> {
  T() {}
}
"#,
        &[("conflicting_type_variable_and_member", 9, 1)],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_method_static`.
#[test]
fn conflicting_type_variable_and_member_method_static() {
    assert_errors_in_code(
        r#"
class A<T> {
  static T() {}
}
"#,
        &[("conflicting_type_variable_and_member", 9, 1)],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_method_wildcard`.
#[test]
fn conflicting_type_variable_and_member_method_wildcard() {
    assert_errors_in_code(
        r#"
class A<_> {
  _() {}
}
"#,
        &[("unused_element", 16, 1)],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_method_wildcard_preWildcards`.
#[test]
fn conflicting_type_variable_and_member_method_wildcard_pre_wildcards() {
    assert_errors_in_code(
        r#"
// @dart = 3.4
// (pre wildcard-variables)

class A<_> {
  _() {}
}
"#,
        &[
            ("conflicting_type_variable_and_member", 53, 1),
            ("unused_element", 60, 1),
        ],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_setter`.
#[test]
fn conflicting_type_variable_and_member_setter() {
    assert_errors_in_code(
        r#"
class A<T> {
  set T(x) {}
}
"#,
        &[("conflicting_type_variable_and_member", 9, 1)],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_constructor_explicit`.
#[test]
fn conflicting_type_variable_and_member_constructor_explicit() {
    assert_errors_in_code(
        r#"
extension type A<T>(int it) {
  A.T(int it) : this(it);
}
"#,
        &[("conflicting_type_variable_and_member", 18, 1)],
    );
}

/// `conflicting_type_variable_and_member_test.dart` `test_constructor_primary`.
#[test]
fn conflicting_type_variable_and_member_constructor_primary() {
    assert_errors_in_code(
        r#"
extension type A<T>.T(int it) {}
"#,
        &[("conflicting_type_variable_and_member", 18, 1)],
    );
}

/// `enum_constant_same_name_as_enclosing_test.dart` `test_name`.
#[test]
fn enum_constant_same_name_as_enclosing_name() {
    assert_errors_in_code(
        r#"
enum E {
  E
}
"#,
        &[("enum_constant_same_name_as_enclosing", 12, 1)],
    );
}

/// `enum_instantiated_to_bounds_is_not_well_bounded_test.dart` `test_enum_it`.
#[test]
#[ignore = "TypeSystem.isWellBounded is not ported"]
fn enum_instantiated_to_bounds_is_not_well_bounded_enum_it() {
    assert_errors_in_code(
        r#"
typedef A<X> = X Function(X);

enum E<T extends A<T>, U> {
  v<Never, int>()
}
"#,
        &[("enum_instantiated_to_bounds_is_not_well_bounded", 37, 1)],
    );
}

/// `enum_with_name_values_test.dart` `test_name`.
#[test]
fn enum_with_name_values_name() {
    assert_errors_in_code(
        r#"
enum values {
  v
}
"#,
        &[("enum_with_name_values", 6, 6)],
    );
}

/// `enum_without_constants_test.dart` `test_hasConstants_inAugmentation`.
#[test]
fn enum_without_constants_has_constants_in_augmentation() {
    assert_errors_in_code(
        r#"
enum E {}
augment enum E {
  v
}
"#,
        &[],
    );
}

/// `enum_without_constants_test.dart` `test_noConstants`.
#[test]
fn enum_without_constants_no_constants() {
    assert_errors_in_code(
        r#"
enum E {}
"#,
        &[("enum_without_constants", 6, 1)],
    );
}

/// `enum_without_constants_test.dart` `test_noConstants_hasAugmentation`.
#[test]
#[ignore = "augmentations experiment: the augmentation checks are not ported"]
fn enum_without_constants_no_constants_has_augmentation() {
    assert_errors_in_code(
        r#"
enum E {}
augment enum E {}
"#,
        &[],
    );
}

/// `export_internal_library_test.dart` `test_export_internal_library`.
#[test]
fn export_internal_library_export_internal_library() {
    assert_errors_in_code(
        r#"
export 'dart:_internal';
"#,
        &[("export_internal_library", 1, 24)],
    );
}

/// `extension_type_implements_itself_test.dart` `test_hasCycle2`.
#[test]
fn extension_type_implements_itself_has_cycle2() {
    assert_errors_in_code(
        r#"
extension type A(int it) implements B {}
extension type B(int it) implements A {}
"#,
        &[
            ("extension_type_implements_itself", 16, 1),
            ("extension_type_implements_itself", 57, 1),
        ],
    );
}

/// `extension_type_implements_itself_test.dart` `test_hasCycle_self`.
#[test]
fn extension_type_implements_itself_has_cycle_self() {
    assert_errors_in_code(
        r#"
extension type A(int it) implements A {}
"#,
        &[("extension_type_implements_itself", 16, 1)],
    );
}

/// `extension_type_representation_depends_on_itself_test.dart` `test_depends_cycle2_direct`.
#[test]
fn extension_type_representation_depends_on_itself_depends_cycle2_direct() {
    assert_errors_in_code(
        r#"
extension type A(B it) {}

extension type B(A it) {}
"#,
        &[
            ("extension_type_representation_depends_on_itself", 16, 1),
            ("extension_type_representation_depends_on_itself", 43, 1),
        ],
    );
}

/// `extension_type_representation_depends_on_itself_test.dart` `test_depends_cycle2_typeArgument`.
#[test]
fn extension_type_representation_depends_on_itself_depends_cycle2_type_argument() {
    assert_errors_in_code(
        r#"
extension type A(List<B> it) {}

extension type B(List<A> it) {}
"#,
        &[
            ("extension_type_representation_depends_on_itself", 16, 1),
            ("extension_type_representation_depends_on_itself", 49, 1),
        ],
    );
}

/// `extension_type_representation_depends_on_itself_test.dart` `test_depends_self_direct`.
#[test]
fn extension_type_representation_depends_on_itself_depends_self_direct() {
    assert_errors_in_code(
        r#"
extension type A(A it) {}
"#,
        &[("extension_type_representation_depends_on_itself", 16, 1)],
    );
}

/// `extension_type_representation_depends_on_itself_test.dart` `test_depends_self_typeArgument`.
#[test]
fn extension_type_representation_depends_on_itself_depends_self_type_argument() {
    assert_errors_in_code(
        r#"
extension type A(List<A> it) {}
"#,
        &[("extension_type_representation_depends_on_itself", 16, 1)],
    );
}

/// `extension_type_representation_type_bottom_test.dart` `test_never`.
#[test]
fn extension_type_representation_type_bottom_never() {
    assert_errors_in_code(
        r#"
extension type A(Never it) {}
"#,
        &[("extension_type_representation_type_bottom", 18, 5)],
    );
}

/// `extension_type_representation_type_bottom_test.dart` `test_neverQuestion`.
#[test]
fn extension_type_representation_type_bottom_never_question() {
    assert_errors_in_code(
        r#"
extension type A(Never? it) {}
"#,
        &[],
    );
}

/// `extension_type_representation_type_bottom_test.dart` `test_typeParameter_never_none`.
#[test]
fn extension_type_representation_type_bottom_type_parameter_never_none() {
    assert_errors_in_code(
        r#"
extension type A<T extends Never>(T it) {}
"#,
        &[("extension_type_representation_type_bottom", 35, 1)],
    );
}

/// `extension_type_representation_type_bottom_test.dart` `test_typeParameter_never_question`.
#[test]
fn extension_type_representation_type_bottom_type_parameter_never_question() {
    assert_errors_in_code(
        r#"
extension type A<T extends Never>(T? it) {}
"#,
        &[],
    );
}

/// `extension_type_representation_type_bottom_test.dart` `test_typeParameter_never_question2`.
#[test]
fn extension_type_representation_type_bottom_type_parameter_never_question2() {
    assert_errors_in_code(
        r#"
extension type A<T extends Never, S extends T>(S? it) {}
"#,
        &[],
    );
}

/// `extension_type_representation_type_bottom_test.dart` `test_typeParameter_never_question3`.
#[test]
fn extension_type_representation_type_bottom_type_parameter_never_question3() {
    assert_errors_in_code(
        r#"
extension type A<T extends Never, S extends T?>(S it) {}
"#,
        &[],
    );
}

/// `illegal_language_version_override_test.dart` `test_hasOverride_equal`.
#[test]
fn illegal_language_version_override_has_override_equal() {
    assert_errors_in_code(
        r#"
// @dart = 2.12
void f() {}
"#,
        &[],
    );
}

/// `illegal_language_version_override_test.dart` `test_hasOverride_greater`.
#[test]
fn illegal_language_version_override_has_override_greater() {
    assert_errors_in_code(
        r#"
// @dart = 2.14
void f() {}
"#,
        &[],
    );
}

/// `illegal_language_version_override_test.dart` `test_hasOverride_less`.
#[test]
#[ignore = "not ported: AnalysisOptions has no sourceLanguageConstraint"]
fn illegal_language_version_override_has_override_less() {
    assert_errors_in_code(
        r#"
// @dart = 2.9
int a = 0;
"#,
        &[("illegal_language_version_override", 1, 14)],
    );
}

/// `illegal_language_version_override_test.dart` `test_hasPackageLanguage_less_hasOverride_greater`.
#[test]
fn illegal_language_version_override_has_package_language_less_has_override_greater() {
    assert_errors_in_code(
        r#"
// @dart = 2.14
void f() {}
"#,
        &[],
    );
}

/// `illegal_language_version_override_test.dart` `test_noOverride`.
#[test]
fn illegal_language_version_override_no_override() {
    assert_errors_in_code(
        r#"
void f() {}
"#,
        &[],
    );
}

/// `implements_repeated_test.dart` `test_class_implements_2times`.
#[test]
fn implements_repeated_class_implements_2times() {
    assert_errors_in_code(
        r#"
class A {}
class B implements A, A {}
"#,
        &[("implements_repeated", 34, 1)],
    );
}

/// `implements_repeated_test.dart` `test_class_implements_2times_augmentation`.
#[test]
fn implements_repeated_class_implements_2times_augmentation() {
    assert_errors_in_code(
        r#"
class A {}
class B implements A {}
augment class B implements A {}
"#,
        &[("implements_repeated", 63, 1)],
    );
}

/// `implements_repeated_test.dart` `test_class_implements_2times_viaTypeAlias`.
#[test]
fn implements_repeated_class_implements_2times_via_type_alias() {
    assert_errors_in_code(
        r#"
class A {}
typedef B = A;
class C implements A, B {}
"#,
        &[("implements_repeated", 49, 1)],
    );
}

/// `implements_repeated_test.dart` `test_class_implements_4times`.
#[test]
fn implements_repeated_class_implements_4times() {
    assert_errors_in_code(
        r#"
class A {} class C{}
class B implements A, A, A, A {}
"#,
        &[
            ("implements_repeated", 44, 1),
            ("implements_repeated", 47, 1),
            ("implements_repeated", 50, 1),
        ],
    );
}

/// `implements_repeated_test.dart` `test_enum_implements_2times`.
#[test]
fn implements_repeated_enum_implements_2times() {
    assert_errors_in_code(
        r#"
class A {}
enum E implements A, A {
  v
}
"#,
        &[("implements_repeated", 33, 1)],
    );
}

/// `implements_repeated_test.dart` `test_enum_implements_2times_augmentation`.
#[test]
fn implements_repeated_enum_implements_2times_augmentation() {
    assert_errors_in_code(
        r#"
class A {}
enum E implements A {v}
augment enum E implements A {}
"#,
        &[("implements_repeated", 62, 1)],
    );
}

/// `implements_repeated_test.dart` `test_enum_implements_2times_viaTypeAlias`.
#[test]
fn implements_repeated_enum_implements_2times_via_type_alias() {
    assert_errors_in_code(
        r#"
class A {}
typedef B = A;
enum E implements A, B {
  v
}
"#,
        &[("implements_repeated", 48, 1)],
    );
}

/// `implements_repeated_test.dart` `test_enum_implements_4times`.
#[test]
fn implements_repeated_enum_implements_4times() {
    assert_errors_in_code(
        r#"
class A {} class C{}
enum E implements A, A, A, A {
  v
}
"#,
        &[
            ("implements_repeated", 43, 1),
            ("implements_repeated", 46, 1),
            ("implements_repeated", 49, 1),
        ],
    );
}

/// `implements_repeated_test.dart` `test_extensionType_implements_2times`.
#[test]
fn implements_repeated_extension_type_implements_2times() {
    assert_errors_in_code(
        r#"
extension type A(int it) implements int, int {}
"#,
        &[("implements_repeated", 42, 3)],
    );
}

/// `implements_repeated_test.dart` `test_extensionType_implements_2times_augmentation`.
#[test]
fn implements_repeated_extension_type_implements_2times_augmentation() {
    assert_errors_in_code(
        r#"
extension type A(int it) implements int {}
augment extension type A implements int {}
"#,
        &[("implements_repeated", 80, 3)],
    );
}

/// `implements_repeated_test.dart` `test_extensionType_implements_2times_viaTypeAlias`.
#[test]
fn implements_repeated_extension_type_implements_2times_via_type_alias() {
    assert_errors_in_code(
        r#"
typedef A = int;
extension type B(int it) implements int, A {}
"#,
        &[("implements_repeated", 59, 1)],
    );
}

/// `implements_repeated_test.dart` `test_extensionType_implements_4times`.
#[test]
fn implements_repeated_extension_type_implements_4times() {
    assert_errors_in_code(
        r#"
extension type A(int it) implements int, int, int, int {}
"#,
        &[
            ("implements_repeated", 42, 3),
            ("implements_repeated", 47, 3),
            ("implements_repeated", 52, 3),
        ],
    );
}

/// `implements_repeated_test.dart` `test_mixin_implements_2times`.
#[test]
fn implements_repeated_mixin_implements_2times() {
    assert_errors_in_code(
        r#"
class A {}
mixin M implements A, A {}
"#,
        &[("implements_repeated", 34, 1)],
    );
}

/// `implements_repeated_test.dart` `test_mixin_implements_2times_augmentation`.
#[test]
fn implements_repeated_mixin_implements_2times_augmentation() {
    assert_errors_in_code(
        r#"
class A {}
mixin M implements A {}
augment mixin M implements A {}
"#,
        &[("implements_repeated", 63, 1)],
    );
}

/// `implements_repeated_test.dart` `test_mixin_implements_4times`.
#[test]
fn implements_repeated_mixin_implements_4times() {
    assert_errors_in_code(
        r#"
class A {}
mixin M implements A, A, A, A {}
"#,
        &[
            ("implements_repeated", 34, 1),
            ("implements_repeated", 37, 1),
            ("implements_repeated", 40, 1),
        ],
    );
}

/// `implements_super_class_constraint_test.dart` `test_it`.
#[test]
fn implements_super_class_constraint_it() {
    assert_errors_in_code(
        r#"
class A {}
mixin M on A implements A {}
"#,
        &[],
    );
}

/// `implements_super_class_constraint_test.dart` `test_it_language305`.
#[test]
fn implements_super_class_constraint_it_language305() {
    assert_errors_in_code(
        r#"
// @dart = 3.5
class A {}
mixin M on A implements A {}
"#,
        &[("implements_super_class_constraint", 51, 1)],
    );
}

/// `implements_super_class_test.dart` `test_class`.
#[test]
fn implements_super_class_class() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A implements A {}
"#,
        &[("implements_super_class", 41, 1)],
    );
}

/// `implements_super_class_test.dart` `test_class_extendsThenAugmentsImplements`.
#[test]
fn implements_super_class_class_extends_then_augments_implements() {
    assert_errors_in_code(
        r#"
class A {}
class B extends A {}
augment class B implements A {}
"#,
        &[("implements_super_class", 60, 1)],
    );
}

/// `implements_super_class_test.dart` `test_class_implementsThenAugmentsExtends`.
#[test]
fn implements_super_class_class_implements_then_augments_extends() {
    assert_errors_in_code(
        r#"
class A {}
class B implements A {}
augment class B extends A {}
"#,
        &[("implements_super_class", 31, 1)],
    );
}

/// `implements_super_class_test.dart` `test_class_Object`.
#[test]
fn implements_super_class_class_object() {
    assert_errors_in_code(
        r#"
class A implements Object {}
"#,
        &[("implements_super_class", 20, 6)],
    );
}

/// `implements_super_class_test.dart` `test_class_viaTypeAlias`.
#[test]
fn implements_super_class_class_via_type_alias() {
    assert_errors_in_code(
        r#"
class A {}
typedef B = A;
class C extends A implements B {}
"#,
        &[("implements_super_class", 56, 1)],
    );
}

/// `implements_super_class_test.dart` `test_classAlias`.
#[test]
fn implements_super_class_class_alias() {
    assert_errors_in_code(
        r#"
class A {}
mixin M {}
class B = A with M implements A;
"#,
        &[("implements_super_class", 53, 1)],
    );
}

/// `implements_super_class_test.dart` `test_classAlias_Object`.
#[test]
fn implements_super_class_class_alias_object() {
    assert_errors_in_code(
        r#"
mixin M {}
class A = Object with M implements Object;
"#,
        &[("implements_super_class", 47, 6)],
    );
}

/// `implements_super_class_test.dart` `test_classAlias_viaTypeAlias`.
#[test]
fn implements_super_class_class_alias_via_type_alias() {
    assert_errors_in_code(
        r#"
class A {}
mixin M {}
typedef B = A;
class C = A with M implements B;
"#,
        &[("implements_super_class", 68, 1)],
    );
}

/// `import_internal_library_test.dart` `test_internal`.
#[test]
fn import_internal_library_internal() {
    assert_errors_in_code(
        r#"
import 'dart:_internal';
"#,
        &[("import_internal_library", 8, 16), ("unused_import", 8, 16)],
    );
}

/// `import_internal_library_test.dart` `test_wasm_fromTest`.
#[test]
fn import_internal_library_wasm_from_test() {
    assert_errors_in_code(
        r#"
import 'dart:_wasm';
"#,
        &[("import_internal_library", 8, 12), ("unused_import", 8, 12)],
    );
}

/// `main_is_not_function_test.dart` `test_class`.
#[test]
fn main_is_not_function_class() {
    assert_errors_in_code(
        r#"
class main {}
"#,
        &[("main_is_not_function", 7, 4)],
    );
}

/// `main_is_not_function_test.dart` `test_classAlias`.
#[test]
fn main_is_not_function_class_alias() {
    assert_errors_in_code(
        r#"
class A {}
mixin M {}
class main = A with M;
"#,
        &[("main_is_not_function", 29, 4)],
    );
}

/// `main_is_not_function_test.dart` `test_enum`.
#[test]
fn main_is_not_function_enum() {
    assert_errors_in_code(
        r#"
enum main {
  v
}
"#,
        &[("main_is_not_function", 6, 4)],
    );
}

/// `main_is_not_function_test.dart` `test_function`.
#[test]
fn main_is_not_function_function() {
    assert_errors_in_code(
        r#"
void main() {}
"#,
        &[],
    );
}

/// `main_is_not_function_test.dart` `test_getter`.
#[test]
fn main_is_not_function_getter() {
    assert_errors_in_code(
        r#"
int get main => 0;
"#,
        &[("main_is_not_function", 9, 4)],
    );
}

/// `main_is_not_function_test.dart` `test_mixin`.
#[test]
fn main_is_not_function_mixin() {
    assert_errors_in_code(
        r#"
class A {}
mixin main on A {}
"#,
        &[("main_is_not_function", 18, 4)],
    );
}

/// `main_is_not_function_test.dart` `test_typedef`.
#[test]
fn main_is_not_function_typedef() {
    assert_errors_in_code(
        r#"
typedef main = void Function();
"#,
        &[("main_is_not_function", 9, 4)],
    );
}

/// `main_is_not_function_test.dart` `test_typedef_legacy`.
#[test]
fn main_is_not_function_typedef_legacy() {
    assert_errors_in_code(
        r#"
typedef void main();
"#,
        &[("main_is_not_function", 14, 4)],
    );
}

/// `main_is_not_function_test.dart` `test_variable`.
#[test]
fn main_is_not_function_variable() {
    assert_errors_in_code(
        r#"
var main = 0;
"#,
        &[("main_is_not_function", 5, 4)],
    );
}

/// `member_with_class_name_test.dart` `test_class_field`.
#[test]
fn member_with_class_name_class_field() {
    assert_errors_in_code(
        r#"
class A {
  int A = 0;
}
"#,
        &[("member_with_class_name", 17, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_class_field_multiple`.
#[test]
fn member_with_class_name_class_field_multiple() {
    assert_errors_in_code(
        r#"
class A {
  int z = 0, A = 0, b = 0;
}
"#,
        &[("member_with_class_name", 24, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_class_getter`.
#[test]
fn member_with_class_name_class_getter() {
    assert_errors_in_code(
        r#"
class A {
  get A => 0;
}
"#,
        &[("member_with_class_name", 17, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_class_getter_static`.
#[test]
fn member_with_class_name_class_getter_static() {
    assert_errors_in_code(
        r#"
class A {
  static int get A => 0;
}
"#,
        &[("member_with_class_name", 28, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_class_setter`.
#[test]
fn member_with_class_name_class_setter() {
    assert_errors_in_code(
        r#"
class A {
  set A(_) {}
}
"#,
        &[("member_with_class_name", 17, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_class_setter_static`.
#[test]
fn member_with_class_name_class_setter_static() {
    assert_errors_in_code(
        r#"
class A {
  static set A(_) {}
}
"#,
        &[("member_with_class_name", 24, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_enum_field`.
#[test]
fn member_with_class_name_enum_field() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  final int E = 0;
}
"#,
        &[("member_with_class_name", 27, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_enum_getter`.
#[test]
fn member_with_class_name_enum_getter() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  int get E => 0;
}
"#,
        &[("member_with_class_name", 25, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_enum_getter_static`.
#[test]
fn member_with_class_name_enum_getter_static() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static int get E => 0;
}
"#,
        &[("member_with_class_name", 32, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_enum_setter`.
#[test]
fn member_with_class_name_enum_setter() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  set E(int _) {}
}
"#,
        &[("member_with_class_name", 21, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_enum_setter_static`.
#[test]
fn member_with_class_name_enum_setter_static() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static set E(int _) {}
}
"#,
        &[("member_with_class_name", 28, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_mixin_getter`.
#[test]
fn member_with_class_name_mixin_getter() {
    assert_errors_in_code(
        r#"
mixin M {
  int get M => 0;
}
"#,
        &[("member_with_class_name", 21, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_mixin_getter_static`.
#[test]
fn member_with_class_name_mixin_getter_static() {
    assert_errors_in_code(
        r#"
mixin M {
  static int get M => 0;
}
"#,
        &[("member_with_class_name", 28, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_mixin_setter`.
#[test]
fn member_with_class_name_mixin_setter() {
    assert_errors_in_code(
        r#"
mixin M {
  void set M(_) {}
}
"#,
        &[("member_with_class_name", 22, 1)],
    );
}

/// `member_with_class_name_test.dart` `test_mixin_setter_static`.
#[test]
fn member_with_class_name_mixin_setter_static() {
    assert_errors_in_code(
        r#"
mixin M {
  static void set M(_) {}
}
"#,
        &[("member_with_class_name", 29, 1)],
    );
}

/// `mixin_application_no_concrete_super_invoked_member_test.dart` `test_class_OK_notInvoked`.
#[test]
fn mixin_application_no_concrete_super_invoked_member_class_o_k_not_invoked() {
    assert_errors_in_code(
        r#"
abstract class A {
  void foo();
}

mixin M on A {}

abstract class X extends A with M {}
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_class_hasRecursion`.
#[test]
fn mixin_application_not_implemented_interface_class_has_recursion() {
    assert_errors_in_code(
        r#"
class A {}
abstract class X with Unresolved, M, CycleWithX {}
mixin M on A {}
mixin CycleWithX on X {}
"#,
        &[
            ("recursive_interface_inheritance", 27, 1),
            ("mixin_of_non_class", 34, 10),
            ("recursive_interface_inheritance", 85, 10),
        ],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_class_matchingInterface`.
#[test]
fn mixin_application_not_implemented_interface_class_matching_interface() {
    assert_errors_in_code(
        r#"
abstract class A<T> {}
class B {}
mixin M<T> on A<T> {}
class C extends A<int> with M {}
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_class_matchingInterface_inPreviousMixin`.
#[test]
fn mixin_application_not_implemented_interface_class_matching_interface_in_previous_mixin() {
    assert_errors_in_code(
        r#"
abstract class A<T> {}
class B {}
mixin M1 implements A<B> {}
mixin M2<T> on A<T> {}
class C extends Object with M1, M2 {}
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_class_noMatchingInterface`.
#[test]
fn mixin_application_not_implemented_interface_class_no_matching_interface() {
    assert_errors_in_code(
        r#"
abstract class A<T> {}
class B {}
mixin M<T> on A<T> {}
class C extends Object with M {}
"#,
        &[("mixin_application_not_implemented_interface", 85, 1)],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_class_noMatchingInterface_fromAugmentation`.
#[test]
fn mixin_application_not_implemented_interface_class_no_matching_interface_from_augmentation() {
    assert_errors_in_code(
        r#"
class B with M {}
mixin M {}
class A {}
augment mixin M on A {}
"#,
        &[("mixin_application_not_implemented_interface", 14, 1)],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_class_noMatchingInterface_withTypeArguments`.
#[test]
fn mixin_application_not_implemented_interface_class_no_matching_interface_with_type_arguments() {
    assert_errors_in_code(
        r#"
abstract class A<T> {}
class B {}
mixin M<T> on A<T> {}
class C extends Object with M<int> {}
"#,
        &[("mixin_application_not_implemented_interface", 85, 1)],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_class_noSuperclassConstraint`.
#[test]
fn mixin_application_not_implemented_interface_class_no_superclass_constraint() {
    assert_errors_in_code(
        r#"
abstract class A<T> {}
class B {}
mixin M<T> {}
class C extends Object with M {}
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_class_recursiveSubtypeCheck`.
#[test]
fn mixin_application_not_implemented_interface_class_recursive_subtype_check() {
    assert_errors_in_code(
        r#"
class ioDirectory implements ioFileSystemEntity {}

class ioFileSystemEntity {}

abstract class _LocalDirectory
    extends _LocalFileSystemEntity<_LocalDirectory, ioDirectory>
    with ForwardingDirectory, DirectoryAddOnsMixin {}

abstract class _LocalFileSystemEntity<T extends FileSystemEntity,
  D extends ioFileSystemEntity> extends ForwardingFileSystemEntity<T, D> {}

abstract class FileSystemEntity implements ioFileSystemEntity {}

abstract class ForwardingFileSystemEntity<T extends FileSystemEntity,
  D extends ioFileSystemEntity> implements FileSystemEntity {}


mixin ForwardingDirectory<T extends Directory>
    on ForwardingFileSystemEntity<T, ioDirectory>
    implements Directory {}

abstract class Directory implements FileSystemEntity, ioDirectory {}

mixin DirectoryAddOnsMixin implements Directory {}
"#,
        &[
            ("conflicting_generic_interfaces", 97, 15),
            ("unused_element", 97, 15),
        ],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_classTypeAlias_generic`.
#[test]
fn mixin_application_not_implemented_interface_class_type_alias_generic() {
    assert_errors_in_code(
        r#"
class A<T> {}

mixin M on A<int> {}

class X = A<double> with M;
"#,
        &[("mixin_application_not_implemented_interface", 63, 1)],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_classTypeAlias_noMatchingInterface`.
#[test]
fn mixin_application_not_implemented_interface_class_type_alias_no_matching_interface() {
    assert_errors_in_code(
        r#"
abstract class A<T> {}
class B {}
mixin M<T> on A<T> {}
class C = Object with M;
"#,
        &[("mixin_application_not_implemented_interface", 79, 1)],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_classTypeAlias_notGeneric`.
#[test]
fn mixin_application_not_implemented_interface_class_type_alias_not_generic() {
    assert_errors_in_code(
        r#"
class A {}

mixin M on A {}

class X = Object with M;
"#,
        &[("mixin_application_not_implemented_interface", 52, 1)],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_classTypeAlias_OK_0`.
#[test]
fn mixin_application_not_implemented_interface_class_type_alias_o_k_0() {
    assert_errors_in_code(
        r#"
mixin M {}

class X = Object with M;
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_classTypeAlias_OK_1`.
#[test]
fn mixin_application_not_implemented_interface_class_type_alias_o_k_1() {
    assert_errors_in_code(
        r#"
class A {}

mixin M on A {}

class X = A with M;
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_classTypeAlias_OK_generic`.
#[test]
fn mixin_application_not_implemented_interface_class_type_alias_o_k_generic() {
    assert_errors_in_code(
        r#"
class A<T> {}

mixin M<T> on A<T> {}

class B<T> implements A<T> {}

class C<T> = B<T> with M<T>;
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_classTypeAlias_OK_previousMixin`.
#[test]
fn mixin_application_not_implemented_interface_class_type_alias_o_k_previous_mixin() {
    assert_errors_in_code(
        r#"
class A {}

mixin M1 implements A {}

mixin M2 on A {}

class X = Object with M1, M2;
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_classTypeAlias_oneOfTwo`.
#[test]
fn mixin_application_not_implemented_interface_class_type_alias_one_of_two() {
    assert_errors_in_code(
        r#"
class A {}
class B {}
class C {}

mixin M on A, B {}

class X = C with M;
"#,
        &[("mixin_application_not_implemented_interface", 72, 1)],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_enum_matchingInterface_inPreviousMixin`.
#[test]
fn mixin_application_not_implemented_interface_enum_matching_interface_in_previous_mixin() {
    assert_errors_in_code(
        r#"
abstract class A {}

mixin M1 implements A {}

mixin M2 on A {}

enum E with M1, M2 {
  v
}
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_enum_noMatchingInterface`.
#[test]
fn mixin_application_not_implemented_interface_enum_no_matching_interface() {
    assert_errors_in_code(
        r#"
abstract class A {}

mixin M on A {}

enum E with M {
  v
}
"#,
        &[("mixin_application_not_implemented_interface", 51, 1)],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_enum_noSuperclassConstraint`.
#[test]
fn mixin_application_not_implemented_interface_enum_no_superclass_constraint() {
    assert_errors_in_code(
        r#"
mixin M {}

enum E with M {
  v;
}
"#,
        &[],
    );
}

/// `mixin_application_not_implemented_interface_test.dart` `test_enum_noSuperclassConstraint_augmented`.
#[test]
fn mixin_application_not_implemented_interface_enum_no_superclass_constraint_augmented() {
    assert_errors_in_code(
        r#"
mixin M {}
enum E {v}
augment enum E with M {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_factory_redirect`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_factory_redirect()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  A.named();
  factory A.x() = A.named;
}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_generative_redirect`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_generative_redirect()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  A() : this.named();
  A.named();
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            19,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_newHead_nonTrivial_blockBody`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_new_head_non_trivial_block_body()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  new() {}
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            19,
            3,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_newHead_nonTrivial_external`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_new_head_non_trivial_external()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  external new();
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            28,
            3,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_newHead_nonTrivial_hasFormalParameter`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_new_head_non_trivial_has_formal_parameter()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  new(int foo);
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            19,
            3,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_newHead_nonTrivial_super`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_new_head_non_trivial_super()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  new(): super();
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            19,
            3,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_newHead_trivial`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_new_head_trivial()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  new();
}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_newHead_trivial_const`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_new_head_trivial_const()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  const new();
}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_trivial_named`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_trivial_named() {
    assert_errors_in_code(
        r#"
mixin class A {
  A.named();
}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_trivial_named_const`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_trivial_named_const()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  const A.named();
}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_typeName_nonTrivial_blockBody`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_type_name_non_trivial_block_body()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  A() {}
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            19,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_typeName_nonTrivial_blockBody_named`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_type_name_non_trivial_block_body_named()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  A.named() {}
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            19,
            7,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_typeName_nonTrivial_external`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_type_name_non_trivial_external()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  external A();
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            28,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_typeName_nonTrivial_hasFormalParameter`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_type_name_non_trivial_has_formal_parameter()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  A(int foo);
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            19,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_typeName_nonTrivial_super`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_type_name_non_trivial_super()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  A(): super();
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            19,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_typeName_trivial`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_type_name_trivial()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  A();
}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_constructor_typeName_trivial_const`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_constructor_type_name_trivial_const()
 {
    assert_errors_in_code(
        r#"
mixin class A {
  const A();
}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_named_nonTrivial_hasBody_block`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_named_non_trivial_has_body_block()
 {
    assert_errors_in_code(
        r#"
mixin class A.named() {
  this {}
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            32,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_named_nonTrivial_hasFormalParameter`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_named_non_trivial_has_formal_parameter()
 {
    assert_errors_in_code(
        r#"
mixin class A.named(int x) {}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            13,
            7,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_named_nonTrivial_hasInitializer`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_named_non_trivial_has_initializer()
 {
    assert_errors_in_code(
        r#"
mixin class A.named() {
  this : assert(true);
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            32,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_named_trivial`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_named_trivial()
 {
    assert_errors_in_code(
        r#"
mixin class A.named() {}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_named_trivial_hasBody_empty`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_named_trivial_has_body_empty()
 {
    assert_errors_in_code(
        r#"
mixin class A.named() {
  this;
}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_unnamed_nonTrivial_hasBody_block`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_unnamed_non_trivial_has_body_block()
 {
    assert_errors_in_code(
        r#"
mixin class A() {
  this {}
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            26,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_unnamed_nonTrivial_hasBody_block_hasInitializer`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_unnamed_non_trivial_has_body_block_has_initializer()
 {
    assert_errors_in_code(
        r#"
mixin class A() {
  this : assert(true) {}
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            26,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_unnamed_nonTrivial_hasFormalParameter`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_unnamed_non_trivial_has_formal_parameter()
 {
    assert_errors_in_code(
        r#"
mixin class A(int x) {}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            13,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_unnamed_nonTrivial_hasInitializer`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_unnamed_non_trivial_has_initializer()
 {
    assert_errors_in_code(
        r#"
mixin class A() {
  this : assert(true);
}
class B with A {}
"#,
        &[(
            "mixin_class_declares_non_trivial_generative_constructor",
            26,
            1,
        )],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_unnamed_trivial`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_unnamed_trivial()
 {
    assert_errors_in_code(
        r#"
mixin class A() {}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_class_declares_non_trivial_generative_constructor_test.dart` `test_mixinClass_primaryConstructor_unnamed_trivial_hasBody_empty`.
#[test]
fn mixin_class_declares_non_trivial_generative_constructor_mixin_class_primary_constructor_unnamed_trivial_has_body_empty()
 {
    assert_errors_in_code(
        r#"
mixin class A() {
  this;
}
class B with A {}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_class_extends`.
#[test]
fn mixin_inherits_from_not_object_class_class_extends() {
    assert_errors_in_code(
        r#"
class A {}
mixin class B extends A {}
class C extends Object with B {}
"#,
        &[
            ("mixin_class_declaration_extends_not_object", 34, 1),
            ("mixin_inherits_from_not_object", 67, 1),
        ],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_class_extends_language219`.
#[test]
fn mixin_inherits_from_not_object_class_class_extends_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B extends A {}
class C extends Object with B {}
"#,
        &[("mixin_inherits_from_not_object", 75, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_class_extends_Object`.
#[test]
fn mixin_inherits_from_not_object_class_class_extends_object() {
    assert_errors_in_code(
        r#"
class A {}
mixin class B extends Object {}
class C extends Object with B {}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_class_extends_Object_language219`.
#[test]
fn mixin_inherits_from_not_object_class_class_extends_object_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B extends Object {}
class C extends Object with B {}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_class_with`.
#[test]
fn mixin_inherits_from_not_object_class_class_with() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B extends Object with A {}
class C extends Object with B {}
"#,
        &[
            ("mixin_class_declaration_with_clause", 47, 6),
            ("mixin_inherits_from_not_object", 85, 1),
        ],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_class_with_language219`.
#[test]
fn mixin_inherits_from_not_object_class_class_with_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B extends Object with A {}
class C extends Object with B {}
"#,
        &[("mixin_inherits_from_not_object", 87, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_classTypeAlias_with`.
#[test]
fn mixin_inherits_from_not_object_class_class_type_alias_with() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B = Object with A;
class C extends Object with B {}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_classTypeAlias_with2`.
#[test]
fn mixin_inherits_from_not_object_class_class_type_alias_with2() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B {}
mixin class C = Object with A, B;
class D extends Object with C {}
"#,
        &[
            (
                "mixin_modifier_mixin_application_class_with_multiple_mixins",
                58,
                9,
            ),
            ("mixin_inherits_from_not_object", 97, 1),
        ],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_classTypeAlias_with2_language219`.
#[test]
fn mixin_inherits_from_not_object_class_class_type_alias_with2_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B {}
class C = Object with A, B;
class D extends Object with C {}
"#,
        &[("mixin_inherits_from_not_object", 93, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_classTypeAlias_with_language219`.
#[test]
fn mixin_inherits_from_not_object_class_class_type_alias_with_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B = Object with A;
class C extends Object with B {}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_class_mixin`.
#[test]
fn mixin_inherits_from_not_object_class_mixin() {
    assert_errors_in_code(
        r#"
class A {}
mixin B on A {}
class C extends A with B {}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_classTypeAlias_class_extends`.
#[test]
fn mixin_inherits_from_not_object_class_type_alias_class_extends() {
    assert_errors_in_code(
        r#"
class A {}
mixin class B extends A {}
class C = Object with B;
"#,
        &[
            ("mixin_class_declaration_extends_not_object", 34, 1),
            ("mixin_inherits_from_not_object", 61, 1),
        ],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_classTypeAlias_class_extends_language219`.
#[test]
fn mixin_inherits_from_not_object_class_type_alias_class_extends_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B extends A {}
class C = Object with B;
"#,
        &[("mixin_inherits_from_not_object", 69, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_classTypeAlias_class_with`.
#[test]
fn mixin_inherits_from_not_object_class_type_alias_class_with() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B extends Object with A {}
class C = Object with B;
"#,
        &[
            ("mixin_class_declaration_with_clause", 47, 6),
            ("mixin_inherits_from_not_object", 79, 1),
        ],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_classTypeAlias_class_with_language219`.
#[test]
fn mixin_inherits_from_not_object_class_type_alias_class_with_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B extends Object with A {}
class C = Object with B;
"#,
        &[("mixin_inherits_from_not_object", 81, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_classTypeAlias_classAlias_with`.
#[test]
fn mixin_inherits_from_not_object_class_type_alias_class_alias_with() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B = Object with A;
class C = Object with B;
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_classTypeAlias_classAlias_with2`.
#[test]
fn mixin_inherits_from_not_object_class_type_alias_class_alias_with2() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B {}
mixin class C = Object with A, B;
class D = Object with C;
"#,
        &[
            (
                "mixin_modifier_mixin_application_class_with_multiple_mixins",
                58,
                9,
            ),
            ("mixin_inherits_from_not_object", 91, 1),
        ],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_classTypeAlias_classAlias_with2_language219`.
#[test]
fn mixin_inherits_from_not_object_class_type_alias_class_alias_with2_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B {}
class C = Object with A, B;
class D = Object with C;
"#,
        &[("mixin_inherits_from_not_object", 87, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_classTypeAlias_classAlias_with_language219`.
#[test]
fn mixin_inherits_from_not_object_class_type_alias_class_alias_with_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B = Object with A;
class C = Object with B;
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_classTypeAlias_mixin`.
#[test]
fn mixin_inherits_from_not_object_class_type_alias_mixin() {
    assert_errors_in_code(
        r#"
class A {}
mixin B on A {}
class C = A with B;
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_class_extends`.
#[test]
fn mixin_inherits_from_not_object_enum_class_extends() {
    assert_errors_in_code(
        r#"
class A {}
mixin class B extends A {}
enum E with B {
  v
}
"#,
        &[
            ("mixin_class_declaration_extends_not_object", 34, 1),
            ("mixin_inherits_from_not_object", 51, 1),
        ],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_class_extends_language219`.
#[test]
fn mixin_inherits_from_not_object_enum_class_extends_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B extends A {}
enum E with B {
  v
}
"#,
        &[("mixin_inherits_from_not_object", 59, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_class_extends_Object`.
#[test]
fn mixin_inherits_from_not_object_enum_class_extends_object() {
    assert_errors_in_code(
        r#"
class A {}
mixin class B extends Object {}
enum E with B {
  v
}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_class_extends_Object_language219`.
#[test]
fn mixin_inherits_from_not_object_enum_class_extends_object_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B extends Object {}
enum E with B {
  v
}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_class_with`.
#[test]
fn mixin_inherits_from_not_object_enum_class_with() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B extends Object with A {}
enum E with B {
  v
}
"#,
        &[
            ("mixin_class_declaration_with_clause", 47, 6),
            ("mixin_inherits_from_not_object", 69, 1),
        ],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_class_with_language219`.
#[test]
fn mixin_inherits_from_not_object_enum_class_with_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B extends Object with A {}
enum E with B {
  v
}
"#,
        &[("mixin_inherits_from_not_object", 71, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_classTypeAlias_with`.
#[test]
fn mixin_inherits_from_not_object_enum_class_type_alias_with() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B = Object with A;
enum E with B {
  v
}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_classTypeAlias_with2`.
#[test]
fn mixin_inherits_from_not_object_enum_class_type_alias_with2() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B {}
class C = Object with A, B;
enum E with C {
  v
}
"#,
        &[("class_used_as_mixin", 75, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_classTypeAlias_with2_language219`.
#[test]
fn mixin_inherits_from_not_object_enum_class_type_alias_with2_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B {}
class C = Object with A, B;
enum E with C {
  v
}
"#,
        &[("mixin_inherits_from_not_object", 77, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_enum_classTypeAlias_with_language219`.
#[test]
fn mixin_inherits_from_not_object_enum_class_type_alias_with_language219() {
    assert_errors_in_code(
        r#"
// @dart=2.19
class A {}
class B = Object with A;
enum E with B {
  v
}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_mixinClass_class_extends`.
#[test]
fn mixin_inherits_from_not_object_mixin_class_class_extends() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B extends A {}
"#,
        &[("mixin_class_declaration_extends_not_object", 40, 1)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_mixinClass_class_extends_Object`.
#[test]
fn mixin_inherits_from_not_object_mixin_class_class_extends_object() {
    assert_errors_in_code(
        r#"
mixin class A extends Object {}
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_mixinClass_class_extends_Object_with`.
#[test]
fn mixin_inherits_from_not_object_mixin_class_class_extends_object_with() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B extends Object with A {}
"#,
        &[("mixin_class_declaration_with_clause", 47, 6)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_mixinClass_class_with`.
#[test]
fn mixin_inherits_from_not_object_mixin_class_class_with() {
    assert_errors_in_code(
        r#"
mixin M {}
mixin class A with M {}
"#,
        &[("mixin_class_declaration_with_clause", 26, 6)],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_mixinClass_classTypeAlias_with`.
#[test]
fn mixin_inherits_from_not_object_mixin_class_class_type_alias_with() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B = Object with A;
"#,
        &[],
    );
}

/// `mixin_inherits_from_not_object_test.dart` `test_mixinClass_classTypeAlias_with2`.
#[test]
fn mixin_inherits_from_not_object_mixin_class_class_type_alias_with2() {
    assert_errors_in_code(
        r#"
mixin class A {}
mixin class B {}
mixin class C = Object with A, B;
"#,
        &[(
            "mixin_modifier_mixin_application_class_with_multiple_mixins",
            58,
            9,
        )],
    );
}

/// `mixin_super_class_constraint_deferred_class_test.dart` `test_error_onClause_deferredClass`.
#[test]
fn mixin_super_class_constraint_deferred_class_error_on_clause_deferred_class() {
    assert_errors_in_code(
        r#"
import 'dart:math' deferred as math;
mixin M on math.Random {}
"#,
        &[("mixin_super_class_constraint_deferred_class", 49, 11)],
    );
}

/// `native_clause_in_non_sdk_code_test.dart` `test_nativeClauseInNonSDKCode`.
#[test]
fn native_clause_in_non_sdk_code_native_clause_in_non_s_d_k_code() {
    assert_errors_in_code(
        r#"
class A native 'string' {}
"#,
        &[("native_clause_in_non_sdk_code", 9, 15)],
    );
}

/// `no_generative_constructors_in_superclass_test.dart` `test_explicit`.
#[test]
fn no_generative_constructors_in_superclass_explicit() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw '';
}
class B extends A {
  B() : super();
}
"#,
        &[("no_generative_constructors_in_superclass", 56, 1)],
    );
}

/// `no_generative_constructors_in_superclass_test.dart` `test_explicit_oneFactory`.
#[test]
fn no_generative_constructors_in_superclass_explicit_one_factory() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw '';
}
class B extends A {
  B() : super();
  factory B.second() => throw '';
}
"#,
        &[("no_generative_constructors_in_superclass", 56, 1)],
    );
}

/// `no_generative_constructors_in_superclass_test.dart` `test_hasFactories`.
#[test]
fn no_generative_constructors_in_superclass_has_factories() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw '';
}
class B extends A {
  factory B() => throw '';
  factory B.second() => throw '';
}
"#,
        &[],
    );
}

/// `no_generative_constructors_in_superclass_test.dart` `test_hasFactory`.
#[test]
fn no_generative_constructors_in_superclass_has_factory() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw '';
}
class B extends A {
  factory B() => throw '';
}
"#,
        &[],
    );
}

/// `no_generative_constructors_in_superclass_test.dart` `test_implicit`.
#[test]
fn no_generative_constructors_in_superclass_implicit() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw '';
}
class B extends A {
  B();
}
"#,
        &[("no_generative_constructors_in_superclass", 56, 1)],
    );
}

/// `no_generative_constructors_in_superclass_test.dart` `test_implicit2`.
#[test]
fn no_generative_constructors_in_superclass_implicit2() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw '';
}
class B extends A {
}
"#,
        &[("no_generative_constructors_in_superclass", 56, 1)],
    );
}

/// `non_covariant_type_parameter_position_in_representation_type_test.dart` `test_contravariant`.
#[test]
fn non_covariant_type_parameter_position_in_representation_type_contravariant() {
    assert_errors_in_code(
        r#"
extension type A<T>(void Function(T) it) {}
"#,
        &[(
            "non_covariant_type_parameter_position_in_representation_type",
            18,
            1,
        )],
    );
}

/// `non_covariant_type_parameter_position_in_representation_type_test.dart` `test_covariant`.
#[test]
fn non_covariant_type_parameter_position_in_representation_type_covariant() {
    assert_errors_in_code(
        r#"
extension type A<T>(T Function() it) {}
"#,
        &[],
    );
}

/// `non_covariant_type_parameter_position_in_representation_type_test.dart` `test_invariant`.
#[test]
fn non_covariant_type_parameter_position_in_representation_type_invariant() {
    assert_errors_in_code(
        r#"
extension type A<T>(T Function(T) it) {}
"#,
        &[(
            "non_covariant_type_parameter_position_in_representation_type",
            18,
            1,
        )],
    );
}

/// `non_generative_implicit_constructor_test.dart` `test_implicit`.
#[test]
fn non_generative_implicit_constructor_implicit() {
    assert_errors_in_code(
        r#"
class A {
  factory A() => throw 0;
  A.named();
}
class B extends A {
}
"#,
        &[("non_generative_implicit_constructor", 58, 1)],
    );
}

/// `on_repeated_test.dart` `test_2times`.
#[test]
fn on_repeated_2times() {
    assert_errors_in_code(
        r#"
class A {}
mixin M on A, A {}
"#,
        &[("on_repeated", 26, 1)],
    );
}

/// `on_repeated_test.dart` `test_2times_augmentation`.
#[test]
fn on_repeated_2times_augmentation() {
    assert_errors_in_code(
        r#"
class A {}
mixin M on A {}
augment mixin M on A {}
"#,
        &[("on_repeated", 47, 1)],
    );
}

/// `on_repeated_test.dart` `test_2times_viaTypeAlias`.
#[test]
fn on_repeated_2times_via_type_alias() {
    assert_errors_in_code(
        r#"
class A {}
typedef B = A;
mixin M on A, B {}
"#,
        &[("on_repeated", 41, 1)],
    );
}

/// `private_collision_in_mixin_application_test.dart` `test_class_superclassAndMixin_sameLibrary`.
#[test]
fn private_collision_in_mixin_application_class_superclass_and_mixin_same_library() {
    assert_errors_in_code(
        r#"
mixin A {
  void _foo() {}
}

mixin B {
  void _foo() {}
}

class C extends Object with A, B {}
"#,
        &[("unused_element", 18, 4), ("unused_element", 48, 4)],
    );
}

/// `private_collision_in_mixin_application_test.dart` `test_class_superclassAndMixin_sameLibrary_mixinClass`.
#[test]
fn private_collision_in_mixin_application_class_superclass_and_mixin_same_library_mixin_class() {
    assert_errors_in_code(
        r#"
mixin class A {
  void _foo() {}
}

mixin class B {
  void _foo() {}
}

class C extends Object with A, B {}
"#,
        &[("unused_element", 24, 4), ("unused_element", 60, 4)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_functionTypeAlias_typeParameterBounds`.
#[test]
fn type_alias_cannot_reference_itself_function_type_alias_type_parameter_bounds() {
    assert_errors_in_code(
        r#"
typedef A<T extends A<int>>();
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_functionTypedParameter_returnType`.
#[test]
fn type_alias_cannot_reference_itself_function_typed_parameter_return_type() {
    assert_errors_in_code(
        r#"
typedef A(A b());
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_generic`.
#[test]
fn type_alias_cannot_reference_itself_generic() {
    assert_errors_in_code(
        r#"
typedef F = void Function(List<G> l);
typedef G = void Function(List<F> l);
main() {
  F? foo(G? g) => g;
  foo(null);
}
"#,
        &[
            ("type_alias_cannot_reference_itself", 9, 1),
            ("type_alias_cannot_reference_itself", 47, 1),
        ],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_genericTypeAlias_typeParameterBounds`.
#[test]
fn type_alias_cannot_reference_itself_generic_type_alias_type_parameter_bounds() {
    assert_errors_in_code(
        r#"
typedef A<T extends A<int>> = void Function();
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_infiniteParameterBoundCycle`.
#[test]
fn type_alias_cannot_reference_itself_infinite_parameter_bound_cycle() {
    assert_errors_in_code(
        r#"
typedef F<X extends F<X>> = F Function();
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_issue11987`.
#[test]
fn type_alias_cannot_reference_itself_issue11987() {
    assert_errors_in_code(
        r#"
typedef void F(List<G> l);
typedef void G(List<F> l);
main() {
  F? foo(G? g) => g;
  foo(null);
}
"#,
        &[
            ("type_alias_cannot_reference_itself", 14, 1),
            ("type_alias_cannot_reference_itself", 41, 1),
        ],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_issue19459`.
#[test]
fn type_alias_cannot_reference_itself_issue19459() {
    assert_errors_in_code(
        r#"
class A<B, C> {}
abstract class D {
  f(E e);
}
abstract class E extends A<dynamic, F> {}
typedef D F();
"#,
        &[],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_nonFunction_aliasedType_cycleOf2`.
#[test]
fn type_alias_cannot_reference_itself_non_function_aliased_type_cycle_of2() {
    assert_errors_in_code(
        r#"
typedef T1 = T2;
typedef T2 = T1;
"#,
        &[
            ("type_alias_cannot_reference_itself", 9, 2),
            ("type_alias_cannot_reference_itself", 26, 2),
        ],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_nonFunction_aliasedType_directly_functionWithIt`.
#[test]
fn type_alias_cannot_reference_itself_non_function_aliased_type_directly_function_with_it() {
    assert_errors_in_code(
        r#"
typedef T = void Function(T);
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_nonFunction_aliasedType_directly_it_none`.
#[test]
fn type_alias_cannot_reference_itself_non_function_aliased_type_directly_it_none() {
    assert_errors_in_code(
        r#"
typedef T = T;
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_nonFunction_aliasedType_directly_it_question`.
#[test]
fn type_alias_cannot_reference_itself_non_function_aliased_type_directly_it_question() {
    assert_errors_in_code(
        r#"
typedef T = T?;
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_nonFunction_aliasedType_directly_ListOfIt`.
#[test]
fn type_alias_cannot_reference_itself_non_function_aliased_type_directly_list_of_it() {
    assert_errors_in_code(
        r#"
typedef T = List<T>;
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_nonFunction_typeParameterBounds`.
#[test]
fn type_alias_cannot_reference_itself_non_function_type_parameter_bounds() {
    assert_errors_in_code(
        r#"
typedef T<X extends T<Never>> = List<X>;
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_parameterType_named`.
#[test]
fn type_alias_cannot_reference_itself_parameter_type_named() {
    assert_errors_in_code(
        r#"
typedef A({A a});
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_parameterType_positional`.
#[test]
fn type_alias_cannot_reference_itself_parameter_type_positional() {
    assert_errors_in_code(
        r#"
typedef A([A a]);
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_parameterType_required`.
#[test]
fn type_alias_cannot_reference_itself_parameter_type_required() {
    assert_errors_in_code(
        r#"
typedef A(A a);
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_parameterType_typeArgument`.
#[test]
fn type_alias_cannot_reference_itself_parameter_type_type_argument() {
    assert_errors_in_code(
        r#"
typedef A(List<A> a);
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_referencesReturnType_inTypeAlias`.
#[test]
fn type_alias_cannot_reference_itself_references_return_type_in_type_alias() {
    assert_errors_in_code(
        r#"
typedef B A();
class B {
  A? a;
}
"#,
        &[],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_returnClass_withTypeAlias`.
#[test]
fn type_alias_cannot_reference_itself_return_class_with_type_alias() {
    assert_errors_in_code(
        r#"
typedef C A();
typedef A B();
class C {
  B? a;
}
"#,
        &[],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_returnType`.
#[test]
fn type_alias_cannot_reference_itself_return_type() {
    assert_errors_in_code(
        r#"
typedef A A();
"#,
        &[("type_alias_cannot_reference_itself", 11, 1)],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_returnType_indirect`.
#[test]
fn type_alias_cannot_reference_itself_return_type_indirect() {
    assert_errors_in_code(
        r#"
typedef B A();
typedef A B();
"#,
        &[
            ("type_alias_cannot_reference_itself", 11, 1),
            ("type_alias_cannot_reference_itself", 26, 1),
        ],
    );
}

/// `type_alias_cannot_reference_itself_test.dart` `test_usingRecordType_directly`.
#[test]
fn type_alias_cannot_reference_itself_using_record_type_directly() {
    assert_errors_in_code(
        r#"
typedef F = (F, int) Function();
"#,
        &[("type_alias_cannot_reference_itself", 9, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_extends_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_extends_function_parameter_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
class A<X> {}
class B<X> extends A<F<X>> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 56, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_extends_function_parameterType_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_extends_function_parameter_type_parameter_type()
 {
    assert_errors_in_code(
        r#"
typedef F1<X> = void Function(X);
typedef F2<X> = void Function(F1<X>);
class A<X> {}
class B<X> extends A<F2<X>> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_extends_function_parameterType_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_extends_function_parameter_type_return_type()
 {
    assert_errors_in_code(
        r#"
typedef F1<X> = X Function();
typedef F2<X> = void Function(F1<X>);
class A<X> {}
class B<X> extends A<F2<X>> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 91, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_extends_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_extends_function_return_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
class A<X> {}
class B<X> extends A<F<X>> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_extends_function_returnType_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_extends_function_return_type_parameter_type()
 {
    assert_errors_in_code(
        r#"
typedef F1<X> = void Function(X);
typedef F2<X> = F1<X> Function();
class A<X> {}
class B<X> extends A<F2<X>> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 91, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_extends_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_extends_without_function() {
    assert_errors_in_code(
        r#"
class A<X> {}
class B<X> extends A<X> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_implements_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_implements_function_parameter_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
class A<X> {}
class B<X> implements A<F<X>> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 56, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_implements_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_implements_function_return_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
class A<X> {}
class B<X> implements A<F<X>> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_implements_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_implements_without_function() {
    assert_errors_in_code(
        r#"
class A<X> {}
class B<X> implements A<X> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_with_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_with_function_parameter_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
mixin A<X> {}
class B<X> extends Object with A<F<X>> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 56, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_with_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_with_function_return_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
mixin A<X> {}
class B<X> extends Object with A<F<X>> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_class_with_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_with_without_function() {
    assert_errors_in_code(
        r#"
mixin A<X> {}
class B<X> extends Object with A<X> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_extends_function_invariant`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_extends_function_invariant() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function(X);
class A<X> {}
mixin M {}
class B<X> = A<F<X>> with M;
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 64, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_extends_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_extends_function_parameter_type()
 {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
class A<X> {}
mixin M {}
class B<X> = A<F<X>> with M;
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 67, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_extends_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_extends_function_return_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
class A<X> {}
mixin M {}
class B<X> = A<F<X>> with M;
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_extends_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_extends_without_function() {
    assert_errors_in_code(
        r#"
class A<X> {}
mixin M {}
class B<X> = A<X> with M;
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_implements_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_implements_function_parameter_type()
 {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
class A<X> {}
mixin M {}
class B<X> = Object with M implements A<F<X>>;
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 67, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_implements_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_implements_function_return_type()
 {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
class A<X> {}
mixin M {}
class B<X> = Object with M implements A<F<X>>;
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_implements_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_implements_without_function() {
    assert_errors_in_code(
        r#"
class A<X> {}
mixin M {}
class B<X> = Object with M implements A<X>;
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_with_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_with_function_parameter_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
mixin M<X> {}
class B<X> = Object with M<F<X>>;
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 56, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_with_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_with_function_return_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
mixin M<X> {}
class B<X> = Object with M<F<X>>;
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_classTypeAlias_with_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_class_type_alias_with_without_function() {
    assert_errors_in_code(
        r#"
mixin M<X> {}
class B<X> = Object with M<X>;
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_enum_implements_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_enum_implements_function_parameter_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
class A<X> {}
enum E<X> implements A<F<X>> {
  v
}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 55, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_enum_implements_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_enum_implements_function_return_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
class A<X> {}
enum E<X> implements A<F<X>> {
  v
}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_enum_implements_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_enum_implements_without_function() {
    assert_errors_in_code(
        r#"
class A<X> {}
enum E<X> implements A<X> {
  v
}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_enum_with_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_enum_with_function_parameter_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
mixin A<X> {}
enum E<X> with A<F<X>> {
  v
}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 55, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_enum_with_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_enum_with_function_return_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
mixin A<X> {}
enum E<X> with A<F<X>> {
  v
}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_enum_with_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_enum_with_without_function() {
    assert_errors_in_code(
        r#"
mixin A<X> {}
enum E<X> with A<X> {
  v
}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_extensionType_contravariant`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_extension_type_contravariant() {
    assert_errors_in_code(
        r#"
class A<T> {}
extension type B<T>(A<void Function(Object?)> it)
  implements A<void Function(T)> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 32, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_extensionType_covariant`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_extension_type_covariant() {
    assert_errors_in_code(
        r#"
class A<T> {}
extension type B<T>(A<Never Function()> it)
  implements A<T Function()> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_extensionType_invariant`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_extension_type_invariant() {
    assert_errors_in_code(
        r#"
class A<T> {}
extension type B<T>(A<Never Function(Object?)> it)
  implements A<T Function(T)> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 32, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_mixin_implements_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_mixin_implements_function_parameter_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
class A<X> {}
mixin B<X> implements A<F<X>> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 56, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_mixin_implements_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_mixin_implements_function_return_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
class A<X> {}
mixin B<X> implements A<F<X>> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_mixin_implements_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_mixin_implements_without_function() {
    assert_errors_in_code(
        r#"
class A<X> {}
mixin B<X> implements A<X> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_mixin_on_function_parameterType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_mixin_on_function_parameter_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = void Function(X);
class A<X> {}
mixin B<X> on A<F<X>> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 56, 1)],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_mixin_on_function_returnType`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_mixin_on_function_return_type() {
    assert_errors_in_code(
        r#"
typedef F<X> = X Function();
class A<X> {}
mixin B<X> on A<F<X>> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_mixin_on_withoutFunction`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_mixin_on_without_function() {
    assert_errors_in_code(
        r#"
class A<X> {}
mixin B<X> on A<X> {}
"#,
        &[],
    );
}

/// `wrong_type_parameter_variance_in_superinterface_test.dart` `test_typeParameter_bound`.
#[test]
fn wrong_type_parameter_variance_in_superinterface_type_parameter_bound() {
    assert_errors_in_code(
        r#"
class A<X> {}
class B<X> extends A<void Function<Y extends X>()> {}
"#,
        &[("wrong_type_parameter_variance_in_superinterface", 23, 1)],
    );
}
