import 'reexporter.dart';
import 'reexporter.dart' as p;

void main() {
  ExportedClass();
  exportedFn();
  var _ = exportedGetter;
  exportedSetter = 2;
  p.ExportedClass();
  p.exportedFn();
  var _ = p.exportedGetter;
  p.exportedSetter = 3;
}
