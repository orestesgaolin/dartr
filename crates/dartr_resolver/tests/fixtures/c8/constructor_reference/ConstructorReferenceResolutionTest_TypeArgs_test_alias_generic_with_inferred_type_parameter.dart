// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest_TypeArgs.test_alias_generic_with_inferred_type_parameter).

class C<T> {
  final T x;
  C(this.x);
}
typedef Direct<T> = C<T>;
void main() {
  var x = const <C<int> Function(int)>[Direct.new];
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
}
