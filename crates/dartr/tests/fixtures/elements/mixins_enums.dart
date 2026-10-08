// Fixture for `elements`: mixins (on, super invocations), enums with
// members.

class Base {
  void run() {}
  int get value => 0;
}

base mixin Logger on Base {
  @override
  void run() {
    super.run();
    print(super.value);
  }
}

mixin Plain<T> {}

enum Color { red, green, blue }

enum Planet implements Comparable<Planet> {
  earth(1.0),
  mars(0.5);

  const Planet(this.mass);
  final double mass;

  static Planet get heaviest => earth;
  bool get isHeavy => mass > 0.8;

  @override
  int compareTo(Planet other) => mass.compareTo(other.mass);
}
