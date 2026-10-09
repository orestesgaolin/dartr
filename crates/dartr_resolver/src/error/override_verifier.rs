// Dart source: pkg/analyzer/lib/src/error/override_verifier.dart

//! `OverrideVerifier`: members annotated with `@override` that do not
//! override a member of a superinterface
//! (`OVERRIDE_ON_NON_OVERRIDING_{FIELD,GETTER,METHOD,SETTER}`).
//!
//! `element.metadata.hasOverride` is read from the annotations of the
//! declaration node (the annotations of the first fragment, which is the
//! node in this unit).

use dartr_ast::{
    Ast, AstVisitor, ClassDeclaration, EnumDeclaration, FieldDeclaration, FunctionDeclaration, Id,
    MethodDeclaration, MixinDeclaration, NodeId, PrimaryConstructorDeclaration,
};
use dartr_diagnostics::diag;
use dartr_element::{EId, ElemRef, ElementId, FieldElement, InterfaceElement, Tag};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, Name as MemberName};

use super::correct_override::{declared_element, first_fragment, has_annotation, token_range};
use super::{UnitVerifier, VerifierHost};
use crate::ast_ext::formal_parameter_parts;

/// Dart `unit.accept(OverrideVerifier(diagnosticReporter))`.
pub fn verify(v: &mut UnitVerifier<'_>) {
    let ast = v.ast;
    let unit = v.unit;
    let mut visitor = OverrideVerifier {
        host: v,
        current_class: None,
    };
    ast.accept(unit, &mut visitor);
}

/// Dart `OverrideVerifier`.
pub struct OverrideVerifier<'v, H> {
    host: &'v mut H,
    /// `_currentClass`: the current class, enum or mixin.
    current_class: Option<EId<InterfaceElement>>,
}

impl<'h, H: VerifierHost<'h>> OverrideVerifier<'_, H> {
    fn with_class(&mut self, ast: &Ast, node: NodeId) {
        self.current_class =
            declared_element(self.host, node).and_then(|e| e.cast::<InterfaceElement>());
        ast.visit_children(node, self);
        self.current_class = None;
    }

    /// `_checkField(fieldElement, errorNode)`.
    fn check_field(&mut self, field: EId<FieldElement>, error_node: (usize, usize)) {
        let ctx = self.host.ctx();
        let data = ctx.get(field);
        if let Some(getter) = data.getter
            && self.is_override(getter.raw())
        {
            return;
        }
        if let Some(setter) = data.setter
            && self.is_override(setter.raw())
        {
            return;
        }
        self.host
            .report(diag::override_on_non_overriding_field().at_offset(error_node.0, error_node.1));
    }

    /// `_isOverride(member)`: whether [member] overrides a member from a
    /// superinterface.
    fn is_override(&self, member: ElementId) -> bool {
        let Some(current_class) = self.current_class else {
            return false;
        };
        let ctx = self.host.ctx();
        if first_fragment(&ctx, current_class.raw()).is_none() {
            return false;
        }
        // We get `null` if the element does not have name. This was already
        // a parse error, avoid the warning.
        let Some(name) = MemberName::for_element(&ctx, ElemRef::Base(member)) else {
            return true;
        };
        InheritanceManager3::new(ctx)
            .get_overridden(current_class, name)
            .is_some()
    }
}

impl<'h, H: VerifierHost<'h>> AstVisitor for OverrideVerifier<'_, H> {
    fn visit_class_declaration(&mut self, ast: &Ast, node: Id<ClassDeclaration>) {
        self.with_class(ast, node.raw());
    }

    fn visit_enum_declaration(&mut self, ast: &Ast, node: Id<EnumDeclaration>) {
        self.with_class(ast, node.raw());
    }

    fn visit_mixin_declaration(&mut self, ast: &Ast, node: Id<MixinDeclaration>) {
        self.with_class(ast, node.raw());
    }

    fn visit_field_declaration(&mut self, ast: &Ast, node: Id<FieldDeclaration>) {
        let has_override = has_annotation(self.host, ast[node].metadata, "dart.core", "override");
        let fields = ast[node].fields;
        for &field in ast.list(ast[fields].variables) {
            let Some(field_element) =
                declared_element(self.host, field).and_then(|e| e.cast::<FieldElement>())
            else {
                continue;
            };
            if has_override {
                self.check_field(field_element, token_range(ast, ast[field].name));
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        if let Some(element) = declared_element(self.host, node)
            && has_annotation(self.host, ast[node].metadata, "dart.core", "override")
        {
            let range = token_range(ast, ast[node].name);
            match element.tag() {
                Tag::Getter => self
                    .host
                    .report(diag::override_on_non_overriding_getter().at_offset(range.0, range.1)),
                Tag::Setter => self
                    .host
                    .report(diag::override_on_non_overriding_setter().at_offset(range.0, range.1)),
                _ => {}
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        if let Some(element) = declared_element(self.host, node)
            && has_annotation(self.host, ast[node].metadata, "dart.core", "override")
            && !self.is_override(element)
        {
            let range = token_range(ast, ast[node].name);
            let d = match element.tag() {
                Tag::Method => Some(diag::override_on_non_overriding_method()),
                Tag::Getter => Some(diag::override_on_non_overriding_getter()),
                Tag::Setter => Some(diag::override_on_non_overriding_setter()),
                _ => None,
            };
            if let Some(d) = d {
                self.host.report(d.at_offset(range.0, range.1));
            }
        }
        ast.visit_children(node, self);
    }

    fn visit_primary_constructor_declaration(
        &mut self,
        ast: &Ast,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        let ctx = self.host.ctx();
        let parameters = ast[node].formal_parameters;
        for &parameter in ast.list(ast[parameters].parameters) {
            let Some(element) = declared_element(self.host, parameter) else {
                continue;
            };
            if element.tag() != Tag::FieldFormalParameter {
                continue;
            }
            let parts = formal_parameter_parts(ast, parameter.raw());
            if !has_annotation(self.host, parts.metadata, "dart.core", "override") {
                continue;
            }
            if !super::correct_override::first_fragment_flags(&ctx, element).contains(
                dartr_element::FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING,
            ) {
                continue;
            }
            let Some(field) = element
                .cast::<dartr_element::FormalParameterElement>()
                .and_then(|p| ctx.get(p).field.get())
            else {
                continue;
            };
            let Some(name) = parts.name else {
                continue;
            };
            self.check_field(field, token_range(ast, name));
        }
        // Dart does not visit the children here.
    }
}
