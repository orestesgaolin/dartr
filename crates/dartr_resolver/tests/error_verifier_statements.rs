//! ErrorVerifier diagnostic tests of section D5, generated from
//! `pkg/analyzer/test/src/diagnostics/<code>_test.dart` (the tests with one
//! library and no other setup) by converting the inline diagnostic
//! markers or the `error(diag.x, offset, length)` lists. Only the codes
//! of `tools/difftest/error_verifier_codes.txt` are compared.

mod ev_support;
mod support;

use ev_support::assert_errors_in_code;

/// `argument_type_not_assignable_test.dart` `test_annotation_extensionType`.
#[test]
fn argument_type_not_assignable_annotation_extension_type() {
    assert_errors_in_code(
        r#"
extension type const A(String _) {}

@A(0)
void f() {}
"#,
        &[("argument_type_not_assignable", 41, 1)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_annotation_namedConstructor`.
#[test]
fn argument_type_not_assignable_annotation_named_constructor() {
    assert_errors_in_code(
        r#"
class A {
  const A.fromInt(int p);
}
@A.fromInt('0')
main() {}
"#,
        &[("argument_type_not_assignable", 50, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_annotation_namedConstructor_generic`.
#[test]
fn argument_type_not_assignable_annotation_named_constructor_generic() {
    assert_errors_in_code(
        r#"
class A<T> {
  const A.fromInt(T p);
}
@A<int>.fromInt('0')
main() {
}"#,
        &[("argument_type_not_assignable", 56, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_annotation_type_arguments_inferred`.
#[test]
fn argument_type_not_assignable_annotation_type_arguments_inferred() {
    assert_errors_in_code(
        r#"
@C([])
int i = 0;

class C<T> {
  const C(List<List<T>> arg);
}
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_annotation_unnamedConstructor`.
#[test]
fn argument_type_not_assignable_annotation_unnamed_constructor() {
    assert_errors_in_code(
        r#"
class A {
  const A(int p);
}
@A('0')
main() {
}"#,
        &[("argument_type_not_assignable", 34, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_binary`.
#[test]
fn argument_type_not_assignable_binary() {
    assert_errors_in_code(
        r#"
class A {
  operator +(int p) {}
}
f(A a) {
  a + '0';
}"#,
        &[("argument_type_not_assignable", 51, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_binary_eqEq_covariantParameterType`.
#[test]
fn argument_type_not_assignable_binary_eq_eq_covariant_parameter_type() {
    assert_errors_in_code(
        r#"
class A {
  bool operator==(covariant A other) => false;
}

void f(A a, A? aq) {
  a == 0;
  aq == 1;
  aq == aq;
  aq == null;
}
"#,
        &[
            ("argument_type_not_assignable", 89, 1),
            ("argument_type_not_assignable", 100, 1),
        ],
    );
}

/// `argument_type_not_assignable_test.dart` `test_call`.
#[test]
fn argument_type_not_assignable_call() {
    assert_errors_in_code(
        r#"
typedef bool Predicate<T>(T object);

Predicate<String> f() => (String s) => false;

void main() {
  f().call(3);
}"#,
        &[("argument_type_not_assignable", 111, 1)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_cascadeSecond`.
#[test]
fn argument_type_not_assignable_cascade_second() {
    assert_errors_in_code(
        r#"
// filler filler filler filler filler filler filler filler filler filler
class A {
  B ma() { return new B(); }
}
class B {
  mb(String p) {}
}

main() {
  A a = new A();
  a..  ma().mb(0);
}"#,
        &[("argument_type_not_assignable", 187, 1)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_const`.
#[test]
fn argument_type_not_assignable_const() {
    assert_errors_in_code(
        r#"
class A {
  const A(String p);
}
main() {
  const A(42);
}"#,
        &[
            ("argument_type_not_assignable", 53, 2),
            ("const_constructor_param_type_mismatch", 53, 2),
        ],
    );
}

/// `argument_type_not_assignable_test.dart` `test_const_super`.
#[test]
fn argument_type_not_assignable_const_super() {
    assert_errors_in_code(
        r#"
class A {
  const A(String p);
}
class B extends A {
  const B() : super(42);
}"#,
        &[("argument_type_not_assignable", 74, 2)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_downcast`.
#[test]
fn argument_type_not_assignable_downcast() {
    assert_errors_in_code(
        r#"
m() {
  num y = 1;
  n(y);
}
n(int x) {}
"#,
        &[("argument_type_not_assignable", 24, 1)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_downcast_nullableNonNullable`.
#[test]
fn argument_type_not_assignable_downcast_nullable_non_nullable() {
    assert_errors_in_code(
        r#"
m() {
  int? y;
  n(y);
}
n(int x) {}
"#,
        &[("argument_type_not_assignable", 21, 1)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_dynamicCast`.
#[test]
fn argument_type_not_assignable_dynamic_cast() {
    assert_errors_in_code(
        r#"
m() {
  dynamic i;
  n(i);
}
n(int i) {}
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_enumConstant`.
#[test]
fn argument_type_not_assignable_enum_constant() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  const E(String a);
}
"#,
        &[
            ("argument_type_not_assignable", 14, 1),
            ("const_constructor_param_type_mismatch", 14, 1),
        ],
    );
}

/// `argument_type_not_assignable_test.dart` `test_enumConstant_implicitDouble`.
#[test]
fn argument_type_not_assignable_enum_constant_implicit_double() {
    assert_errors_in_code(
        r#"
enum E {
  v(0);
  const E(double a);
}
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_expressionFromConstructorTearoff_withoutTypeArgs`.
#[test]
fn argument_type_not_assignable_expression_from_constructor_tearoff_without_type_args() {
    assert_errors_in_code(
        r#"
class C<T> {
  C(T a);
}

var g = C.new;
var x = g('Hello');
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_expressionFromConstructorTearoff_withTypeArgs_assignable`.
#[test]
fn argument_type_not_assignable_expression_from_constructor_tearoff_with_type_args_assignable() {
    assert_errors_in_code(
        r#"
class C<T> {
  C(T a);
}

var g = C<int>.new;
var x = g(0);
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_expressionFromConstructorTearoff_withTypeArgs_notAssignable`.
#[test]
fn argument_type_not_assignable_expression_from_constructor_tearoff_with_type_args_not_assignable()
{
    assert_errors_in_code(
        r#"
class C<T> {
  C(T a);
}

var g = C<int>.new;
var x = g('Hello');
"#,
        &[("argument_type_not_assignable", 57, 7)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_expressionFromFunctionTearoff_withoutTypeArgs`.
#[test]
fn argument_type_not_assignable_expression_from_function_tearoff_without_type_args() {
    assert_errors_in_code(
        r#"
void f<T>(T a) {}

var g = f;
var x = g('Hello');
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_expressionFromFunctionTearoff_withTypeArgs_assignable`.
#[test]
fn argument_type_not_assignable_expression_from_function_tearoff_with_type_args_assignable() {
    assert_errors_in_code(
        r#"
void f<T>(T a) {}

var g = f<int>;
var x = g(0);
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_expressionFromFunctionTearoff_withTypeArgs_notAssignable`.
#[test]
fn argument_type_not_assignable_expression_from_function_tearoff_with_type_args_not_assignable() {
    assert_errors_in_code(
        r#"
void f<T>(T a) {}

var g = f<int>;
var x = g('Hello');
"#,
        &[("argument_type_not_assignable", 46, 7)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_for_element_type_inferred_from_rewritten_node`.
#[test]
fn argument_type_not_assignable_for_element_type_inferred_from_rewritten_node() {
    assert_errors_in_code(
        r#"
void f<T>(Iterable<T> Function() g, int Function(T) h) {
  [for (var x in g()) if (x is String) h(x)];
}
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_functionExpressionInvocation_required`.
#[test]
fn argument_type_not_assignable_function_expression_invocation_required() {
    assert_errors_in_code(
        r#"
main() {
  (int x) {} ('');
}"#,
        &[("argument_type_not_assignable", 24, 2)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_functionType`.
#[test]
fn argument_type_not_assignable_function_type() {
    assert_errors_in_code(
        r#"
m() {
  var a = new A();
  a.n(() => 0);
}
class A {
  n(void f(int i)) {}
}
"#,
        &[("argument_type_not_assignable", 32, 7)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_implicitCallReference`.
#[test]
fn argument_type_not_assignable_implicit_call_reference() {
    assert_errors_in_code(
        r#"
class A {
  void call(int p) {}
}
void f(void Function(int) a) {}
void g(A a) {
  f(a);
}
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_implicitCallReference_named`.
#[test]
fn argument_type_not_assignable_implicit_call_reference_named() {
    assert_errors_in_code(
        r#"
class A {
  void call(int p) {}
}
void defaultFunc(int p) {}
void f({void Function(int) a = defaultFunc}) {}
void g(A a) {
  f(a: a);
}
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_implicitCallReference_namedAndRequired`.
#[test]
fn argument_type_not_assignable_implicit_call_reference_named_and_required() {
    assert_errors_in_code(
        r#"
class A {
  void call(int p) {}
}
void f({required void Function(int) a}) {}
void g(A a) {
  f(a: a);
}
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_index_invalidRead`.
#[test]
fn argument_type_not_assignable_index_invalid_read() {
    assert_errors_in_code(
        r#"
class A {
  int operator [](int index) => 0;
}
f(A a) {
  a['0'];
}"#,
        &[("argument_type_not_assignable", 61, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_index_invalidRead_validWrite`.
#[test]
fn argument_type_not_assignable_index_invalid_read_valid_write() {
    assert_errors_in_code(
        r#"
class A {
  int operator [](int index) => 0;
  operator []=(String index, int value) {}
}
f(A a) {
  a['0'] += 0;
  ++a['0'];
  a['0']++;
}"#,
        &[
            ("argument_type_not_assignable", 104, 3),
            ("argument_type_not_assignable", 121, 3),
            ("argument_type_not_assignable", 131, 3),
        ],
    );
}

/// `argument_type_not_assignable_test.dart` `test_index_invalidWrite`.
#[test]
fn argument_type_not_assignable_index_invalid_write() {
    assert_errors_in_code(
        r#"
class A {
  operator []=(int index, int value) {}
}
f(A a) {
  a['0'] = 0;
}"#,
        &[("argument_type_not_assignable", 66, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_index_validRead_invalidWrite`.
#[test]
fn argument_type_not_assignable_index_valid_read_invalid_write() {
    assert_errors_in_code(
        r#"
class A {
  int operator [](String index) => 0;
  operator []=(int index, int value) {}
}
f(A a) {
  a['0'] += 0;
  ++a['0'];
  a['0']++;
}"#,
        &[
            ("argument_type_not_assignable", 104, 3),
            ("argument_type_not_assignable", 121, 3),
            ("argument_type_not_assignable", 131, 3),
        ],
    );
}

/// `argument_type_not_assignable_test.dart` `test_interfaceType`.
#[test]
fn argument_type_not_assignable_interface_type() {
    assert_errors_in_code(
        r#"
m() {
  var i = '';
  n(i);
}
n(int i) {}
"#,
        &[("argument_type_not_assignable", 25, 1)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_callParameter`.
#[test]
fn argument_type_not_assignable_invocation_call_parameter() {
    assert_errors_in_code(
        r#"
class A {
  call(int p) {}
}
f(A a) {
  a('0');
}"#,
        &[("argument_type_not_assignable", 43, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_callVariable`.
#[test]
fn argument_type_not_assignable_invocation_call_variable() {
    assert_errors_in_code(
        r#"
class A {
  call(int p) {}
}
main() {
  A a = new A();
  a('0');
}"#,
        &[("argument_type_not_assignable", 60, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_functionParameter`.
#[test]
fn argument_type_not_assignable_invocation_function_parameter() {
    assert_errors_in_code(
        r#"
a(b(int p)) {
  b('0');
}"#,
        &[("argument_type_not_assignable", 19, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_functionTypes_optional`.
#[test]
fn argument_type_not_assignable_invocation_function_types_optional() {
    assert_errors_in_code(
        r#"
void acceptFunOptBool(void funNumOptBool([bool b])) {}
void funBool(bool b) {}
main() {
  acceptFunOptBool(funBool);
}"#,
        &[("argument_type_not_assignable", 108, 7)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_functionTypes_optional_method`.
#[test]
fn argument_type_not_assignable_invocation_function_types_optional_method() {
    assert_errors_in_code(
        r#"
void acceptFunOptBool(void funOptBool([bool b])) {}
class C {
  static void funBool(bool b) {}
}
main() {
  acceptFunOptBool(C.funBool);
}"#,
        &[("argument_type_not_assignable", 126, 9)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_generic`.
#[test]
fn argument_type_not_assignable_invocation_generic() {
    assert_errors_in_code(
        r#"
class A<T> {
  m(T t) {}
}
f(A<String> a) {
  a.m(1);
}"#,
        &[("argument_type_not_assignable", 51, 1)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_named`.
#[test]
fn argument_type_not_assignable_invocation_named() {
    assert_errors_in_code(
        r#"
f({String p = ''}) {}
main() {
  f(p: 42);
}"#,
        &[("argument_type_not_assignable", 39, 2)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_optional`.
#[test]
fn argument_type_not_assignable_invocation_optional() {
    assert_errors_in_code(
        r#"
f([String p = '']) {}
main() {
  f(42);
}"#,
        &[("argument_type_not_assignable", 36, 2)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_required`.
#[test]
fn argument_type_not_assignable_invocation_required() {
    assert_errors_in_code(
        r#"
f(String p) {}
main() {
  f(42);
}"#,
        &[("argument_type_not_assignable", 29, 2)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_typedef_generic`.
#[test]
fn argument_type_not_assignable_invocation_typedef_generic() {
    assert_errors_in_code(
        r#"
typedef A<T>(T p);
f(A<int> a) {
  a('1');
}"#,
        &[("argument_type_not_assignable", 38, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_typedef_local`.
#[test]
fn argument_type_not_assignable_invocation_typedef_local() {
    assert_errors_in_code(
        r#"
typedef A(int p);
A getA() => throw '';
main() {
  A a = getA();
  a('1');
}"#,
        &[("argument_type_not_assignable", 70, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_invocation_typedef_parameter`.
#[test]
fn argument_type_not_assignable_invocation_typedef_parameter() {
    assert_errors_in_code(
        r#"
typedef A(int p);
f(A a) {
  a('1');
}"#,
        &[("argument_type_not_assignable", 32, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_map_indexGet`.
#[test]
fn argument_type_not_assignable_map_index_get() {
    assert_errors_in_code(
        r#"
main() {
  Map<int, int> m = <int, int>{};
  m['x'];
}
"#,
        &[],
    );
}

/// `argument_type_not_assignable_test.dart` `test_map_indexSet`.
#[test]
fn argument_type_not_assignable_map_index_set() {
    assert_errors_in_code(
        r#"
main() {
  Map<int, int> m = <int, int>{};
  m['x'] = 0;
}
"#,
        &[("argument_type_not_assignable", 48, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_map_indexSet_ifNull`.
#[test]
fn argument_type_not_assignable_map_index_set_if_null() {
    assert_errors_in_code(
        r#"
main() {
  Map<int, int> m = <int, int>{};
  m['x'] ??= 0;
}
"#,
        &[("argument_type_not_assignable", 48, 3)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_new_generic`.
#[test]
fn argument_type_not_assignable_new_generic() {
    assert_errors_in_code(
        r#"
class A<T> {
  A(T p) {}
}
main() {
  new A<String>(42);
}"#,
        &[("argument_type_not_assignable", 53, 2)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_new_optional`.
#[test]
fn argument_type_not_assignable_new_optional() {
    assert_errors_in_code(
        r#"
class A {
  A([String p = '']) {}
}
main() {
  new A(42);
}"#,
        &[("argument_type_not_assignable", 54, 2)],
    );
}

/// `argument_type_not_assignable_test.dart` `test_new_required`.
#[test]
fn argument_type_not_assignable_new_required() {
    assert_errors_in_code(
        r#"
class A {
  A(String p) {}
}
main() {
  new A(42);
}"#,
        &[("argument_type_not_assignable", 47, 2)],
    );
}

/// `assignment_to_const_test.dart` `test_instanceVariable`.
#[test]
fn assignment_to_const_instance_variable() {
    assert_errors_in_code(
        r#"
class A {
  static const v = 0;
}
f() {
  A.v = 1;
}"#,
        &[("assignment_to_const", 45, 1)],
    );
}

/// `assignment_to_const_test.dart` `test_instanceVariable_plusEq`.
#[test]
fn assignment_to_const_instance_variable_plus_eq() {
    assert_errors_in_code(
        r#"
class A {
  static const v = 0;
}
f() {
  A.v += 1;
}"#,
        &[("assignment_to_const", 45, 1)],
    );
}

/// `assignment_to_const_test.dart` `test_localVariable`.
#[test]
fn assignment_to_const_local_variable() {
    assert_errors_in_code(
        r#"
f() {
  const x = 0;
  x = 1;
}"#,
        &[
            ("unused_local_variable", 15, 1),
            ("assignment_to_const", 24, 1),
        ],
    );
}

/// `assignment_to_const_test.dart` `test_localVariable_plusEq`.
#[test]
fn assignment_to_const_local_variable_plus_eq() {
    assert_errors_in_code(
        r#"
f() {
  const x = 0;
  x += 1;
}"#,
        &[
            ("unused_local_variable", 15, 1),
            ("assignment_to_const", 24, 1),
        ],
    );
}

/// `assignment_to_final_no_setter_test.dart` `test_prefixedIdentifier_class_instanceGetter`.
#[test]
fn assignment_to_final_no_setter_prefixed_identifier_class_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get x => 0;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
"#,
        &[
            ("assignment_to_final_no_setter", 50, 1),
            ("assignment_to_final_no_setter", 61, 1),
            ("assignment_to_final_no_setter", 75, 1),
            ("assignment_to_final_no_setter", 82, 1),
        ],
    );
}

/// `assignment_to_final_no_setter_test.dart` `test_propertyAccess_class_instanceGetter`.
#[test]
fn assignment_to_final_no_setter_property_access_class_instance_getter() {
    assert_errors_in_code(
        r#"
class A {
  int get x => 0;
}

void f(A a) {
  (a).x = 0;
  (a).x += 0;
  ++(a).x;
  (a).x++;
}
"#,
        &[
            ("assignment_to_final_no_setter", 52, 1),
            ("assignment_to_final_no_setter", 65, 1),
            ("assignment_to_final_no_setter", 81, 1),
            ("assignment_to_final_no_setter", 90, 1),
        ],
    );
}

/// `assignment_to_final_no_setter_test.dart` `test_propertyAccess_extension_instanceGetter`.
#[test]
fn assignment_to_final_no_setter_property_access_extension_instance_getter() {
    assert_errors_in_code(
        r#"
extension E on int {
  int get x => 0;
}

void f() {
  0.x = 0;
  0.x += 0;
  ++0.x;
  0.x++;
}
"#,
        &[
            ("assignment_to_final_no_setter", 58, 1),
            ("assignment_to_final_no_setter", 69, 1),
            ("assignment_to_final_no_setter", 83, 1),
            ("assignment_to_final_no_setter", 90, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_instanceField`.
#[test]
fn assignment_to_final_prefixed_identifier_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  var x = 0;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
"#,
        &[],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_instanceField_abstract`.
#[test]
fn assignment_to_final_prefixed_identifier_instance_field_abstract() {
    assert_errors_in_code(
        r#"
abstract class A {
  abstract int x;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
"#,
        &[],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_instanceField_abstractFinal`.
#[test]
fn assignment_to_final_prefixed_identifier_instance_field_abstract_final() {
    assert_errors_in_code(
        r#"
abstract class A {
  abstract final int x;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
"#,
        &[
            ("assignment_to_final", 65, 1),
            ("assignment_to_final", 76, 1),
            ("assignment_to_final", 90, 1),
            ("assignment_to_final", 97, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_instanceField_external`.
#[test]
fn assignment_to_final_prefixed_identifier_instance_field_external() {
    assert_errors_in_code(
        r#"
abstract class A {
  external int x;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
"#,
        &[],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_instanceField_externalFinal`.
#[test]
fn assignment_to_final_prefixed_identifier_instance_field_external_final() {
    assert_errors_in_code(
        r#"
abstract class A {
  external final int x;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
"#,
        &[
            ("assignment_to_final", 65, 1),
            ("assignment_to_final", 76, 1),
            ("assignment_to_final", 90, 1),
            ("assignment_to_final", 97, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_instanceField_final`.
#[test]
fn assignment_to_final_prefixed_identifier_instance_field_final() {
    assert_errors_in_code(
        r#"
class A {
  final x = 0;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
"#,
        &[
            ("assignment_to_final", 47, 1),
            ("assignment_to_final", 58, 1),
            ("assignment_to_final", 72, 1),
            ("assignment_to_final", 79, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_instanceField_lateFinal`.
#[test]
fn assignment_to_final_prefixed_identifier_instance_field_late_final() {
    assert_errors_in_code(
        r#"
abstract class A {
  late final int x;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
"#,
        &[],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_instanceField_lateFinal_hasInitializer`.
#[test]
fn assignment_to_final_prefixed_identifier_instance_field_late_final_has_initializer() {
    assert_errors_in_code(
        r#"
abstract class A {
  late final int x = 0;
}

void f(A a) {
  a.x = 0;
  a.x += 0;
  ++a.x;
  a.x++;
}
"#,
        &[
            ("assignment_to_final", 65, 1),
            ("assignment_to_final", 76, 1),
            ("assignment_to_final", 90, 1),
            ("assignment_to_final", 97, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_staticField_externalFinal`.
#[test]
fn assignment_to_final_prefixed_identifier_static_field_external_final() {
    assert_errors_in_code(
        r#"
abstract class A {
  external static final int x;
}

void f() {
  A.x = 0;
  A.x += 0;
  ++A.x;
  A.x++;
}
"#,
        &[
            ("assignment_to_final", 69, 1),
            ("assignment_to_final", 80, 1),
            ("assignment_to_final", 94, 1),
            ("assignment_to_final", 101, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_staticField_lateFinal`.
#[test]
fn assignment_to_final_prefixed_identifier_static_field_late_final() {
    assert_errors_in_code(
        r#"
abstract class A {
  static late final int x;
}

void f() {
  A.x = 0;
  A.x += 0;
  ++A.x;
  A.x++;
}
"#,
        &[],
    );
}

/// `assignment_to_final_test.dart` `test_prefixedIdentifier_staticField_lateFinal_hasInitializer`.
#[test]
fn assignment_to_final_prefixed_identifier_static_field_late_final_has_initializer() {
    assert_errors_in_code(
        r#"
abstract class A {
  static late final int x = 0;
}

void f() {
  A.x = 0;
  A.x += 0;
  ++A.x;
  A.x++;
}
"#,
        &[
            ("assignment_to_final", 69, 1),
            ("assignment_to_final", 80, 1),
            ("assignment_to_final", 94, 1),
            ("assignment_to_final", 101, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_propertyAccess_instanceField_lateFinal`.
#[test]
fn assignment_to_final_property_access_instance_field_late_final() {
    assert_errors_in_code(
        r#"
abstract class A {
  late final int x;
}

void f(A a) {
  (a).x = 0;
  (a).x += 0;
  ++(a).x;
  (a).x++;
}
"#,
        &[],
    );
}

/// `assignment_to_final_test.dart` `test_propertyAccess_instanceField_lateFinal_hasInitializer`.
#[test]
fn assignment_to_final_property_access_instance_field_late_final_has_initializer() {
    assert_errors_in_code(
        r#"
abstract class A {
  late final int x = 0;
}

void f(A a) {
  (a).x = 0;
  (a).x += 0;
  ++(a).x;
  (a).x++;
}
"#,
        &[
            ("assignment_to_final", 67, 1),
            ("assignment_to_final", 80, 1),
            ("assignment_to_final", 96, 1),
            ("assignment_to_final", 105, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_simpleIdentifier_topLevelGetter`.
#[test]
fn assignment_to_final_simple_identifier_top_level_getter() {
    assert_errors_in_code(
        r#"
int get x => 0;

void f() {
  x = 0;
  x += 0;
  ++x;
  x++;
}
"#,
        &[
            ("assignment_to_final", 31, 1),
            ("assignment_to_final", 40, 1),
            ("assignment_to_final", 52, 1),
            ("assignment_to_final", 57, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_simpleIdentifier_topLevelVariable`.
#[test]
fn assignment_to_final_simple_identifier_top_level_variable() {
    assert_errors_in_code(
        r#"
var x = 0;

void f() {
  x = 0;
  x += 0;
  ++x;
  x++;
}
"#,
        &[],
    );
}

/// `assignment_to_final_test.dart` `test_simpleIdentifier_topLevelVariable_external`.
#[test]
fn assignment_to_final_simple_identifier_top_level_variable_external() {
    assert_errors_in_code(
        r#"
external int x;

void f() {
  x = 0;
  x += 0;
  ++x;
  x++;
}
"#,
        &[],
    );
}

/// `assignment_to_final_test.dart` `test_simpleIdentifier_topLevelVariable_externalFinal`.
#[test]
fn assignment_to_final_simple_identifier_top_level_variable_external_final() {
    assert_errors_in_code(
        r#"
external final x;

void f() {
  x = 0;
  x += 0;
  ++x;
  x++;
}
"#,
        &[
            ("assignment_to_final", 33, 1),
            ("assignment_to_final", 42, 1),
            ("assignment_to_final", 54, 1),
            ("assignment_to_final", 59, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_simpleIdentifier_topLevelVariable_final`.
#[test]
fn assignment_to_final_simple_identifier_top_level_variable_final() {
    assert_errors_in_code(
        r#"
final x = 0;

void f() {
  x = 0;
  x += 0;
  ++x;
  x++;
}
"#,
        &[
            ("assignment_to_final", 28, 1),
            ("assignment_to_final", 37, 1),
            ("assignment_to_final", 49, 1),
            ("assignment_to_final", 54, 1),
        ],
    );
}

/// `assignment_to_final_test.dart` `test_simpleIdentifier_topLevelVariable_lateFinal`.
#[test]
fn assignment_to_final_simple_identifier_top_level_variable_late_final() {
    assert_errors_in_code(
        r#"
late final int x;

void f() {
  x = 0;
  x += 0;
  ++x;
  x++;
}
"#,
        &[],
    );
}

/// `assignment_to_final_test.dart` `test_simpleIdentifier_topLevelVariable_lateFinal_hasInitializer`.
#[test]
fn assignment_to_final_simple_identifier_top_level_variable_late_final_has_initializer() {
    assert_errors_in_code(
        r#"
late final int x = 0;

void f() {
  x = 0;
  x += 0;
  ++x;
  x++;
}
"#,
        &[
            ("assignment_to_final", 37, 1),
            ("assignment_to_final", 46, 1),
            ("assignment_to_final", 58, 1),
            ("assignment_to_final", 63, 1),
        ],
    );
}

/// `assignment_to_function_test.dart` `test_function`.
#[test]
fn assignment_to_function_function() {
    assert_errors_in_code(
        r#"
f() {}
main() {
  f = null;
}"#,
        &[("assignment_to_function", 19, 1)],
    );
}

/// `assignment_to_method_test.dart` `test_instance_extendedHasMethod_extensionHasSetter`.
#[test]
fn assignment_to_method_instance_extended_has_method_extension_has_setter() {
    assert_errors_in_code(
        r#"
class C {
  void foo() {}
}

extension E on C {
  void set foo(int _) {}
}

void f(C c) {
  c.foo = 0;
  c.foo += 1;
  c.foo++;
  --c.foo;
}
"#,
        &[
            ("assignment_to_method", 95, 3),
            ("assignment_to_method", 108, 3),
            ("assignment_to_method", 122, 3),
            ("assignment_to_method", 135, 3),
        ],
    );
}

/// `assignment_to_method_test.dart` `test_prefixedIdentifier_instanceMethod`.
#[test]
fn assignment_to_method_prefixed_identifier_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

void f(A a) {
  a.foo = 0;
  a.foo += 1;
  a.foo++;
  ++a.foo;
}
"#,
        &[
            ("assignment_to_method", 48, 3),
            ("assignment_to_method", 61, 3),
            ("assignment_to_method", 75, 3),
            ("assignment_to_method", 88, 3),
        ],
    );
}

/// `assignment_to_method_test.dart` `test_propertyAccess_instanceMethod`.
#[test]
fn assignment_to_method_property_access_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  void foo() {}
}

void f(A a) {
  (a).foo = 0;
  (a).foo += 1;
  (a).foo++;
  ++(a).foo;
}
"#,
        &[
            ("assignment_to_method", 50, 3),
            ("assignment_to_method", 65, 3),
            ("assignment_to_method", 81, 3),
            ("assignment_to_method", 96, 3),
        ],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_late`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_late() {
    assert_errors_in_code(
        r#"
class A(int x) {
  late int y = x = 0;
}
"#,
        &[("undefined_identifier", 33, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late() {
    assert_errors_in_code(
        r#"
class A(int x) {
  int y = x = 0;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 28, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_closure`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_closure() {
    assert_errors_in_code(
        r#"
class A(int x) {
  var f = () {
    x = 0;
  };
}
"#,
        &[("assignment_to_primary_constructor_parameter", 37, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_nullAware`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_null_aware() {
    assert_errors_in_code(
        r#"
class A(int? x) {
  int y = x ??= 0;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 29, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_pattern_list`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_pattern_list() {
    assert_errors_in_code(
        r#"
class A(int x) {
  List<int> y = [x] = [2];
}
"#,
        &[("assignment_to_primary_constructor_parameter", 35, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_pattern_logicalAnd`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_pattern_logical_and() {
    assert_errors_in_code(
        r#"
class A(int x, int z) {
  int y = (x && z) = 2;
}
"#,
        &[
            ("assignment_to_primary_constructor_parameter", 36, 1),
            ("assignment_to_primary_constructor_parameter", 41, 1),
        ],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_pattern_map`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_pattern_map() {
    assert_errors_in_code(
        r#"
class A(int x) {
  Map<int?, int> y = {null: x} = {null: 2};
}
"#,
        &[("assignment_to_primary_constructor_parameter", 46, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_pattern_nullAssert`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_pattern_null_assert() {
    assert_errors_in_code(
        r#"
class A(int? x) {
  int y = (x!) = 2;
}
"#,
        &[
            ("assignment_to_primary_constructor_parameter", 30, 1),
            ("unnecessary_null_assert_pattern", 31, 1),
        ],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_pattern_object`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_pattern_object() {
    assert_errors_in_code(
        r#"
class A(int x) {
  Object y = int(sign: x) = 2;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 41, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_pattern_parenthesized`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_pattern_parenthesized() {
    assert_errors_in_code(
        r#"
class A(int x) {
  int y = (x) = 0;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 29, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_pattern_record`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_pattern_record() {
    assert_errors_in_code(
        r#"
class A(int x) {
  (int, {bool name}) y = (x, name: _) = (2, name: true);
}
"#,
        &[("assignment_to_primary_constructor_parameter", 44, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_plusEq`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_plus_eq() {
    assert_errors_in_code(
        r#"
class A(int x) {
  int y = x += 1;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 28, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_postfix`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_postfix() {
    assert_errors_in_code(
        r#"
class A(int x) {
  int y = x++;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 28, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_fieldInitializer_notLate_prefix`.
#[test]
fn assignment_to_primary_constructor_parameter_field_initializer_not_late_prefix() {
    assert_errors_in_code(
        r#"
class A(int x) {
  int y = ++x;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 30, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_primaryConstructor_initializer`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assignment_to_primary_constructor_parameter_primary_constructor_initializer() {
    assert_errors_in_code(
        r#"
class A(int x) {
  int y;
  this : y = x = 0;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 40, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_primaryConstructor_initializer_closure`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assignment_to_primary_constructor_parameter_primary_constructor_initializer_closure() {
    assert_errors_in_code(
        r#"
class A(int x) {
  var f;
  this : f = (() {
    x = 0;
  });
}
"#,
        &[("assignment_to_primary_constructor_parameter", 50, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_primaryConstructor_initializer_nullAware`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assignment_to_primary_constructor_parameter_primary_constructor_initializer_null_aware() {
    assert_errors_in_code(
        r#"
class A(int? x) {
  int y;
  this : y = x ??= 0;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 41, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_primaryConstructor_initializer_plusEq`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assignment_to_primary_constructor_parameter_primary_constructor_initializer_plus_eq() {
    assert_errors_in_code(
        r#"
class A(int x) {
  int y;
  this : y = x += 1;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 40, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_primaryConstructor_initializer_postfix`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assignment_to_primary_constructor_parameter_primary_constructor_initializer_postfix() {
    assert_errors_in_code(
        r#"
class A(int x) {
  int y;
  this : y = x++;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 40, 1)],
    );
}

/// `assignment_to_primary_constructor_parameter_test.dart` `test_primaryConstructor_initializer_prefix`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn assignment_to_primary_constructor_parameter_primary_constructor_initializer_prefix() {
    assert_errors_in_code(
        r#"
class A(int x) {
  int y;
  this : y = ++x;
}
"#,
        &[("assignment_to_primary_constructor_parameter", 42, 1)],
    );
}

/// `assignment_to_type_test.dart` `test_class`.
#[test]
fn assignment_to_type_class() {
    assert_errors_in_code(
        r#"
class C {}
main() {
  C = null;
}
"#,
        &[("assignment_to_type", 23, 1)],
    );
}

/// `assignment_to_type_test.dart` `test_dynamic`.
#[test]
fn assignment_to_type_dynamic() {
    assert_errors_in_code(
        r#"
void f() {
  dynamic = 1;
}
"#,
        &[("assignment_to_type", 14, 7)],
    );
}

/// `assignment_to_type_test.dart` `test_enum`.
#[test]
fn assignment_to_type_enum() {
    assert_errors_in_code(
        r#"
enum E { e }
main() {
  E = null;
}
"#,
        &[("assignment_to_type", 25, 1)],
    );
}

/// `assignment_to_type_test.dart` `test_typedef_functionType`.
#[test]
fn assignment_to_type_typedef_function_type() {
    assert_errors_in_code(
        r#"
typedef void F();
main() {
  F = null;
}
"#,
        &[("assignment_to_type", 30, 1)],
    );
}

/// `assignment_to_type_test.dart` `test_typedef_interfaceType`.
#[test]
fn assignment_to_type_typedef_interface_type() {
    assert_errors_in_code(
        r#"
typedef F = List<int>;

void f() {
  F = null;
}
"#,
        &[("assignment_to_type", 38, 1)],
    );
}

/// `await_in_late_local_variable_initializer_test.dart` `test_closure_late_await`.
#[test]
fn await_in_late_local_variable_initializer_closure_late_await() {
    assert_errors_in_code(
        r#"
main() {
  var v = () async {
    late var v2 = await 42;
    print(v2);
  };
  print(v);
}
"#,
        &[("await_in_late_local_variable_initializer", 49, 5)],
    );
}

/// `await_in_late_local_variable_initializer_test.dart` `test_late_await`.
#[test]
fn await_in_late_local_variable_initializer_late_await() {
    assert_errors_in_code(
        r#"
main() async {
  late var v = await 42;
  print(v);
}
"#,
        &[("await_in_late_local_variable_initializer", 31, 5)],
    );
}

/// `await_in_late_local_variable_initializer_test.dart` `test_late_await_inClosure_blockBody`.
#[test]
fn await_in_late_local_variable_initializer_late_await_in_closure_block_body() {
    assert_errors_in_code(
        r#"
main() async {
  late var v = () async {
    await 42;
  };
  print(v);
}
"#,
        &[],
    );
}

/// `await_in_late_local_variable_initializer_test.dart` `test_late_await_inClosure_expressionBody`.
#[test]
fn await_in_late_local_variable_initializer_late_await_in_closure_expression_body() {
    assert_errors_in_code(
        r#"
main() async {
  late var v = () async => await 42;
  print(v);
}
"#,
        &[],
    );
}

/// `await_in_late_local_variable_initializer_test.dart` `test_no_await`.
#[test]
fn await_in_late_local_variable_initializer_no_await() {
    assert_errors_in_code(
        r#"
main() async {
  late var v = 42;
  print(v);
}
"#,
        &[],
    );
}

/// `await_in_late_local_variable_initializer_test.dart` `test_not_late`.
#[test]
fn await_in_late_local_variable_initializer_not_late() {
    assert_errors_in_code(
        r#"
main() async {
  var v = await 42;
  print(v);
}
"#,
        &[],
    );
}

/// `await_of_incompatible_type_test.dart` `test_extensionType_implementsFuture`.
#[test]
fn await_of_incompatible_type_extension_type_implements_future() {
    assert_errors_in_code(
        r#"
extension type A(Future<int> it) implements Future<int> {}

void f(A a) async {
  await a;
}
"#,
        &[],
    );
}

/// `await_of_incompatible_type_test.dart` `test_extensionType_notImplementsFuture`.
#[test]
fn await_of_incompatible_type_extension_type_not_implements_future() {
    assert_errors_in_code(
        r#"
extension type A(int it) {}

void f(A a) async {
  await a;
}
"#,
        &[("await_of_incompatible_type", 52, 5)],
    );
}

/// `await_of_incompatible_type_test.dart` `test_typeParameter_bound_extensionType_implementsFuture`.
#[test]
fn await_of_incompatible_type_type_parameter_bound_extension_type_implements_future() {
    assert_errors_in_code(
        r#"
extension type A(Future<int> it) implements Future<int> {}

void f<T extends A>(T a) async {
  await a;
}
"#,
        &[],
    );
}

/// `await_of_incompatible_type_test.dart` `test_typeParameter_bound_extensionType_notImplementsFuture`.
#[test]
fn await_of_incompatible_type_type_parameter_bound_extension_type_not_implements_future() {
    assert_errors_in_code(
        r#"
extension type A(Future<int> it) {}

void f<T extends A>(T a) async {
  await a;
}
"#,
        &[("await_of_incompatible_type", 73, 5)],
    );
}

/// `dead_null_aware_expression_test.dart` `test_assignCompound_map`.
#[test]
fn dead_null_aware_expression_assign_compound_map() {
    assert_errors_in_code(
        r#"
class MyMap<K, V> {
  V? operator[](K key) => null;
  void operator[]=(K key, V value) {}
}

f(MyMap<int, int> map) {
  map[0] ??= 0;
}
"#,
        &[],
    );
}

/// `dead_null_aware_expression_test.dart` `test_assignCompound_nonNullable`.
#[test]
fn dead_null_aware_expression_assign_compound_non_nullable() {
    assert_errors_in_code(
        r#"
f(int x) {
  x ??= 0;
}
"#,
        &[("dead_code", 20, 2), ("dead_null_aware_expression", 20, 1)],
    );
}

/// `dead_null_aware_expression_test.dart` `test_assignCompound_nullable`.
#[test]
fn dead_null_aware_expression_assign_compound_nullable() {
    assert_errors_in_code(
        r#"
f(int? x) {
  x ??= 0;
}
"#,
        &[],
    );
}

/// `dead_null_aware_expression_test.dart` `test_binary_nonNullable`.
#[test]
fn dead_null_aware_expression_binary_non_nullable() {
    assert_errors_in_code(
        r#"
f(int x) {
  x ?? 0;
}
"#,
        &[("dead_code", 16, 4), ("dead_null_aware_expression", 19, 1)],
    );
}

/// `dead_null_aware_expression_test.dart` `test_binary_nullable`.
#[test]
fn dead_null_aware_expression_binary_nullable() {
    assert_errors_in_code(
        r#"
f(int? x) {
  x ?? 0;
}
"#,
        &[],
    );
}

/// `dead_null_aware_expression_test.dart` `test_binary_nullType`.
#[test]
fn dead_null_aware_expression_binary_null_type() {
    assert_errors_in_code(
        r#"
f(Null x) {
  x ?? 1;
}
"#,
        &[],
    );
}

/// `field_initializer_not_assignable_test.dart` `test_class_implicitCallReference`.
#[test]
fn field_initializer_not_assignable_class_implicit_call_reference() {
    assert_errors_in_code(
        r#"
class C {
  void call(int p) {}
}
class A {
  void Function(int) x;
  A() : x = C();
}
"#,
        &[],
    );
}

/// `field_initializer_not_assignable_test.dart` `test_class_implicitCallReference_genericFunctionInstantiation`.
#[test]
fn field_initializer_not_assignable_class_implicit_call_reference_generic_function_instantiation() {
    assert_errors_in_code(
        r#"
class C {
  void call<T>(T p) {}
}
class A {
  void Function(int) x;
  A() : x = C();
}
"#,
        &[],
    );
}

/// `field_initializer_not_assignable_test.dart` `test_class_unrelated`.
#[test]
fn field_initializer_not_assignable_class_unrelated() {
    assert_errors_in_code(
        r#"
class A {
  int x;
  A() : x = '';
}
"#,
        &[("field_initializer_not_assignable", 32, 2)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_class_fieldInitializer_commentReference_prefixedIdentifier`.
#[test]
fn implicit_this_reference_in_initializer_class_field_initializer_comment_reference_prefixed_identifier()
 {
    assert_errors_in_code(
        r#"
class A {
  int a = 0;
  /// foo [a.isEven] bar
  int x = 1;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_class_fieldInitializer_commentReference_simpleIdentifier`.
#[test]
fn implicit_this_reference_in_initializer_class_field_initializer_comment_reference_simple_identifier()
 {
    assert_errors_in_code(
        r#"
class A {
  int a = 0;
  /// foo [a] bar
  int x = 1;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_class_fieldInitializer_late_invokeInstanceMethod`.
#[test]
fn implicit_this_reference_in_initializer_class_field_initializer_late_invoke_instance_method() {
    assert_errors_in_code(
        r#"
class A {
  late int x = foo();
  int foo() => 0;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_class_fieldInitializer_late_invokeStaticMethod`.
#[test]
fn implicit_this_reference_in_initializer_class_field_initializer_late_invoke_static_method() {
    assert_errors_in_code(
        r#"
class A {
  late int x = foo();
  static int foo() => 0;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_class_fieldInitializer_late_readInstanceField`.
#[test]
fn implicit_this_reference_in_initializer_class_field_initializer_late_read_instance_field() {
    assert_errors_in_code(
        r#"
class A {
  int a = 0;
  late int x = a;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_class_fieldInitializer_late_readStaticField`.
#[test]
fn implicit_this_reference_in_initializer_class_field_initializer_late_read_static_field() {
    assert_errors_in_code(
        r#"
class A {
  static int a = 0;
  late int x = a;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_constructorInitializer_assert_superClass`.
#[test]
fn implicit_this_reference_in_initializer_constructor_initializer_assert_super_class() {
    assert_errors_in_code(
        r#"
class A {
  int get f => 0;
}

class B extends A {
  B() : assert(f != 0);
}
"#,
        &[("implicit_this_reference_in_initializer", 67, 1)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_constructorInitializer_assert_thisClass`.
#[test]
fn implicit_this_reference_in_initializer_constructor_initializer_assert_this_class() {
    assert_errors_in_code(
        r#"
class A {
  A() : assert(f != 0);
  int get f => 0;
}
"#,
        &[("implicit_this_reference_in_initializer", 26, 1)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_constructorInitializer_field`.
#[test]
fn implicit_this_reference_in_initializer_constructor_initializer_field() {
    assert_errors_in_code(
        r#"
class A {
  var v;
  A() : v = f;
  var f;
}
"#,
        &[("implicit_this_reference_in_initializer", 32, 1)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_constructorName`.
#[test]
fn implicit_this_reference_in_initializer_constructor_name() {
    assert_errors_in_code(
        r#"
class A {
  A.named() {}
}
class B {
  var v;
  B() : v = new A.named();
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_fieldInitializer`.
#[test]
fn implicit_this_reference_in_initializer_field_initializer() {
    assert_errors_in_code(
        r#"
class A {
  final x = 0;
  final y = x;
}
"#,
        &[("implicit_this_reference_in_initializer", 38, 1)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_fieldInitializer_functionReference`.
#[test]
fn implicit_this_reference_in_initializer_field_initializer_function_reference() {
    assert_errors_in_code(
        r#"
class A {
  void x<T>() {}
  final y = x<int>;
}
"#,
        &[("implicit_this_reference_in_initializer", 40, 1)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_fieldInitializer_nestedLocal`.
#[test]
fn implicit_this_reference_in_initializer_field_initializer_nested_local() {
    assert_errors_in_code(
        r#"
class A {
  Map foo = {
    'a': () {
      var v = 0; // (1)
      v;
    },
    'b': _foo // (2)
  };

  void _foo() {}
}
"#,
        &[("implicit_this_reference_in_initializer", 88, 4)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_invocation`.
#[test]
fn implicit_this_reference_in_initializer_invocation() {
    assert_errors_in_code(
        r#"
class A {
  var v;
  A() : v = f();
  f() {}
}
"#,
        &[("implicit_this_reference_in_initializer", 32, 1)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_invocationInStatic`.
#[test]
fn implicit_this_reference_in_initializer_invocation_in_static() {
    assert_errors_in_code(
        r#"
class A {
  static var F = m();
  int m() => 0;
}
"#,
        &[("implicit_this_reference_in_initializer", 28, 1)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_mixin_field_late_readInstanceField`.
#[test]
fn implicit_this_reference_in_initializer_mixin_field_late_read_instance_field() {
    assert_errors_in_code(
        r#"
mixin M {
  int a = 0;
  late int x = a;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_prefixedIdentifier`.
#[test]
fn implicit_this_reference_in_initializer_prefixed_identifier() {
    assert_errors_in_code(
        r#"
class A {
  var f;
}
class B {
  var v;
  B(A a) : v = a.f;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_qualifiedMethodInvocation`.
#[test]
fn implicit_this_reference_in_initializer_qualified_method_invocation() {
    assert_errors_in_code(
        r#"
class A {
  f() {}
}
class B {
  var v;
  B() : v = new A().f();
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_qualifiedPropertyAccess`.
#[test]
fn implicit_this_reference_in_initializer_qualified_property_access() {
    assert_errors_in_code(
        r#"
class A {
  var f;
}
class B {
  var v;
  B() : v = new A().f;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_redirectingConstructorInvocation`.
#[test]
fn implicit_this_reference_in_initializer_redirecting_constructor_invocation() {
    assert_errors_in_code(
        r#"
class A {
  A(p) {}
  A.named() : this(f);
  var f;
}
"#,
        &[("implicit_this_reference_in_initializer", 40, 1)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_staticField_thisClass`.
#[test]
fn implicit_this_reference_in_initializer_static_field_this_class() {
    assert_errors_in_code(
        r#"
class A {
  var v;
  A() : v = f;
  static var f;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_staticGetter`.
#[test]
fn implicit_this_reference_in_initializer_static_getter() {
    assert_errors_in_code(
        r#"
class A {
  var v;
  A() : v = f;
  static get f => 42;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_staticMethod`.
#[test]
fn implicit_this_reference_in_initializer_static_method() {
    assert_errors_in_code(
        r#"
class A {
  var v;
  A() : v = f();
  static f() => 42;
}
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_superConstructorInvocation`.
#[test]
fn implicit_this_reference_in_initializer_super_constructor_invocation() {
    assert_errors_in_code(
        r#"
class A {
  A(p) {}
}
class B extends A {
  B() : super(f);
  var f;
}
"#,
        &[("implicit_this_reference_in_initializer", 57, 1)],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_topLevelField`.
#[test]
fn implicit_this_reference_in_initializer_top_level_field() {
    assert_errors_in_code(
        r#"
class A {
  var v;
  A() : v = f;
}
var f = 42;
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_topLevelFunction`.
#[test]
fn implicit_this_reference_in_initializer_top_level_function() {
    assert_errors_in_code(
        r#"
class A {
  var v;
  A() : v = f();
}
f() => 42;
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_topLevelGetter`.
#[test]
fn implicit_this_reference_in_initializer_top_level_getter() {
    assert_errors_in_code(
        r#"
class A {
  var v;
  A() : v = f;
}
get f => 42;
"#,
        &[],
    );
}

/// `implicit_this_reference_in_initializer_test.dart` `test_typeParameter`.
#[test]
fn implicit_this_reference_in_initializer_type_parameter() {
    assert_errors_in_code(
        r#"
class A<T> {
  var v;
  A(p) : v = (p is T);
}
"#,
        &[],
    );
}

/// `integer_literal_imprecise_as_double_test.dart` `test_excessiveExponent`.
#[test]
fn integer_literal_imprecise_as_double_excessive_exponent() {
    assert_errors_in_code(
        r#"
double x = 0xfffffffffffff8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000;
"#,
        &[("integer_literal_imprecise_as_double", 12, 259)],
    );
}

/// `integer_literal_imprecise_as_double_test.dart` `test_excessiveMantissa`.
#[test]
fn integer_literal_imprecise_as_double_excessive_mantissa() {
    assert_errors_in_code(
        r#"
double x = 9223372036854775809;
"#,
        &[("integer_literal_imprecise_as_double", 12, 19)],
    );
}

/// `integer_literal_imprecise_as_double_test.dart` `test_excessiveMantissa_withSeparators`.
#[test]
fn integer_literal_imprecise_as_double_excessive_mantissa_with_separators() {
    assert_errors_in_code(
        r#"
double x = 9_223_372_036_854_775_809;
"#,
        &[("integer_literal_imprecise_as_double", 12, 25)],
    );
}

/// `integer_literal_out_of_range_test.dart` `test_hex`.
#[test]
fn integer_literal_out_of_range_hex() {
    assert_errors_in_code(
        r#"
int x = 0xFFFF_FFFF_FFFF_FFFF_FFFF;
"#,
        &[("integer_literal_out_of_range", 9, 26)],
    );
}

/// `integer_literal_out_of_range_test.dart` `test_negative`.
#[test]
fn integer_literal_out_of_range_negative() {
    assert_errors_in_code(
        r#"
int x = -9223372036854775809;
"#,
        &[("integer_literal_out_of_range", 10, 19)],
    );
}

/// `integer_literal_out_of_range_test.dart` `test_positive`.
#[test]
fn integer_literal_out_of_range_positive() {
    assert_errors_in_code(
        r#"
int x = 9223372036854775808;
"#,
        &[("integer_literal_out_of_range", 9, 19)],
    );
}

/// `integer_literal_out_of_range_test.dart` `test_separators`.
#[test]
fn integer_literal_out_of_range_separators() {
    assert_errors_in_code(
        r#"
int x = 9_223_372_036_854_775_808;
"#,
        &[("integer_literal_out_of_range", 9, 25)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_genericBoundedCall_nonGenericContext`.
#[test]
fn invalid_assignment_invalid_generic_bounded_call_non_generic_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T extends num>(T t) => t;
}

String Function(String) f = C();
"#,
        &[("invalid_assignment", 77, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_genericCall_genericEnclosingClass_nonGenericContext`.
#[test]
fn invalid_assignment_invalid_generic_call_generic_enclosing_class_non_generic_context() {
    assert_errors_in_code(
        r#"
class C<T> {
  C(T a);
  void call<U>(T t, U u) {}
}

void Function(bool, String) f = C(7);
"#,
        &[("invalid_assignment", 87, 4)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_genericCall_nonGenericContext`.
#[test]
fn invalid_assignment_invalid_generic_call_non_generic_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T>(T t) => t;
}

void Function() f = C();
"#,
        &[("invalid_assignment", 57, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_genericCall_nonGenericContext_withoutConstructorTearoffs`.
#[test]
fn invalid_assignment_invalid_generic_call_non_generic_context_without_constructor_tearoffs() {
    assert_errors_in_code(
        r#"
// @dart=2.12
class C {
  T call<T>(T t) => t;
}

int Function(int) f = C();
"#,
        &[("invalid_assignment", 73, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_interfaceType_enum_interfaces`.
#[test]
fn invalid_assignment_invalid_interface_type_enum_interfaces() {
    assert_errors_in_code(
        r#"
class I {}
class J {}
enum E implements J {
  v
}
I x = E.v;
"#,
        &[("invalid_assignment", 57, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_message_preferTypeAlias_functionType`.
#[test]
fn invalid_assignment_invalid_message_prefer_type_alias_function_type() {
    assert_errors_in_code(
        r#"
typedef A<T> = T Function();

void f(A<int> a) {
  A<String> b = a;
}
"#,
        &[
            ("unused_local_variable", 62, 1),
            ("invalid_assignment", 66, 1),
        ],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_message_preferTypeAlias_interfaceType`.
#[test]
fn invalid_assignment_invalid_message_prefer_type_alias_interface_type() {
    assert_errors_in_code(
        r#"
typedef A<T> = List<T>;

void f(A<int> a) {
  A<String> b = a;
}
"#,
        &[
            ("unused_local_variable", 57, 1),
            ("invalid_assignment", 61, 1),
        ],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_message_preferTypeAlias_recordType`.
#[test]
fn invalid_assignment_invalid_message_prefer_type_alias_record_type() {
    assert_errors_in_code(
        r#"
typedef A<T> = (T, T);

void f(A<int> a) {
  A<String> b = a;
}
"#,
        &[
            ("unused_local_variable", 56, 1),
            ("invalid_assignment", 60, 1),
        ],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_noCall_functionContext`.
#[test]
fn invalid_assignment_invalid_no_call_function_context() {
    assert_errors_in_code(
        r#"
class C {}

Function f = C();
"#,
        &[("invalid_assignment", 26, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_noCall_functionTypeContext`.
#[test]
fn invalid_assignment_invalid_no_call_function_type_context() {
    assert_errors_in_code(
        r#"
class C {}

String Function(String) f = C();
"#,
        &[("invalid_assignment", 41, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_nonGenericCall`.
#[test]
fn invalid_assignment_invalid_non_generic_call() {
    assert_errors_in_code(
        r#"
class C {
  void call(int a) {}
}

void Function(String) f = C();
"#,
        &[("invalid_assignment", 62, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_nonGenericCall_typeVariableExtendsFunctionContext`.
#[test]
fn invalid_assignment_invalid_non_generic_call_type_variable_extends_function_context() {
    assert_errors_in_code(
        r#"
class C {
  void call(int a) {}
}
class D<U extends Function> {
  U f = C();
}
"#,
        &[("invalid_assignment", 73, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_invalid_nonGenericCall_typeVariableExtendsFunctionTypeContext`.
#[test]
fn invalid_assignment_invalid_non_generic_call_type_variable_extends_function_type_context() {
    assert_errors_in_code(
        r#"
class C {
  void call(int a) {}
}
class D<U extends void Function(int)> {
  U f = C();
}
"#,
        &[("invalid_assignment", 83, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericBoundedCall_nonGenericContext`.
#[test]
fn invalid_assignment_valid_generic_bounded_call_non_generic_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T extends num>(T t) => t;
}

int Function(int) f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_functionContext`.
#[test]
fn invalid_assignment_valid_generic_call_function_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T>(T t) => t;
}

Function f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_futureOrFunctionContext`.
#[test]
fn invalid_assignment_valid_generic_call_future_or_function_context() {
    assert_errors_in_code(
        r#"
import 'dart:async';
class C {
  T call<T>(T t) => t;
}

FutureOr<Function> f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_futureOrFunctionTypeContext_generic`.
#[test]
fn invalid_assignment_valid_generic_call_future_or_function_type_context_generic() {
    assert_errors_in_code(
        r#"
import 'dart:async';
class C {
  T call<T>(T t) => t;
}

FutureOr<T Function<T>(T)> f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_futureOrFunctionTypeContext_nonGeneric`.
#[test]
fn invalid_assignment_valid_generic_call_future_or_function_type_context_non_generic() {
    assert_errors_in_code(
        r#"
import 'dart:async';
class C {
  T call<T>(T t) => t;
}

FutureOr<int Function(int)> f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_genericContext`.
#[test]
fn invalid_assignment_valid_generic_call_generic_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T>(T t) => t;
}

T Function<T>(T) f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_genericEnclosingClass_nonGenericContext`.
#[test]
fn invalid_assignment_valid_generic_call_generic_enclosing_class_non_generic_context() {
    assert_errors_in_code(
        r#"
class C<T> {
  C(T a);
  void call<U>(T t, U u) {}
}

void Function(int, String) f = C(7);
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_genericTypedefContext`.
#[test]
fn invalid_assignment_valid_generic_call_generic_typedef_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T>(T t) => t;
}
typedef Fn<T> = T Function(T);
class D<U> {
  Fn<U> f = C();
}

"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_nonGenericContext`.
#[test]
fn invalid_assignment_valid_generic_call_non_generic_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T>(T t) => t;
}

int Function(int) f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_nullableFunctionContext`.
#[test]
fn invalid_assignment_valid_generic_call_nullable_function_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T>(T t) => t;
}

Function? f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_nullableNonGenericContext`.
#[test]
fn invalid_assignment_valid_generic_call_nullable_non_generic_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T>(T t) => t;
}

int Function(int)? f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_genericCall_typedefOfGenericContext`.
#[test]
fn invalid_assignment_valid_generic_call_typedef_of_generic_context() {
    assert_errors_in_code(
        r#"
class C {
  T call<T>(T t) => t;
}

typedef Fn = T Function<T>(T);

Fn f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_interfaceType_enum_interfaces`.
#[test]
fn invalid_assignment_valid_interface_type_enum_interfaces() {
    assert_errors_in_code(
        r#"
class I {}
enum E implements I {
  v
}
I x = E.v;
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_nonGenericCall`.
#[test]
fn invalid_assignment_valid_non_generic_call() {
    assert_errors_in_code(
        r#"
class C {
  void call(int a) {}
}

void Function(int) f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_nonGenericCall_declaredOnMixin`.
#[test]
fn invalid_assignment_valid_non_generic_call_declared_on_mixin() {
    assert_errors_in_code(
        r#"
mixin M {
  void call(int a) {}
}
class C with M {}

Function f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_nonGenericCall_inCascade`.
#[test]
fn invalid_assignment_valid_non_generic_call_in_cascade() {
    assert_errors_in_code(
        r#"
class C {
  void call(int a) {}
}
class D {
  late void Function(int) f;
}

void foo() {
  D()..f = C();
}
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_nonGenericCall_subTypeViaParameter`.
#[test]
fn invalid_assignment_valid_non_generic_call_sub_type_via_parameter() {
    assert_errors_in_code(
        r#"
class C {
  void call(num a) {}
}

void Function(int) f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_valid_nonGenericCall_subTypeViaReturnType`.
#[test]
fn invalid_assignment_valid_non_generic_call_sub_type_via_return_type() {
    assert_errors_in_code(
        r#"
class C {
  int call() => 7;
}

num Function() f = C();
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_assignment_to_dynamic`.
#[test]
fn invalid_assignment_assignment_to_dynamic() {
    assert_errors_in_code(
        r#"
f() {
  var g;
  g = () => 0;
}
"#,
        &[("unused_local_variable", 13, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_cascadeExpression`.
#[test]
fn invalid_assignment_cascade_expression() {
    assert_errors_in_code(
        r#"
void f(int a) {
  // ignore:unused_local_variable
  String v = (a)..isEven;
}
"#,
        &[("invalid_assignment", 65, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_compoundAssignment`.
#[test]
fn invalid_assignment_compound_assignment() {
    assert_errors_in_code(
        r#"
class byte {
  int _value;
  byte(this._value);
  byte operator +(int val) { return this; }
}

void main() {
  byte b = new byte(52);
  b += 3;
}
"#,
        &[("unused_field", 20, 6), ("unused_local_variable", 117, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_constructorTearoff_inferredTypeArgs`.
#[test]
fn invalid_assignment_constructor_tearoff_inferred_type_args() {
    assert_errors_in_code(
        r#"
class C<T> {
  C(T a);
}

var g = C<int>.new;
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_constructorTearoff_withExplicitTypeArgs`.
#[test]
fn invalid_assignment_constructor_tearoff_with_explicit_type_args() {
    assert_errors_in_code(
        r#"
class C<T> {
  C(T a);
}

C Function(int) g = C<int>.new;
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_constructorTearoff_withExplicitTypeArgs_invalid`.
#[test]
fn invalid_assignment_constructor_tearoff_with_explicit_type_args_invalid() {
    assert_errors_in_code(
        r#"
class C<T> {
  C(T a);
}

C Function(String) g = C<int>.new;
"#,
        &[("invalid_assignment", 50, 10)],
    );
}

/// `invalid_assignment_test.dart` `test_defaultValue_named`.
#[test]
fn invalid_assignment_default_value_named() {
    assert_errors_in_code(
        r#"
f({String x = 0}) {
}
"#,
        &[("invalid_assignment", 15, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_defaultValue_named_sameType`.
#[test]
fn invalid_assignment_default_value_named_same_type() {
    assert_errors_in_code(
        r#"
f({String x = '0'}) {
}"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_defaultValue_optional`.
#[test]
fn invalid_assignment_default_value_optional() {
    assert_errors_in_code(
        r#"
f([String x = 0]) {
}"#,
        &[("invalid_assignment", 15, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_defaultValue_optional_sameType`.
#[test]
fn invalid_assignment_default_value_optional_same_type() {
    assert_errors_in_code(
        r#"
f([String x = '0']) {
}
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_functionExpressionInvocation`.
#[test]
fn invalid_assignment_function_expression_invocation() {
    assert_errors_in_code(
        r#"
class C {
  String x = (() => 5)();
}
"#,
        &[("invalid_assignment", 24, 11)],
    );
}

/// `invalid_assignment_test.dart` `test_functionInstantiation_topLevelVariable_genericContext_assignable`.
#[test]
fn invalid_assignment_function_instantiation_top_level_variable_generic_context_assignable() {
    assert_errors_in_code(
        r#"
T f<T>(T a) => a;
U Function<U>(U) foo = f;
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_functionInstantiation_topLevelVariable_genericContext_nonAssignable`.
#[test]
fn invalid_assignment_function_instantiation_top_level_variable_generic_context_non_assignable() {
    assert_errors_in_code(
        r#"
T f<T>(T a) => a;
U Function<U>(U, int) foo = f;
"#,
        &[("invalid_assignment", 47, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_functionInstantiation_topLevelVariable_nonGenericContext_assignable`.
#[test]
fn invalid_assignment_function_instantiation_top_level_variable_non_generic_context_assignable() {
    assert_errors_in_code(
        r#"
T f<T>(T a) => a;
int Function(int) foo = f;
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_functionInstantiation_topLevelVariable_nonGenericContext_nonAssignable`.
#[test]
fn invalid_assignment_function_instantiation_top_level_variable_non_generic_context_non_assignable()
{
    assert_errors_in_code(
        r#"
T f<T>(T a) => a;
int Function(int, int) foo = f;
"#,
        &[("invalid_assignment", 48, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_functionTearoff_genericInstantiation`.
#[test]
fn invalid_assignment_function_tearoff_generic_instantiation() {
    assert_errors_in_code(
        r#"
int Function() foo(int Function<T extends int>() f) {
  return f;
}
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_functionTearoff_inferredTypeArgs`.
#[test]
fn invalid_assignment_function_tearoff_inferred_type_args() {
    assert_errors_in_code(
        r#"
void f<T>(T a) {}

var g = f<int>;
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_functionTearoff_withExplicitTypeArgs`.
#[test]
fn invalid_assignment_function_tearoff_with_explicit_type_args() {
    assert_errors_in_code(
        r#"
void f<T>(T a) {}

void Function(int) g = f<int>;
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_functionTearoff_withExplicitTypeArgs_invalid`.
#[test]
fn invalid_assignment_function_tearoff_with_explicit_type_args_invalid() {
    assert_errors_in_code(
        r#"
void f<T>(T a) {}

void Function(String) g = f<int>;
"#,
        &[("invalid_assignment", 46, 6)],
    );
}

/// `invalid_assignment_test.dart` `test_ifNullAssignment`.
#[test]
fn invalid_assignment_if_null_assignment() {
    assert_errors_in_code(
        r#"
void f(int i) {
  double? d;
  d ??= i;
}
"#,
        &[("invalid_assignment", 38, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_ifNullAssignment_sameType`.
#[test]
fn invalid_assignment_if_null_assignment_same_type() {
    assert_errors_in_code(
        r#"
void f(int i) {
  int? j;
  j ??= i;
}
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_ifNullAssignment_superType`.
#[test]
fn invalid_assignment_if_null_assignment_super_type() {
    assert_errors_in_code(
        r#"
void f(int i) {
  num? n;
  n ??= i;
}
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_invalidAssignment`.
#[test]
fn invalid_assignment_invalid_assignment() {
    assert_errors_in_code(
        r#"
f() {
  var x;
  var y;
  x = y;
}
"#,
        &[("unused_local_variable", 13, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_localLevelVariable_never_null`.
#[test]
fn invalid_assignment_local_level_variable_never_null() {
    assert_errors_in_code(
        r#"
void f(Never x) {
  x = null;
}
"#,
        &[("invalid_assignment", 25, 4)],
    );
}

/// `invalid_assignment_test.dart` `test_localVariable`.
#[test]
fn invalid_assignment_local_variable() {
    assert_errors_in_code(
        r#"
f() {
  int x;
  x = '0';
}
"#,
        &[
            ("unused_local_variable", 13, 1),
            ("invalid_assignment", 22, 3),
        ],
    );
}

/// `invalid_assignment_test.dart` `test_parenthesizedExpression`.
#[test]
fn invalid_assignment_parenthesized_expression() {
    assert_errors_in_code(
        r#"
void f(int a) {
  // ignore:unused_local_variable
  String v = (a);
}
"#,
        &[("invalid_assignment", 65, 1)],
    );
}

/// `invalid_assignment_test.dart` `test_postfixExpression_localVariable`.
#[test]
fn invalid_assignment_postfix_expression_local_variable() {
    assert_errors_in_code(
        r#"
class A {
  B operator+(_) => new B();
}

class B {}

f(A a) {
  a++;
}
"#,
        &[("invalid_assignment", 66, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_postfixExpression_localVariable_sameType`.
#[test]
fn invalid_assignment_postfix_expression_local_variable_same_type() {
    assert_errors_in_code(
        r#"
class A {
  A operator+(_) => this;
}

f(A a) {
  a++;
}
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_postfixExpression_property`.
#[test]
fn invalid_assignment_postfix_expression_property() {
    assert_errors_in_code(
        r#"
class A {
  B operator+(_) => new B();
}

class B {}

class C {
  A a = A();
}

f(C c) {
  c.a++;
}
"#,
        &[("invalid_assignment", 92, 5)],
    );
}

/// `invalid_assignment_test.dart` `test_postfixExpression_property_sameType`.
#[test]
fn invalid_assignment_postfix_expression_property_same_type() {
    assert_errors_in_code(
        r#"
class A {
  A operator+(_) => this;
}

class C {
  A a = A();
}

f(C c) {
  c.a++;
}
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_prefixExpression_localVariable`.
#[test]
fn invalid_assignment_prefix_expression_local_variable() {
    assert_errors_in_code(
        r#"
class A {
  B operator+(_) => new B();
}

class B {}

f(A a) {
  ++a;
}
"#,
        &[("invalid_assignment", 66, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_prefixExpression_localVariable_sameType`.
#[test]
fn invalid_assignment_prefix_expression_local_variable_same_type() {
    assert_errors_in_code(
        r#"
class A {
  A operator+(_) => this;
}

f(A a) {
  ++a;
}
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_prefixExpression_property`.
#[test]
fn invalid_assignment_prefix_expression_property() {
    assert_errors_in_code(
        r#"
class A {
  B operator+(_) => new B();
}

class B {}

class C {
  A a = A();
}

f(C c) {
  ++c.a;
}
"#,
        &[("invalid_assignment", 92, 5)],
    );
}

/// `invalid_assignment_test.dart` `test_prefixExpression_property_sameType`.
#[test]
fn invalid_assignment_prefix_expression_property_same_type() {
    assert_errors_in_code(
        r#"
class A {
  A operator+(_) => this;
}

class C {
  A a = A();
}

f(C c) {
  ++c.a;
}
"#,
        &[],
    );
}

/// `invalid_assignment_test.dart` `test_regressionInIssue18468Fix`.
#[test]
fn invalid_assignment_regression_in_issue18468_fix() {
    assert_errors_in_code(
        r#"
class C<T> {
  T t = int;
}
"#,
        &[("invalid_assignment", 22, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_staticVariable`.
#[test]
fn invalid_assignment_static_variable() {
    assert_errors_in_code(
        r#"
class A {
  static int x = 1;
}
f() {
  A.x = '0';
}
"#,
        &[("invalid_assignment", 47, 3)],
    );
}

/// `invalid_assignment_test.dart` `test_topLevelVariable_never_null`.
#[test]
fn invalid_assignment_top_level_variable_never_null() {
    assert_errors_in_code(
        r#"
Never x = throw 0;

void f() {
  x = null;
}
"#,
        &[("invalid_assignment", 38, 4)],
    );
}

/// `invalid_assignment_test.dart` `test_topLevelVariableDeclaration`.
#[test]
fn invalid_assignment_top_level_variable_declaration() {
    assert_errors_in_code(
        r#"
int x = 'string';
"#,
        &[("invalid_assignment", 9, 8)],
    );
}

/// `invalid_assignment_test.dart` `test_variableDeclaration`.
#[test]
fn invalid_assignment_variable_declaration() {
    assert_errors_in_code(
        r#"
class A {
  int x = 'string';
}
"#,
        &[("invalid_assignment", 21, 8)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_instanceField_initializer`.
#[test]
fn invalid_reference_to_this_class_instance_field_initializer() {
    assert_errors_in_code(
        r#"
class A {
  var f = this;
}
"#,
        &[("invalid_reference_to_this", 21, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_instanceField_lateInitializer`.
#[test]
fn invalid_reference_to_this_class_instance_field_late_initializer() {
    assert_errors_in_code(
        r#"
class A {
  late var f = this;
}
"#,
        &[],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_instanceMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_class_instance_method_default_value() {
    assert_errors_in_code(
        r#"
class A {
  void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 34, 4),
            ("invalid_reference_to_this", 34, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_primaryConstructor_assertInitializer`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn invalid_reference_to_this_class_primary_constructor_assert_initializer() {
    assert_errors_in_code(
        r#"
class A(int a) {
  this : assert(this.hashCode == 0);
}
"#,
        &[("invalid_reference_to_this", 34, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_primaryConstructor_defaultValue`.
#[test]
fn invalid_reference_to_this_class_primary_constructor_default_value() {
    assert_errors_in_code(
        r#"
class A([int p = this]) {}
"#,
        &[
            ("non_constant_default_value", 18, 4),
            ("invalid_reference_to_this", 18, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_primaryConstructor_fieldInitializer`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn invalid_reference_to_this_class_primary_constructor_field_initializer() {
    assert_errors_in_code(
        r#"
class A() {
  var f;
  this : f = this;
}
"#,
        &[("invalid_reference_to_this", 35, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_primaryConstructor_superInitializer`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn invalid_reference_to_this_class_primary_constructor_super_initializer() {
    assert_errors_in_code(
        r#"
class A(Object x);
class B() extends A {
  this : super(this);
}
"#,
        &[("invalid_reference_to_this", 57, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_secondaryConstructor_factory_body`.
#[test]
fn invalid_reference_to_this_class_secondary_constructor_factory_body() {
    assert_errors_in_code(
        r#"
class A {
  factory A() { return this; }
}
"#,
        &[("invalid_reference_to_this", 34, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_secondaryConstructor_factory_defaultValue`.
#[test]
fn invalid_reference_to_this_class_secondary_constructor_factory_default_value() {
    assert_errors_in_code(
        r#"
class A {
  factory A([Object p = this]) => throw 0;
}
"#,
        &[
            ("non_constant_default_value", 35, 4),
            ("invalid_reference_to_this", 35, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_secondaryConstructor_generative_assertInitializer`.
#[test]
fn invalid_reference_to_this_class_secondary_constructor_generative_assert_initializer() {
    assert_errors_in_code(
        r#"
class A {
  A() : assert(this.hashCode == 0);
}
"#,
        &[("invalid_reference_to_this", 26, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_secondaryConstructor_generative_defaultValue`.
#[test]
fn invalid_reference_to_this_class_secondary_constructor_generative_default_value() {
    assert_errors_in_code(
        r#"
class A {
  A([Object p = this]);
}
"#,
        &[
            ("non_constant_default_value", 27, 4),
            ("invalid_reference_to_this", 27, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_secondaryConstructor_generative_fieldInitializer`.
#[test]
fn invalid_reference_to_this_class_secondary_constructor_generative_field_initializer() {
    assert_errors_in_code(
        r#"
class A {
  var f;
  A() : f = this;
}
"#,
        &[("invalid_reference_to_this", 32, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_secondaryConstructor_generative_redirectingInitializer`.
#[test]
fn invalid_reference_to_this_class_secondary_constructor_generative_redirecting_initializer() {
    assert_errors_in_code(
        r#"
class A {
  A(Object x);
  A.named() : this(this);
}
"#,
        &[("invalid_reference_to_this", 45, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_secondaryConstructor_generative_superInitializer`.
#[test]
fn invalid_reference_to_this_class_secondary_constructor_generative_super_initializer() {
    assert_errors_in_code(
        r#"
class A {
  A(x) {}
}
class B extends A {
  B() : super(this);
}
"#,
        &[("invalid_reference_to_this", 57, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_staticField_initializer`.
#[test]
fn invalid_reference_to_this_class_static_field_initializer() {
    assert_errors_in_code(
        r#"
class A {
  static A f = this;
}
"#,
        &[("invalid_reference_to_this", 26, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_staticField_lateInitializer`.
#[test]
fn invalid_reference_to_this_class_static_field_late_initializer() {
    assert_errors_in_code(
        r#"
class A {
  static late var f = this;
}
"#,
        &[("invalid_reference_to_this", 33, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_class_staticMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_class_static_method_default_value() {
    assert_errors_in_code(
        r#"
class A {
  static void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 41, 4),
            ("invalid_reference_to_this", 41, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_instanceField_initializer`.
#[test]
fn invalid_reference_to_this_enum_instance_field_initializer() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  final f = this;
}
"#,
        &[("invalid_reference_to_this", 27, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_instanceField_lateInitializer`.
#[test]
fn invalid_reference_to_this_enum_instance_field_late_initializer() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  late final f = this;
}
"#,
        &[("late_final_field_with_const_constructor", 17, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_instanceMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_enum_instance_method_default_value() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 38, 4),
            ("invalid_reference_to_this", 38, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_primaryConstructor_assertInitializer`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn invalid_reference_to_this_enum_primary_constructor_assert_initializer() {
    assert_errors_in_code(
        r#"
enum E() {
  v;
  this : assert(this.hashCode == 0);
}
"#,
        &[
            ("invalid_constant", 33, 13),
            ("invalid_reference_to_this", 33, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_primaryConstructor_defaultValue`.
#[test]
fn invalid_reference_to_this_enum_primary_constructor_default_value() {
    assert_errors_in_code(
        r#"
enum E([int p = this]) {
  v;
}
"#,
        &[
            ("unused_element_parameter", 13, 1),
            ("non_constant_default_value", 17, 4),
            ("invalid_reference_to_this", 17, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_primaryConstructor_fieldInitializer`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn invalid_reference_to_this_enum_primary_constructor_field_initializer() {
    assert_errors_in_code(
        r#"
enum E() {
  v;
  final Object f;
  this : f = this;
}
"#,
        &[
            ("invalid_constant", 48, 4),
            ("invalid_reference_to_this", 48, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_secondaryConstructor_factory_defaultValue`.
#[test]
fn invalid_reference_to_this_enum_secondary_constructor_factory_default_value() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  factory E.named([Object p = this]) => throw 0;
}
"#,
        &[
            ("non_constant_default_value", 45, 4),
            ("invalid_reference_to_this", 45, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_secondaryConstructor_generative_assertInitializer`.
#[test]
fn invalid_reference_to_this_enum_secondary_constructor_generative_assert_initializer() {
    assert_errors_in_code(
        r#"
enum E {
  v.named();
  const E.named() : assert(this.hashCode == 0);
}
"#,
        &[
            ("invalid_constant", 50, 13),
            ("invalid_reference_to_this", 50, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_secondaryConstructor_generative_defaultValue`.
#[test]
fn invalid_reference_to_this_enum_secondary_constructor_generative_default_value() {
    assert_errors_in_code(
        r#"
enum E {
  v.named();
  const E.named([Object p = this]);
}
"#,
        &[
            ("unused_element_parameter", 47, 1),
            ("non_constant_default_value", 51, 4),
            ("invalid_reference_to_this", 51, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_secondaryConstructor_generative_fieldInitializer`.
#[test]
fn invalid_reference_to_this_enum_secondary_constructor_generative_field_initializer() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  final Object f;
  const E() : f = this;
}
"#,
        &[
            ("invalid_constant", 51, 4),
            ("invalid_reference_to_this", 51, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_secondaryConstructor_generative_redirectingInitializer`.
#[test]
fn invalid_reference_to_this_enum_secondary_constructor_generative_redirecting_initializer() {
    assert_errors_in_code(
        r#"
enum E {
  v.named();
  const E.named() : this(this);
  const E(Object o);
}
"#,
        &[
            ("invalid_constant", 48, 4),
            ("invalid_reference_to_this", 48, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_staticField_initializer`.
#[test]
fn invalid_reference_to_this_enum_static_field_initializer() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static var f = this;
}
"#,
        &[("invalid_reference_to_this", 32, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_staticField_lateInitializer`.
#[test]
fn invalid_reference_to_this_enum_static_field_late_initializer() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static late final f = this;
}
"#,
        &[("invalid_reference_to_this", 39, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_enum_staticMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_enum_static_method_default_value() {
    assert_errors_in_code(
        r#"
enum E {
  v;
  static void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 45, 4),
            ("invalid_reference_to_this", 45, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extension_instanceMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_extension_instance_method_default_value() {
    assert_errors_in_code(
        r#"
extension E on int {
  void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 45, 4),
            ("invalid_reference_to_this", 45, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extension_staticField_initializer`.
#[test]
fn invalid_reference_to_this_extension_static_field_initializer() {
    assert_errors_in_code(
        r#"
extension E on int {
  static var f = this;
}
"#,
        &[("invalid_reference_to_this", 39, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extension_staticField_lateInitializer`.
#[test]
fn invalid_reference_to_this_extension_static_field_late_initializer() {
    assert_errors_in_code(
        r#"
extension E on int {
  static late var f = this;
}
"#,
        &[("invalid_reference_to_this", 44, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extension_staticMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_extension_static_method_default_value() {
    assert_errors_in_code(
        r#"
extension E on int {
  static void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 52, 4),
            ("invalid_reference_to_this", 52, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_instanceMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_extension_type_instance_method_default_value() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 51, 4),
            ("invalid_assignment", 51, 4),
            ("invalid_reference_to_this", 51, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_primaryConstructor_assertInitializer`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn invalid_reference_to_this_extension_type_primary_constructor_assert_initializer() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  this : assert(this.hashCode == 0);
}
"#,
        &[("invalid_reference_to_this", 44, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_primaryConstructor_defaultValue`.
#[test]
fn invalid_reference_to_this_extension_type_primary_constructor_default_value() {
    assert_errors_in_code(
        r#"
extension type E([int it = this]) {}
"#,
        &[
            ("non_constant_default_value", 28, 4),
            ("invalid_reference_to_this", 28, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_primaryConstructor_fieldInitializer`.
#[test]
#[ignore = "primary constructors experiment: not at parity"]
fn invalid_reference_to_this_extension_type_primary_constructor_field_initializer() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  this : it = this.hashCode;
}
"#,
        &[
            ("field_initialized_in_parameter_and_initializer", 37, 2),
            ("invalid_reference_to_this", 42, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_secondaryConstructor_factory_defaultValue`.
#[test]
fn invalid_reference_to_this_extension_type_secondary_constructor_factory_default_value() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  factory E.named([Object p = this]) => throw 0;
}
"#,
        &[
            ("non_constant_default_value", 58, 4),
            ("invalid_assignment", 58, 4),
            ("invalid_reference_to_this", 58, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_secondaryConstructor_generative_assertInitializer`.
#[test]
fn invalid_reference_to_this_extension_type_secondary_constructor_generative_assert_initializer() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named() : it = 0, assert(this.hashCode == 0);
}
"#,
        &[("invalid_reference_to_this", 57, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_secondaryConstructor_generative_defaultValue`.
#[test]
fn invalid_reference_to_this_extension_type_secondary_constructor_generative_default_value() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named([Object p = this]) : it = 0;
}
"#,
        &[
            ("non_constant_default_value", 50, 4),
            ("invalid_assignment", 50, 4),
            ("invalid_reference_to_this", 50, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_secondaryConstructor_generative_fieldInitializer`.
#[test]
fn invalid_reference_to_this_extension_type_secondary_constructor_generative_field_initializer() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named() : it = this.hashCode;
}
"#,
        &[("invalid_reference_to_this", 47, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_secondaryConstructor_generative_redirectingInitializer`.
#[test]
fn invalid_reference_to_this_extension_type_secondary_constructor_generative_redirecting_initializer()
 {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  E.named() : this(this.hashCode);
}
"#,
        &[("invalid_reference_to_this", 47, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_staticField_initializer`.
#[test]
fn invalid_reference_to_this_extension_type_static_field_initializer() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  static var f = this;
}
"#,
        &[("invalid_reference_to_this", 45, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_staticField_lateInitializer`.
#[test]
fn invalid_reference_to_this_extension_type_static_field_late_initializer() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  static late var f = this;
}
"#,
        &[("invalid_reference_to_this", 50, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_extensionType_staticMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_extension_type_static_method_default_value() {
    assert_errors_in_code(
        r#"
extension type E(int it) {
  static void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 58, 4),
            ("invalid_assignment", 58, 4),
            ("invalid_reference_to_this", 58, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_mixin_instanceField_initializer`.
#[test]
fn invalid_reference_to_this_mixin_instance_field_initializer() {
    assert_errors_in_code(
        r#"
mixin M {
  var f = this;
}
"#,
        &[("invalid_reference_to_this", 21, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_mixin_instanceField_lateInitializer`.
#[test]
fn invalid_reference_to_this_mixin_instance_field_late_initializer() {
    assert_errors_in_code(
        r#"
mixin A {
  late var f = this;
}
"#,
        &[],
    );
}

/// `invalid_reference_to_this_test.dart` `test_mixin_instanceMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_mixin_instance_method_default_value() {
    assert_errors_in_code(
        r#"
mixin M {
  void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 34, 4),
            ("invalid_reference_to_this", 34, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_mixin_staticField_initializer`.
#[test]
fn invalid_reference_to_this_mixin_static_field_initializer() {
    assert_errors_in_code(
        r#"
mixin M {
  static var f = this;
}
"#,
        &[("invalid_reference_to_this", 28, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_mixin_staticField_lateInitializer`.
#[test]
fn invalid_reference_to_this_mixin_static_field_late_initializer() {
    assert_errors_in_code(
        r#"
mixin M {
  static late var f = this;
}
"#,
        &[("invalid_reference_to_this", 33, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_mixin_staticMethod_defaultValue`.
#[test]
fn invalid_reference_to_this_mixin_static_method_default_value() {
    assert_errors_in_code(
        r#"
mixin M {
  static void foo([Object p = this]) {}
}
"#,
        &[
            ("non_constant_default_value", 41, 4),
            ("invalid_reference_to_this", 41, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_topLevelFunction__body`.
#[test]
fn invalid_reference_to_this_top_level_function_body() {
    assert_errors_in_code(
        r#"
void f() {
  this;
}
"#,
        &[("invalid_reference_to_this", 14, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_topLevelFunction__defaultValue`.
#[test]
fn invalid_reference_to_this_top_level_function_default_value() {
    assert_errors_in_code(
        r#"
void f([Object p = this]) {}
"#,
        &[
            ("non_constant_default_value", 20, 4),
            ("invalid_reference_to_this", 20, 4),
        ],
    );
}

/// `invalid_reference_to_this_test.dart` `test_topLevelGetter_body`.
#[test]
fn invalid_reference_to_this_top_level_getter_body() {
    assert_errors_in_code(
        r#"
int get f {
  this;
  return 0;
}
"#,
        &[("invalid_reference_to_this", 15, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_topLevelSetter_body`.
#[test]
fn invalid_reference_to_this_top_level_setter_body() {
    assert_errors_in_code(
        r#"
set f(int _) {
  this;
}
"#,
        &[("invalid_reference_to_this", 18, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_topLevelVariable_initializer`.
#[test]
fn invalid_reference_to_this_top_level_variable_initializer() {
    assert_errors_in_code(
        r#"
int f = this;
"#,
        &[("invalid_reference_to_this", 9, 4)],
    );
}

/// `invalid_reference_to_this_test.dart` `test_topLevelVariable_lateInitializer`.
#[test]
fn invalid_reference_to_this_top_level_variable_late_initializer() {
    assert_errors_in_code(
        r#"
late var f = this;
"#,
        &[("invalid_reference_to_this", 14, 4)],
    );
}

/// `native_function_body_in_non_sdk_code_test.dart` `test_function`.
#[test]
fn native_function_body_in_non_sdk_code_function() {
    assert_errors_in_code(
        r#"
int m(a) native 'string';
"#,
        &[("native_function_body_in_non_sdk_code", 10, 16)],
    );
}

/// `native_function_body_in_non_sdk_code_test.dart` `test_method`.
#[test]
fn native_function_body_in_non_sdk_code_method() {
    assert_errors_in_code(
        r#"
class A {
  static int m(a) native 'string';
}
"#,
        &[("native_function_body_in_non_sdk_code", 29, 16)],
    );
}

/// `native_function_body_in_non_sdk_code_test.dart` `test_mixinMethod`.
#[test]
fn native_function_body_in_non_sdk_code_mixin_method() {
    assert_errors_in_code(
        r#"
mixin A {
  static int m(a) native 'string';
}
"#,
        &[("native_function_body_in_non_sdk_code", 29, 16)],
    );
}

/// `non_bool_condition_test.dart` `test_conditional`.
#[test]
fn non_bool_condition_conditional() {
    assert_errors_in_code(
        r#"
f() { return 3 ? 2 : 1; }
"#,
        &[("non_bool_condition", 14, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_conditional_fromLiteral`.
#[test]
fn non_bool_condition_conditional_from_literal() {
    assert_errors_in_code(
        r#"
f() { return [1, 2, 3] ? 2 : 1; }
"#,
        &[("non_bool_condition", 14, 9)],
    );
}

/// `non_bool_condition_test.dart` `test_conditional_fromSupertype`.
#[test]
fn non_bool_condition_conditional_from_supertype() {
    assert_errors_in_code(
        r#"
f(Object o) { return o ? 2 : 1; }
"#,
        &[("non_bool_condition", 22, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_const_list_ifElement`.
#[test]
fn non_bool_condition_const_list_if_element() {
    assert_errors_in_code(
        r#"
const dynamic c = 2;
const x = [1, if (c) 2 else 3, 4];
"#,
        &[("non_bool_condition", 40, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_const_list_ifElement_static`.
#[test]
fn non_bool_condition_const_list_if_element_static() {
    assert_errors_in_code(
        r#"
const x = [1, if (1) 2 else 3, 4];
"#,
        &[("non_bool_condition", 19, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_do`.
#[test]
fn non_bool_condition_do() {
    assert_errors_in_code(
        r#"
f() {
  do {} while (3);
}
"#,
        &[("non_bool_condition", 22, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_do_fromLiteral`.
#[test]
fn non_bool_condition_do_from_literal() {
    assert_errors_in_code(
        r#"
f(Object o) {
  do {} while ([1, 2, 3]);
}
"#,
        &[("non_bool_condition", 30, 9)],
    );
}

/// `non_bool_condition_test.dart` `test_do_fromSupertype`.
#[test]
fn non_bool_condition_do_from_supertype() {
    assert_errors_in_code(
        r#"
f(Object o) {
  do {} while (o);
}
"#,
        &[("non_bool_condition", 30, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_for`.
#[test]
fn non_bool_condition_for() {
    assert_errors_in_code(
        r#"
f() {
  for (;3;) {}
}
"#,
        &[("non_bool_condition", 15, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_for_declaration`.
#[test]
fn non_bool_condition_for_declaration() {
    assert_errors_in_code(
        r#"
f() {
  for (int i = 0; 3;) {}
}
"#,
        &[
            ("unused_local_variable", 18, 1),
            ("non_bool_condition", 25, 1),
        ],
    );
}

/// `non_bool_condition_test.dart` `test_for_expression`.
#[test]
fn non_bool_condition_for_expression() {
    assert_errors_in_code(
        r#"
f() {
  int i;
  for (i = 0; 3;) {}
}"#,
        &[
            ("unused_local_variable", 13, 1),
            ("non_bool_condition", 30, 1),
        ],
    );
}

/// `non_bool_condition_test.dart` `test_for_fromLiteral`.
#[test]
fn non_bool_condition_for_from_literal() {
    assert_errors_in_code(
        r#"
f() {
  for (;[1, 2, 3];) {}
}
"#,
        &[("non_bool_condition", 15, 9)],
    );
}

/// `non_bool_condition_test.dart` `test_for_fromSupertype`.
#[test]
fn non_bool_condition_for_from_supertype() {
    assert_errors_in_code(
        r#"
f(Object o) {
  for (;o;) {}
}
"#,
        &[("non_bool_condition", 23, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_forElement`.
#[test]
fn non_bool_condition_for_element() {
    assert_errors_in_code(
        r#"
var v = [for (; 0;) 1];
"#,
        &[("non_bool_condition", 17, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_guardedPattern_whenClause`.
#[test]
fn non_bool_condition_guarded_pattern_when_clause() {
    assert_errors_in_code(
        r#"
void f() {
  if (0 case _ when 1) {}
}
"#,
        &[("non_bool_condition", 32, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_if`.
#[test]
fn non_bool_condition_if() {
    assert_errors_in_code(
        r#"
f() {
  if (3) return 2; else return 1;
}
"#,
        &[("non_bool_condition", 13, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_if_fromLiteral`.
#[test]
fn non_bool_condition_if_from_literal() {
    assert_errors_in_code(
        r#"
f() {
  if ([1, 2, 3]) return 2; else return 1;
}
"#,
        &[("non_bool_condition", 13, 9)],
    );
}

/// `non_bool_condition_test.dart` `test_if_fromSupertype`.
#[test]
fn non_bool_condition_if_from_supertype() {
    assert_errors_in_code(
        r#"
f(Object o) {
  if (o) return 2; else return 1;
}
"#,
        &[("non_bool_condition", 21, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_if_map`.
#[test]
fn non_bool_condition_if_map() {
    assert_errors_in_code(
        r#"
const dynamic nonBool = null;
const c = const {if (nonBool) 'a' : 1};
"#,
        &[("non_bool_condition", 52, 7)],
    );
}

/// `non_bool_condition_test.dart` `test_if_null`.
#[test]
fn non_bool_condition_if_null() {
    assert_errors_in_code(
        r#"
void f(Null a) {
  if (a) {}
}
"#,
        &[("non_bool_condition", 24, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_if_set`.
#[test]
fn non_bool_condition_if_set() {
    assert_errors_in_code(
        r#"
const dynamic nonBool = 'a';
const c = const {if (nonBool) 3};
"#,
        &[("non_bool_condition", 51, 7)],
    );
}

/// `non_bool_condition_test.dart` `test_ifElement`.
#[test]
fn non_bool_condition_if_element() {
    assert_errors_in_code(
        r#"
var v = [if (3) 1];
"#,
        &[("non_bool_condition", 14, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_ifElement_fromLiteral`.
#[test]
fn non_bool_condition_if_element_from_literal() {
    assert_errors_in_code(
        r#"
var v = [if ([1, 2, 3]) 'x'];
"#,
        &[("non_bool_condition", 14, 9)],
    );
}

/// `non_bool_condition_test.dart` `test_ifElement_fromSupertype`.
#[test]
fn non_bool_condition_if_element_from_supertype() {
    assert_errors_in_code(
        r#"
final o = Object();
var v = [if (o) 'x'];
"#,
        &[("non_bool_condition", 34, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_ternary_condition_null`.
#[test]
fn non_bool_condition_ternary_condition_null() {
    assert_errors_in_code(
        r#"
void f(Null a) {
  a ? 0 : 1;
}
"#,
        &[("non_bool_condition", 20, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_while`.
#[test]
fn non_bool_condition_while() {
    assert_errors_in_code(
        r#"
f() {
  while (3) {}
}
"#,
        &[("non_bool_condition", 16, 1)],
    );
}

/// `non_bool_condition_test.dart` `test_while_fromLiteral`.
#[test]
fn non_bool_condition_while_from_literal() {
    assert_errors_in_code(
        r#"
f() {
  while ([1, 2, 3]) {}
}
"#,
        &[("non_bool_condition", 16, 9)],
    );
}

/// `non_bool_condition_test.dart` `test_while_fromSupertype`.
#[test]
fn non_bool_condition_while_from_supertype() {
    assert_errors_in_code(
        r#"
f(Object o) {
  while (o) {}
}
"#,
        &[("non_bool_condition", 24, 1)],
    );
}

/// `non_bool_negation_expression_test.dart` `test_nonBool`.
#[test]
fn non_bool_negation_expression_non_bool() {
    assert_errors_in_code(
        r#"
f() {
  !42;
}
"#,
        &[("non_bool_negation_expression", 10, 2)],
    );
}

/// `non_bool_negation_expression_test.dart` `test_nonBool_fromLiteral`.
#[test]
fn non_bool_negation_expression_non_bool_from_literal() {
    assert_errors_in_code(
        r#"
f() {
  ![1, 2, 3];
}
"#,
        &[("non_bool_negation_expression", 10, 9)],
    );
}

/// `non_bool_negation_expression_test.dart` `test_nonBool_fromSupertype`.
#[test]
fn non_bool_negation_expression_non_bool_from_supertype() {
    assert_errors_in_code(
        r#"
f(Object o) {
  !o;
}
"#,
        &[("non_bool_negation_expression", 18, 1)],
    );
}

/// `non_bool_negation_expression_test.dart` `test_null`.
#[test]
fn non_bool_negation_expression_null() {
    assert_errors_in_code(
        r#"
void m(Null x) {
  !x;
}
"#,
        &[("non_bool_negation_expression", 21, 1)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_argument_invalid`.
#[test]
fn record_literal_one_positional_no_trailing_comma_argument_invalid() {
    assert_errors_in_code(
        r#"
void f((int,) i) {
  f((''));
}
"#,
        &[("argument_type_not_assignable", 25, 2)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_argument_notParenthesized`.
#[test]
fn record_literal_one_positional_no_trailing_comma_argument_not_parenthesized() {
    assert_errors_in_code(
        r#"
void f((int,) i) {
  f(1);
}
"#,
        &[("argument_type_not_assignable", 24, 1)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_argument_parenthesized`.
#[test]
fn record_literal_one_positional_no_trailing_comma_argument_parenthesized() {
    assert_errors_in_code(
        r#"
void f((int,) i) {
  f((1));
}
"#,
        &[("record_literal_one_positional_no_trailing_comma", 24, 3)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_argument_valid`.
#[test]
fn record_literal_one_positional_no_trailing_comma_argument_valid() {
    assert_errors_in_code(
        r#"
void f((int,) i) {
  f((1,));
}
"#,
        &[],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_assignment_invalid`.
#[test]
fn record_literal_one_positional_no_trailing_comma_assignment_invalid() {
    assert_errors_in_code(
        r#"
void f((int,) r) {
  r = ('');
}
"#,
        &[("invalid_assignment", 26, 4)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_assignment_notParenthesized`.
#[test]
fn record_literal_one_positional_no_trailing_comma_assignment_not_parenthesized() {
    assert_errors_in_code(
        r#"
void f((int,) r) {
  r = 1;
}
"#,
        &[("invalid_assignment", 26, 1)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_assignment_parenthesized`.
#[test]
fn record_literal_one_positional_no_trailing_comma_assignment_parenthesized() {
    assert_errors_in_code(
        r#"
void f((int,) r) {
  r = (1);
}
"#,
        &[("record_literal_one_positional_no_trailing_comma", 26, 3)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_assignment_valid`.
#[test]
fn record_literal_one_positional_no_trailing_comma_assignment_valid() {
    assert_errors_in_code(
        r#"
void f((int,) r) {
  r = (1,);
}
"#,
        &[],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_declaration`.
#[test]
fn record_literal_one_positional_no_trailing_comma_declaration() {
    assert_errors_in_code(
        r#"
(int,) r = (1);
"#,
        &[("record_literal_one_positional_no_trailing_comma", 12, 3)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_declaration_invalid`.
#[test]
fn record_literal_one_positional_no_trailing_comma_declaration_invalid() {
    assert_errors_in_code(
        r#"
(int,) r = ('');
"#,
        &[("invalid_assignment", 13, 2)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_declaration_valid`.
#[test]
fn record_literal_one_positional_no_trailing_comma_declaration_valid() {
    assert_errors_in_code(
        r#"
(int,) r = (1,);
"#,
        &[],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_return_blockBody_notParenthesized`.
#[test]
fn record_literal_one_positional_no_trailing_comma_return_block_body_not_parenthesized() {
    assert_errors_in_code(
        r#"
(int,) f() {
  return 1;
}
"#,
        &[("return_of_invalid_type", 23, 1)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_return_blockBody_parenthesized`.
#[test]
#[ignore = "reported by ReturnTypeVerifier (error/*, branch wd-errors)"]
fn record_literal_one_positional_no_trailing_comma_return_block_body_parenthesized() {
    assert_errors_in_code(
        r#"
(int,) f() {
  return (1);
}
"#,
        &[("record_literal_one_positional_no_trailing_comma", 23, 3)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_return_expressionBody_invalid`.
#[test]
fn record_literal_one_positional_no_trailing_comma_return_expression_body_invalid() {
    assert_errors_in_code(
        r#"
(int,) f() => ('');
"#,
        &[("return_of_invalid_type", 15, 4)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_return_expressionBody_notParenthesized`.
#[test]
fn record_literal_one_positional_no_trailing_comma_return_expression_body_not_parenthesized() {
    assert_errors_in_code(
        r#"
(int,) f() => 1;
"#,
        &[("return_of_invalid_type", 15, 1)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_return_expressionBody_parenthesized`.
#[test]
#[ignore = "reported by ReturnTypeVerifier (error/*, branch wd-errors)"]
fn record_literal_one_positional_no_trailing_comma_return_expression_body_parenthesized() {
    assert_errors_in_code(
        r#"
(int,) f() => (1);
"#,
        &[("record_literal_one_positional_no_trailing_comma", 15, 3)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_return_invalid`.
#[test]
fn record_literal_one_positional_no_trailing_comma_return_invalid() {
    assert_errors_in_code(
        r#"
(int,) f() { return (''); }
"#,
        &[("return_of_invalid_type", 21, 4)],
    );
}

/// `record_literal_one_positional_no_trailing_comma_test.dart` `test_return_valid`.
#[test]
fn record_literal_one_positional_no_trailing_comma_return_valid() {
    assert_errors_in_code(
        r#"
(int,) f() { return (1,); }
"#,
        &[],
    );
}

/// `rethrow_outside_catch_test.dart` `test_withoutCatch`.
#[test]
fn rethrow_outside_catch_without_catch() {
    assert_errors_in_code(
        r#"
void f() {
  rethrow;
}
"#,
        &[("rethrow_outside_catch", 14, 7)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_annotation`.
#[test]
fn static_access_to_instance_member_annotation() {
    assert_errors_in_code(
        r#"
class A {
  const A.name();
}
@A.name()
main() {
}
"#,
        &[],
    );
}

/// `static_access_to_instance_member_test.dart` `test_extension_getter`.
#[test]
fn static_access_to_instance_member_extension_getter() {
    assert_errors_in_code(
        r#"
extension E on int {
  int get g => 0;
}
f() {
  E.g;
}
"#,
        &[("static_access_to_instance_member", 52, 1)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_extension_method`.
#[test]
fn static_access_to_instance_member_extension_method() {
    assert_errors_in_code(
        r#"
extension E on int {
  void m() {}
}
f() {
  E.m();
}
"#,
        &[("static_access_to_instance_member", 48, 1)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_extension_setter`.
#[test]
fn static_access_to_instance_member_extension_setter() {
    assert_errors_in_code(
        r#"
extension E on int {
  void set s(int i) {}
}
f() {
  E.s = 2;
}
"#,
        &[("static_access_to_instance_member", 57, 1)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_method_invocation`.
#[test]
fn static_access_to_instance_member_method_invocation() {
    assert_errors_in_code(
        r#"
class A {
  m() {}
}
main() {
  A.m();
}"#,
        &[("static_access_to_instance_member", 35, 1)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_method_reference`.
#[test]
fn static_access_to_instance_member_method_reference() {
    assert_errors_in_code(
        r#"
class A {
  m() {}
}
main() {
  A.m;
}"#,
        &[("static_access_to_instance_member", 35, 1)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_propertyAccess_field`.
#[test]
fn static_access_to_instance_member_property_access_field() {
    assert_errors_in_code(
        r#"
class A {
  var f;
}
main() {
  A.f;
}"#,
        &[("static_access_to_instance_member", 35, 1)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_propertyAccess_field_toplevel_generic`.
#[test]
fn static_access_to_instance_member_property_access_field_toplevel_generic() {
    assert_errors_in_code(
        r#"
class C<T> {
  List<T> t = [];
}
var x = C.t;
"#,
        &[("static_access_to_instance_member", 44, 1)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_propertyAccess_getter`.
#[test]
fn static_access_to_instance_member_property_access_getter() {
    assert_errors_in_code(
        r#"
class A {
  get f => 42;
}
main() {
  A.f;
}"#,
        &[("static_access_to_instance_member", 41, 1)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_propertyAccess_setter`.
#[test]
fn static_access_to_instance_member_property_access_setter() {
    assert_errors_in_code(
        r#"
class A {
  set f(x) {}
}
main() {
  A.f = 42;
}"#,
        &[("static_access_to_instance_member", 40, 1)],
    );
}

/// `static_access_to_instance_member_test.dart` `test_static_method`.
#[test]
fn static_access_to_instance_member_static_method() {
    assert_errors_in_code(
        r#"
class A {
  static m() {}
}
main() {
  A.m;
  A.m();
}
"#,
        &[],
    );
}

/// `static_access_to_instance_member_test.dart` `test_static_propertyAccess_field`.
#[test]
fn static_access_to_instance_member_static_property_access_field() {
    assert_errors_in_code(
        r#"
class A {
  static var f;
}
main() {
  A.f;
  A.f = 1;
}
"#,
        &[],
    );
}

/// `static_access_to_instance_member_test.dart` `test_static_propertyAccess_propertyAccessor`.
#[test]
fn static_access_to_instance_member_static_property_access_property_accessor() {
    assert_errors_in_code(
        r#"
class A {
  static get f => 42;
  static set f(x) {}
}
main() {
  A.f;
  A.f = 1;
}
"#,
        &[],
    );
}

/// `throw_of_invalid_type_test.dart` `test_dynamic`.
#[test]
fn throw_of_invalid_type_dynamic() {
    assert_errors_in_code(
        r#"
f(dynamic a) {
  throw a;
}
"#,
        &[],
    );
}

/// `throw_of_invalid_type_test.dart` `test_nonNullable`.
#[test]
fn throw_of_invalid_type_non_nullable() {
    assert_errors_in_code(
        r#"
f(int a) {
  throw a;
}
"#,
        &[],
    );
}

/// `throw_of_invalid_type_test.dart` `test_nullable`.
#[test]
fn throw_of_invalid_type_nullable() {
    assert_errors_in_code(
        r#"
f(int? a) {
  throw a;
}
"#,
        &[("throw_of_invalid_type", 21, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_argumentList_argument_parameterTypeDynamic_error`.
#[test]
fn use_of_void_result_argument_list_argument_parameter_type_dynamic_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  g(x);
}
void g(dynamic x) { }
"#,
        &[("use_of_void_result", 22, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_argumentList_argument_parameterTypeVoid_ok`.
#[test]
fn use_of_void_result_argument_list_argument_parameter_type_void_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  g(x);
}
void g(void x) {}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_asExpression_expression_ok`.
#[test]
fn use_of_void_result_as_expression_expression_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  use(x as int);
}

void use(Object? x) {}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_assignmentExpression_compound_read_error`.
#[test]
fn use_of_void_result_assignment_expression_compound_read_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x += 1;
}
"#,
        &[("use_of_void_result", 22, 2)],
    );
}

/// `use_of_void_result_test.dart` `test_assignmentExpression_simple_propertyAccess_target_error`.
#[test]
fn use_of_void_result_assignment_expression_simple_property_access_target_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x.foo = null;
}
"#,
        &[("use_of_void_result", 22, 3)],
    );
}

/// `use_of_void_result_test.dart` `test_awaitExpression_expression_error`.
#[test]
fn use_of_void_result_await_expression_expression_error() {
    assert_errors_in_code(
        r#"
void f(void x) async {
  await x;
}
"#,
        &[("use_of_void_result", 32, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_awaitForIn_streamElementTypeVoid_variableTypeNonVoid_error`.
#[test]
fn use_of_void_result_await_for_in_stream_element_type_void_variable_type_non_void_error() {
    assert_errors_in_code(
        r#"
void f(Stream<void> values) async {
  await for (Object? _ in values) {}
  await for (dynamic _ in values) {}
}
"#,
        &[
            ("use_of_void_result", 63, 6),
            ("use_of_void_result", 100, 6),
        ],
    );
}

/// `use_of_void_result_test.dart` `test_awaitForIn_streamElementTypeVoid_variableTypeVoidOrInferred_ok`.
#[test]
fn use_of_void_result_await_for_in_stream_element_type_void_variable_type_void_or_inferred_ok() {
    assert_errors_in_code(
        r#"
void f(Stream<void> values) async {
  await for (void _ in values) {}
  await for (var _ in values) {}
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_binaryExpression_ifNull_leftOperand_error`.
#[test]
fn use_of_void_result_binary_expression_if_null_left_operand_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x ?? 1;
}
"#,
        &[("use_of_void_result", 20, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_binaryExpression_ifNull_rightOperand_ok`.
#[test]
fn use_of_void_result_binary_expression_if_null_right_operand_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  null ?? x;
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_binaryExpression_logicalAnd_leftOperand_error`.
#[test]
fn use_of_void_result_binary_expression_logical_and_left_operand_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x && true;
}
"#,
        &[("use_of_void_result", 20, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_binaryExpression_logicalAnd_rightOperand_error`.
#[test]
fn use_of_void_result_binary_expression_logical_and_right_operand_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  true && x;
}
"#,
        &[("use_of_void_result", 28, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_binaryExpression_logicalOr_leftOperand_error`.
#[test]
fn use_of_void_result_binary_expression_logical_or_left_operand_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x || true;
}
"#,
        &[("use_of_void_result", 20, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_binaryExpression_logicalOr_rightOperand_error`.
#[test]
fn use_of_void_result_binary_expression_logical_or_right_operand_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  false || x;
}
"#,
        &[("use_of_void_result", 29, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_conditionalExpression_condition_error`.
#[test]
fn use_of_void_result_conditional_expression_condition_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x ? null : null;
}
"#,
        &[("use_of_void_result", 20, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_conditionalExpression_elseExpression_ok`.
#[test]
fn use_of_void_result_conditional_expression_else_expression_ok() {
    assert_errors_in_code(
        r#"
void f(bool c, void x) {
  c ? null : x;
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_conditionalExpression_thenExpression_ok`.
#[test]
fn use_of_void_result_conditional_expression_then_expression_ok() {
    assert_errors_in_code(
        r#"
void f(bool c, void x) {
  c ? x : null;
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_constructorFieldInitializer_fieldTypeDynamic_error`.
#[test]
fn use_of_void_result_constructor_field_initializer_field_type_dynamic_error() {
    assert_errors_in_code(
        r#"
class A {
  dynamic f;
  A(void x) : f = x;
}
"#,
        &[("use_of_void_result", 42, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_constructorFieldInitializer_fieldTypeVoid_ok`.
#[test]
fn use_of_void_result_constructor_field_initializer_field_type_void_ok() {
    assert_errors_in_code(
        r#"
class A {
  void f;
  A(void x) : f = x;
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_doStatement_condition_error`.
#[test]
fn use_of_void_result_do_statement_condition_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  do {} while (x);
}
"#,
        &[("use_of_void_result", 33, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_expressionStatement_ok`.
#[test]
fn use_of_void_result_expression_statement_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x;
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_extensionOverride_argument_error`.
#[test]
fn use_of_void_result_extension_override_argument_error() {
    assert_errors_in_code(
        r#"
extension E on String {
  int get g => 0;
}

void f() {}

void h() {
  E(f()).g;
}
"#,
        &[("use_of_void_result", 74, 3)],
    );
}

/// `use_of_void_result_test.dart` `test_forIn_iterable_typeVoid_declaredVariable_error`.
#[test]
fn use_of_void_result_for_in_iterable_type_void_declared_variable_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  for (var v in x) {}
}
"#,
        &[
            ("unused_local_variable", 29, 1),
            ("unchecked_use_of_nullable_value", 34, 1),
            ("use_of_void_result", 34, 1),
        ],
    );
}

/// `use_of_void_result_test.dart` `test_forIn_iterable_typeVoid_error`.
#[test]
fn use_of_void_result_for_in_iterable_type_void_error() {
    assert_errors_in_code(
        r#"
void f(void x, y) {
  for (y in x) {}
}
"#,
        &[
            ("unchecked_use_of_nullable_value", 33, 1),
            ("use_of_void_result", 33, 1),
        ],
    );
}

/// `use_of_void_result_test.dart` `test_forIn_iterableElementTypeVoid_declaredVariableTypeNonVoid_error`.
#[test]
fn use_of_void_result_for_in_iterable_element_type_void_declared_variable_type_non_void_error() {
    assert_errors_in_code(
        r#"
void f(List<void> values) {
  for (Object? _ in values) {}
  for (dynamic _ in values) {}
}
"#,
        &[("use_of_void_result", 49, 6), ("use_of_void_result", 80, 6)],
    );
}

/// `use_of_void_result_test.dart` `test_forIn_iterableElementTypeVoid_declaredVariableTypeVoidOrInferred_ok`.
#[test]
fn use_of_void_result_for_in_iterable_element_type_void_declared_variable_type_void_or_inferred_ok()
{
    assert_errors_in_code(
        r#"
void f(List<void> values) {
  for (void _ in values) {}
  for (var _ in values) {}
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_forIn_loopVariable_typeVoid_error`.
#[test]
#[ignore = "not at parity yet (open point of the D4-D7 port)"]
fn use_of_void_result_for_in_loop_variable_type_void_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  for (x in [1, 2]) {}
}
"#,
        &[("use_of_void_result", 25, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_forPartsWithExpression_initializationAndUpdaters_ok`.
#[test]
fn use_of_void_result_for_parts_with_expression_initialization_and_updaters_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  for (x; true; x) {}
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_indexExpression_index_error`.
#[test]
fn use_of_void_result_index_expression_index_error() {
    assert_errors_in_code(
        r#"
void f(List list, void x) {
  list[x];
}
"#,
        &[("use_of_void_result", 36, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_indexExpression_index_inAssignment_error`.
#[test]
fn use_of_void_result_index_expression_index_in_assignment_error() {
    assert_errors_in_code(
        r#"
void f(List list, void x) {
  list[x] = null;
}
"#,
        &[("use_of_void_result", 36, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_indexExpression_target_error`.
#[test]
fn use_of_void_result_index_expression_target_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x[0];
}
"#,
        &[("use_of_void_result", 21, 3)],
    );
}

/// `use_of_void_result_test.dart` `test_interpolationExpression_expression_error`.
#[test]
fn use_of_void_result_interpolation_expression_expression_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  "$x";
}
"#,
        &[("use_of_void_result", 22, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_isExpression_expression_error`.
#[test]
fn use_of_void_result_is_expression_expression_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x is int;
}
"#,
        &[("use_of_void_result", 20, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_listLiteral_topLevelElement_toDynamic_error`.
#[test]
#[ignore = "reported by LiteralElementVerifier (error/*, branch wd-errors)"]
fn use_of_void_result_list_literal_top_level_element_to_dynamic_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  <dynamic>[x];
}
"#,
        &[("use_of_void_result", 30, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_listLiteral_topLevelElement_toVoid_ok`.
#[test]
fn use_of_void_result_list_literal_top_level_element_to_void_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  [x];
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_mapLiteral_topLevelKey_toDynamic_error`.
#[test]
#[ignore = "reported by LiteralElementVerifier (error/*, branch wd-errors)"]
fn use_of_void_result_map_literal_top_level_key_to_dynamic_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  <dynamic, int>{x : 4};
}
"#,
        &[("use_of_void_result", 35, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_mapLiteral_topLevelKey_toVoid_ok`.
#[test]
fn use_of_void_result_map_literal_top_level_key_to_void_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  ({x : 4});
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_mapLiteral_topLevelValue_toDynamic_error`.
#[test]
#[ignore = "reported by LiteralElementVerifier (error/*, branch wd-errors)"]
fn use_of_void_result_map_literal_top_level_value_to_dynamic_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  <int, dynamic>{4: x};
}
"#,
        &[("use_of_void_result", 38, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_mapLiteral_topLevelValue_toVoid_ok`.
#[test]
fn use_of_void_result_map_literal_top_level_value_to_void_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  ({4: x});
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_mapLiteralEntry_keyQuestion_keyTypeVoid_error`.
#[test]
#[ignore = "reported by LiteralElementVerifier (error/*, branch wd-errors)"]
fn use_of_void_result_map_literal_entry_key_question_key_type_void_error() {
    assert_errors_in_code(
        r#"
void f(void key) {
  <void, int>{?key: 0};
}
"#,
        &[("use_of_void_result", 35, 3)],
    );
}

/// `use_of_void_result_test.dart` `test_mapLiteralEntry_keyQuestion_valueTypeVoid_ok`.
#[test]
fn use_of_void_result_map_literal_entry_key_question_value_type_void_ok() {
    assert_errors_in_code(
        r#"
void f(int? key, void value) {
  <int, void>{?key: value};
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_mapLiteralEntry_valueQuestion_keyTypeVoid_ok`.
#[test]
fn use_of_void_result_map_literal_entry_value_question_key_type_void_ok() {
    assert_errors_in_code(
        r#"
void f(void key, int? value) {
  <void, int>{key: ?value};
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_mapLiteralEntry_valueQuestion_valueTypeVoid_error`.
#[test]
#[ignore = "reported by LiteralElementVerifier (error/*, branch wd-errors)"]
fn use_of_void_result_map_literal_entry_value_question_value_type_void_error() {
    assert_errors_in_code(
        r#"
void f(void value) {
  <int, void>{0: ?value};
}
"#,
        &[("use_of_void_result", 40, 5)],
    );
}

/// `use_of_void_result_test.dart` `test_nullAwareElement_list_error`.
#[test]
#[ignore = "reported by LiteralElementVerifier (error/*, branch wd-errors)"]
fn use_of_void_result_null_aware_element_list_error() {
    assert_errors_in_code(
        r#"
void f(void value) {
  <void>[?value];
}
"#,
        &[("use_of_void_result", 32, 5)],
    );
}

/// `use_of_void_result_test.dart` `test_nullAwareElement_set_error`.
#[test]
#[ignore = "reported by LiteralElementVerifier (error/*, branch wd-errors)"]
fn use_of_void_result_null_aware_element_set_error() {
    assert_errors_in_code(
        r#"
void f(void value) {
  <void>{?value};
}
"#,
        &[("use_of_void_result", 32, 5)],
    );
}

/// `use_of_void_result_test.dart` `test_postfixExpression_bang_operand_error`.
#[test]
fn use_of_void_result_postfix_expression_bang_operand_error() {
    assert_errors_in_code(
        r#"
f(void x) {
  x!;
}
"#,
        &[("use_of_void_result", 15, 2)],
    );
}

/// `use_of_void_result_test.dart` `test_prefixExpression_bang_operand_error`.
#[test]
fn use_of_void_result_prefix_expression_bang_operand_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  !x;
}
"#,
        &[("use_of_void_result", 21, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_prefixExpression_minus_identifier_error`.
#[test]
fn use_of_void_result_prefix_expression_minus_identifier_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  -x;
}
"#,
        &[
            ("unchecked_use_of_nullable_value", 20, 1),
            ("use_of_void_result", 21, 1),
        ],
    );
}

/// `use_of_void_result_test.dart` `test_prefixExpression_minus_invocation_error`.
#[test]
fn use_of_void_result_prefix_expression_minus_invocation_error() {
    assert_errors_in_code(
        r#"
void test(void f()) {
  -f();
}
"#,
        &[
            ("unchecked_use_of_nullable_value", 25, 1),
            ("use_of_void_result", 26, 3),
        ],
    );
}

/// `use_of_void_result_test.dart` `test_propertyAccess_nullAware_target_error`.
#[test]
fn use_of_void_result_property_access_null_aware_target_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x?.foo;
}
"#,
        &[("use_of_void_result", 23, 3)],
    );
}

/// `use_of_void_result_test.dart` `test_propertyAccess_target_error`.
#[test]
fn use_of_void_result_property_access_target_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  x.foo;
}
"#,
        &[("use_of_void_result", 22, 3)],
    );
}

/// `use_of_void_result_test.dart` `test_recordLiteral_namedField_error`.
#[test]
fn use_of_void_result_record_literal_named_field_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  (one: x,);
}
"#,
        &[("use_of_void_result", 21, 6)],
    );
}

/// `use_of_void_result_test.dart` `test_recordLiteral_positionalField_error`.
#[test]
fn use_of_void_result_record_literal_positional_field_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  (x,);
}
"#,
        &[("use_of_void_result", 21, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_returnStatement_nonVoidFunction_error`.
#[test]
fn use_of_void_result_return_statement_non_void_function_error() {
    assert_errors_in_code(
        r#"
dynamic f(void x) {
  return x;
}
"#,
        &[("return_of_invalid_type", 30, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_returnStatement_voidFunction_ok`.
#[test]
fn use_of_void_result_return_statement_void_function_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  return x;
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_switchStatement_expression_error`.
#[test]
fn use_of_void_result_switch_statement_expression_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  switch(x) {}
}
"#,
        &[("use_of_void_result", 27, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_throwExpression_expression_error`.
#[test]
fn use_of_void_result_throw_expression_expression_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  throw x;
}
"#,
        &[
            ("use_of_void_result", 26, 1),
            ("throw_of_invalid_type", 26, 1),
        ],
    );
}

/// `use_of_void_result_test.dart` `test_variableDeclaration_initializer_nonVoidReturn_ok`.
#[test]
fn use_of_void_result_variable_declaration_initializer_non_void_return_ok() {
    assert_errors_in_code(
        r#"
int f() => 1;
g() {
  var a = f();
}
"#,
        &[("unused_local_variable", 27, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_variableDeclaration_initializer_toDynamic_error`.
#[test]
fn use_of_void_result_variable_declaration_initializer_to_dynamic_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  // ignore:unused_local_variable
  dynamic v = x;
}
"#,
        &[("use_of_void_result", 66, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_variableDeclaration_initializer_toDynamic_withUnusedLocal_error`.
#[test]
fn use_of_void_result_variable_declaration_initializer_to_dynamic_with_unused_local_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  dynamic z = x;
}
"#,
        &[
            ("unused_local_variable", 28, 1),
            ("use_of_void_result", 32, 1),
        ],
    );
}

/// `use_of_void_result_test.dart` `test_variableDeclaration_initializer_toVoid_ok`.
#[test]
fn use_of_void_result_variable_declaration_initializer_to_void_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  // ignore:unused_local_variable
  void v = x;
}
"#,
        &[],
    );
}

/// `use_of_void_result_test.dart` `test_variableDeclaration_initializer_toVoid_withUnusedLocal_ok`.
#[test]
fn use_of_void_result_variable_declaration_initializer_to_void_with_unused_local_ok() {
    assert_errors_in_code(
        r#"
void f(void x) {
  void y = x;
}
"#,
        &[("unused_local_variable", 25, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_whileStatement_condition_error`.
#[test]
fn use_of_void_result_while_statement_condition_error() {
    assert_errors_in_code(
        r#"
void f(void x) {
  while (x) {};
}
"#,
        &[("use_of_void_result", 27, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_yieldStatement_asyncStar_error`.
#[test]
fn use_of_void_result_yield_statement_async_star_error() {
    assert_errors_in_code(
        r#"
dynamic f(void x) async* {
  yield x;
}
"#,
        &[("use_of_void_result", 36, 1)],
    );
}

/// `use_of_void_result_test.dart` `test_yieldStatement_star_asyncStar_error`.
#[test]
fn use_of_void_result_yield_statement_star_async_star_error() {
    assert_errors_in_code(
        r#"
Object? f(void x) async* {
  yield* x;
}
"#,
        &[
            ("unchecked_use_of_nullable_value", 37, 1),
            ("yield_of_invalid_type", 37, 1),
            ("use_of_void_result", 37, 1),
        ],
    );
}

/// `use_of_void_result_test.dart` `test_yieldStatement_star_syncStar_error`.
#[test]
fn use_of_void_result_yield_statement_star_sync_star_error() {
    assert_errors_in_code(
        r#"
Object? f(void x) sync* {
  yield* x;
}
"#,
        &[
            ("unchecked_use_of_nullable_value", 36, 1),
            ("yield_of_invalid_type", 36, 1),
            ("use_of_void_result", 36, 1),
        ],
    );
}

/// `use_of_void_result_test.dart` `test_yieldStatement_syncStar_error`.
#[test]
fn use_of_void_result_yield_statement_sync_star_error() {
    assert_errors_in_code(
        r#"
dynamic f(void x) sync* {
  yield x;
}
"#,
        &[("use_of_void_result", 35, 1)],
    );
}
