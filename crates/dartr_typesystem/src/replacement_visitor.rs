// Dart source: pkg/analyzer/lib/src/dart/element/replacement_visitor.dart

//! `ReplacementVisitor`: rebuilds a type when a nested type is replaced and
//! returns `None` otherwise.
//!
//! Dart subclasses override some `visit*` / `create*` methods and call
//! `super`. In Rust the trait methods have default bodies that call the
//! `super_*` free functions of this module; an override calls the free
//! function for `super.visitX(...)`.
//!
//! The linker type builders (`visitFunctionTypeBuilder`,
//! `visitNamedTypeBuilder`, `visitRecordTypeBuilder`) are not part of the
//! Rust type model and are not ported.

use dartr_element::{
    AliasId, AliasRef, Ctx, EId, FnParam, NamedType, Nullability,
    ParameterKind, TypeId, TypeKind, TypeParameterElement,
};
use indexmap::IndexMap;

use crate::type_algebra::MapSubstitution;
use crate::type_ext::TypeExt;

/// `ReplacementVisitor`. Implementors give [`ReplacementVisitor::ctx`] and
/// override what they need.
pub trait ReplacementVisitor<'a> {
    fn ctx(&self) -> Ctx<'a>;

    /// `changeVariance()`.
    fn change_variance(&mut self) {}

    /// `type.accept(this)`.
    fn visit(&mut self, t: TypeId) -> Option<TypeId> {
        match *self.ctx().ty(t) {
            TypeKind::Dynamic => self.visit_dynamic_type(t),
            TypeKind::Void => self.visit_void_type(t),
            TypeKind::Invalid => self.visit_invalid_type(t),
            TypeKind::Unknown => self.visit_unknown_inferred_type(t),
            TypeKind::Never(_) => self.visit_never_type(t),
            TypeKind::Interface { .. } => self.visit_interface_type(t),
            TypeKind::Function(_) => self.visit_function_type(t),
            TypeKind::Record { .. } => self.visit_record_type(t),
            TypeKind::TypeParameter { .. } => self.visit_type_parameter_type(t),
        }
    }

    /// `createFunctionType(...)`.
    fn create_function_type(
        &mut self,
        t: TypeId,
        new_alias: Option<AliasId>,
        new_type_parameters: Option<Vec<EId<TypeParameterElement>>>,
        new_parameters: Option<Vec<FnParam>>,
        new_return_type: Option<TypeId>,
        new_nullability: Option<Nullability>,
    ) -> Option<TypeId> {
        super_create_function_type(
            &self.ctx(),
            t,
            new_alias,
            new_type_parameters,
            new_parameters,
            new_return_type,
            new_nullability,
        )
    }

    /// `createInterfaceType(...)`.
    fn create_interface_type(
        &mut self,
        t: TypeId,
        new_alias: Option<AliasId>,
        new_type_arguments: Option<Vec<TypeId>>,
        new_nullability: Option<Nullability>,
    ) -> Option<TypeId> {
        super_create_interface_type(&self.ctx(), t, new_alias, new_type_arguments, new_nullability)
    }

    /// `createNeverType(...)`.
    fn create_never_type(&mut self, t: TypeId, new_nullability: Option<Nullability>) -> Option<TypeId> {
        let n = new_nullability?;
        Some(self.ctx().with_nullability(t, n))
    }

    /// `createPromotedTypeParameterType(...)`.
    fn create_promoted_type_parameter_type(
        &mut self,
        t: TypeId,
        new_nullability: Option<Nullability>,
        new_promoted_bound: Option<TypeId>,
    ) -> Option<TypeId> {
        super_create_promoted_type_parameter_type(&self.ctx(), t, new_nullability, new_promoted_bound)
    }

    /// `createTypeParameterType(...)`.
    fn create_type_parameter_type(
        &mut self,
        t: TypeId,
        new_nullability: Option<Nullability>,
    ) -> Option<TypeId> {
        super_create_type_parameter_type(&self.ctx(), t, new_nullability)
    }

    fn visit_dynamic_type(&mut self, _t: TypeId) -> Option<TypeId> {
        None
    }

    fn visit_function_type(&mut self, t: TypeId) -> Option<TypeId> {
        super_visit_function_type(self, t)
    }

    fn visit_interface_type(&mut self, t: TypeId) -> Option<TypeId> {
        super_visit_interface_type(self, t)
    }

    fn visit_invalid_type(&mut self, _t: TypeId) -> Option<TypeId> {
        None
    }

    fn visit_never_type(&mut self, t: TypeId) -> Option<TypeId> {
        let n = self.visit_nullability(t);
        self.create_never_type(t, n)
    }

    /// `visitNullability(type)`.
    fn visit_nullability(&mut self, _t: TypeId) -> Option<Nullability> {
        None
    }

    /// `visitParameterKind(kind)`.
    fn visit_parameter_kind(&mut self, _kind: ParameterKind) -> Option<ParameterKind> {
        None
    }

    fn visit_record_type(&mut self, t: TypeId) -> Option<TypeId> {
        super_visit_record_type(self, t)
    }

    /// `visitTypeArgument(parameter, argument)`.
    fn visit_type_argument(
        &mut self,
        _parameter: EId<TypeParameterElement>,
        argument: TypeId,
    ) -> Option<TypeId> {
        self.visit(argument)
    }

    /// `visitTypeParameterBound(type)`.
    fn visit_type_parameter_bound(&mut self, t: TypeId) -> Option<TypeId> {
        self.visit(t)
    }

    fn visit_type_parameter_type(&mut self, t: TypeId) -> Option<TypeId> {
        super_visit_type_parameter_type(self, t)
    }

    fn visit_unknown_inferred_type(&mut self, _t: TypeId) -> Option<TypeId> {
        None
    }

    fn visit_void_type(&mut self, _t: TypeId) -> Option<TypeId> {
        None
    }
}

