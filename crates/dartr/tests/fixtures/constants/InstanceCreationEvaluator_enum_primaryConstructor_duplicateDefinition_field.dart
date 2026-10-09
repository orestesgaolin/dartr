
enum E(final int x) {
//               ^
// [context 1] The first definition of this name.
  v(2);

  final int x = 1;
//          ^
// [diag.duplicateDefinition][context 1] The name 'x' is already defined.
}

const a = E.v;
