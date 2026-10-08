class A<X extends A<X>> {}
class B<X extends B<X, Y>, Y extends List<X>> {}
class C<X extends C> {}
class D<X, Y extends X> {}
class E<X extends void Function(X)> {}
typedef F<X extends F<X>> = List<X>;
class G<in X, out Y, inout Z> {}
class H<T extends Comparable<T>> with M<T> {}
mixin M<T> {}
class I extends A<I> {}
class J {
  A raw = A();
  B? rawB;
}
