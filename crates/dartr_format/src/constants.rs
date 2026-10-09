// Dart source: dart_style lib/src/constants.dart

/// Constants for the cost heuristics used to determine which set of splits is
/// most desirable.
pub struct Cost;

impl Cost {
    /// The cost of splitting after the `=>` in a lambda or arrow-bodied member.
    ///
    /// We make this zero because there is already a span around the entire body
    /// and we generally do prefer splitting after the `=>` over other places.
    pub const ARROW: i32 = 0;

    /// The default cost.
    ///
    /// This isn't zero because we want to ensure all splitting has *some* cost,
    /// otherwise, the formatter won't try to keep things on one line at all.
    /// Most splits and spans use this. Greater costs tend to come from a greater
    /// number of nested spans.
    pub const NORMAL: i32 = 1;

    /// Splitting after a "=".
    pub const ASSIGN: i32 = 1;

    /// Splitting after a "=" when the right-hand side is a collection or cascade.
    pub const ASSIGN_BLOCK: i32 = 2;

    /// Splitting before the first argument when it happens to be a function
    /// expression with a block body.
    pub const FIRST_BLOCK_ARGUMENT: i32 = 2;

    /// The series of positional arguments.
    pub const POSITIONAL_ARGUMENTS: i32 = 2;

    /// Splitting inside the brackets of a list with only one element.
    pub const SINGLE_ELEMENT_LIST: i32 = 2;

    /// Splitting the internals of block arguments.
    ///
    /// Used to prefer splitting at the argument boundary over splitting the block
    /// contents.
    pub const SPLIT_BLOCKS: i32 = 2;

    /// Splitting on the "." in a named constructor.
    pub const CONSTRUCTOR_NAME: i32 = 4;

    /// Splitting a `[...]` index operator.
    pub const INDEX: i32 = 4;

    /// Splitting before a type argument or type parameter.
    pub const TYPE_ARGUMENT: i32 = 4;

    /// Split between a formal parameter name and its type.
    pub const PARAMETER_TYPE: i32 = 4;
}

/// Constants for the number of spaces for various kinds of indentation.
pub struct Indent;

impl Indent {
    /// Reset back to no indentation.
    pub const NONE: i32 = 0;

    /// The number of spaces in a block or collection body.
    pub const BLOCK: i32 = 2;

    /// How much wrapped cascade sections indent.
    pub const CASCADE: i32 = 2;

    /// The number of spaces in a single level of expression nesting.
    pub const EXPRESSION: i32 = 4;

    /// The ":" on a wrapped constructor initialization list.
    pub const CONSTRUCTOR_INITIALIZER: i32 = 4;

    /// A wrapped constructor initializer after the first one when the parameter
    /// list does not have optional or named parameters, like:
    ///
    ///     Constructor(
    ///       parameter,
    ///     ) : first,
    ///         second;
    ///       ^^ This indentation.
    pub const INITIALIZER: i32 = 2;

    /// A wrapped constructor initializer after the first one when the parameter
    /// list has optional or named parameters, like:
    ///
    ///     Constructor([
    ///       parameter,
    ///     ]) : first,
    ///          second;
    ///       ^^^ This indentation.
    pub const INITIALIZER_WITH_OPTIONAL_PARAMETER: i32 = 3;
}
