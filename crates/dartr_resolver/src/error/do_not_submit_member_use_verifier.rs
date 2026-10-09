// Dart source: pkg/analyzer/lib/src/error/do_not_submit_member_use_verifier.dart

//! The elements annotated with `@doNotSubmit` as a usage set of the
//! [`super::element_usage_detector::ElementUsageDetector`]
//! (`DoNotSubmitElementUsageSet`, `DoNotSubmitElementUsageReporter`).

use dartr_diagnostics::diag;
use dartr_element::{Ctx, ElementId};

use super::{UnitVerifier, VerifierHost};
use crate::element_metadata::{UnitAst, element_has, flags};

/// Dart `DoNotSubmitElementUsageSet.getTagInfo`.
pub fn get_tag_info(ctx: &Ctx<'_>, element: ElementId, unit: Option<UnitAst<'_>>) -> Option<()> {
    element_has(ctx, element, flags::DO_NOT_SUBMIT, unit).then_some(())
}

/// Dart `DoNotSubmitElementUsageReporter.report`.
pub fn report(v: &mut UnitVerifier<'_>, error_entity: (u32, u32), display_name: &str) {
    let d = diag::invalid_use_of_do_not_submit_member(display_name);
    v.report(d.at_offset(error_entity.0 as usize, error_entity.1 as usize));
}
