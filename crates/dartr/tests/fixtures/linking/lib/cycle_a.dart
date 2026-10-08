import 'cycle_b.dart';
export 'cycle_b.dart' show CycleB;

class CycleA extends CycleB {
  CycleB? other;
}
