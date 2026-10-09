//! Ports of the FFI diagnostic tests of
//! `pkg/analyzer/test/src/diagnostics/*_test.dart` (the tests whose code
//! imports `dart:ffi`), generated from the Dart sources: same code, same
//! offsets. Only the codes of `tools/difftest/ffi_verifier_codes.txt` are
//! compared (on both sides).

mod support;

use std::collections::BTreeSet;
use std::sync::OnceLock;

/// The codes of `tools/difftest/ffi_verifier_codes.txt`.
fn ffi_verifier_codes() -> &'static BTreeSet<String> {
    static CODES: OnceLock<BTreeSet<String>> = OnceLock::new();
    CODES.get_or_init(|| {
        include_str!("../../../tools/difftest/ffi_verifier_codes.txt")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect()
    })
}

/// Dart `assertErrorsInCode` / `resolveTestCodeWithDiagnostics` for the
/// FFI codes: [expected] is `(code, offset, length)`.
fn assert_ffi_errors_in_code(source: &str, expected: &[(&str, usize, usize)]) {
    let Some(a) = support::analyze(&[("main.dart", source)]) else {
        eprintln!("skipped: no Dart SDK on PATH");
        return;
    };
    let unit = a.unit();
    assert!(unit.panic.is_none(), "{:?}", unit.panic);
    let codes = ffi_verifier_codes();
    let mut actual: Vec<(String, usize, usize)> = unit
        .diagnostics
        .iter()
        .filter(|d| codes.contains(d.code.name))
        .map(|d| (d.code.name.to_string(), d.offset, d.length))
        .collect();
    actual.sort();
    let mut expected: Vec<(String, usize, usize)> = expected
        .iter()
        .map(|&(c, o, l)| (c.to_string(), o, l))
        .collect();
    expected.sort();
    assert_eq!(actual, expected, "diagnostics of:\n{source}");
}

/// Dart `abi_specific_integer_mapping_test.dart` `test_doubleMapping`.
#[test]
fn abi_specific_integer_mapping__double_mapping() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@AbiSpecificIntegerMapping({})
@AbiSpecificIntegerMapping({})
final class UintPtr extends AbiSpecificInteger {
  const UintPtr();
}
"#,
        &[("abi_specific_integer_mapping_extra", 51, 25)],
    );
}

/// Dart `abi_specific_integer_mapping_test.dart` `test_invalidMapping`.
#[test]
fn abi_specific_integer_mapping__invalid_mapping() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@AbiSpecificIntegerMapping({
  Abi.androidArm: Uint32(),
  Abi.androidArm64: IntPtr(),
  Abi.androidIA32: UintPtr(),
})
final class UintPtr extends AbiSpecificInteger {
  const UintPtr();
}
"#,
        &[("abi_specific_integer_mapping_unsupported", 96, 8), ("abi_specific_integer_mapping_unsupported", 125, 9)],
    );
}

/// Dart `abi_specific_integer_mapping_test.dart` `test_invalidMapping_identifier`.
#[test]
fn abi_specific_integer_mapping__invalid_mapping_identifier() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
const c = {
  Abi.androidArm: Uint32(),
  Abi.androidArm64: IntPtr(),
  Abi.androidIA32: UintPtr(),
};
@AbiSpecificIntegerMapping(c)
final class UintPtr extends AbiSpecificInteger {
  const UintPtr();
}
"#,
        &[("abi_specific_integer_mapping_unsupported", 149, 1), ("abi_specific_integer_mapping_unsupported", 149, 1)],
    );
}

/// Dart `abi_specific_integer_mapping_test.dart` `test_noMapping`.
#[test]
fn abi_specific_integer_mapping__no_mapping() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class UintPtr extends AbiSpecificInteger {
  const UintPtr();
}
"#,
        &[("abi_specific_integer_mapping_missing", 31, 7)],
    );
}

/// Dart `abi_specific_integer_mapping_test.dart` `test_singleMapping`.
#[test]
fn abi_specific_integer_mapping__single_mapping() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@AbiSpecificIntegerMapping({})
final class UintPtr extends AbiSpecificInteger {
  const UintPtr();
}
"#,
        &[],
    );
}

/// Dart `abi_specific_integer_mapping_test.dart` `test_validMapping`.
#[test]
fn abi_specific_integer_mapping__valid_mapping() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@AbiSpecificIntegerMapping({
  Abi.androidArm: Uint32(),
  Abi.androidArm64: Uint64(),
  Abi.androidIA32: Uint32(),
})
final class UintPtr extends AbiSpecificInteger {
  const UintPtr();
}
"#,
        &[],
    );
}

/// Dart `annotation_on_pointer_field_test.dart` `test_double`.
#[test]
fn annotation_on_pointer_field__double() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  @Double()
  external Pointer<Int8> x;
}
"#,
        &[("annotation_on_pointer_field", 52, 9)],
    );
}

/// Dart `annotation_on_pointer_field_test.dart` `test_int32`.
#[test]
fn annotation_on_pointer_field__int32() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  @Int32()
  external Pointer<Float> x;
}
"#,
        &[("annotation_on_pointer_field", 52, 8)],
    );
}

/// Dart `argument_must_be_a_constant_test.dart` `test_AsFunctionIsLeafGlobal`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn argument_must_be_a_constant__as_function_is_leaf_global() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef Int8UnOp = Int8 Function(Int8);
typedef IntUnOp = int Function(int);
bool isLeaf = false;
doThings() {
  Pointer<NativeFunction<Int8UnOp>> p = Pointer.fromAddress(1337);
  IntUnOp f = p.asFunction(isLeaf:isLeaf);
  f(8);
}
"#,
        &[("argument_must_be_a_constant", 231, 6)],
    );
}

/// Dart `argument_must_be_a_constant_test.dart` `test_AsFunctionIsLeafLocal`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn argument_must_be_a_constant__as_function_is_leaf_local() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef Int8UnOp = Int8 Function(Int8);
typedef IntUnOp = int Function(int);
doThings() {
  bool isLeaf = false;
  Pointer<NativeFunction<Int8UnOp>> p = Pointer.fromAddress(1337);
  IntUnOp f = p.asFunction(isLeaf:isLeaf);
  f(8);
}
"#,
        &[("argument_must_be_a_constant", 233, 6)],
    );
}

/// Dart `argument_must_be_a_constant_test.dart` `test_AsFunctionIsLeafParam`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn argument_must_be_a_constant__as_function_is_leaf_param() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef Int8UnOp = Int8 Function(Int8);
typedef IntUnOp = int Function(int);
doThings(bool isLeaf) {
  Pointer<NativeFunction<Int8UnOp>> p = Pointer.fromAddress(1337);
  IntUnOp f = p.asFunction(isLeaf:isLeaf);
  f(8);
}
"#,
        &[("argument_must_be_a_constant", 221, 6)],
    );
}

/// Dart `argument_must_be_a_constant_test.dart` `test_FromFunctionExceptionReturn`.
#[test]
fn argument_must_be_a_constant__from_function_exception_return() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef NativeDoubleUnOp = Double Function(Double);
double myTimesThree(double d) => d * 3;
void testFromFunctionFunctionExceptionValueMustBeConst() {
  final notAConst = 1.1;
  Pointer.fromFunction<NativeDoubleUnOp>(myTimesThree, notAConst);
}
"#,
        &[("argument_must_be_a_constant", 250, 9)],
    );
}

/// Dart `argument_must_be_a_constant_test.dart` `test_LookupFunctionIsLeaf`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn argument_must_be_a_constant__lookup_function_is_leaf() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef Int8UnOp = Int8 Function(Int8);
typedef IntUnOp = int Function(int);
doThings(bool isLeaf) {
  DynamicLibrary l = DynamicLibrary.open("my_lib");
  l.lookupFunction<Int8UnOp, IntUnOp>("timesFour", isLeaf:isLeaf);
}
"#,
        &[("argument_must_be_a_constant", 230, 6)],
    );
}

/// Dart `creation_of_struct_or_union_test.dart` `test_struct`.
#[test]
fn creation_of_struct_or_union__struct() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class A extends Struct {
  @Int32()
  external int a;
}

void f() {
  A();
}
"#,
        &[("creation_of_struct_or_union", 96, 1)],
    );
}

/// Dart `creation_of_struct_or_union_test.dart` `test_union`.
#[test]
fn creation_of_struct_or_union__union() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class A extends Union {
  @Int32()
  external int a;
}

void f() {
  A();
}
"#,
        &[("creation_of_struct_or_union", 95, 1)],
    );
}

/// Dart `extra_annotation_on_struct_field_test.dart` `test_one`.
#[test]
fn extra_annotation_on_struct_field__one() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  @Int32()
  external int x;
}
"#,
        &[],
    );
}

/// Dart `extra_annotation_on_struct_field_test.dart` `test_two`.
#[test]
fn extra_annotation_on_struct_field__two() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  @Int32()
  @Int16()
  external int x;
}
"#,
        &[("extra_annotation_on_struct_field", 63, 8)],
    );
}

