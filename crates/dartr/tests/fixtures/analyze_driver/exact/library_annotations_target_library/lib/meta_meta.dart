library meta_meta;

class Target {
  final Set<TargetKind> kinds;
  const Target(this.kinds);
}

class TargetKind {
  final String name;
  const TargetKind._(this.name);
  static const library = TargetKind._('library');
}
