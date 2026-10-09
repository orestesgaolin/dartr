// Dart source: pkg/linter/lib/src/rules/library_private_types_in_public_api.dart
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{ElemRef, FormalParameterElement, TypeKind};
use dartr_typesystem::{TypeExt, member};
pub fn register(r: &mut RuleVisitorRegistry) {
    r.add_named_type("library_private_types_in_public_api", check);
    r.add_field_formal_parameter("library_private_types_in_public_api", check_implicit_formal);
    r.add_super_formal_parameter("library_private_types_in_public_api", check_implicit_formal);
}
fn private_token(ctx: &LinterContext<'_>, t: dartr_syntax::TokenId) -> bool {
    ctx.ast.tokens.lexeme(t).starts_with('_')
}

fn enclosing_type_declaration(ctx: &LinterContext<'_>, mut node: NodeId) -> Option<NodeId> {
    while let Some(parent) = ctx.ast.parent(node) {
        if matches!(
            ctx.ast.kind(parent),
            NodeKind::ClassDeclaration
                | NodeKind::EnumDeclaration
                | NodeKind::ExtensionTypeDeclaration
        ) {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn is_effectively_private_constructor(ctx: &LinterContext<'_>, node: NodeId) -> bool {
    let Some(owner) = enclosing_type_declaration(ctx, node) else {
        return false;
    };
    if ctx.ast.kind(owner) == NodeKind::EnumDeclaration {
        return true;
    }
    if ctx.ast.kind(owner) != NodeKind::ClassDeclaration {
        // `@internal` also makes extension type constructors effectively
        // private, but resolved metadata flags are not available yet.
        return false;
    }

    if let Some(resolved) = ctx.resolved
        && let Some(element) = ctx.declared_element(owner)
    {
        return dartr_link::dump::is_sealed(&resolved.ctx, element)
            || (dartr_link::dump::is_abstract(&resolved.ctx, element)
                && (dartr_link::dump::is_final(&resolved.ctx, element)
                    || dartr_link::dump::is_interface(&resolved.ctx, element)));
    }

    let class = &ctx.ast[Id::<ClassDeclaration>::from_raw(owner)];
    class.sealed_keyword.is_some()
        || (class.abstract_keyword.is_some()
            && (class.final_keyword.is_some() || class.interface_keyword.is_some()))
}

fn library_is_private(ctx: &LinterContext<'_>) -> bool {
    ctx.resolved
        .and_then(|resolved| resolved.ctx.element_name(resolved.library.raw()))
        .is_some_and(|name| name.starts_with('_'))
}

fn public_api_position(ctx: &LinterContext<'_>, mut node: NodeId) -> bool {
    let mut public_declaration = false;
    let mut within_regular_formal = false;
    while let Some(p) = ctx.ast.parent(node) {
        match ctx.ast.kind(p) {
            NodeKind::FunctionExpression
                if ctx.ast.parent(p).is_some_and(|parent| {
                    ctx.ast.kind(parent) == NodeKind::FunctionDeclaration
                }) =>
            {
                public_declaration = true;
            }
            kind if Expression::test(kind) => return false,
            NodeKind::BlockFunctionBody
            | NodeKind::ExpressionFunctionBody
            | NodeKind::VariableDeclarationStatement
            | NodeKind::ForStatement => return false,
            NodeKind::MethodDeclaration => {
                if private_token(ctx, ctx.ast[Id::<MethodDeclaration>::from_raw(p)].name) {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::FunctionDeclaration => {
                if private_token(ctx, ctx.ast[Id::<FunctionDeclaration>::from_raw(p)].name) {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::ConstructorDeclaration => {
                let constructor = &ctx.ast[Id::<ConstructorDeclaration>::from_raw(p)];
                if node != constructor.parameters.raw()
                    || constructor.name.is_some_and(|n| private_token(ctx, n))
                    || is_effectively_private_constructor(ctx, p)
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::PrimaryConstructorDeclaration => {
                let constructor = &ctx.ast[Id::<PrimaryConstructorDeclaration>::from_raw(p)];
                if node != constructor.formal_parameters.raw() {
                    return false;
                }
                match enclosing_type_declaration(ctx, p).map(|owner| ctx.ast.kind(owner)) {
                    Some(NodeKind::ExtensionTypeDeclaration) if !within_regular_formal => {
                        return false;
                    }
                    Some(NodeKind::ExtensionTypeDeclaration) => {}
                    Some(NodeKind::EnumDeclaration) => return false,
                    _ if constructor
                        .constructor_name
                        .is_some_and(|name| private_token(ctx, ctx.ast[name].name))
                        || is_effectively_private_constructor(ctx, p) =>
                    {
                        return false;
                    }
                    _ => {}
                }
                public_declaration = true;
            }
            NodeKind::FieldDeclaration => {
                let field = &ctx.ast[Id::<FieldDeclaration>::from_raw(p)];
                if node != field.fields.raw()
                    || (field.static_keyword.is_none()
                        && enclosing_type_declaration(ctx, p).is_some_and(|owner| {
                            ctx.ast.kind(owner) == NodeKind::ExtensionTypeDeclaration
                        }))
                {
                    return false;
                }
                let list = &ctx.ast[field.fields];
                if !ctx
                    .ast
                    .list(list.variables)
                    .iter()
                    .any(|v| !private_token(ctx, ctx.ast[*v].name))
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::TopLevelVariableDeclaration => {
                let declaration = &ctx.ast[Id::<TopLevelVariableDeclaration>::from_raw(p)];
                if node != declaration.variables.raw() {
                    return false;
                }
                let list = &ctx.ast[declaration.variables];
                if !ctx
                    .ast
                    .list(list.variables)
                    .iter()
                    .any(|v| !private_token(ctx, ctx.ast[*v].name))
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::ClassDeclaration => {
                let declaration = &ctx.ast[Id::<ClassDeclaration>::from_raw(p)];
                if private_token(ctx, ctx.ast.begin_token(declaration.name_part))
                    || (node != declaration.name_part.raw() && node != declaration.body.raw())
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::MixinDeclaration => {
                let declaration = &ctx.ast[Id::<MixinDeclaration>::from_raw(p)];
                if private_token(ctx, declaration.name)
                    || (Some(node) != declaration.type_parameters.map(|n| n.raw())
                        && Some(node) != declaration.on_clause.map(|n| n.raw())
                        && node != declaration.body.raw())
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::EnumDeclaration => {
                let declaration = &ctx.ast[Id::<EnumDeclaration>::from_raw(p)];
                if private_token(ctx, ctx.ast.begin_token(declaration.name_part))
                    || (node != declaration.name_part.raw() && node != declaration.body.raw())
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::ExtensionTypeDeclaration => {
                let declaration = &ctx.ast[Id::<ExtensionTypeDeclaration>::from_raw(p)];
                if private_token(ctx, ctx.ast.begin_token(declaration.name_part))
                    || (node != declaration.name_part.raw() && node != declaration.body.raw())
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::ExtensionDeclaration => {
                let declaration = &ctx.ast[Id::<ExtensionDeclaration>::from_raw(p)];
                if declaration.name.is_none_or(|n| private_token(ctx, n))
                    || (Some(node) != declaration.type_parameters.map(|n| n.raw())
                        && Some(node) != declaration.on_clause.map(|n| n.raw())
                        && node != declaration.body.raw())
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::RegularFormalParameter => {
                let n = &ctx.ast[Id::<RegularFormalParameter>::from_raw(p)];
                if n.kind == dartr_element::ParameterKind::Named
                    && n.name.is_some_and(|t| private_token(ctx, t))
                {
                    return false;
                }
                within_regular_formal = true;
            }
            NodeKind::FieldFormalParameter => {
                let n = &ctx.ast[Id::<FieldFormalParameter>::from_raw(p)];
                if n.kind == dartr_element::ParameterKind::Named && private_token(ctx, n.name) {
                    return false;
                }
            }
            NodeKind::SuperFormalParameter => {
                let n = &ctx.ast[Id::<SuperFormalParameter>::from_raw(p)];
                if n.kind == dartr_element::ParameterKind::Named && private_token(ctx, n.name) {
                    return false;
                }
            }
            NodeKind::ClassTypeAlias => {
                let alias = &ctx.ast[Id::<ClassTypeAlias>::from_raw(p)];
                if private_token(ctx, alias.name)
                    || Some(node) != alias.type_parameters.map(|n| n.raw())
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::FunctionTypeAlias => {
                if private_token(ctx, ctx.ast[Id::<FunctionTypeAlias>::from_raw(p)].name) {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::GenericTypeAlias => {
                let alias = &ctx.ast[Id::<GenericTypeAlias>::from_raw(p)];
                if private_token(ctx, alias.name)
                    || (Some(node) != alias.type_parameters.map(|n| n.raw())
                        && (node != alias.type_.raw()
                            || ctx.ast.kind(alias.type_) != NodeKind::GenericFunctionType))
                {
                    return false;
                }
                public_declaration = true;
            }
            NodeKind::CompilationUnit => {
                return public_declaration && !library_is_private(ctx);
            }
            _ => {}
        }
        node = p;
    }
    false
}

fn check_implicit_formal(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let (type_, name, kind) = match ctx.ast.kind(node) {
        NodeKind::FieldFormalParameter => {
            let parameter = &ctx.ast[Id::<FieldFormalParameter>::from_raw(node)];
            (parameter.type_, parameter.name, parameter.kind)
        }
        NodeKind::SuperFormalParameter => {
            let parameter = &ctx.ast[Id::<SuperFormalParameter>::from_raw(node)];
            (parameter.type_, parameter.name, parameter.kind)
        }
        _ => return,
    };
    if type_.is_some()
        || (kind == dartr_element::ParameterKind::Named && private_token(ctx, name))
        || !public_api_position(ctx, node)
    {
        return;
    }
    let Some(resolved) = ctx.resolved else {
        return;
    };
    let Some(parameter) = ctx
        .declared_element(node)
        .and_then(|element| element.cast::<FormalParameterElement>())
    else {
        return;
    };
    let type_ = member::type_(&resolved.ctx, ElemRef::Base(parameter.raw()));
    let TypeKind::Interface { element, .. } = *resolved.ctx.ty(type_) else {
        return;
    };
    if resolved
        .ctx
        .element_name(element.raw())
        .is_some_and(|name| name.starts_with('_'))
    {
        ctx.report_token(out, &diag::LIBRARY_PRIVATE_TYPES_IN_PUBLIC_API, name, &[]);
    }
}
fn check(ctx: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(r) = ctx.resolved else {
        return;
    };
    let Some(e) = ctx.element(node) else {
        return;
    };
    let base = member::base_element(&r.ctx, e);
    if r.ctx.element_name(base).is_some_and(|n| n.starts_with('_'))
        && public_api_position(ctx, node)
    {
        ctx.report_token(
            out,
            &diag::LIBRARY_PRIVATE_TYPES_IN_PUBLIC_API,
            ctx.ast[Id::<NamedType>::from_raw(node)].name,
            &[],
        );
    }
}
