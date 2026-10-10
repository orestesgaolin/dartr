@deprecated
void oldFn() {}

@Deprecated('use newClass')
class OldClass {
  @deprecated
  int oldField = 0;

  @Deprecated('use newMethod')
  void oldMethod([@deprecated int x = 0]) {}

  @override
  String toString() => '';
}