/// Dart `extra_size_annotation_carray_test.dart` `test_const`.
#[test]
fn extra_size_annotation_carray__const() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

const EIGHT = 8;

final class Struct8BytesInlineArrayInt extends Struct {
  @Array(EIGHT)
  external Array<Uint8> a0;
}
"#,
        &[],
    );
}

/// Dart `extra_size_annotation_carray_test.dart` `test_one`.
#[test]
fn extra_size_annotation_carray__one() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  @Array(8)
  external Array<Uint8> a0;
}
"#,
        &[],
    );
}

/// Dart `extra_size_annotation_carray_test.dart` `test_two`.
#[test]
fn extra_size_annotation_carray__two() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  @Array(8)
  @Array(8)
  external Array<Uint8> a0;
}
"#,
        &[("extra_size_annotation_carray", 65, 9)],
    );
}

/// Dart `ffi_address_of_cast_test.dart` `test_struct_error_1`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn ffi_address_of_cast__struct_error_1() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>()
external void myNonLeafNative(Pointer<Void> buffer);

main() {
  final myStruct = Struct.create<MyStruct>();
  myNonLeafNative(myStruct.arr.address.cast());
}

final class MyStruct extends Struct {
  @Int8()
  external int value;
  @Array(2)
  external Array<Int8> arr;
}
"#,
        &[("address_position", 200, 7)],
    );
}

/// Dart `ffi_address_of_cast_test.dart` `test_struct_error_2`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn ffi_address_of_cast__struct_error_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>()
external void myNonLeafNative(Pointer<Void> buffer);

main() {
  final myStruct = Struct.create<MyStruct>();
  myNonLeafNative(myStruct.value.address.cast());
}

final class MyStruct extends Struct {
  @Int8()
  external int value;
  @Array(2)
  external Array<Int8> arr;
}
"#,
        &[("address_position", 202, 7)],
    );
}

/// Dart `ffi_address_of_cast_test.dart` `test_struct_no_error`.
#[test]
fn ffi_address_of_cast__struct_no_error() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>(isLeaf: true)
external void myNative(Pointer<Void> buffer);

main() {
  final myStruct = Struct.create<MyStruct>();
  myNative(myStruct.arr.address.cast());
  myNative(myStruct.arr.address.cast<Void>());
}

final class MyStruct extends Struct {
  @Int8()
  external int value;
  @Array(2)
  external Array<Int8> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_address_of_cast_test.dart` `test_typed_data_error`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn ffi_address_of_cast__typed_data_error() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
import 'dart:typed_data';

@Native<Void Function(Pointer<Void>)>()
external void myNonLeafNative(Pointer<Void> buffer);

main() {
  final buffer = Int8List(2);
  myNonLeafNative(buffer.address.cast());
}
"#,
        &[("address_position", 204, 7)],
    );
}

/// Dart `ffi_address_of_cast_test.dart` `test_typed_data_no_error`.
#[test]
fn ffi_address_of_cast__typed_data_no_error() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
import 'dart:typed_data';

@Native<Void Function(Pointer<Void>)>(isLeaf: true)
external void myNative(Pointer<Void> buffer);

main() {
  final buffer = Int8List(2);
  myNative(buffer.address.cast());
}
"#,
        &[],
    );
}

/// Dart `ffi_address_of_cast_test.dart` `test_union_error_1`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn ffi_address_of_cast__union_error_1() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>()
external void myNonLeafNative(Pointer<Void> buffer);

main() {
  final myUnion = Union.create<MyUnion>();
  myNonLeafNative(myUnion.arr.address.cast());
}

final class MyUnion extends Union {
  @Int8()
  external int value;
  @Array(2)
  external Array<Int8> arr;
}
"#,
        &[("address_position", 196, 7)],
    );
}

/// Dart `ffi_address_of_cast_test.dart` `test_union_error_2`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn ffi_address_of_cast__union_error_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>()
external void myNonLeafNative(Pointer<Void> buffer);

main() {
  final myUnion = Union.create<MyUnion>();
  myNonLeafNative(myUnion.value.address.cast());
}
final class MyUnion extends Union {
  @Int8()
  external int value;
  @Array(2)
  external Array<Int8> arr;
}
"#,
        &[("address_position", 198, 7)],
    );
}

/// Dart `ffi_address_of_cast_test.dart` `test_union_no_error`.
#[test]
fn ffi_address_of_cast__union_no_error() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function(Pointer<Void>)>(isLeaf: true)
external void myNative(Pointer<Void> buffer);

main() {
  final myUnion = Union.create<MyUnion>();
  myNative(myUnion.arr.address.cast());
  myNative(myUnion.arr.address.cast<Void>());
}
final class MyUnion extends Union {
  @Int8()
  external int value;
  @Array(2)
  external Array<Int8> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_array_test.dart` `test_array_negativeDimension`.
#[test]
fn ffi_array__array_negative_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array(-1)
  external Array<Int8> arr;
}
"#,
        &[("non_positive_array_dimension", 67, 2)],
    );
}

/// Dart `ffi_array_test.dart` `test_array_positiveDimension`.
#[test]
fn ffi_array__array_positive_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array(1)
  external Array<Int8> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_array_test.dart` `test_array_zeroDimension`.
#[test]
fn ffi_array__array_zero_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array(0)
  external Array<Int8> arr;
}
"#,
        &[("non_positive_array_dimension", 67, 1)],
    );
}

/// Dart `ffi_array_test.dart` `test_multi_negativeDimension`.
#[test]
fn ffi_array__multi_negative_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.multi([-2, 2])
  external Array<Array<Int8>> arr;
}
"#,
        &[("non_positive_array_dimension", 74, 2)],
    );
}

/// Dart `ffi_array_test.dart` `test_multi_positiveDimension`.
#[test]
fn ffi_array__multi_positive_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.multi([2, 2])
  external Array<Array<Int8>> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_array_test.dart` `test_multi_zeroDimension`.
#[test]
fn ffi_array__multi_zero_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.multi([0, 2])
  external Array<Array<Int8>> arr;
}
"#,
        &[("non_positive_array_dimension", 74, 1)],
    );
}

/// Dart `ffi_array_test.dart` `test_variable_negativeDimension`.
#[test]
fn ffi_array__variable_negative_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variable(-1)
  external Array<Array<Int8>> arr;
}
"#,
        &[("non_positive_array_dimension", 76, 2)],
    );
}

/// Dart `ffi_array_test.dart` `test_variable_positiveDimension`.
#[test]
fn ffi_array__variable_positive_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variable(1)
  external Array<Array<Int8>> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_array_test.dart` `test_variable_valid`.
#[test]
fn ffi_array__variable_valid() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variable()
  external Array<Int8> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_array_test.dart` `test_variable_zeroDimension`.
#[test]
fn ffi_array__variable_zero_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variable(0)
  external Array<Array<Int8>> arr;
}
"#,
        &[("non_positive_array_dimension", 76, 1)],
    );
}

/// Dart `ffi_array_test.dart` `test_variableMulti_negativeDimension`.
#[test]
fn ffi_array__variable_multi_negative_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableMulti(variableDimension: -1, [2, 2])
  external Array<Array<Array<Int8>>> arr;
}
"#,
        &[("negative_variable_dimension", 100, 2)],
    );
}

/// Dart `ffi_array_test.dart` `test_variableMulti_positiveDimension`.
#[test]
fn ffi_array__variable_multi_positive_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableMulti(variableDimension: 1, [2, 2])
  external Array<Array<Array<Int8>>> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_array_test.dart` `test_variableMulti_valid`.
#[test]
fn ffi_array__variable_multi_valid() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableMulti([2, 2])
  external Array<Array<Array<Int8>>> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_array_test.dart` `test_variableMulti_zeroDimension`.
#[test]
fn ffi_array__variable_multi_zero_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableMulti(variableDimension: 0, [2, 2])
  external Array<Array<Array<Int8>>> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_array_test.dart` `test_variableWithVariableDimension_negativeDimension`.
#[test]
fn ffi_array__variable_with_variable_dimension_negative_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableWithVariableDimension(-1)
  external Array<Int8> arr;
}
"#,
        &[("negative_variable_dimension", 97, 2)],
    );
}

/// Dart `ffi_array_test.dart` `test_variableWithVariableDimension_positiveDimension`.
#[test]
fn ffi_array__variable_with_variable_dimension_positive_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableWithVariableDimension(1)
  external Array<Int8> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_array_test.dart` `test_variableWithVariableDimension_zeroDimension`.
