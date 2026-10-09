// Dart source: pkg/linter/lib/src/rules/prefer_asserts_in_initializer_lists.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{
    ClassElement, ConstructorElement, EId, ElemRef, ElementId, InterfaceElement, Tag,
};
use dartr_typesystem::{TypeExt, member};
use indexmap::IndexSet;

pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_constructor_declaration("prefer_asserts_in_initializer_lists", check);
    r.add_primary_constructor_body("prefer_asserts_in_initializer_lists", check);
}

/// The `ClassDeclaration` that Dart `_Visitor.visitClassDeclaration` visited
/// last before [node] (the visitor keeps it in `_classAndSuperClasses`): the
/// last class declaration that starts before [node] in visit order.
fn last_visited_class(ctx: &LinterContext<'_>, node: NodeId) -> Option<NodeId> {
    let offset = ctx.ast.offset(node);
    (0..ctx.ast.node_count())
        .map(NodeId::from_index)
        .filter(|&n| ctx.ast.kind(n) == NodeKind::ClassDeclaration && ctx.ast.offset(n) <= offset)
        .max_by_key(|&n| ctx.ast.offset(n))
}

/// Dart `_ClassAndSuperClasses.classes`.
fn class_and_super_classes(
    ctx: &LinterContext<'_>,
    element: Option<EId<ClassElement>>,
) -> IndexSet<EId<InterfaceElement>> {
    fn add(
        ctx: &dartr_element::Ctx<'_>,
        element: Option<EId<InterfaceElement>>,
        classes: &mut IndexSet<EId<InterfaceElement>>,
    ) {
        let Some(element) = element else { return };
        if !classes.insert(element) {
            return;
        }
        for &t in ctx.element_mixins(element) {
            add(ctx, ctx.interface_element(t), classes);
        }
        add(
            ctx,
            ctx.element_supertype(element)
                .and_then(|t| ctx.interface_element(t)),
            classes,
        );
    }
    let mut classes = IndexSet::new();
    if let Some(resolved) = ctx.resolved {
        add(&resolved.ctx, element.map(|e| e.upcast()), &mut classes);
    }
    classes
}

/// Dart `_AssertVisitor._paramMatchesField`.
fn param_matches_field(
    ctx: &dartr_element::Ctx<'_>,
    element: ElementId,
    constructor: EId<ConstructorElement>,
) -> bool {
    for &p in &ctx.get(constructor).formal_params {
        let parameter = if p.raw().tag() == Tag::SuperFormalParameter {
            dartr_link::outline::super_constructor_parameter(ctx, p)
        } else {
            Some(p)
        };
        if let Some(parameter) = parameter
            && parameter.raw().tag() == Tag::FieldFormalParameter
            && ctx
                .get(parameter)
                .field
                .get()
                .and_then(|field| ctx.get(field).getter)
                .is_some_and(|getter| getter.raw() == element)
        {
            return true;
        }
    }
    false
}

/// Dart `_AssertVisitor` over the children of [statement]: `needInstance`.
fn need_instance(
    ctx: &LinterContext<'_>,
    statement: NodeId,
    constructor: EId<ConstructorElement>,
    classes: &IndexSet<EId<InterfaceElement>>,
) -> bool {
    let Some(resolved) = ctx.resolved else {
        return false;
    };
    let r = &resolved.ctx;
    let in_classes = |element: ElementId| {
        r.element_data(element)
            .and_then(|d| d.enclosing)
            .and_then(|e| e.cast::<InterfaceElement>())
            .is_some_and(|e| classes.contains(&e))
    };
    let mut stack = ctx.ast.children(statement);
    while let Some(n) = stack.pop() {
        match ctx.ast.kind(n) {
            NodeKind::ThisExpression => return true,
            NodeKind::SimpleIdentifier => {
                if let Some(element) = ctx.write_or_read_element(n) {
                    let base = member::base_element(r, element);
                    let is_static = member::is_static(r, ElemRef::Base(base));
                    match base.tag() {
                        Tag::Method if !is_static && in_classes(base) => return true,
                        Tag::Getter | Tag::Setter
                            if !is_static
                                && in_classes(base)
                                && !param_matches_field(r, base, constructor) =>
                        {
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        stack.extend(ctx.ast.children(n));
    }
    false
}

fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(resolved) = ctx.resolved else {
        return;
    };
    let (declaration, body) = match ctx.ast.kind(node) {
        NodeKind::ConstructorDeclaration => {
            let n = &ctx.ast[Id::<ConstructorDeclaration>::from_raw(node)];
            (Some(node), n.body)
        }
        NodeKind::PrimaryConstructorBody => {
            // Dart `PrimaryConstructorBody.declaration`.
            let declaration = ctx
                .ast
                .parent(node)
                .and_then(|body| ctx.ast.parent(body))
                .and_then(|type_declaration| match ctx.ast.kind(type_declaration) {
                    NodeKind::ClassDeclaration => {
                        Some(ctx.ast[Id::<ClassDeclaration>::from_raw(type_declaration)].name_part)
                    }
                    NodeKind::EnumDeclaration => {
                        Some(ctx.ast[Id::<EnumDeclaration>::from_raw(type_declaration)].name_part)
                    }
                    NodeKind::ExtensionTypeDeclaration => Some(
                        ctx.ast[Id::<ExtensionTypeDeclaration>::from_raw(type_declaration)]
                            .name_part,
                    ),
                    _ => None,
                })
                .and_then(|part| ctx.ast.cast::<PrimaryConstructorDeclaration>(part))
                .map(|p| p.raw());
            (
                declaration,
                ctx.ast[Id::<PrimaryConstructorBody>::from_raw(node)].body,
            )
        }
        _ => return,
    };
    let Some(constructor) = declaration
        .and_then(|d| ctx.declared_element(d))
        .and_then(|e| e.cast::<ConstructorElement>())
    else {
        return;
    };
    // Dart `declaredElement.isFactory`.
    let is_factory = resolved
        .ctx
        .fragment_data(resolved.ctx.get(constructor).first_fragment().raw())
        .is_some_and(|f| {
            f.flags
                .has(dartr_element::FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
        });
    if is_factory {
        return;
    }
    let Some(block_body) = ctx.ast.cast::<BlockFunctionBody>(body.raw()) else {
        return;
    };
    let class_element = last_visited_class(ctx, node)
        .and_then(|class| ctx.declared_element(class))
        .and_then(|e| e.cast::<ClassElement>());
    let classes = class_and_super_classes(ctx, class_element);
    for statement in ctx.ast.list(ctx.ast[ctx.ast[block_body].block].statements) {
        if ctx.ast.kind(*statement) != NodeKind::AssertStatement {
            break;
        }
        if !need_instance(ctx, statement.raw(), constructor, &classes) {
            ctx.report_token(
                out,
                &diag::PREFER_ASSERTS_IN_INITIALIZER_LISTS,
                ctx.ast.begin_token(*statement),
                &[],
            );
        }
    }
}
