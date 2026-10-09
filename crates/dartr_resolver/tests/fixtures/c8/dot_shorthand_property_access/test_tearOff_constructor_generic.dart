// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_property_access_test.dart (DotShorthandPropertyAccessResolutionTest.test_tearOff_constructor_generic).

class C<T> {
  T t;
  C(this.t);
  C.id(this.t);
}

void main() {
  Object? o = C<int>(0);
  if (o is C<int>) {
    o = .new;
    if (o is Function) {
       o(1).t;
    }
  }
}
