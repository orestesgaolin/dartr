import 'meta.dart';

@optionalTypeArgs
class OptionalBox<T> {}

class StrictBox<T> {}

void f(OptionalBox a, StrictBox b) {}
