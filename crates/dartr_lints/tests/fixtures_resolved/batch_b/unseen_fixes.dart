// Reduced from bench/corpus/analyzer-9.0.0 (lib/src/fine/requirements.dart,
// lib/src/dart/resolver/scope.dart).
class Writer {
  void writeBool(bool value) {}
}

extension _WriterExtension on Writer {
  void writeOptionalBool(bool? value) {
    if (value != null) writeBool(value);
  }
}

class Base {
  void m(bool flag) {}
}

class Sub extends Base {
  @override
  void m(bool flag) {}
}

void useWriter(Writer w) => w.writeOptionalBool(null);

Map<String, int> hide(Map<String, int> definedNames, List<String> hiddenNames) {
  var newNames = {...definedNames};
  for (var name in hiddenNames) {
    newNames.remove(name);
  }
  var list = [...hiddenNames];
  var set = {...hiddenNames};
  print(list);
  print(set);
  return newNames;
}
