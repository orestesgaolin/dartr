// @dart = 3.12
void untyped(var value) {}
void typed(var int value) {}
class VarParameter {
  int value;
  VarParameter(var this.value);
}
