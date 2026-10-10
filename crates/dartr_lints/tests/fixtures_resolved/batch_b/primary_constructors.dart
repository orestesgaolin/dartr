// use_declaring_parameters: primary constructors (Dart 3.13).
class DeclaringFieldFormal(this.x) {
  final int x;
}

class DeclaringInitializer(int x) {
  final int x;
  this : x = x;
}

class DeclaringPrivate(int value) {
  final int _value;
  this : _value = value;
  int get value => _value;
}

class AlreadyDeclaring(final int x);
