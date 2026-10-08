/// A library doc with [List] and [dart:core] references.
///
/// {@template t}
/// Template body with [Map.from] and [Map.new].
/// {@endtemplate}
/// @docImport 'dart:math' as math;
library;

/// Code blocks:
///
///     indented code [notARef]
///
/// ```dart
/// fenced [notARef]
/// ```
///
/// Inline `code [notARef]` and [:old style:] and [link](http://x) and
/// [ref]: http://y and [a] [b] and [new A] and [A.b.c] and [operator +]
/// and [+] and [this] and [unterminated
/// @nodoc
class A {
  /** Multi-line doc with [A] and [b].
   *
   * {@macro t}
   * {@unknownDirective}
   * {@youtube 560 315 https://www.youtube.com/watch?v=abc
   * {@animation 100 200 http://a id=b}
   * {@end-tool}
   * {@tool snippet}
   * ```
   * code
   * ```
   */
  int b = 0;

  /// Ünïcödé 😀 text before [A] and [b].
  void m() {}
}

/// Comment before metadata.
@deprecated
/// Comment after metadata.
class B {}

/// @docImport 'package:foo/foo.dart' show Foo;
/// @docImport 'dart:async' hide;
/// @docImport broken
/// @docImport 'dart:io' as;
void f() {}
