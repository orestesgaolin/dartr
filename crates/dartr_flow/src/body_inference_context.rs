// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/body_inference_context.dart

//! `SharedBodyInferenceContext`.

use crate::shared_type::SharedTypeSchemaView;

/// The part of the client's body inference context that the shared type
/// analyzer needs (`TypeAnalyzer.bodyContext`).
///
/// `T` is the client's type structure. Object safe.
pub trait SharedBodyInferenceContext<T> {
    /// Returns `true` if this is an `async` or an `async*` function.
    fn is_async(&self) -> bool;

    /// The typing expectation for the subexpression of a `yield` statement
    /// inside the function. For `sync*` and `async*` functions, the expected
    /// type is the element type of the generated `Iterable` or `Stream`.
    fn shared_yield_context(&self) -> SharedTypeSchemaView<T>;
}