#[test]
fn ffi_array__variable_with_variable_dimension_zero_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Array.variableWithVariableDimension(0)
  external Array<Int8> arr;
}
"#,
        &[],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_argumentMustBeAConstant`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_argument_must_be_aconstant() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  int e = 123;
  NativeCallable<Int32 Function(Int32)>.isolateLocal(f, exceptionalReturn: e);
}
"#,
        &[("argument_must_be_a_constant", 143, 1)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_exceptionMustBeASubtype`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_exception_must_be_asubtype() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>.isolateLocal(f, exceptionalReturn: '?');
}
"#,
        &[("must_be_a_subtype", 128, 3)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_inferred`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_inferred() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>? callback;
  callback = NativeCallable.isolateLocal(f, exceptionalReturn: 4);
  callback.close();
}
"#,
        &[],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_invalidExceptionValue`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_invalid_exception_value() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>.isolateLocal(f, exceptionalReturn: 4);
}
"#,
        &[("invalid_exception_value", 109, 20)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_missingExceptionValue`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_missing_exception_value() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>.isolateLocal(f);
}
"#,
        &[("missing_exception_value", 55, 53)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_mustBeANativeFunctionType`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_must_be_anative_function_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<int Function(int)>.isolateLocal(f, exceptionalReturn: 4);
}
"#,
        &[("must_be_a_native_function_type", 55, 46)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_mustBeASubtype`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_must_be_asubtype() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Double)>.isolateLocal(f, exceptionalReturn: 4);
}
"#,
        &[("must_be_a_subtype", 107, 1)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_mustHaveTypeArgs`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_must_have_type_args() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable.isolateLocal(f, exceptionalReturn: 4);
}
"#,
        &[("must_be_a_native_function_type", 55, 27)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_ok`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_ok() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>.isolateLocal(f, exceptionalReturn: 4);
}
"#,
        &[],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_okVoid`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_ok_void() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>.isolateLocal(f);
}
"#,
        &[],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_isolateLocal_voidReturnPermissive`.
#[test]
fn ffi_async_callback__native_callable_isolate_local_void_return_permissive() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>.isolateLocal(f);
}
"#,
        &[],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_listener_inferred`.
#[test]
fn ffi_async_callback__native_callable_listener_inferred() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>? callback;
  callback = NativeCallable.listener(f);
  callback.close();
}
"#,
        &[],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_listener_mustBeANativeFunctionType`.
#[test]
fn ffi_async_callback__native_callable_listener_must_be_anative_function_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<void Function(int)>.listener(f);
}
"#,
        &[("must_be_a_native_function_type", 56, 43)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_listener_mustBeASubtype`.
#[test]
fn ffi_async_callback__native_callable_listener_must_be_asubtype() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Double)>.listener(f);
}
"#,
        &[("must_be_a_subtype", 103, 1)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_listener_mustHaveTypeArgs`.
#[test]
fn ffi_async_callback__native_callable_listener_must_have_type_args() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable.listener(f);
}
"#,
        &[("must_be_a_native_function_type", 55, 23)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_listener_mustReturnVoid`.
#[test]
fn ffi_async_callback__native_callable_listener_must_return_void() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Int32 Function(Int32)>.listener(f);
}
"#,
        &[("must_return_void", 102, 1)],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_listener_ok`.
#[test]
fn ffi_async_callback__native_callable_listener_ok() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>.listener(f);
}
"#,
        &[],
    );
}

/// Dart `ffi_async_callback_test.dart` `test_NativeCallable_listener_voidReturnPermissive`.
#[test]
fn ffi_async_callback__native_callable_listener_void_return_permissive() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
int f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Int32)>.listener(f);
}
"#,
        &[],
    );
}

/// Dart `ffi_leaf_call_must_not_use_handle_test.dart` `test_AsFunctionReturnsHandle`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn ffi_leaf_call_must_not_use_handle__as_function_returns_handle() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef NativeReturnsHandle = Handle Function();
typedef ReturnsHandle = Object Function();
doThings() {
  Pointer<NativeFunction<NativeReturnsHandle>> p = Pointer.fromAddress(1337);
  ReturnsHandle f = p.asFunction(isLeaf:true);
  f();
}
"#,
        &[("leaf_call_must_not_return_handle", 224, 10)],
    );
}

/// Dart `ffi_leaf_call_must_not_use_handle_test.dart` `test_AsFunctionTakesHandle`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn ffi_leaf_call_must_not_use_handle__as_function_takes_handle() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef NativeTakesHandle = Void Function(Handle);
typedef TakesHandle = void Function(Object);
class MyClass {}
doThings() {
  Pointer<NativeFunction<NativeTakesHandle>> p = Pointer.fromAddress(1337);
  TakesHandle f = p.asFunction(isLeaf:true);
  f(MyClass());
}
"#,
        &[("leaf_call_must_not_take_handle", 241, 10)],
    );
}

/// Dart `ffi_leaf_call_must_not_use_handle_test.dart` `test_class_getter`.
#[test]
fn ffi_leaf_call_must_not_use_handle__class_getter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

base class NativeFieldWrapperClass1 {}

base class A extends NativeFieldWrapperClass1 {
  @Native<Handle Function(Pointer<Void>)>(symbol: 'foo', isLeaf:true)
  external Object get foo;
}
"#,
        &[("leaf_call_must_not_return_handle", 200, 3)],
    );
}

/// Dart `ffi_leaf_call_must_not_use_handle_test.dart` `test_LookupFunctionReturnsHandle`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn ffi_leaf_call_must_not_use_handle__lookup_function_returns_handle() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef NativeReturnsHandle = Handle Function();
typedef ReturnsHandle = Object Function();
doThings() {
  DynamicLibrary l = DynamicLibrary.open("my_lib");
  l.lookupFunction<NativeReturnsHandle, ReturnsHandle>("timesFour", isLeaf:true);
}
"#,
        &[("leaf_call_must_not_return_handle", 195, 19)],
    );
}

/// Dart `ffi_leaf_call_must_not_use_handle_test.dart` `test_LookupFunctionTakesHandle`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn ffi_leaf_call_must_not_use_handle__lookup_function_takes_handle() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef NativeTakesHandle = Void Function(Handle);
typedef TakesHandle = void Function(Object);
class MyClass {}
doThings() {
  DynamicLibrary l = DynamicLibrary.open("my_lib");
  l.lookupFunction<NativeTakesHandle, TakesHandle>("timesFour", isLeaf:true);
}
"#,
        &[("leaf_call_must_not_take_handle", 216, 17)],
    );
}

/// Dart `ffi_leaf_call_must_not_use_handle_test.dart` `test_unit_getter`.
#[test]
fn ffi_leaf_call_must_not_use_handle__unit_getter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Handle Function()>(symbol: 'foo', isLeaf:true)
external Object get foo;
"#,
        &[("leaf_call_must_not_return_handle", 95, 3)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_Lambda`.
#[test]
fn ffi_native__invalid_lambda() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

