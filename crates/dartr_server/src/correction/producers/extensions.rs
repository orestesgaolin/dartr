// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_extension_member.dart (CreateExtensionGetter, CreateExtensionMethod, _CreateExtensionMember)

//! Create a getter or method in a new or existing extension of the target
//! type. Types with type parameters are not handled (no fix).

use dartr_ast::*;
use dartr_element::{Ctx, Tag, TypeId, TypeKind};

use super::super::change_builder::{ChangeBuilder, EditBuilder};
use super::super::dart_edit::WriteType;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::create::{
    ArgumentInfo, Inferred, argument_infos, infer_undefined_expression_type, real_target,
    static_type,
};
use super::members::{climb_property_access, enclosing_instance_element, in_static_context};

/// Whether [ty] mentions a type parameter.
fn has_type_parameters(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    match *ctx.ty(ty) {
        TypeKind::TypeParameter { .. } => true,
        TypeKind::Interface { args, .. } => {
            ctx.list(args).iter().any(|t| has_type_parameters(ctx, *t))
        }
        TypeKind::Function(f) => {
            !ctx.list(f.type_params).is_empty()
                || has_type_parameters(ctx, f.ret)
                || ctx
                    .list(f.params)
                    .iter()
                    .any(|p| has_type_parameters(ctx, p.ty))
        }
        TypeKind::Record {
            positional, named, ..
        } => {
            ctx.list(positional)
                .iter()
                .any(|t| has_type_parameters(ctx, *t))
                || ctx
                    .list(named)
                    .iter()
                    .any(|n| has_type_parameters(ctx, n.ty))
        }
        _ => false,
    }
}

/// The target expression of a property name (Dart, getter case).
fn property_target(ast: &Ast, name: NodeId) -> Option<NodeId> {
    let parent = ast.parent(name)?;
    if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
        if ast[p].identifier.raw() == name {
            return Some(ast[p].prefix.raw());
        }
    } else if let Some(p) = ast.cast::<PropertyAccess>(parent) {
        if ast[p].property_name.raw() == name {
            return ast[p].target.map(|t| t.raw());
        }
    }
    None
}

/// The target type and extension element (Dart, shared code of the
/// producers).
fn target_type(
    c: &ProducerContext<'_>,
    target: Option<NodeId>,
    add_static: &mut bool,
) -> Option<(TypeId, Option<NodeId>)> {
    let ast = c.ast;
    let ctx = c.ctx;
    let mut ty = match target {
        Some(t) if ast.is::<ExtensionOverride>(t) => return None,
        None => {
            let e = enclosing_instance_element(c, c.node)?;
            if e.tag() == Tag::Extension {
                // An existing extension: not ported.
                return None;
            }
            let i = e.cast::<dartr_element::InterfaceElement>()?;
            Some(ctx.interface(i).this_type.try_get().copied()?)
        }
        Some(t) => static_type(c, t),
    };
    if ty.is_none() {
        if let Some(t) = target.and_then(|t| ast.cast::<SimpleIdentifier>(t)) {
            if c.element_of(t.raw())
                .is_some_and(|e| e.tag() == Tag::Extension)
            {
                *add_static = true;
                return None;
            }
        }
    }
    let ty = ty.take()?;
    if matches!(ctx.ty(ty), TypeKind::Dynamic | TypeKind::Invalid) {
        return None;
    }
    Some((ty, None))
}

/// Dart `_existingExtension`: an extension of the unit with the extended
/// type [ty].
fn existing_extension(c: &ProducerContext<'_>, ty: TypeId) -> Option<NodeId> {
    let ast = c.ast;
    for &d in ast.list_raw(ast[c.unit].declarations) {
        if !ast.is::<ExtensionDeclaration>(d) {
            continue;
        }
        let e = c
            .locator()
            .declared_element(d)?
            .cast::<dartr_element::ExtensionElement>()?;
        if c.ctx.get(e).extended_type.get() == Some(ty) {
            return Some(d);
        }
    }
    None
}

