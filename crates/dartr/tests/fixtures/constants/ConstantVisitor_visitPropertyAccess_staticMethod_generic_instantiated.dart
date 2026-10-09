
import '' as self;
class C {
  static void f<T>(T a) {}
}
const void Function(int) g = self.C.f;
