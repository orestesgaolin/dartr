// Dart source: pkg/linter/lib/src/rules/prefer_initializing_formals.dart
use super::util::*;
use crate::{ExperimentalFlag, LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, ElementId, FragmentFlags, Tag};
use indexmap::{IndexMap, IndexSet};

pub fn register(r: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    r.add(
        NodeKind::ConstructorDeclaration,
        "prefer_initializing_formals",
        visit_constructor_declaration,
    );
    r.add(
        NodeKind::PrimaryConstructorDeclaration,
        "prefer_initializing_formals",
        visit_primary_constructor_declaration,
    );
}

fn visit_constructor_declaration(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let n = &c.ast[Id::<ConstructorDeclaration>::from_raw(node)];
    if n.factory_keyword.is_some() {
        return;
    }
    let checker = ConstructorChecker::new(
        c,
        c.declared_element(node),
        n.parameters,
        c.ast.list_raw(n.initializers).to_vec(),
        Some(n.body.raw()),
    );
    checker.check(out);
}

fn visit_primary_constructor_declaration(
    c: &LinterContext<'_>,
    node: NodeId,
    out: &mut Vec<Diagnostic>,
) {
    let n = &c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
    let body = super::tighten_type_of_initializing_formals::primary_constructor_body(c, node);
    let checker = ConstructorChecker::new(
        c,
        c.declared_element(node),
        n.formal_parameters,
        body.map(|b| c.ast.list_raw(c.ast[b].initializers).to_vec())
            .unwrap_or_default(),
        body.map(|b| c.ast[b].body.raw()),
    );
    checker.check(out);
}

/// Dart `_ConstructorChecker`.
struct ConstructorChecker<'c, 'a> {
    c: &'c LinterContext<'a>,
    constructor: Option<ElementId>,
    parameter_list: Id<FormalParameterList>,
    initializers: Vec<NodeId>,
    body: Option<NodeId>,
    parameters: Vec<Option<ElementId>>,
    initializing_parameters: IndexSet<ElementId>,
    nodes_to_lint_by_field: IndexMap<ElementId, Vec<NodeId>>,
    private_named_parameters_enabled: bool,
}

impl<'c, 'a> ConstructorChecker<'c, 'a> {
    fn new(
        c: &'c LinterContext<'a>,
        constructor: Option<ElementId>,
        parameter_list: Id<FormalParameterList>,
        initializers: Vec<NodeId>,
        body: Option<NodeId>,
    ) -> Self {
        let parameters = parameters(c, Some(parameter_list))
            .into_iter()
            .filter(|&p| kind(c, p) != NodeKind::SuperFormalParameter)
            .map(|p| c.declared_element(p))
            .collect();
        Self {
            c,
            constructor,
            parameter_list,
            initializers,
            body,
            parameters,
            initializing_parameters: IndexSet::new(),
            nodes_to_lint_by_field: IndexMap::new(),
            private_named_parameters_enabled: c
                .is_feature_enabled(ExperimentalFlag::PrivateNamedParameters),
        }
    }

