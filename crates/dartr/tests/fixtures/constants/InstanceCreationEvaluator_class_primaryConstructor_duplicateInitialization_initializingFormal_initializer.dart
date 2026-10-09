
class const A(this.x) {
//            ^^^^^^
// [diag.initializingFormalForNonExistentField] 'x' isn't a field in the enclosing class.
  this : x = 2;
//       ^^^^^
// [diag.initializerForNonExistentField] 'x' isn't a field in the enclosing class.
}

const a = A(1);
