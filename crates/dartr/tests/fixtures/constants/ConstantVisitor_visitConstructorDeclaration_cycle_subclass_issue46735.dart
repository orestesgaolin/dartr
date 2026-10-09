
void main() {
  const EmptyInjector();
}

abstract class BaseInjector {
  final BaseInjector parent;

  const BaseInjector([BaseInjector? parent])
//      ^^^^^^^^^^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
      : parent = parent ?? const EmptyInjector();
}

abstract class Injector implements BaseInjector {
  const Injector();
}

class EmptyInjector extends BaseInjector implements Injector {
  const EmptyInjector();
//      ^^^^^^^^^^^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
}
