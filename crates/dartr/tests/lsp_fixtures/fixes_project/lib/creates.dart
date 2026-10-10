class Target {
  int field = 0;
  void existing() {}
}

void creates(Target t) {
  t.missingMethod(1, 'a');
  print(t.missingGetter);
  t.missingSetter = 3;
  undefinedFunction(1);
  missingLocal;
  Target.namedMissing();
  MissingClass();
  print(MissingType);
}
