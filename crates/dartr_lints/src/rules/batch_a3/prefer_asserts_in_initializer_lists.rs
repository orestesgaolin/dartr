// Dart source: pkg/linter/lib/src/rules/prefer_asserts_in_initializer_lists.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{
    ConstructorElement, ElementId, FieldElement, FormalParameterElement, InterfaceElement,
};
use dartr_typesystem::{TypeExt, member};
use indexmap::IndexSet;
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_constructor_declaration("prefer_asserts_in_initializer_lists", check);
    r.add_primary_constructor_body("prefer_asserts_in_initializer_lists", check);
}
fn enclosing_constructor(ctx: &LinterContext<'_>, mut node: NodeId) -> Option<NodeId> {
    loop {
        if matches!(
            ctx.ast.kind(node),
            NodeKind::ConstructorDeclaration | NodeKind::PrimaryConstructorDeclaration
        ) {
            return Some(node);
        }
        let primary = match ctx.ast.kind(node) {
            NodeKind::ClassDeclaration => Some(
                ctx.ast[Id::<ClassDeclaration>::from_raw(node)]
                    .name_part
                    .raw(),
            ),
            NodeKind::EnumDeclaration => Some(
                ctx.ast[Id::<EnumDeclaration>::from_raw(node)]
                    .name_part
                    .raw(),
            ),
            NodeKind::ExtensionTypeDeclaration => Some(
                ctx.ast[Id::<ExtensionTypeDeclaration>::from_raw(node)]
                    .name_part
                    .raw(),
            ),
            _ => None,
        };
        if primary
            .is_some_and(|primary| ctx.ast.kind(primary) == NodeKind::PrimaryConstructorDeclaration)
        {
            return primary;
        }
        node = ctx.ast.parent(node)?;
    }
}

fn matches_field_formal(ctx: &LinterContext<'_>, constructor: NodeId, field: ElementId) -> bool {
    (0..ctx.ast.node_count()).any(|index| {
        let node = NodeId::from_index(index);
        if !matches!(
            ctx.ast.kind(node),
            NodeKind::RegularFormalParameter
                | NodeKind::FieldFormalParameter
                | NodeKind::SuperFormalParameter
        ) || enclosing_constructor(ctx, node) != Some(constructor)
        {
            return false;
        }
        let Some(resolved) = ctx.resolved else {
            return false;
        };
        let Some(parameter) = ctx
            .declared_element(node)
            .and_then(|element| element.cast::<FormalParameterElement>())
        else {
            return false;
        };
        let parameter = if ctx.ast.kind(node) == NodeKind::SuperFormalParameter {
            let Some(parameter) =
                dartr_link::outline::super_constructor_parameter(&resolved.ctx, parameter)
            else {
                return false;
            };
            parameter
        } else {
            parameter
        };
        resolved
            .ctx
            .get(parameter)
            .field
            .get()
            .is_some_and(|parameter_field| parameter_field.raw() == field)
    })
}

fn constructor_hierarchy(
    ctx: &LinterContext<'_>,
    constructor: NodeId,
) -> IndexSet<dartr_element::EId<InterfaceElement>> {
    fn add(
        resolved: crate::ResolvedLintContext<'_>,
        class: dartr_element::EId<InterfaceElement>,
        classes: &mut IndexSet<dartr_element::EId<InterfaceElement>>,
    ) {
        if !classes.insert(class) {
            return;
        }
        for &mixin in resolved.ctx.element_mixins(class) {
            if let Some(element) = resolved.ctx.interface_element(mixin) {
                add(resolved, element, classes);
            }
        }
        if let Some(supertype) = resolved.ctx.element_supertype(class)
            && let Some(element) = resolved.ctx.interface_element(supertype)
        {
            add(resolved, element, classes);
        }
    }

    let mut classes = IndexSet::new();
    let Some(resolved) = ctx.resolved else {
        return classes;
    };
    let class = ctx
        .declared_element(constructor)
        .and_then(|element| element.cast::<ConstructorElement>())
        .and_then(|element| resolved.ctx.element_data(element.raw())?.enclosing)
        .and_then(|element| element.cast::<InterfaceElement>());
    if let Some(class) = class {
        add(resolved, class, &mut classes);
    }
    classes
}

fn needs_instance(ctx: &LinterContext<'_>, root: NodeId, constructor: NodeId) -> bool {
    let hierarchy = constructor_hierarchy(ctx, constructor);
    let mut stack = ctx.ast.children(root);
    while let Some(n) = stack.pop() {
        if ctx.ast.kind(n) == NodeKind::ThisExpression {
            return true;
        }
        if ctx.ast.kind(n) == NodeKind::SimpleIdentifier
            && let (Some(r), Some(element)) = (ctx.resolved, ctx.element(n))
        {
            let base = member::base_element(&r.ctx, element);
            if dartr_link::dump::is_static(&r.ctx, base) {
                stack.extend(ctx.ast.children(n));
                continue;
            }
            let field = base
                .cast::<FieldElement>()
                .map(|field| field.raw())
                .or_else(|| {
                    member::variable(&r.ctx, element)
                        .map(|field| member::base_element(&r.ctx, field))
                })
                .filter(|field| field.kind() == dartr_element::ElementKind::Field);
            if field.is_some_and(|field| matches_field_formal(ctx, constructor, field)) {
                stack.extend(ctx.ast.children(n));
                continue;
            }
            if matches!(
                base.kind(),
                dartr_element::ElementKind::Method
                    | dartr_element::ElementKind::Getter
                    | dartr_element::ElementKind::Setter
            ) && member::enclosing_interface(&r.ctx, element)
                .is_some_and(|class| hierarchy.contains(&class))
            {
                return true;
            }
        }
        stack.extend(ctx.ast.children(n));
    }
    false
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(constructor) = enclosing_constructor(ctx, node) else {
        return;
    };
    let body = match ctx.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            let n = &ctx.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            if n.factory_keyword.is_some() {
                return;
            }
            n.body
        }
        NodeKind::PrimaryConstructorBody => {
            ctx.ast[Id::<PrimaryConstructorBody>::from_raw(node)].body
        }
        _ => return,
    };
    let Some(block_body) = ctx.ast.cast::<BlockFunctionBody>(body.raw()) else {
        return;
    };
    for statement in ctx.ast.list(ctx.ast[ctx.ast[block_body].block].statements) {
        if ctx.ast.kind(*statement) != NodeKind::AssertStatement {
            break;
        }
        if !needs_instance(ctx, statement.raw(), constructor) {
            ctx.report_token(
                out,
                &diag::PREFER_ASSERTS_IN_INITIALIZER_LISTS,
                ctx.ast.begin_token(*statement),
                &[],
            );
        }
    }
}
