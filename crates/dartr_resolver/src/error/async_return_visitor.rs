// Dart source: pkg/analyzer/lib/src/error/async_return_visitor.dart

//! STUB (wd-errors): the `AsyncReturnVisitor` API that `ErrorVerifier` calls
//! (`generated/error_verifier.dart`). The functions report no diagnostics;
//! the error/* port (branch `wd-errors`) replaces this file.

use dartr_ast::NodeId;

use crate::error_verifier::ErrorVerifier;

/// Dart `node.accept(_asyncReturnVisitor)` (an `AsyncReturnVisitor` with
/// `withinTryBlock: true`): reports `unawaited_return_in_try_block`.
pub fn accept(ev: &mut ErrorVerifier<'_>, node: NodeId) {
    let _ = (ev, node);
}
