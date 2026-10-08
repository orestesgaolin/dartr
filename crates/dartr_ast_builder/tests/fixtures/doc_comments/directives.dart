/// {@template foo}
/// Text [a].
/// {@endtemplate}
class A {}

/// {@template unclosed}
/// Text.
class B {}

/// {@endtemplate}
class C {}

/// {@unknownDirective arg}
/// {@youtube 560 315 https://www.youtube.com/watch?v=abc}
/// {@animation 300 200 https://x.y/z id=named}
/// {@macro foo
/// {@tool snippet}
/// {@inject-html}
/// <b>x</b>
/// {@end-tool}
/// {@category A B C}
/// {@subCategory X}
/// {@canonicalFor a.b}
/// {@example /path/to/file.dart region=r lang=dart}
/// {@macro name=}
/// {@
/// {@}
class D {}

/// @nodoc
class E {}

/// @nodocx [r]
/// text @nodoc
class F {}

/// {@template outer}
/// {@template inner}
/// {@endtemplate}
class G {}
