/// Text [a].
///
///     indented [notRef]
///     more
///
/// ```dart
/// fenced [notRef]
/// ```
///
/// ````
/// four ticks [notRef]
/// ````
/// after [b]
///
///   ```
///   indented fence
///   ```
/// [c]
class A {}

/// ```
/// unterminated fence [d]
class B {}

///     indented first [e]
/// not indented [f]
class C {}

/**
 * ```html
 * <a>[x]</a>
 * ```
 *
 *     code [y]
 * text [z]
 */
class D {}
