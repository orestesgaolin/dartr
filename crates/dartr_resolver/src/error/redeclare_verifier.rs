// Dart source: pkg/analyzer/lib/src/error/redeclare_verifier.dart

//! `RedeclareVerifier`: members of extension types annotated with
//! `@redeclare` that do not redeclare a member of a superinterface
//! (`REDECLARE_ON_NON_REDECLARING_MEMBER`).
//!
//! `element.metadata.hasRedeclare` is read from the annotations of the
//! declaration node.

use dartr_ast::{Ast, AstVisitor, ExtensionTypeDeclaration, Id, MethodDeclaration};
use dartr_diagnostics::diag;
use dartr_element::{EId, ElemRef, ElementId, ExtensionTypeElement, Tag};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name as MemberName};
use dartr_typesystem::member;

use super::correct_override::{declared_element, has_annotation, token_range};
use super::{UnitVerifier, VerifierHost};

/// Dart `unit.accept(RedeclareVerifier(diagnosticReporter))`.
pub fn verify(v: &mut UnitVerifier<'_>) {
    let ast = v.ast;
    let unit = v.unit;
    let mut visitor = RedeclareVerifier {
        host: v,
        current_extension_type: None,
    };
    ast.accept(unit, &mut visitor);
}

/// Dart `RedeclareVerifier`.
pub struct RedeclareVerifier<'v, H> {
    host: &'v mut H,
    /// `_currentExtensionType`.
    current_extension_type: Option<EId<ExtensionTypeElement>>,
}

impl<'h, H: VerifierHost<'h>> RedeclareVerifier<'_, H> {
    /// `_redeclaresMember(member)`: whether [member] redeclares a member
    /// from a superinterface.
    fn redeclares_member(&self, member: ElementId) -> bool {
        let Some(current_type) = self.current_extension_type else {
            return false;
        };
        let ctx = self.host.ctx();
        match MemberName::for_element(&ctx, ElemRef::Base(member)) {
            Some(name) => InheritanceManager3::new(ctx)
                .get_inherited(current_type.upcast(), name)
                .is_some(),
            None => false,
        }
    }
}

impl<'h, H: VerifierHost<'h>> AstVisitor for RedeclareVerifier<'_, H> {
    fn visit_extension_type_declaration(&mut self, ast: &Ast, node: Id<ExtensionTypeDeclaration>) {
        self.current_extension_type =
            declared_element(self.host, node).and_then(|e| e.cast::<ExtensionTypeElement>());
        ast.visit_children(node, self);
        self.current_extension_type = None;
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        // Only check if we're in an extension type declaration.
        if self.current_extension_type.is_none() {
            return;
        }
        let Some(element) = declared_element(self.host, node) else {
            return;
        };
        let ctx = self.host.ctx();
        // Static members can't redeclare.
        if member::is_static(&ctx, ElemRef::Base(element)) {
            return;
        }
        if !has_annotation(self.host, ast[node].metadata, "meta", "redeclare")
            || self.redeclares_member(element)
        {
            return;
        }
        let kind = match element.tag() {
            Tag::Method => "method",
            Tag::Getter => "getter",
            Tag::Setter => "setter",
            _ => return,
        };
        let range = token_range(ast, ast[node].name);
        self.host
            .report(diag::redeclare_on_non_redeclaring_member(kind).at_offset(range.0, range.1));
    }
}
