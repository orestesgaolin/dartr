//! Constant evaluation and verification (wave D, units D1–D3): ports of
//! `pkg/analyzer/lib/src/dart/constant/{evaluation,compute,utilities,
//! potentially_constant,constant_verifier}.dart`.
//!
//! # Where the expressions come from
//!
//! Dart evaluates the constant expressions that the linker stored in the
//! elements (`constantInitializer`, `constantInitializers`, default values,
//! `annotationAst`), resolved while linking, and maps error locations back
//! to the AST of the file under analysis (`ConstantEvaluationConfiguration`).
//! The linker of this port does not resolve its `ConstExprs` yet, so the
//! evaluator reads the *resolved unit ASTs* instead: the units of the
//! library under analysis, and, for an element of another library, the
//! resolved unit that declares it ([`evaluation::ExternalUnits`], resolved on
//! demand and cached by the caller). The values are the same; the error
//! locations need no mapping (an enum constant is evaluated at its
//! `EnumConstantDeclaration`, Dart's error node for the synthetic
//! initializer).
//!
//! # Results
//!
//! [`evaluation::ConstantValues`]: the `evaluationResult` of every evaluated
//! variable, formal parameter and annotation (Dart stores them in the
//! elements), kept in [`crate::library_analyzer::ResolvedLibrary`].

pub mod compute;
pub mod evaluation;
pub mod exhaustiveness;
pub mod potentially_constant;
pub mod utilities;
