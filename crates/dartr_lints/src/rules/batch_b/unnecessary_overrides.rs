// Dart source: pkg/linter/lib/src/rules/unnecessary_overrides.dart
use super::util::*;
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{EId, ElemRef, ElementId, FormalParameterElement, InterfaceElement, Tag};
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{GetMemberOptions, InheritanceManager3, Name};
use dartr_typesystem::member;

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(NodeKind::MethodDeclaration, "unnecessary_overrides", visit_method_declaration);
}

#[derive(Clone, Copy, PartialEq)]
enum Visitor {
    Operator,
    Getter,
    Setter,
    Method,
}

/// Dart `_AbstractUnnecessaryOverrideVisitor` state.
struct State<'c, 'a> {
    c: &'c LinterContext<'a>,
    visitor: Visitor,
    inherited_method: ElemRef,
    declaration: Id<MethodDeclaration>,
}

fn visit_method_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
    if n.modifier_keyword.is_some_and(|k| lexeme(c, k) == "static") {
        return;
    }
    let visitor = if n.operator_keyword.is_some() {
        Visitor::Operator
    } else {
        match n.property_keyword.map(|k| lexeme(c, k)) {
            Some("get") => Visitor::Getter,
            Some("set") => Visitor::Setter,
            _ => Visitor::Method,
        }
    };
    // Dart `_AbstractUnnecessaryOverrideVisitor.visitMethodDeclaration`.
    if lexeme(c, n.name) == "noSuchMethod" || n.documentation_comment.is_some() {
        return;
    }
    let Some(inherited_method) = get_inherited_element(c, visitor, node) else { return };
    let state = State { c, visitor, inherited_method, declaration: Id::from_raw(node) };
    if state.adds_metadata() || !state.have_same_declaration() || state.makes_public_from_protected() {
        return;
    }
    state.accept(n.body.raw(), out);
}

/// Dart `getInheritedElement` of the four visitors (`thisType.lookUp*`
/// with `concrete: true, inherited: true`).
fn get_inherited_element(c: &LinterContext<'_>, visitor: Visitor, node: NodeId) -> Option<ElemRef> {
    let ctx = rctx(c)?;
    let element = c.declared_element(node)?;
    let enclosing = enclosing(c, element)?.cast::<InterfaceElement>()?;
    let library = member::library(&ctx, ElemRef::Base(element))?;
    let name = match visitor {
        Visitor::Getter | Visitor::Operator => name(c, element)?.to_string(),
        Visitor::Method => lexeme(c, c.ast[Id::<MethodDeclaration>::from_raw(node)].name).to_string(),
        Visitor::Setter => format!("{}=", lexeme(c, c.ast[Id::<MethodDeclaration>::from_raw(node)].name)),
    };
    let this_type = ctx.interface_this_type(enclosing);
    let result = InheritanceManager3::new(ctx).get_member3(
        this_type,
        Name::new(&ctx, Some(library), &name),
        GetMemberOptions { for_super: true, ..GetMemberOptions::default() },
    )?;
    let tag = member::base_element(&ctx, result).tag();
    let expected = match visitor {
        Visitor::Getter => Tag::Getter,
        Visitor::Setter => Tag::Setter,
        Visitor::Method | Visitor::Operator => Tag::Method,
    };
    (tag == expected).then_some(result)
}

impl State<'_, '_> {
    fn declared_element(&self) -> Option<ElementId> {
        self.c.declared_element(self.declaration)
    }

    fn inherited_has_protected(&self) -> bool {
        let base = member::base_element(&rctx(self.c).unwrap(), self.inherited_method);
        self.c.has_package_meta_getter(base, "protected")
    }

