
void main() {
  const RequiresNonEmptyList([1]);
//^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalPropertyAccess][context 1] The property 'length' can't be accessed on the type 'List<int>' in a constant expression.
}

class RequiresNonEmptyList {
  const RequiresNonEmptyList(List<int> numbers) : assert(numbers.length > 0);
//                                                       ^^^^^^^^^^^^^^
// [context 1] The error is in the assert initializer of 'RequiresNonEmptyList', and occurs here.
}