    fn check(mut self, out: &mut Vec<Diagnostic>) {
        let c = self.c;
        for parameter in parameters(c, Some(self.parameter_list)) {
            if let Some(element) = c.declared_element(parameter)
                && element.tag() == Tag::FieldFormalParameter
            {
                self.initializing_parameters.insert(element);
            }
        }
        for initializer in self.initializers.clone() {
            let Some(field_initializer) = c.ast.cast::<ConstructorFieldInitializer>(initializer)
            else {
                continue;
            };
            let expression = c.ast[field_initializer].expression.raw();
            if kind(c, expression) != NodeKind::SimpleIdentifier {
                continue;
            }
            let field = c
                .element(c.ast[field_initializer].field_name)
                .map(|e| base(c, e));
            let parameter = c.element(expression).map(|e| base(c, e));
            self.check_initializer(initializer, field, parameter);
        }
        if let Some(body) = self.body
            && let Some(block) = c.ast.cast::<BlockFunctionBody>(body)
        {
            let block = c.ast[block].block;
            for &statement in c.ast.list_raw(c.ast[block].statements) {
                let Some(statement) = c.ast.cast::<ExpressionStatement>(statement) else {
                    continue;
                };
                let Some(assignment) = c
                    .ast
                    .cast::<AssignmentExpression>(c.ast[statement].expression.raw())
                else {
                    continue;
                };
                let a = &c.ast[assignment];
                let Some(access) = c.ast.cast::<PropertyAccess>(a.left_hand_side.raw()) else {
                    continue;
                };
                if !c.ast[access]
                    .target
                    .is_some_and(|t| kind(c, t.raw()) == NodeKind::ThisExpression)
                {
                    continue;
                }
                let resolved = c.resolved.as_ref().map(|r| r.tables);
                let field = resolved
                    .and_then(|t| t.write_element.get(assignment.raw()).copied())
                    .and_then(|e| c.canonical_element2(e));
                let parameter = canonical_element(c, a.right_hand_side.raw())
                    .map(|e| c.canonical_element2(e).unwrap_or(base(c, e)));
                self.check_initializer(assignment.raw(), field, parameter);
            }
        }
        for (&field, nodes) in &self.nodes_to_lint_by_field {
            let field_name = name(c, field).unwrap_or_default();
            for &node in nodes {
                c.report_node(out, &diag::PREFER_INITIALIZING_FORMALS, node, &[field_name]);
            }
        }
    }

    /// Dart `_checkInitializer`.
    fn check_initializer(
        &mut self,
        node: NodeId,
        field: Option<ElementId>,
        parameter: Option<ElementId>,
    ) {
        let c = self.c;
        let Some(ctx) = rctx(c) else { return };
        let Some(field) = field.filter(|f| f.tag() == Tag::Field) else {
            return;
        };
        if is_static(c, field) {
            return;
        }
        if parameter.is_some_and(|p| self.initializing_parameters.contains(&p)) {
            return;
        }
        if !flags(c, field)
            .contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION)
        {
            return;
        }
        let Some(parameter) = parameter.filter(|p| {
            matches!(
                p.tag(),
                Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
            )
        }) else {
            return;
        };
        if !self.parameters.contains(&Some(parameter)) {
            return;
        }
        let Some(ts) = c.type_system() else { return };
        let parameter_type = dartr_typesystem::member::type_(&ctx, ElemRef::Base(parameter));
        let field_type = dartr_typesystem::member::type_(&ctx, ElemRef::Base(field));
        if !ts.is_subtype_of(parameter_type, field_type) {
            return;
        }
        let field_name = name(c, field);
        let parameter_name = name(c, parameter);
        if field_name.is_some_and(|n| n.starts_with('_')) {
            if !self.private_named_parameters_enabled {
                return;
            }
            let p = ctx.get(
                dartr_element::EId::<dartr_element::FormalParameterElement>::from_raw(parameter),
            );
            if p.kind.is_positional() {
                return;
            }
            let underscored = parameter_name.map(|n| format!("_{n}"));
            if field_name != parameter_name && field_name != underscored.as_deref() {
                return;
            }
        } else if field_name != parameter_name {
            return;
        }
        let constructor_enclosing = self.constructor.and_then(|e| enclosing(c, e));
        if enclosing(c, field) != constructor_enclosing {
            return;
        }
        // Dart `_ReferenceCounter` over the initializers and the body.
        let mut count = 0;
        let mut stack: Vec<NodeId> = self.initializers.iter().rev().copied().collect();
        if let Some(body) = self.body {
            stack.insert(0, body);
        }
        while let Some(n) = stack.pop() {
            if kind(c, n) == NodeKind::SimpleIdentifier
                && c.element(n).is_some_and(|e| e == ElemRef::Base(parameter))
            {
                count += 1;
            }
            stack.extend(c.ast.children(n).into_iter().rev());
        }
        if count > 1 {
            return;
        }
        self.nodes_to_lint_by_field
            .entry(field)
            .or_default()
            .push(node);
    }
}
