
class C<T> {
  const C.named();
}

const x = C<int>.();
//        ^^^^^^^^
// [diag.classInstantiationAccessToUnknownMember] The class 'C' doesn't have a constructor named '('.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//               ^
// [diag.missingIdentifier] Expected an identifier.
