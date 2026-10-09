// Dart source: pkg/analyzer/lib/src/error/annotation_verifier.dart

//! STUB (wd-errors): not ported yet. Entry points used by
//! `best_practices_verifier`.

use std::sync::Arc;

use dartr_ast::{Annotation, Id};

use super::UnitVerifier;
use crate::element_metadata::WorkspacePackage;

/// Dart `AnnotationVerifier`.
pub struct AnnotationVerifier {
    /// Dart `_workspacePackage`.
    pub workspace_package: Option<Arc<WorkspacePackage>>,
    /// Dart `_inPackagePublicApi`.
    pub in_package_public_api: bool,
}

impl AnnotationVerifier {
    pub fn new(v: &UnitVerifier<'_>, workspace_package: Option<Arc<WorkspacePackage>>) -> Self {
        let in_package_public_api = workspace_package.as_ref().is_some_and(|p| {
            let library = v.ctx.get(v.library);
            let path = &v.ctx.fragment(library.first_fragment()).source.path;
            p.source_is_in_public_api(path)
        });
        AnnotationVerifier {
            workspace_package,
            in_package_public_api,
        }
    }

    /// Dart `checkAnnotation(node)`.
    pub fn check_annotation(&self, v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
        let _ = (v, node);
    }
}
