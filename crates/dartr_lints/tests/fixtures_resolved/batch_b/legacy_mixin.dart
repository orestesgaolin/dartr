// @dart=2.19
// prefer_mixin applies to classes used as mixins before Dart 3.
class Legacy {}

class UsesLegacy with Legacy {}

mixin RealMixin {}

class UsesMixin with RealMixin {}

// prefer_final_parameters applies without primary constructors.
void legacyParams(int x, {String? y}) {
  print(x);
  print(y);
}

class LegacyParams {
  LegacyParams(int a) {
    print(a);
  }
  void m(int b) => print(b);
}
