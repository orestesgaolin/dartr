// Dart source: pkg/linter/lib/src/rules/avoid_slow_async_io.dart

use super::helpers::{element_library_uri, element_name};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::{Id, MethodInvocation, NodeId, NodeKind};
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::ElemRef;
use dartr_typesystem::member;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(NodeKind::MethodInvocation, "avoid_slow_async_io", check);
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = c.resolved else { return };
    let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
    let Some(element) = c.element(n.method_name) else {
        return;
    };
    if element_library_uri(c, element) != Some("dart:io") {
        return;
    }
    let Some(enclosing) = member::enclosing_element(&r.ctx, element) else {
        return;
    };
    let class_name = element_name(c, ElemRef::Base(enclosing));
    let method_name = element_name(c, element);
    let slow = class_name == Some("File") && method_name == Some("lastModified")
        || class_name == Some("FileSystemEntity")
            && matches!(
                method_name,
                Some("exists" | "isDirectory" | "isFile" | "isLink" | "stat" | "type")
            );
    if slow {
        c.report_node(out, &diag::AVOID_SLOW_ASYNC_IO, node, &[]);
    }
}
