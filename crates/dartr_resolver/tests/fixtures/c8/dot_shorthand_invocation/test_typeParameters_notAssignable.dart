// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_typeParameters_notAssignable).

class C<T> {
  static C<int> member() => C(1);

  final T t;
  C(this.t);
}

void main() {
  C<bool> c = .member();
//            ^^^^^^^^^
// [diag.invalidAssignment] A value of type 'C<int>' can't be assigned to a variable of type 'C<bool>'.
  print(c);
}