void main() => print(Native.addressOf(() => 3));
"#,
        &[("argument_must_be_native", 58, 7)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_MismatchedInferredType`.
#[test]
fn ffi_native__invalid_mismatched_inferred_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external Pointer<IntPtr> global;

void main() => print(Native.addressOf<Pointer<Double>>(global));
"#,
        &[("must_be_a_subtype", 85, 41)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_MismatchingType`.
#[test]
fn ffi_native__invalid_mismatching_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function()>()
external void foo();

void main() {
  print(Native.addressOf<NativeFunction<Int8 Function()>>(foo));
}
"#,
        &[("must_be_a_subtype", 91, 54)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_MissingType`.
#[test]
fn ffi_native__invalid_missing_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function()>()
external void foo();

void main() {
  print(Native.addressOf(foo));
}
"#,
        &[("must_be_a_native_function_type", 91, 21)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_MissingType2`.
#[test]
fn ffi_native__invalid_missing_type2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external void foo();

void main() {
  print(Native.addressOf(foo));
}
"#,
        &[("must_be_a_native_function_type", 74, 21)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_MissingType3`.
#[test]
fn ffi_native__invalid_missing_type3() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external Pointer<IntPtr> global;

void main() => print(Native.addressOf(global));
"#,
        &[("must_be_a_subtype", 85, 24)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_NotAConstant`.
#[test]
fn ffi_native__invalid_not_aconstant() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function()>()
external void foo();
@Native<Void Function()>()
external void bar();

void entry(bool condition) {
  print(Native.addressOf(condition ? foo : bar));
}
"#,
        &[("argument_must_be_native", 171, 21)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_NotAPreciseType`.
#[test]
fn ffi_native__invalid_not_aprecise_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function()>()
external void foo();

void main() => print(Native.addressOf<NativeFunction>(foo));
"#,
        &[("must_be_a_subtype", 90, 37)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_NotAPreciseType2`.
#[test]
fn ffi_native__invalid_not_aprecise_type2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external void foo();

void main() => print(Native.addressOf<NativeFunction>(foo));
"#,
        &[("must_be_a_subtype", 73, 37)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_String`.
#[test]
fn ffi_native__invalid_string() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

void main() => print(Native.addressOf('malloc'));
"#,
        &[("argument_must_be_native", 58, 8)],
    );
}

/// Dart `ffi_native_test.dart` `test_valid`.
#[test]
fn ffi_native__valid() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Void Function()>()
external void foo();

@Native()
external void foo2();

@Native()
external Pointer<IntPtr> global;

void main() {
  print(Native.addressOf<NativeFunction<Void Function()>>(foo));
  print(Native.addressOf<NativeFunction<Void Function()>>(foo2));
  print(Native.addressOf<Pointer<IntPtr>>(global));
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_duplicate`.
#[test]
fn ffi_native__invalid_duplicate() {
    assert_ffi_errors_in_code(
        r#"@DefaultAsset('foo')
@DefaultAsset('bar')
library;

import 'dart:ffi';
"#,
        &[("ffi_native_invalid_duplicate_default_asset", 22, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_invalid_duplicateFromConst`.
#[test]
fn ffi_native__invalid_duplicate_from_const() {
    assert_ffi_errors_in_code(
        r#"@DefaultAsset('bar')
@defaults
library;

import 'dart:ffi';

const defaults = DefaultAsset('foo');
"#,
        &[("ffi_native_invalid_duplicate_default_asset", 22, 8)],
    );
}

/// Dart `ffi_native_test.dart` `test_valid`.
#[test]
fn ffi_native__valid_2() {
    assert_ffi_errors_in_code(
        r#"@DefaultAsset('bar')
library;

import 'dart:ffi';

@Native<Void Function()>()
external void foo();
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_validFromConst`.
#[test]
fn ffi_native__valid_from_const() {
    assert_ffi_errors_in_code(
        r#"@defaults
library;

import 'dart:ffi';

const defaults = DefaultAsset('foo');

@Native<Void Function()>()
external void foo();
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_annotation_FfiNative_getters`.
#[test]
fn ffi_native__annotation_ffi_native_getters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

base class NativeFieldWrapperClass1 {}

base class Paragraph extends NativeFieldWrapperClass1 {
  @Native<Double Function(Pointer<Void>)>(symbol: 'Paragraph::ideographicBaseline', isLeaf: true)
  external double get ideographicBaseline;

  @Native<Void Function(Pointer<Void>, Double)>(symbol: 'Paragraph::ideographicBaseline', isLeaf: true)
  external set ideographicBaseline(double d);
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_annotation_FfiNative_noArguments`.
#[test]
fn ffi_native__annotation_ffi_native_no_arguments() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native
external int foo();
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_annotation_FfiNative_noTypeArguments`.
#[test]
fn ffi_native__annotation_ffi_native_no_type_arguments() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external int foo();
"#,
        &[("native_function_missing_type", 43, 3)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeCanUseHandles`.
#[test]
fn ffi_native__ffi_native_can_use_handles() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Handle Function(Handle)>(symbol: 'DoesntMatter')
external Object doesntMatter(Object);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeCanUseLeaf`.
#[test]
fn ffi_native__ffi_native_can_use_leaf() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Int8 Function(Int64)>(symbol: 'DoesntMatter', isLeaf:true)
external int doesntMatter(int x);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeInstanceMethodsMustHaveReceiver`.
#[test]
fn ffi_native__ffi_native_instance_methods_must_have_receiver() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class K {
  @Native<Void Function(Double)>(symbol: 'DoesntMatter')
  external void doesntMatter(double x);
}
"#,
        &[("ffi_native_unexpected_number_of_parameters_with_receiver", 102, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeLeafMustNotReturnHandle`.
#[test]
fn ffi_native__ffi_native_leaf_must_not_return_handle() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Handle Function()>(symbol: 'DoesntMatter', isLeaf:true)
external Object doesntMatter();
"#,
        &[("leaf_call_must_not_return_handle", 99, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeLeafMustNotTakeHandles`.
#[test]
fn ffi_native__ffi_native_leaf_must_not_take_handles() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Void Function(Handle)>(symbol: 'DoesntMatter', isLeaf:true)
external void doesntMatter(Object o);
"#,
        &[("leaf_call_must_not_take_handle", 101, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeNonFfiParameter`.
#[test]
fn ffi_native__ffi_native_non_ffi_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<IntPtr Function(int)>(symbol: 'doesntmatter')
external int nonFfiParameter(int v);
"#,
        &[("must_be_a_native_function_type", 86, 15)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeNonFfiReturnType`.
#[test]
fn ffi_native__ffi_native_non_ffi_return_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<double Function(IntPtr)>(symbol: 'doesntmatter')
external double nonFfiReturnType(int v);
"#,
        &[("must_be_a_native_function_type", 92, 16)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeOnExtension_valid`.
#[test]
fn ffi_native__ffi_native_on_extension_valid() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

extension on int {
  @Native<Bool Function(Int64, Int64)>(symbol: 'x')
  external bool f(int m);
}

void g() {
  0.f(0);
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeOnExtension_wrongNumberOfParameters`.
#[test]
fn ffi_native__ffi_native_on_extension_wrong_number_of_parameters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

extension on int {
  @Native<Bool Function(Int64)>(symbol: 'x')
  external bool f(int m);
}

void g() {
  0.f(0);
}
"#,
        &[("ffi_native_unexpected_number_of_parameters", 100, 1)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeOnExtension_wrongReceiverType`.
#[test]
fn ffi_native__ffi_native_on_extension_wrong_receiver_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

extension on double {
  @Native<Bool Function(Int64, Int64)>(symbol: 'Dart_PostInteger')
  external bool postInteger(int message);
}

void f() {
  0.0.postInteger(0);
}
"#,
        &[("must_be_a_subtype", 125, 11)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeOnExtensionType_wrongReceiverType`.
#[test]
fn ffi_native__ffi_native_on_extension_type_wrong_receiver_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

extension type NativeSendPort(int id) {
  @Native<Bool Function(Int64, Int64)>(symbol: 'Dart_PostInteger')
  external bool postInteger(int message);
}
"#,
        &[("must_be_a_subtype", 143, 11)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeOnExtensionType_wrongRepresentationType`.
#[test]
fn ffi_native__ffi_native_on_extension_type_wrong_representation_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

extension type InvalidNativeSendPort._(double id) {
  @Native<Bool Function(Int64, Int64)>(symbol: 'Dart_PostInteger')
  external bool postInteger(int message);
}
"#,
        &[("must_be_a_subtype", 155, 11)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativePointerParameter`.
#[test]
fn ffi_native__ffi_native_pointer_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Void Function(Pointer)>(symbol: 'free')
external void posixFree(Pointer pointer);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeTooFewParameters`.
#[test]
fn ffi_native__ffi_native_too_few_parameters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Void Function(Double)>(symbol: 'DoesntMatter')
external void doesntMatter(double x, double y);
"#,
        &[("ffi_native_unexpected_number_of_parameters", 88, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeTooManyParameters`.
#[test]
fn ffi_native__ffi_native_too_many_parameters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Void Function(Double, Double)>(symbol: 'DoesntMatter')
external void doesntMatter(double x);
"#,
        &[("ffi_native_unexpected_number_of_parameters", 96, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeVoidReturn`.
#[test]
fn ffi_native__ffi_native_void_return() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Handle Function(Uint32, Uint32, Handle)>(symbol: 'doesntmatter')
external void voidReturn(int width, int height, Object outImage);
"#,
        &[("must_be_a_subtype", 106, 10)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeWrongFfiParameter`.
#[test]
fn ffi_native__ffi_native_wrong_ffi_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<IntPtr Function(Double)>(symbol: 'doesntmatter')
external int wrongFfiParameter(int v);
"#,
        &[("must_be_a_subtype", 89, 17)],
    );
}

/// Dart `ffi_native_test.dart` `test_FfiNativeWrongFfiReturnType`.
#[test]
fn ffi_native__ffi_native_wrong_ffi_return_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<IntPtr Function(IntPtr)>(symbol: 'doesntmatter')
external double wrongFfiReturnType(int v);
"#,
        &[("must_be_a_subtype", 92, 18)],
    );
}

/// Dart `ffi_native_test.dart` `test_AbiSpecific`.
#[test]
fn ffi_native__abi_specific() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Int>()
external int foo;
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_Accessors`.
#[test]
fn ffi_native__accessors() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<IntPtr>()
external int get foo;

@Native<IntPtr>()
external set foo(int value);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_Array_InvalidDimension`.
#[test]
fn ffi_native__array_invalid_dimension() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
@Array(0)
external Array<IntPtr> field;
"#,
        &[("non_positive_array_dimension", 37, 1)],
    );
}

/// Dart `ffi_native_test.dart` `test_Array_InvalidDimensionCount`.
#[test]
fn ffi_native__array_invalid_dimension_count() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
@Array(10, 20)
external Array<IntPtr> field;
"#,
        &[("size_annotation_dimensions", 30, 14)],
    );
}

/// Dart `ffi_native_test.dart` `test_Array_MissingAnnotation`.
#[test]
fn ffi_native__array_missing_annotation() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external Array<IntPtr> field;
"#,
        &[("missing_size_annotation_carray", 53, 5)],
    );
}

/// Dart `ffi_native_test.dart` `test_Array_Valid`.
#[test]
fn ffi_native__array_valid() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
@Array(12)
external Array<IntPtr> field0;

@Array(10, 20)
@Native()
external Array<Array<IntPtr>> field1;
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_Infer`.
#[test]
fn ffi_native__infer() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  external Pointer<MyStruct> next;
}

@Native()
external MyStruct first;

@Native()
external Pointer<MyStruct> last;
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InvalidFunctionType`.
#[test]
fn ffi_native__invalid_function_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<IntPtr Function(IntPtr)>()
external int field;
"#,
        &[("native_field_invalid_type", 67, 5)],
    );
}

/// Dart `ffi_native_test.dart` `test_InvalidInstanceMember`.
#[test]
fn ffi_native__invalid_instance_member() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

class Foo {
  @Native<IntPtr>()
  external int field;
}
"#,
        &[("native_field_not_static", 67, 5)],
    );
}

/// Dart `ffi_native_test.dart` `test_InvalidNotExternal`.
#[test]
fn ffi_native__invalid_not_external() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<IntPtr>()
int field;
"#,
        &[("ffi_native_must_be_external", 42, 5)],
    );
}

/// Dart `ffi_native_test.dart` `test_MismatchingFunctionType`.
#[test]
fn ffi_native__mismatching_function_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<NativeFunction<Double Function()>>()
external int Function() field;
"#,
        &[("must_be_a_subtype", 89, 5)],
    );
}

/// Dart `ffi_native_test.dart` `test_MismatchingType`.
#[test]
fn ffi_native__mismatching_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Double>()
external int field;
"#,
        &[("must_be_a_subtype", 51, 5)],
    );
}

