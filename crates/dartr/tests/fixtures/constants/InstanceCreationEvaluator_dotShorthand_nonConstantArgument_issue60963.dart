
class A {
  int cannotBeConst;
  A(): cannotBeConst = 0;
}
extension type const B(A a) {}

const B b = .new(A());
//               ^^^
// [diag.constWithNonConst] The constructor being called isn't a const constructor.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
