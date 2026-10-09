part of 'generics.dart';

class B<U extends num> {
  final U value;
  B(this.value);
  List<U> twice() => [value, value];
}
