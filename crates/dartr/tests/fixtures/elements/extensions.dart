// Fixture for `elements`: named and unnamed extensions, extension types.

extension StringX on String {
  int get doubled => length * 2;
  String shout() => toUpperCase();
  static int zero() => 0;
}

extension<T> on List<T> {
  T? get firstOrNull => isEmpty ? null : first;
}

extension type const Meters(double value) implements double {
  Meters.zero() : this(0);
  Meters operator +(Meters other) => Meters(value + other.value);
}

extension type Id<T>._(int raw) {
  factory Id(int raw) = Id._;
}
