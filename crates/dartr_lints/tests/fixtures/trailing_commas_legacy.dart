// @dart = 3.6
void trailing(int value) {}
void calls() {
  trailing(
    1
  );
  var missingComma = <int>[
    1
  ];
  var hasComma = <int>[
    1,
  ];
  assert(missingComma.length == hasComma.length);
}