/// Dart `ffi_native_test.dart` `test_MissingType`.
#[test]
fn ffi_native__missing_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external int invalid;

@Native()
external Pointer<IntPtr> valid;
"#,
        &[("native_field_missing_type", 43, 7)],
    );
}

/// Dart `ffi_native_test.dart` `test_Unsupported_Function`.
#[test]
fn ffi_native__unsupported_function() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<NativeFunction<Void Function()>>()
external void Function() field;
"#,
        &[("native_field_invalid_type", 88, 5)],
    );
}

/// Dart `ffi_native_test.dart` `test_Unsupported_Handle`.
#[test]
fn ffi_native__unsupported_handle() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<Handle>()
external Object field;
"#,
        &[("native_field_invalid_type", 54, 5)],
    );
}

/// Dart `ffi_native_test.dart` `test_annotation_InvalidFieldType`.
#[test]
fn ffi_native__annotation_invalid_field_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native<IntPtr>()
external int foo();
"#,
        &[("must_be_a_native_function_type", 51, 3)],
    );
}

/// Dart `ffi_native_test.dart` `test_annotation_MissingType`.
#[test]
fn ffi_native__annotation_missing_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external int foo();
"#,
        &[("native_function_missing_type", 43, 3)],
    );
}

/// Dart `ffi_native_test.dart` `test_annotation_MissingTypeConst`.
#[test]
fn ffi_native__annotation_missing_type_const() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

const a = Native();

@a
external int foo();
"#,
        &[("native_function_missing_type", 57, 3)],
    );
}

/// Dart `ffi_native_test.dart` `test_annotation_Native_getters`.
#[test]
fn ffi_native__annotation_native_getters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

base class NativeFieldWrapperClass1 {}