/// Dart `_addNewExtension` (without type parameters) or the update of an
/// existing extension.
fn add_member(
    c: &ProducerContext<'_>,
    builder: &mut ChangeBuilder<'_>,
    ty: TypeId,
    is_getter: bool,
    write: &dyn Fn(&mut EditBuilder<'_, '_, '_>),
) {
    let ast = c.ast;
    if let Some(extension) = existing_extension(c, ty) {
        builder.add_dart_file_edit(c.path, |b| {
            if is_getter {
                b.insert_getter(extension, |e| write(e));
            } else {
                b.insert_method(extension, |e| write(e));
            }
        });
        return;
    }
    // Dart `enclosingUnitChild`.
    let Some(member) = ast.this_or_ancestor_matching(c.node, |a, n| {
        a.parent(n).is_some_and(|p| a.is::<CompilationUnit>(p))
    }) else {
        return;
    };
    let end = ast.end(member);
    builder.add_dart_file_edit(c.path, |b| {
        b.add_insertion(end, |e| {
            e.newline();
            e.newline();
            e.write("extension ");
            e.write("on ");
            e.write_type(Some(ty), &WriteType::default());
            e.writeln(" {");
            e.write("  ");
            write(e);
            e.newline();
            e.write("}");
        });
    });
}

/// Dart `CreateExtensionGetter`.
pub struct CreateExtensionGetter {
    name: String,
}

impl CreateExtensionGetter {
    pub fn new() -> Self {
        CreateExtensionGetter {
            name: String::new(),
        }
    }
}

impl CorrectionProducer for CreateExtensionGetter {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_EXTENSION_GETTER)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let ctx = c.ctx;
        let mut add_static = in_static_context(c);
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        // Dart `inGetterContext`.
        let climbed = climb_property_access(ast, c.node);
        if let Some(a) = ast
            .parent(climbed)
            .and_then(|p| ast.cast::<AssignmentExpression>(p))
        {
            if ast[a].left_hand_side.raw() == climbed && c.lexeme(ast[a].operator) == "=" {
                return;
            }
        }
        self.name = c.lexeme(ast[id].token).to_string();
        let target = property_target(ast, c.node);
        let Some((ty, _)) = target_type(c, target, &mut add_static) else {
            return;
        };
        let field_type = match infer_undefined_expression_type(c, climbed) {
            Inferred::Invalid => return,
            Inferred::Type(t) => Some(t),
            Inferred::Unknown => None,
        };
        if has_type_parameters(ctx, ty) || field_type.is_some_and(|t| has_type_parameters(ctx, t)) {
            return;
        }
        let name = self.name.clone();
        let write = move |e: &mut EditBuilder<'_, '_, '_>| {
            if add_static {
                e.write("static ");
            }
            if let Some(t) = field_type {
                e.write_type(Some(t), &WriteType::default());
                e.write(" ");
            }
            e.write(&format!("get {name} => "));
            e.add_linked_edit("VALUE", |e| e.write("null"));
            e.write(";");
        };
        add_member(c, builder, ty, true, &write);
    }
}

/// Dart `CreateExtensionMethod` (for invocations).
pub struct CreateExtensionMethod {
    name: String,
}

impl CreateExtensionMethod {
    pub fn new() -> Self {
        CreateExtensionMethod {
            name: String::new(),
        }
    }
}

impl CorrectionProducer for CreateExtensionMethod {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_EXTENSION_METHOD)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let ctx = c.ctx;
        let mut add_static = in_static_context(c);
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        let Some(invocation) = ast.parent(id).and_then(|p| ast.cast::<MethodInvocation>(p)) else {
            // Dart: function-typed properties (not ported).
            return;
        };
        if ast[invocation].method_name != id {
            return;
        }
        self.name = c.lexeme(ast[id].token).to_string();
        let target = real_target(ast, invocation);
        let Some((ty, _)) = target_type(c, target, &mut add_static) else {
            return;
        };
        let return_type = match infer_undefined_expression_type(c, invocation.raw()) {
            Inferred::Invalid => return,
            Inferred::Type(t) => Some(t),
            Inferred::Unknown => None,
        };
        let infos: Vec<ArgumentInfo> = argument_infos(c, ast[invocation].argument_list);
        if has_type_parameters(ctx, ty)
            || return_type.is_some_and(|t| has_type_parameters(ctx, t))
            || infos.iter().any(|i| has_type_parameters(ctx, i.ty))
            || ast[invocation].type_arguments.is_some()
        {
            return;
        }
        let name = self.name.clone();
        let write = move |e: &mut EditBuilder<'_, '_, '_>| {
            if add_static {
                e.write("static ");
            }
            let options = WriteType {
                group_name: Some("RETURN_TYPE".into()),
                ..Default::default()
            };
            if e.write_type(return_type, &options) {
                e.write(" ");
            }
            e.add_linked_edit("NAME", |e| e.write(&name));
            e.write("(");
            e.write_parameters_matching_arguments(ctx, &infos);
            e.write(")");
            e.write(" {}");
        };
        add_member(c, builder, ty, false, &write);
    }
}
