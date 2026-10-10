// Dart source: pkg/analyzer/lib/src/error/experimental_member_use_verifier.dart

//! The experimental elements as a usage set of the
//! [`super::element_usage_detector::ElementUsageDetector`]
//! (`ExperimentalElementUsageSet`, `ExperimentalElementUsageReporter`).

use dartr_diagnostics::diag;
use dartr_element::{Ctx, ElementId};

use super::{UnitVerifier, VerifierHost};
use crate::element_metadata::{UnitAst, element_has, flags};

/// Dart `ExperimentalElementUsageSet.getTagInfo`.
pub fn get_tag_info(ctx: &Ctx<'_>, element: ElementId, unit: Option<UnitAst<'_>>) -> Option<()> {
    element_has(ctx, element, flags::EXPERIMENTAL, unit).then_some(())
}

/// Dart `ExperimentalElementUsageReporter.report`.
pub fn report(
    v: &mut UnitVerifier<'_>,
    error_entity: (u32, u32),
    display_name: &str,
    is_in_same_package: bool,
) {
    // Use of an experimental API from within the same package is OK.
    if is_in_same_package {
        return;
    }
    let d = diag::experimental_member_use(display_name);
    v.report(d.at_offset(error_entity.0 as usize, error_entity.1 as usize));
}
