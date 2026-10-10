import 'package:meta/meta.dart';

@optionalTypeArgs
class Optional<T> {
  Optional();
}

class Required<T> {
  Required();
}

@optionalTypeArgs
void optionalFunction<T>() {}

void requiredFunction<T>() {}

class WithPrimary(Map<int, int> values) {
  final Map<int, int> copy = values;
  final List<int> keys = values.keys.toList();

  Map<int, int> get all => copy;
  List<int> get allKeys => keys;
}

void main() {
  Optional();
  Required();
  optionalFunction();
  requiredFunction();
  print(WithPrimary({}).all);
}
