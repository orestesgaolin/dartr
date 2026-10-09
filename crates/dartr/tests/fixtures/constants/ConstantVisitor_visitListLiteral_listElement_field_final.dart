
class A {
  final String bar = '';
  const A();
  List<String> foo() => const [bar];
//                             ^^^
// [diag.nonConstantListElement] The values in a const list literal must be constants.
}
