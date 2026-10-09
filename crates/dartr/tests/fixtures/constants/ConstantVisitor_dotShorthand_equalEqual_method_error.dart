
class A {
  static A method() => A();
}

const v = A() == .method();
//        ^^^
// [diag.constWithNonConst] The constructor being called isn't a const constructor.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
