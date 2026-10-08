/// Simple [a], prefixed [a.b], three parts [a.b.c].
/// Constructors [new A], [new A.named], [A.new], [a.A.new].
/// Operators [operator +], [+], [operator []], [==], [operator <=].
/// Keywords [this], [null], [true] and [List<int>] are not references.
/// Empty [] and spaced [ a ] and [a b].
/// Link text [a](b) and [c] [d] and [e]
/// [f]: http://example.com
/// [:code:] and `[g]` in code, ``[h]`` and `unterminated [i]
/// Unterminated [j
class A {
  /// Synthetic [a.
  int x = 0;

  /// Recovery [new
  void m() {}

  /// [k.l.
  int y = 1;
}

/**
 * Multi-line [a] and [b.c].
 *
 * * star [d]
 *[e]
 *
 */
void f() {}

/** One line [x]. */
var g = 1;

/// Before
// plain comment [no]
/// after [z]
int h = 0;

/// Ünïcödé 😀 text before [ref] and [other.ref].
class B {}

/// Mixed /** */ inner [ok]
typedef T = int;
