class Primary(final int value) {}

class InitializeClass(int input) {
  final int stored;
  this : stored = input;
}

enum const InitializeEnum(int input) {
  one(1);

  final int stored;
  this : stored = input;
}

void parameter(final int value) {}
