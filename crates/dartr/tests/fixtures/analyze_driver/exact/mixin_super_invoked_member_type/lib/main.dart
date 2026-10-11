abstract class Iface {
  void foo({int a = 0});
}

class Base implements Iface {
  @override
  void foo({int a = 0, int b = 1}) {}
}

mixin M on Iface {
  void bar() {
    super.foo(a: 1);
  }
}

class Sub extends Base with M {}
