// Override inference with mixins, mixin inference and constructors
// (_MixinInference, InstanceMemberInferrer._inferConstructor).

abstract class I<T> {
  T get value;
  void accept(T t);
}

mixin M<T> on I<T> {
  get value;
  accept(t);
  set extra(covariant T t) {}
}

class Base implements I<int> {
  int get value => 0;
  void accept(int t) {}
}

class WithM extends Base with M {}

class Alias = Base with M;

mixin N<X, Y> on I<X> {}

class WithN extends Base with N {}

class Bounded<T extends num> {}

mixin MB<T extends num> on Bounded<T> {}

class WithMB extends Bounded<double> with MB {}

mixin NoConstraint<T extends Comparable<T>> {}

class WithNoConstraint with NoConstraint {}

class SubWithM extends WithM {
  set extra(int t) {}
  get value => 1;
}

class Fields {
  final int a;
  var b;
  final c;
  Fields(this.a, this.b, this.c);
  Fields.named({this.a = 0, this.b, this.c});
}

class SuperParams extends Fields {
  SuperParams(super.a, super.b, super.c);
  SuperParams.named({super.a, super.b});
}

class GenericSuper<T> {
  final T t;
  GenericSuper(this.t);
}

class GenericSub extends GenericSuper<String> {
  GenericSub(super.t);
}

mixin Mixin2 {}

class MixinApp = GenericSuper<double> with Mixin2;

class Overrides extends Fields {
  Overrides() : super(0, 0, 0);
  get a => 1;
  set b(value) {}
}