/// The base `createFunctionType`.
pub fn super_create_function_type(
    ctx: &Ctx<'_>,
    t: TypeId,
    new_alias: Option<AliasId>,
    new_type_parameters: Option<Vec<EId<TypeParameterElement>>>,
    new_parameters: Option<Vec<FnParam>>,
    new_return_type: Option<TypeId>,
    new_nullability: Option<Nullability>,
) -> Option<TypeId> {
    if new_alias.is_none()
        && new_nullability.is_none()
        && new_return_type.is_none()
        && new_parameters.is_none()
    {
        return None;
    }
    let TypeKind::Function(f) = *ctx.ty(t) else {
        unreachable!()
    };
    let type_params = new_type_parameters.unwrap_or_else(|| ctx.list(f.type_params).to_vec());
    let params = new_parameters.unwrap_or_else(|| ctx.list(f.params).to_vec());
    Some(ctx.function_type(
        &type_params,
        &params,
        new_return_type.unwrap_or(f.ret),
        new_nullability.unwrap_or(f.nullability),
        new_alias.or(f.alias),
    ))
}

/// The base `createInterfaceType`.
pub fn super_create_interface_type(
    ctx: &Ctx<'_>,
    t: TypeId,
    new_alias: Option<AliasId>,
    new_type_arguments: Option<Vec<TypeId>>,
    new_nullability: Option<Nullability>,
) -> Option<TypeId> {
    if new_alias.is_none() && new_type_arguments.is_none() && new_nullability.is_none() {
        return None;
    }
    let TypeKind::Interface {
        element,
        args,
        nullability,
        alias,
    } = *ctx.ty(t)
    else {
        unreachable!()
    };
    let args = new_type_arguments.unwrap_or_else(|| ctx.list(args).to_vec());
    Some(ctx.interface_type_with_alias(
        element,
        &args,
        new_nullability.unwrap_or(nullability),
        new_alias.or(alias),
    ))
}

/// The base `createPromotedTypeParameterType`.
pub fn super_create_promoted_type_parameter_type(
    ctx: &Ctx<'_>,
    t: TypeId,
    new_nullability: Option<Nullability>,
    new_promoted_bound: Option<TypeId>,
) -> Option<TypeId> {
    if new_nullability.is_none() && new_promoted_bound.is_none() {
        return None;
    }
    let TypeKind::TypeParameter {
        param,
        nullability,
        promoted_bound,
        alias,
    } = *ctx.ty(t)
    else {
        unreachable!()
    };
    Some(ctx.intern(TypeKind::TypeParameter {
        param,
        nullability: new_nullability.unwrap_or(nullability),
        promoted_bound: new_promoted_bound.or(promoted_bound),
        alias,
    }))
}

