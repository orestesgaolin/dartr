abstract class Base {
  final (int, int)? pair;
  Base([int? x]) : pair = null;
  Base.protected([this.pair]);
}

class Sub(int value, [super.pair]) extends Base {
  this : super.protected();

  Sub._(int value, [super.pair]) : this(value);
}

Sub makeSub((int, int) p) => Sub._(1, p);
