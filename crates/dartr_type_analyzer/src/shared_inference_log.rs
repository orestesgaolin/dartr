// Dart source: pkg/_fe_analyzer_shared/lib/src/type_inference/shared_inference_log.dart

//! The shared inference log (not ported).
//!
//! In Dart, `shared_inference_log.dart` defines `SharedInferenceLogWriter`, a
//! debugging aid that records the steps of type inference (expressions,
//! statements, constraint generation, generic inference) and can dump them
//! when an assertion fails. The log only observes inference: it never changes
//! a type, a constraint or a diagnostic. So it does not affect the output of
//! dartr, and it is not ported.
//!
//! The ported code of
//! [`type_analyzer_operations`](crate::type_analyzer_operations) does not
//! call the log (the Dart `type_analyzer_operations.dart` does not import
//! it). Other ported files that call `inferenceLogWriter?.…` drop these
//! calls; this module is the place to add no-op hooks if a later port needs
//! a call site to keep the Dart structure.
