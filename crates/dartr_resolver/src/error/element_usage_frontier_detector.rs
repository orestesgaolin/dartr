// Dart source: pkg/analyzer/lib/src/error/element_usage_frontier_detector.dart

//! `ElementUsageFrontierDetector`: an [`ElementUsageDetector`] that reports
//! only the usages from elements that are not in the usage set themselves
//! (the "usage frontier"): a deprecated member may use other deprecated
//! members.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use dartr_ast::NodeId;
use dartr_element::ElementId;

use super::UnitVerifier;
use super::element_usage_detector::{ElementUsageDetector, UsageSet};
use crate::element_metadata::WorkspacePackage;

/// Dart `ElementUsageFrontierDetector`.
pub struct ElementUsageFrontierDetector {
    pub detector: ElementUsageDetector,
}

impl Deref for ElementUsageFrontierDetector {
    type Target = ElementUsageDetector;
    fn deref(&self) -> &ElementUsageDetector {
        &self.detector
    }
}

impl DerefMut for ElementUsageFrontierDetector {
    fn deref_mut(&mut self) -> &mut ElementUsageDetector {
        &mut self.detector
    }
}

impl ElementUsageFrontierDetector {
    pub fn new(workspace_package: Option<Arc<WorkspacePackage>>, usages: Vec<UsageSet>) -> Self {
        let mut detector = ElementUsageDetector::new(workspace_package, usages);
        detector.in_element_stacks = Some(vec![vec![false]; detector.usages_metadata_only.len()]);
        ElementUsageFrontierDetector { detector }
    }

    /// Dart `checkUsage(element, node)`: nothing when every stack is
    /// inside a tagged element.
    pub fn check_usage(
        &mut self,
        v: &mut UnitVerifier<'_>,
        element: Option<ElementId>,
        node: NodeId,
    ) {
        let all_true = self
            .detector
            .in_element_stacks
            .as_ref()
            .is_some_and(|stacks| stacks.iter().all(|s| s.last().copied().unwrap_or(false)));
        if all_true {
            return;
        }
        self.detector.check_usage(v, element, node);
    }

    /// Dart `popElement()`.
    pub fn pop_element(&mut self) {
        if let Some(stacks) = &mut self.detector.in_element_stacks {
            for stack in stacks {
                stack.pop();
            }
        }
    }

    /// Dart `pushElement(element)`.
    pub fn push_element(&mut self, v: &UnitVerifier<'_>, element: Option<ElementId>) {
        let usages = self.detector.usages_metadata_only.clone();
        let Some(stacks) = &mut self.detector.in_element_stacks else {
            return;
        };
        for (i, stack) in stacks.iter_mut().enumerate() {
            let mut new_value = stack.last().copied().unwrap_or(false);
            if !new_value && let Some(element) = element {
                new_value = usages[i].get_tag_info(v, element).is_some();
            }
            stack.push(new_value);
        }
    }
}