    /// Dart `_addsMetadata`.
    fn adds_metadata(&self) -> bool {
        let c = self.c;
        let (Some(element), Some(resolved)) = (self.declared_element(), c.resolved.as_ref()) else {
            return false;
        };
        let Some(metadata) = resolved.metadata else { return false };
        let ctx = &resolved.ctx;
        let is_top_getter = |a: Option<ElementId>, library_name: &str, name: &str| {
            a.is_some_and(|a| {
                a.tag() == Tag::Getter
                    && ctx.element_name(a) == Some(name)
                    && member::library(ctx, ElemRef::Base(a))
                        .is_some_and(|l| ctx.element_name(l.raw()) == Some(library_name))
            })
        };
        for annotation in metadata.annotations(element) {
            let a = metadata.annotation_element(annotation);
            if is_top_getter(a, "dart.core", "override") {
                continue;
            }
            if is_top_getter(a, "meta", "protected") && self.inherited_has_protected() {
                continue;
            }
            return true;
        }
        false
    }

    /// Dart `_haveSameDeclaration`.
    fn have_same_declaration(&self) -> bool {
        let c = self.c;
        let Some(ctx) = rctx(c) else { return false };
        let Some(declared) = self.declared_element() else { return false };
        let declared = ElemRef::Base(declared);
        if !types_equal(c, member::return_type(&ctx, declared), member::return_type(&ctx, self.inherited_method)) {
            return false;
        }
        let params = member::formal_parameters(&ctx, declared);
        let super_params = member::formal_parameters(&ctx, self.inherited_method);
        if params.len() != super_params.len() {
            return false;
        }
        let kind = |p: ElemRef| ctx.get(EId::<FormalParameterElement>::from_raw(member::base_element(&ctx, p))).kind;
        for (&param, &super_param) in params.iter().zip(&super_params) {
            if !types_equal(c, member::type_(&ctx, param), member::type_(&ctx, super_param)) {
                return false;
            }
            if member::name(&ctx, param) != member::name(&ctx, super_param) {
                return false;
            }
            if member::is_covariant(&ctx, param) != member::is_covariant(&ctx, super_param) {
                return false;
            }
            let (first, second) = (kind(param), kind(super_param));
            let same_kind = if first.is_required() {
                second.is_required()
            } else if first.is_optional_positional() {
                second.is_optional_positional()
            } else {
                second.is_named()
            };
            if !same_kind {
                return false;
            }
            let code = |p: ElemRef| c.default_value_code(member::base_element(&ctx, p));
            if code(param) != code(super_param) {
                return false;
            }
        }
        true
    }

    /// Dart `_makesPublicFromProtected`.
    fn makes_public_from_protected(&self) -> bool {
        let Some(declared) = self.declared_element() else { return false };
        if self.c.has_package_meta_getter(declared, "protected") {
            return false;
        }
        self.inherited_has_protected()
    }

    fn inherited_name(&self) -> Option<&str> {
        member::name(&rctx(self.c)?, self.inherited_method)
    }

    fn declaration_parameters(&self) -> Option<Vec<NodeId>> {
        let list = self.c.ast[self.declaration].parameters?;
        Some(parameters(self.c, Some(list)))
    }

