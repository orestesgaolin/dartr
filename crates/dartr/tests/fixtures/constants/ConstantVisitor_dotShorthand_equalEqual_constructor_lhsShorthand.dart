
class A {
  const A();
}

const v = .new() == A();
//        ^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//         ^^^
// [diag.dotShorthandUndefinedInvocation] The static method or constructor 'new' isn't defined for the context type '_'.