/// The base `createTypeParameterType`.
pub fn super_create_type_parameter_type(
    ctx: &Ctx<'_>,
    t: TypeId,
    new_nullability: Option<Nullability>,
) -> Option<TypeId> {
    let n = new_nullability?;
    let TypeKind::TypeParameter { param, alias, .. } = *ctx.ty(t) else {
        unreachable!()
    };
    Some(ctx.intern(TypeKind::TypeParameter {
        param,
        nullability: n,
        promoted_bound: None,
        alias,
    }))
}

/// The base `visitFunctionType`.
pub fn super_visit_function_type<'a, V: ReplacementVisitor<'a> + ?Sized>(
    v: &mut V,
    t: TypeId,
) -> Option<TypeId> {
    let ctx = v.ctx();
    let TypeKind::Function(node) = *ctx.ty(t) else {
        unreachable!()
    };
    let new_nullability = v.visit_nullability(t);

    let type_parameters = ctx.list(node.type_params);
    let mut new_type_parameters: Option<Vec<EId<TypeParameterElement>>> = None;
    for (i, &type_parameter) in type_parameters.iter().enumerate() {
        if let Some(bound) = ctx.type_parameter_bound(type_parameter)
            && let Some(new_bound) = v.visit_type_parameter_bound(bound)
        {
            let list = new_type_parameters.get_or_insert_with(|| type_parameters.to_vec());
            // `typeParameter.freshCopy()..bound = newBound` (no variance).
            let fresh = ctx.fresh_copy(type_parameter);
            ctx.get(fresh).bound.set(Some(new_bound));
            list[i] = fresh;
        }
    }

    let mut substitution: Option<MapSubstitution> = None;
    if let Some(new_type_parameters) = &new_type_parameters {
        let mut map = IndexMap::new();
        for (i, &new_type_parameter) in new_type_parameters.iter().enumerate() {
            map.insert(
                type_parameters[i],
                ctx.type_parameter_type(new_type_parameter, Nullability::None),
            );
        }
        let s = MapSubstitution::from_map(map);
        for &new_type_parameter in new_type_parameters {
            if let Some(bound) = ctx.type_parameter_bound(new_type_parameter) {
                let new_bound = s.substitute_type(&ctx, bound);
                ctx.get(new_type_parameter).bound.set(Some(new_bound));
            }
        }
        substitution = Some(s);
    }

    let visit_type = |v: &mut V, ty: TypeId| -> Option<TypeId> {
        let mut result = v.visit(ty);
        if let Some(s) = &substitution {
            result = Some(s.substitute_type(&ctx, result.unwrap_or(ty)));
        }
        result
    };

    let new_return_type = visit_type(v, node.ret);

    let mut new_alias = None;
    if let Some(alias) = node.alias {
        let a = *ctx.alias(alias);
        let alias_arguments = ctx.list(a.args);
        let mut new_arguments: Option<Vec<TypeId>> = None;
        for (i, &arg) in alias_arguments.iter().enumerate() {
            if let Some(s) = v.visit(arg) {
                new_arguments.get_or_insert_with(|| alias_arguments.to_vec())[i] = s;
            }
        }
        if let Some(new_arguments) = new_arguments {
            new_alias = Some(ctx.intern_alias(AliasRef {
                args: ctx.intern_list(&new_arguments),
                ..a
            }));
        }
    }

    v.change_variance();

    let parameters = ctx.list(node.params);
    let mut new_parameters: Option<Vec<FnParam>> = None;
    for (i, parameter) in parameters.iter().enumerate() {
        let new_type = visit_type(v, parameter.ty);
        let new_kind = v.visit_parameter_kind(parameter.kind);
        if new_type.is_some() || new_kind.is_some() {
            new_parameters.get_or_insert_with(|| parameters.to_vec())[i] = FnParam {
                ty: new_type.unwrap_or(parameter.ty),
                kind: new_kind.unwrap_or(parameter.kind),
                ..*parameter
            };
        }
    }

    v.change_variance();

    v.create_function_type(
        t,
        new_alias,
        new_type_parameters,
        new_parameters,
        new_return_type,
        new_nullability,
    )
}