base class Paragraph extends NativeFieldWrapperClass1 {
  @Native<Double Function(Pointer<Void>)>(isLeaf: true)
  external double get ideographicBaseline;

  @Native<Void Function(Pointer<Void>, Double)>(isLeaf: true)
  external set ideographicBaseline(double d);
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_annotation_Native_noArguments`.
#[test]
fn ffi_native__annotation_native_no_arguments() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native
external int foo();
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferPointerReturnNoParameters`.
#[test]
fn ffi_native__infer_pointer_return_no_parameters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external Pointer foo();
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferPointerReturnPointerParameter`.
#[test]
fn ffi_native__infer_pointer_return_pointer_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external Pointer foo(Pointer x);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferPointerReturnStructParameter`.
#[test]
fn ffi_native__infer_pointer_return_struct_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external Pointer foo(MyStruct x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferPointerReturnUnionParameter`.
#[test]
fn ffi_native__infer_pointer_return_union_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external Pointer foo(MyUnion x);

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferStructReturnNoParameters`.
#[test]
fn ffi_native__infer_struct_return_no_parameters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external MyStruct foo();

final class MyStruct extends Struct {
  @Int8()
  external int value;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferStructReturnPointerParameter`.
#[test]
fn ffi_native__infer_struct_return_pointer_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external MyStruct foo(Pointer x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferStructReturnStructParameter`.
#[test]
fn ffi_native__infer_struct_return_struct_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external MyStruct foo(MyStruct x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferStructReturnUnionParameter`.
#[test]
fn ffi_native__infer_struct_return_union_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external MyStruct foo(MyUnion x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferUnionReturnNoParameters`.
#[test]
fn ffi_native__infer_union_return_no_parameters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external MyUnion foo();

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferUnionReturnPointerParameter`.
#[test]
fn ffi_native__infer_union_return_pointer_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external MyUnion foo(Pointer x);

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferUnionReturnStructParameter`.
#[test]
fn ffi_native__infer_union_return_struct_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external MyUnion foo(MyStruct x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferUnionReturnUnionParameter`.
#[test]
fn ffi_native__infer_union_return_union_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external MyUnion foo(MyUnion x);

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferVoidReturnNoParameters`.
#[test]
fn ffi_native__infer_void_return_no_parameters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external void foo();
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferVoidReturnPointerParameter`.
#[test]
fn ffi_native__infer_void_return_pointer_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external void foo(Pointer x);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferVoidReturnStructParameter`.
#[test]
fn ffi_native__infer_void_return_struct_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external void foo(MyStruct x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_InferVoidReturnUnionParameter`.
#[test]
fn ffi_native__infer_void_return_union_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Native()
external void foo(MyUnion x);

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeCanUseHandles`.
#[test]
fn ffi_native__native_can_use_handles() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Handle Function(Handle)>()
external Object doesntMatter(Object);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeCanUseLeaf`.
#[test]
fn ffi_native__native_can_use_leaf() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Int8 Function(Int64)>(isLeaf:true)
external int doesntMatter(int x);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeDuplicateAnnotation`.
#[test]
fn ffi_native__native_duplicate_annotation() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Int32 Function(Int32)>()
@Native<Int32 Function(Int32)>(isLeaf: true)
external int foo(int v);
"#,
        &[("ffi_native_invalid_multiple_annotations", 53, 6)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeDuplicateAnnotationConst`.
#[test]
fn ffi_native__native_duplicate_annotation_const() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

const duplicate = Native<Int32 Function(Int32)>(isLeaf: true);

@Native<Int32 Function(Int32)>()
@duplicate
external int foo(int v);
"#,
        &[("ffi_native_invalid_multiple_annotations", 118, 9)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeFromConst`.
#[test]
fn ffi_native__native_from_const() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

const annotation = Native<Int32 Function(Int32)>();

@annotation
external int wrongFfiReturnType(int v);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeInstanceMethodsMustHaveReceiver`.
#[test]
fn ffi_native__native_instance_methods_must_have_receiver() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class K {
  @Native<Void Function(Double)>()
  external void doesntMatter(double x);
}
"#,
        &[("ffi_native_unexpected_number_of_parameters_with_receiver", 80, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeLeafMustNotReturnHandle`.
#[test]
fn ffi_native__native_leaf_must_not_return_handle() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Handle Function()>(isLeaf:true)
external Object doesntMatter();
"#,
        &[("leaf_call_must_not_return_handle", 75, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeLeafMustNotReturnHandleConst`.
#[test]
fn ffi_native__native_leaf_must_not_return_handle_const() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
const annotation = Native<Handle Function()>(isLeaf:true);

@annotation
external Object doesntMatter();
"#,
        &[("leaf_call_must_not_return_handle", 107, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeLeafMustNotTakeHandles`.
#[test]
fn ffi_native__native_leaf_must_not_take_handles() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Void Function(Handle)>(symbol: 'DoesntMatter', isLeaf:true)
external void doesntMatter(Object o);
"#,
        &[("leaf_call_must_not_take_handle", 101, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeLeafMustNotTakeHandlesConst`.
#[test]
fn ffi_native__native_leaf_must_not_take_handles_const() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
const annotation = Native<Void Function(Handle)>(symbol: 'DoesntMatter', isLeaf:true);

@annotation
external void doesntMatter(Object o);
"#,
        &[("leaf_call_must_not_take_handle", 133, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeNonFfiParameter`.
#[test]
fn ffi_native__native_non_ffi_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<IntPtr Function(int)>()
external int nonFfiParameter(int v);
"#,
        &[("must_be_a_native_function_type", 64, 15)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeNonFfiReturnType`.
#[test]
fn ffi_native__native_non_ffi_return_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<double Function(IntPtr)>()
external double nonFfiReturnType(int v);
"#,
        &[("must_be_a_native_function_type", 70, 16)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativePointerParameter`.
#[test]
fn ffi_native__native_pointer_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Void Function(Pointer)>()
external void free(Pointer pointer);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeTooFewParameters`.
#[test]
fn ffi_native__native_too_few_parameters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Void Function(Double)>()
external void doesntMatter(double x, double y);
"#,
        &[("ffi_native_unexpected_number_of_parameters", 66, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeTooManyParameters`.
#[test]
fn ffi_native__native_too_many_parameters() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Void Function(Double, Double)>()
external void doesntMatter(double x);
"#,
        &[("ffi_native_unexpected_number_of_parameters", 74, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeVarArgs`.
#[test]
fn ffi_native__native_var_args() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Int8 Function(Int64, VarArgs<(Int32, Double)>)>()
external int doesntMatter(int x, int y, double z);
"#,
        &[],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeVarArgsTooFew`.
#[test]
fn ffi_native__native_var_args_too_few() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Int8 Function(Int64, VarArgs<(Int32, Double)>)>()
external int doesntMatter(int x, int y);
"#,
        &[("ffi_native_unexpected_number_of_parameters", 90, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeVarArgsTooMany`.
#[test]
fn ffi_native__native_var_args_too_many() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Int8 Function(Int64, VarArgs<(Int32, Double)>)>()
external int doesntMatter(int x, int y, double z, int superfluous);
"#,
        &[("ffi_native_unexpected_number_of_parameters", 90, 12)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeVoidReturn`.
#[test]
fn ffi_native__native_void_return() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<Handle Function(Uint32, Uint32, Handle)>()
external void voidReturn(int width, int height, Object outImage);
"#,
        &[("must_be_a_subtype", 84, 10)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeWrongFfiParameter`.
#[test]
fn ffi_native__native_wrong_ffi_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<IntPtr Function(Double)>()
external int wrongFfiParameter(int v);
"#,
        &[("must_be_a_subtype", 67, 17)],
    );
}

/// Dart `ffi_native_test.dart` `test_NativeWrongFfiReturnType`.
#[test]
fn ffi_native__native_wrong_ffi_return_type() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@Native<IntPtr Function(IntPtr)>()
external double wrongFfiReturnType(int v);
"#,
        &[("must_be_a_subtype", 70, 18)],
    );
}

/// Dart `field_must_be_external_in_struct_test.dart` `test_struct`.
#[test]
fn field_must_be_external_in_struct__struct() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class A extends Struct {
  @Int16()
  int a;
}
"#,
        &[("field_must_be_external_in_struct", 68, 1)],
    );
}

/// Dart `field_must_be_external_in_struct_test.dart` `test_union`.
#[test]
fn field_must_be_external_in_struct__union() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class A extends Union {
  @Int16()
  int a;
}
"#,
        &[("field_must_be_external_in_struct", 67, 1)],
    );
}

/// Dart `generic_struct_subclass_test.dart` `test_genericStruct`.
#[test]
fn generic_struct_subclass__generic_struct() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class S<T> extends Struct {
  external Pointer notEmpty;
}
"#,
        &[("generic_struct_subclass", 31, 1)],
    );
}

/// Dart `generic_struct_subclass_test.dart` `test_genericUnion`.
#[test]
fn generic_struct_subclass__generic_union() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class S<T> extends Union {
  external Pointer notEmpty;
}
"#,
        &[("generic_struct_subclass", 31, 1)],
    );
}

/// Dart `generic_struct_subclass_test.dart` `test_validStruct`.
#[test]
fn generic_struct_subclass__valid_struct() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class S extends Struct {
  external Pointer notEmpty;
}
"#,
        &[],
    );
}

/// Dart `invalid_exception_value_test.dart` `test_missing`.
#[test]
fn invalid_exception_value__missing() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef T = Void Function(Int8);
void f(int i) {}
void g() {
  Pointer.fromFunction<T>(f, 42);
}
"#,
        &[("invalid_exception_value", 109, 2)],
    );
}

/// Dart `invalid_field_type_in_struct_test.dart` `test_instance_invalid`.
#[test]
fn invalid_field_type_in_struct__instance_invalid() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  external String str;

  external Pointer notEmpty;
}
"#,
        &[("invalid_field_type_in_struct", 61, 6)],
    );
}

/// Dart `invalid_field_type_in_struct_test.dart` `test_instance_invalid2`.
#[test]
fn invalid_field_type_in_struct__instance_invalid2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Union {
  external String str;

  external Pointer notEmpty;
}
"#,
        &[("invalid_field_type_in_struct", 60, 6)],
    );
}

/// Dart `invalid_field_type_in_struct_test.dart` `test_instance_invalid3`.
#[test]
fn invalid_field_type_in_struct__instance_invalid3() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  external Pointer? p;
}
"#,
        &[("invalid_field_type_in_struct", 61, 8)],
    );
}

/// Dart `invalid_field_type_in_struct_test.dart` `test_instance_valid`.
#[test]
fn invalid_field_type_in_struct__instance_valid() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  external Pointer p;
}
"#,
        &[],
    );
}

/// Dart `invalid_field_type_in_struct_test.dart` `test_static`.
#[test]
fn invalid_field_type_in_struct__static() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  static String? str;

  external Pointer notEmpty;
}
"#,
        &[],
    );
}

/// Dart `mismatched_annotation_on_struct_field_test.dart` `test_double_on_int`.
#[test]
fn mismatched_annotation_on_struct_field__double_on_int() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  @Double()
  external int x;
}
"#,
        &[("mismatched_annotation_on_struct_field", 52, 9)],
    );
}

/// Dart `mismatched_annotation_on_struct_field_test.dart` `test_int32_on_double`.
#[test]
fn mismatched_annotation_on_struct_field__int32_on_double() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  @Int32()
  external double x;
}
"#,
        &[("mismatched_annotation_on_struct_field", 52, 8)],
    );
}

/// Dart `missing_annotation_on_struct_field_test.dart` `test_missing_int`.
#[test]
fn missing_annotation_on_struct_field__missing_int() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  external int x;
}
"#,
        &[("missing_annotation_on_struct_field", 61, 3)],
    );
}

/// Dart `missing_annotation_on_struct_field_test.dart` `test_notMissing`.
#[test]
fn missing_annotation_on_struct_field__not_missing() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  @Int32()
  external int x;
}
"#,
        &[],
    );
}

/// Dart `missing_exception_value_test.dart` `test_missing`.
#[test]
fn missing_exception_value__missing() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef T = Int8 Function(Int8);
int f(int i) => i * 2;
void g() {
  Pointer.fromFunction<T>(f);
}
"#,
        &[("missing_exception_value", 96, 12)],
    );
}

/// Dart `missing_field_type_in_struct_test.dart` `test_missing`.
#[test]
fn missing_field_type_in_struct__missing() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  external var str;

  external Pointer notEmpty;
}
"#,
        &[("missing_field_type_in_struct", 65, 3)],
    );
}

/// Dart `missing_field_type_in_struct_test.dart` `test_valid`.
#[test]
fn missing_field_type_in_struct__valid() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  external Pointer p;
}
"#,
        &[],
    );
}

/// Dart `missing_size_annotation_carray_test.dart` `test_one`.
#[test]
fn missing_size_annotation_carray__one() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  @Array(8)
  external Array<Uint8> a0;
}
"#,
        &[],
    );
}

/// Dart `missing_size_annotation_carray_test.dart` `test_two`.
#[test]
fn missing_size_annotation_carray__two() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  external Array<Uint8> a0;
}
"#,
        &[("missing_size_annotation_carray", 62, 12)],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn must_be_a_native_function_type__lookup_function() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef S = int Function(int);
typedef F = String Function(String);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
}
"#,
        &[("must_be_a_native_function_type", 137, 1)],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction_Pointer`.
#[test]
fn must_be_a_native_function_type__lookup_function_pointer() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef S = Void Function(Pointer);
typedef F = void Function(Pointer);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
}
"#,
        &[],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction_PointerNativeFunction`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn must_be_a_native_function_type__lookup_function_pointer_native_function() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef S = Void Function(Pointer<NativeFunction>);
typedef F = void Function(Pointer<NativeFunction>);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
}
"#,
        &[("must_be_a_native_function_type", 173, 1)],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction_PointerNativeFunction2`.
#[test]
fn must_be_a_native_function_type__lookup_function_pointer_native_function2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef S = Void Function(Pointer<NativeFunction<Int8 Function()>>);
typedef F = void Function(Pointer<NativeFunction<Int8 Function()>>);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
}
"#,
        &[],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction_PointerVoid`.
#[test]
fn must_be_a_native_function_type__lookup_function_pointer_void() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef S = Pointer<Void> Function(Pointer<Void>);
typedef F = Pointer<Void> Function(Pointer<Void>);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
}
"#,
        &[],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction_VarArgs1`.
#[test]
fn must_be_a_native_function_type__lookup_function_var_args1() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final lib = DynamicLibrary.open('dontcare');
final variadicAt1Doublex2 =
  lib.lookupFunction<
    Double Function(Double, VarArgs<(Double,)>),
    double Function(double, double)
  >(
    "VariadicAt1Doublex2"
  );
"#,
        &[],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction_VarArgs2`.
#[test]
fn must_be_a_native_function_type__lookup_function_var_args2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final lib = DynamicLibrary.open('dontcare');
final variadicAt1Int64x5Leaf =
  lib.lookupFunction<
    Int64 Function(Int64, VarArgs<(Int64, Int64, Int64, Int64)>),
    int Function(int, int, int, int, int)
  >(
    "VariadicAt1Int64x5",
    isLeaf:true
  );
"#,
        &[],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction_VarArgs3`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn must_be_a_native_function_type__lookup_function_var_args3() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final lib = DynamicLibrary.open('dontcare');
final variadicAt1Int64x5Leaf =
  lib.lookupFunction<
    Int64 Function(Int64, VarArgs<(Int64, Int64, Int64, Int64)>),
    int Function(int, int, int, int, double)
  >(
    "VariadicAt1Int64x5",
    isLeaf:true
  );
"#,
        &[("must_be_a_subtype", 187, 40)],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction_VarArgs4`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn must_be_a_native_function_type__lookup_function_var_args4() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final lib = DynamicLibrary.open('dontcare');
final variadicAt1Int64x5Leaf =
  lib.lookupFunction<
    Int64 Function(Int64, VarArgs<(Int64, Int64, Int64, {Int64 named})>),
    int Function(int, int, int, int)
  >(
    "VariadicAt1Int64x5",
    isLeaf:true
  );
"#,
        &[("must_be_a_native_function_type", 121, 68)],
    );
}

/// Dart `must_be_a_native_function_type_test.dart` `test_lookupFunction_VarArgs5`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn must_be_a_native_function_type__lookup_function_var_args5() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final lib = DynamicLibrary.open('dontcare');
final variadicAt1Int64x5Leaf =
  lib.lookupFunction<
    Int64 Function(Int64, VarArgs<(Int64, Int64, Int64)>, Int64),
    int Function(int, int, int, int, int)
  >(
    "VariadicAt1Int64x5",
    isLeaf:true
  );
"#,
        &[("must_be_a_native_function_type", 121, 60)],
    );
}

/// Dart `must_be_a_subtype_test.dart` `test_fromFunction_firstArgument`.
#[test]
fn must_be_a_subtype__from_function_first_argument() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef T = Int8 Function(Int8);
String f(int i) => i.toString();
void g() {
  Pointer.fromFunction<T>(f, 5);
}
"#,
        &[("must_be_a_subtype", 122, 1)],
    );
}

/// Dart `must_be_a_subtype_test.dart` `test_fromFunction_secondArgument`.
#[test]
fn must_be_a_subtype__from_function_second_argument() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef T = Int8 Function(Int8);
int f(int i) => i * 2;
void g() {
  Pointer.fromFunction<T>(f, '');
}
"#,
        &[("must_be_a_subtype", 115, 2)],
    );
}

/// Dart `must_be_a_subtype_test.dart` `test_fromFunction_valid_oneArgument`.
#[test]
fn must_be_a_subtype__from_function_valid_one_argument() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef T = Void Function(Int8);
void f(int i) {}
void g() {
  Pointer.fromFunction<T>(f);
}
"#,
        &[],
    );
}

/// Dart `must_be_a_subtype_test.dart` `test_fromFunction_valid_twoArguments`.
#[test]
fn must_be_a_subtype__from_function_valid_two_arguments() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef T = Int8 Function(Int8);
int f(int i) => i * 2;
void g() {
  Pointer.fromFunction<T>(f, 42);
}
"#,
        &[],
    );
}

/// Dart `must_be_a_subtype_test.dart` `test_fromFunction_valid_voidReturnPermissive`.
#[test]
fn must_be_a_subtype__from_function_valid_void_return_permissive() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
typedef T = Void Function(Int8);
int f(int i) => i * 2;
void g() {
  Pointer.fromFunction<T>(f);
}
"#,
        &[],
    );
}

/// Dart `non_constant_type_argument_test.dart` `test_ref_class`.
#[test]
fn non_constant_type_argument__ref_class() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Uint8()
  external int myField;
}

void main() {
  final pointer = Pointer<MyStruct>.fromAddress(0);
  pointer.ref.myField = 1;
}
"#,
        &[],
    );
}

/// Dart `non_constant_type_argument_test.dart` `test_ref_class_cascade`.
#[test]
fn non_constant_type_argument__ref_class_cascade() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Uint8()
  external int myField;
}

void main() {
  final pointer = Pointer<MyStruct>.fromAddress(0)
    ..ref.myField = 1;
  print(pointer);
}
"#,
        &[],
    );
}

/// Dart `non_constant_type_argument_test.dart` `test_ref_typeParameter`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn non_constant_type_argument__ref_type_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

T genericRef<T extends Struct>(Pointer<T> p) =>
    p.ref;
"#,
        &[("non_constant_type_argument", 72, 5)],
    );
}

/// Dart `non_constant_type_argument_test.dart` `test_refWithFinalizer_class`.
#[test]
fn non_constant_type_argument__ref_with_finalizer_class() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Uint8()
  external int myField;
}

void main() {
  final pointer = Pointer<MyStruct>.fromAddress(0);
  pointer.refWithFinalizer(nullptr).myField = 1;
}
"#,
        &[],
    );
}

/// Dart `non_constant_type_argument_test.dart` `test_refWithFinalizer_class_cascade`.
#[test]
fn non_constant_type_argument__ref_with_finalizer_class_cascade() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class MyStruct extends Struct {
  @Uint8()
  external int myField;
}

void main() {
  final pointer = Pointer<MyStruct>.fromAddress(0)
    ..refWithFinalizer(nullptr).myField = 1;
  print(pointer);
}
"#,
        &[],
    );
}

/// Dart `non_constant_type_argument_test.dart` `test_refWithFinalizer_typeParameter`.
#[test]
#[ignore = "the FFI members are extension members (asFunction, lookupFunction, address): needs the extension resolution of unit C6"]
fn non_constant_type_argument__ref_with_finalizer_type_parameter() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

T genericRefWithFinalizer<T extends Struct>(Pointer<T> p) =>
    p.refWithFinalizer(nullptr);
"#,
        &[("non_constant_type_argument", 85, 27)],
    );
}

/// Dart `non_native_function_type_argument_to_pointer_test.dart` `test_asFunction_Pointer_Opaque`.
#[test]
fn non_native_function_type_argument_to_pointer__as_function_pointer_opaque() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
main() {
  DynamicLibrary.open('dontcare')
      .lookup<NativeFunction<Void Function(Pointer<Opaque>)>>('dontcare')
      .asFunction<void Function(Pointer<Opaque>)>(isLeaf: true);
}
"#,
        &[],
    );
}

/// Dart `non_sized_type_argument_test.dart` `test_invalid_struct`.
#[test]
fn non_sized_type_argument__invalid_struct() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  @Array(8)
  external Array<Void> a0;
}
"#,
        &[("non_sized_type_argument", 80, 4)],
    );
}

/// Dart `non_sized_type_argument_test.dart` `test_invalid_union`.
#[test]
fn non_sized_type_argument__invalid_union() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Union {
  @Array(8)
  external Array<Void> a0;
}
"#,
        &[("non_sized_type_argument", 79, 4)],
    );
}

/// Dart `non_sized_type_argument_test.dart` `test_valid`.
#[test]
fn non_sized_type_argument__valid() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  @Array(8)
  external Array<Uint8> a0;
}
"#,
        &[],
    );
}

/// Dart `packed_annotation_alignment_test.dart` `test_error`.
#[test]
fn packed_annotation_alignment__error() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Packed(3)
final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}
"#,
        &[("packed_annotation_alignment", 28, 1)],
    );
}

/// Dart `packed_annotation_alignment_test.dart` `test_no_error`.
#[test]
fn packed_annotation_alignment__no_error() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Packed(1)
final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}
"#,
        &[],
    );
}

/// Dart `packed_annotation_test.dart` `test_error_double`.
#[test]
fn packed_annotation__error_double() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Packed(1)
@Packed(1)
final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}
"#,
        &[("packed_annotation", 31, 10)],
    );
}

/// Dart `packed_annotation_test.dart` `test_error_missing`.
#[test]
fn packed_annotation__error_missing() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Packed()
final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}
"#,
        &[("packed_annotation_alignment", 20, 9)],
    );
}

/// Dart `packed_annotation_test.dart` `test_no_error_struct_no_annotation`.
#[test]
fn packed_annotation__no_error_struct_no_annotation() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}
"#,
        &[],
    );
}

/// Dart `packed_annotation_test.dart` `test_no_error_struct_one_annotation`.
#[test]
fn packed_annotation__no_error_struct_one_annotation() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Packed(1)
final class C extends Struct {
  external Pointer<Uint8> notEmpty;
}
"#,
        &[],
    );
}

/// Dart `packed_annotation_test.dart` `test_no_error_union_no_annotation`.
#[test]
fn packed_annotation__no_error_union_no_annotation() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Union {
  external Pointer<Uint8> notEmpty;
}
"#,
        &[],
    );
}

/// Dart `packed_annotation_test.dart` `test_no_error_union_one_annotation`.
#[test]
fn packed_annotation__no_error_union_one_annotation() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Packed(1)
final class C extends Union {
  external Pointer<Uint8> notEmpty;
}
"#,
        &[],
    );
}

/// Dart `packed_annotation_test.dart` `test_no_error_union_two_annotations`.
#[test]
fn packed_annotation__no_error_union_two_annotations() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

@Packed(1)
@Packed(1)
final class C extends Union {
  external Pointer<Uint8> notEmpty;
}
"#,
        &[],
    );
}

/// Dart `size_annotation_dimensions_test.dart` `test_error_array_2_3`.
#[test]
fn size_annotation_dimensions__error_array_2_3() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  @Array(8, 8)
  external Array<Array<Array<Uint8>>> a0;
}
"#,
        &[("size_annotation_dimensions", 53, 12)],
    );
}

/// Dart `size_annotation_dimensions_test.dart` `test_error_array_3_2`.
#[test]
fn size_annotation_dimensions__error_array_3_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  @Array(8, 8, 8)
  external Array<Array<Uint8>> a0;
}
"#,
        &[("size_annotation_dimensions", 53, 15)],
    );
}

/// Dart `size_annotation_dimensions_test.dart` `test_error_multi_2_3`.
#[test]
fn size_annotation_dimensions__error_multi_2_3() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  @Array.multi([8, 8])
  external Array<Array<Array<Uint8>>> a0;
}
"#,
        &[("size_annotation_dimensions", 53, 20)],
    );
}

/// Dart `size_annotation_dimensions_test.dart` `test_no_error`.
#[test]
fn size_annotation_dimensions__no_error() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';

final class C extends Struct {
  @Array(8, 8)
  external Array<Array<Uint8>> a0;
}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Double`.
#[test]
fn subtype_of_ffi_class__double() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Double {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Double_language219`.
#[test]
fn subtype_of_ffi_class__double_language219() {
    assert_ffi_errors_in_code(
        r#"// @dart=2.19
import 'dart:ffi';
class C extends Double {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Finalizable`.
#[test]
fn subtype_of_ffi_class__finalizable() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Finalizable {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Float`.
#[test]
fn subtype_of_ffi_class__float() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Float {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int16`.
#[test]
fn subtype_of_ffi_class__int16() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Int16 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int32`.
#[test]
fn subtype_of_ffi_class__int32() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Int32 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int64`.
#[test]
fn subtype_of_ffi_class__int64() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Int64 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int8`.
#[test]
fn subtype_of_ffi_class__int8() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Int8 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Pointer`.
#[test]
fn subtype_of_ffi_class__pointer() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Pointer {
  external factory C();
}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Struct`.
#[test]
fn subtype_of_ffi_class__struct() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Struct {
  external Pointer notEmpty;
}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint16`.
#[test]
fn subtype_of_ffi_class__uint16() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Uint16 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint32`.
#[test]
fn subtype_of_ffi_class__uint32() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Uint32 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint64`.
#[test]
fn subtype_of_ffi_class__uint64() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Uint64 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint8`.
#[test]
fn subtype_of_ffi_class__uint8() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Uint8 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Union`.
#[test]
fn subtype_of_ffi_class__union() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C extends Union {
  external Pointer notEmpty;
}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Void`.
#[test]
fn subtype_of_ffi_class__void() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C extends Void {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Double`.
#[test]
fn subtype_of_ffi_class__double_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Double {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Double_language219`.
#[test]
fn subtype_of_ffi_class__double_language219_2() {
    assert_ffi_errors_in_code(
        r#"// @dart=2.19
import 'dart:ffi';
class C implements Double {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Double_prefixed`.
#[test]
fn subtype_of_ffi_class__double_prefixed() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi' as ffi;
class C implements ffi.Double {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Finalizable`.
#[test]
fn subtype_of_ffi_class__finalizable_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Finalizable {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Float`.
#[test]
fn subtype_of_ffi_class__float_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Float {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int16`.
#[test]
fn subtype_of_ffi_class__int16_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Int16 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int32`.
#[test]
fn subtype_of_ffi_class__int32_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Int32 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int64`.
#[test]
fn subtype_of_ffi_class__int64_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Int64 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int8`.
#[test]
fn subtype_of_ffi_class__int8_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Int8 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Pointer`.
#[test]
fn subtype_of_ffi_class__pointer_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Pointer {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Struct`.
#[test]
fn subtype_of_ffi_class__struct_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C implements Struct {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint16`.
#[test]
fn subtype_of_ffi_class__uint16_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Uint16 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint32`.
#[test]
fn subtype_of_ffi_class__uint32_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Uint32 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint64`.
#[test]
fn subtype_of_ffi_class__uint64_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Uint64 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint8`.
#[test]
fn subtype_of_ffi_class__uint8_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Uint8 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Union`.
#[test]
fn subtype_of_ffi_class__union_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C implements Union {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Void`.
#[test]
fn subtype_of_ffi_class__void_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C implements Void {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Double`.
#[test]
fn subtype_of_ffi_class__double_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Double {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Double_language219`.
#[test]
fn subtype_of_ffi_class__double_language219_2_2() {
    assert_ffi_errors_in_code(
        r#"// @dart=2.19
import 'dart:ffi';
class C with Double {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Double_prefixed`.
#[test]
fn subtype_of_ffi_class__double_prefixed_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi' as ffi;
class C with ffi.Double {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Float`.
#[test]
fn subtype_of_ffi_class__float_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Float {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int16`.
#[test]
fn subtype_of_ffi_class__int16_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Int16 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int32`.
#[test]
fn subtype_of_ffi_class__int32_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Int32 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int64`.
#[test]
fn subtype_of_ffi_class__int64_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Int64 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Int8`.
#[test]
fn subtype_of_ffi_class__int8_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Int8 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Pointer`.
#[test]
fn subtype_of_ffi_class__pointer_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Pointer {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Struct`.
#[test]
fn subtype_of_ffi_class__struct_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C with Struct {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint16`.
#[test]
fn subtype_of_ffi_class__uint16_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Uint16 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint32`.
#[test]
fn subtype_of_ffi_class__uint32_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Uint32 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint64`.
#[test]
fn subtype_of_ffi_class__uint64_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Uint64 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Uint8`.
#[test]
fn subtype_of_ffi_class__uint8_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Uint8 {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Union`.
#[test]
fn subtype_of_ffi_class__union_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class C with Union {}
"#,
        &[],
    );
}

/// Dart `subtype_of_ffi_class_test.dart` `test_Void`.
#[test]
fn subtype_of_ffi_class__void_2_2() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
class C with Void {}
"#,
        &[],
    );
}

/// Dart `subtype_of_struct_class_test.dart` `test_extends_struct`.
#[test]
fn subtype_of_struct_class__extends_struct() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class S extends Struct {
  external Pointer notEmpty;
}
final class C extends S {}
"#,
        &[("subtype_of_struct_class", 103, 1)],
    );
}

/// Dart `subtype_of_struct_class_test.dart` `test_extends_union`.
#[test]
fn subtype_of_struct_class__extends_union() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class S extends Union {
  external Pointer notEmpty;
}
final class C extends S {}
"#,
        &[("subtype_of_struct_class", 102, 1)],
    );
}

/// Dart `subtype_of_struct_class_test.dart` `test_implements_abi_specific_int`.
#[test]
fn subtype_of_struct_class__implements_abi_specific_int() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
@AbiSpecificIntegerMapping({
  Abi.androidArm: Uint32(),
})
final class AbiSpecificInteger1 extends AbiSpecificInteger {
  const AbiSpecificInteger1();
}
final class AbiSpecificInteger4 implements AbiSpecificInteger1 {
  const AbiSpecificInteger4();
}
"#,
        &[("subtype_of_struct_class", 216, 19)],
    );
}

/// Dart `subtype_of_struct_class_test.dart` `test_implements_struct`.
#[test]
fn subtype_of_struct_class__implements_struct() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class S extends Struct {}
final class C implements S {}
"#,
        &[("empty_struct", 31, 1), ("subtype_of_struct_class", 76, 1)],
    );
}

/// Dart `subtype_of_struct_class_test.dart` `test_implements_union`.
#[test]
fn subtype_of_struct_class__implements_union() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class S extends Union {}
final class C implements S {}
"#,
        &[("empty_struct", 31, 1), ("subtype_of_struct_class", 75, 1)],
    );
}

/// Dart `subtype_of_struct_class_test.dart` `test_with_struct`.
#[test]
fn subtype_of_struct_class__with_struct() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class S extends Struct {}
final class C with S {}
"#,
        &[("empty_struct", 31, 1), ("subtype_of_struct_class", 70, 1)],
    );
}

/// Dart `subtype_of_struct_class_test.dart` `test_with_union`.
#[test]
fn subtype_of_struct_class__with_union() {
    assert_ffi_errors_in_code(
        r#"import 'dart:ffi';
final class S extends Union {}
final class C with S {}
"#,
        &[("empty_struct", 31, 1), ("subtype_of_struct_class", 69, 1)],
    );
}