    /// The visitor `accept` of [node].
    fn accept(&self, node: NodeId, out: &mut Vec<Diagnostic>) {
        let c = self.c;
        let preceded_by_comments =
            |n: NodeId| c.ast.tokens.get(c.ast.begin_token(n)).preceding_comments.is_some();
        match kind(c, node) {
            NodeKind::Block => {
                let statements = c.ast.list_raw(c.ast[Id::<Block>::from_raw(node)].statements);
                if statements.len() == 1 {
                    self.accept(statements[0], out);
                }
            }
            NodeKind::BlockFunctionBody => self.accept(c.ast[Id::<BlockFunctionBody>::from_raw(node)].block.raw(), out),
            NodeKind::ExpressionFunctionBody => {
                self.accept(c.ast[Id::<ExpressionFunctionBody>::from_raw(node)].expression.raw(), out)
            }
            NodeKind::ExpressionStatement => {
                self.accept(c.ast[Id::<ExpressionStatement>::from_raw(node)].expression.raw(), out)
            }
            NodeKind::ParenthesizedExpression => self.accept(unparenthesized(c, node), out),
            NodeKind::ReturnStatement => {
                if preceded_by_comments(node) {
                    return;
                }
                if let Some(e) = c.ast[Id::<ReturnStatement>::from_raw(node)].expression {
                    self.accept(e.raw(), out);
                }
            }
            NodeKind::SuperExpression => self.visit_super_expression(node, out),
            NodeKind::PropertyAccess if self.visitor == Visitor::Getter => {
                let n = &c.ast[Id::<PropertyAccess>::from_raw(node)];
                if Some(simple_name(c, n.property_name)) == self.inherited_name()
                    && let Some(target) = n.target
                {
                    self.accept(target.raw(), out);
                }
            }
            NodeKind::MethodInvocation if self.visitor == Visitor::Method => {
                let n = &c.ast[Id::<MethodInvocation>::from_raw(node)];
                if let Some(parameters) = self.declaration_parameters()
                    && Some(simple_name(c, n.method_name)) == self.inherited_name()
                    && arguments_match_parameters(c, c.ast.list_raw(c.ast[n.argument_list].arguments), &parameters)
                    && let Some(target) = n.target
                {
                    self.accept(target.raw(), out);
                }
            }
            NodeKind::BinaryExpression if self.visitor == Visitor::Operator => {
                let n = &c.ast[Id::<BinaryExpression>::from_raw(node)];
                if let Some(parameters) = self.declaration_parameters()
                    && lexeme(c, n.operator) == lexeme(c, c.ast[self.declaration].name)
                    && parameters.len() == 1
                    && c.declared_element(parameters[0]).is_some()
                    && c.declared_element(parameters[0]) == canonical(c, n.right_operand.raw())
                {
                    let left = unparenthesized(c, n.left_operand.raw());
                    if kind(c, left) == NodeKind::SuperExpression {
                        self.visit_super_expression(left, out);
                    }
                }
            }
            NodeKind::PrefixExpression if self.visitor == Visitor::Operator => {
                let n = &c.ast[Id::<PrefixExpression>::from_raw(node)];
                if let Some(parameters) = self.declaration_parameters()
                    && lexeme(c, n.operator) == lexeme(c, c.ast[self.declaration].name)
                    && parameters.is_empty()
                {
                    let operand = unparenthesized(c, n.operand.raw());
                    if kind(c, operand) == NodeKind::SuperExpression {
                        self.visit_super_expression(operand, out);
                    }
                }
            }
            NodeKind::AssignmentExpression if self.visitor == Visitor::Setter => {
                let n = &c.ast[Id::<AssignmentExpression>::from_raw(node)];
                if let Some(parameters) = self.declaration_parameters()
                    && parameters.len() == 1
                    && c.declared_element(parameters[0]).is_some()
                    && c.declared_element(parameters[0]) == canonical(c, n.right_hand_side.raw())
                {
                    let left = unparenthesized(c, n.left_hand_side.raw());
                    if let Some(access) = c.ast.cast::<PropertyAccess>(left) {
                        let write_name = c
                            .resolved
                            .as_ref()
                            .and_then(|r| r.tables.write_element.get(node).copied())
                            .and_then(|e| member::name(&rctx(c).unwrap(), e));
                        if write_name.is_some() && write_name == self.inherited_name()
                            && let Some(target) = c.ast[access].target
                        {
                            self.accept(target.raw(), out);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_super_expression(&self, node: NodeId, out: &mut Vec<Diagnostic>) {
        let c = self.c;
        if c.ast.tokens.get(c.ast.begin_token(node)).preceding_comments.is_some() {
            return;
        }
        c.report_token(out, &diag::UNNECESSARY_OVERRIDES, c.ast[self.declaration].name, &[]);
    }
}

/// Dart `ExpressionExtension.canonicalElement` (analyzer).
fn canonical(c: &LinterContext<'_>, expression: NodeId) -> Option<ElementId> {
    canonical_element(c, expression).and_then(|e| c.canonical_element2(e))
}

/// Dart `argumentsMatchParameters`.
fn arguments_match_parameters(c: &LinterContext<'_>, arguments: &[NodeId], parameters: &[NodeId]) -> bool {
    super::unnecessary_lambdas::arguments_match_parameters(c, arguments, parameters)
}