/// `_typeArguments(parameters, arguments)`.
pub fn replace_type_arguments<'a, V: ReplacementVisitor<'a> + ?Sized>(
    v: &mut V,
    parameters: &[EId<TypeParameterElement>],
    arguments: &[TypeId],
) -> Option<Vec<TypeId>> {
    if arguments.len() != parameters.len() {
        return None;
    }
    let mut new_arguments: Option<Vec<TypeId>> = None;
    for (i, &argument) in arguments.iter().enumerate() {
        if let Some(s) = v.visit_type_argument(parameters[i], argument) {
            new_arguments.get_or_insert_with(|| arguments.to_vec())[i] = s;
        }
    }
    new_arguments
}

fn replace_alias<'a, V: ReplacementVisitor<'a> + ?Sized>(
    v: &mut V,
    alias: Option<AliasId>,
) -> Option<AliasId> {
    let ctx = v.ctx();
    let a = *ctx.alias(alias?);
    let parameters = &ctx.get(a.element).type_params;
    let new_arguments = replace_type_arguments(v, parameters, ctx.list(a.args))?;
    Some(ctx.intern_alias(AliasRef {
        args: ctx.intern_list(&new_arguments),
        ..a
    }))
}

/// The base `visitInterfaceType`.
pub fn super_visit_interface_type<'a, V: ReplacementVisitor<'a> + ?Sized>(
    v: &mut V,
    t: TypeId,
) -> Option<TypeId> {
    let ctx = v.ctx();
    let TypeKind::Interface {
        element,
        args,
        alias,
        ..
    } = *ctx.ty(t)
    else {
        unreachable!()
    };
    let new_nullability = v.visit_nullability(t);
    let new_alias = replace_alias(v, alias);
    let new_type_arguments =
        replace_type_arguments(v, ctx.interface_type_parameters(element), ctx.list(args));
    v.create_interface_type(t, new_alias, new_type_arguments, new_nullability)
}

/// The base `visitRecordType`.
pub fn super_visit_record_type<'a, V: ReplacementVisitor<'a> + ?Sized>(
    v: &mut V,
    t: TypeId,
) -> Option<TypeId> {
    let ctx = v.ctx();
    let TypeKind::Record {
        positional,
        named,
        nullability,
        alias,
    } = *ctx.ty(t)
    else {
        unreachable!()
    };
    let new_nullability = v.visit_nullability(t);
    let new_alias = replace_alias(v, alias);

    let positional_fields = ctx.list(positional);
    let mut new_positional: Option<Vec<TypeId>> = None;
    for (i, &field) in positional_fields.iter().enumerate() {
        if let Some(new_type) = v.visit(field) {
            new_positional.get_or_insert_with(|| positional_fields.to_vec())[i] = new_type;
        }
    }

    let named_fields = ctx.list(named);
    let mut new_named: Option<Vec<NamedType>> = None;
    for (i, field) in named_fields.iter().enumerate() {
        if let Some(new_type) = v.visit(field.ty) {
            new_named.get_or_insert_with(|| named_fields.to_vec())[i] = NamedType {
                name: field.name,
                ty: new_type,
            };
        }
    }

    if new_alias.is_none()
        && new_positional.is_none()
        && new_named.is_none()
        && new_nullability.is_none()
    {
        return None;
    }

    Some(ctx.record_type(
        new_positional.as_deref().unwrap_or(positional_fields),
        new_named.as_deref().unwrap_or(named_fields),
        new_nullability.unwrap_or(nullability),
        new_alias.or(alias),
    ))
}

/// The base `visitTypeParameterType`.
pub fn super_visit_type_parameter_type<'a, V: ReplacementVisitor<'a> + ?Sized>(
    v: &mut V,
    t: TypeId,
) -> Option<TypeId> {
    let ctx = v.ctx();
    let TypeKind::TypeParameter { promoted_bound, .. } = *ctx.ty(t) else {
        unreachable!()
    };
    let new_nullability = v.visit_nullability(t);
    if let Some(promoted_bound) = promoted_bound {
        let new_promoted_bound = v.visit(promoted_bound);
        return v.create_promoted_type_parameter_type(t, new_nullability, new_promoted_bound);
    }
    v.create_type_parameter_type(t, new_nullability)
}
