// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/nullability_suffix.dart

//! `NullabilitySuffix`.

/// Suffix indicating the nullability of a type.
///
/// This enum describes whether a `?` or `*` would be used at the end of the
/// canonical representation of a type. It's subtly different the notions of
/// "nullable", "non-nullable", "potentially nullable", and "potentially
/// non-nullable" defined by the spec. For example, the type `Null` is
/// nullable, even though it lacks a trailing `?`.
///
/// Exported by `package:analyzer/dart/element/nullability_suffix.dart`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum NullabilitySuffix {
    /// The canonical representation of the type ends with `?`. Types having
    /// this nullability suffix should be interpreted as being unioned with
    /// the Null type.
    Question,

    /// The canonical representation of the type ends with `*`. Types having
    /// this nullability suffix are called "legacy types".
    Star,

    /// The canonical representation of the type does not end with either `?`
    /// or `*`.
    None,
}
