// Dart source: pkg/analyzer/lib/src/dart/element/replace_top_bottom_visitor.dart

//! `ReplaceTopBottomVisitor`.
//!
//! Replace every "top" type in a covariant position with `bottom_type`.
//! Replace every "bottom" type in a contravariant position with `top_type`.

use dartr_element::{FnParam, TypeId, TypeKind, Variance};

use crate::type_ext::TypeExt;
use crate::type_system::TypeSystem;

/// `Variance.combine(other)` (`_fe_analyzer_shared/.../shared_type.dart`).
fn combine(this: Variance, other: Variance) -> Variance {
    if this == Variance::Unrelated || other == Variance::Unrelated {
        return Variance::Unrelated;
    }
    if this == Variance::Invariant || other == Variance::Invariant {
        return Variance::Invariant;
    }
    if this == other {
        Variance::Covariant
    } else {
        Variance::Contravariant
    }
}

/// `ReplaceTopBottomVisitor`.
struct ReplaceTopBottomVisitor<'a> {
    type_system: TypeSystem<'a>,
    top_type: TypeId,
    bottom_type: TypeId,
}

impl ReplaceTopBottomVisitor<'_> {
    /// `process(type, variance)`.
    fn process(&self, t: TypeId, variance: Variance) -> TypeId {
        let ctx = self.type_system.ctx;
        if variance == Variance::Contravariant {
            // ...replacing every occurrence in `T` of a type `S` in a contravariant
            // position where `S <: Never` by `Object?`
            if self.type_system.is_subtype_of(t, TypeId::NEVER) {
                return self.top_type;
            }
        } else {
            // ...and every occurrence in `T` of a top type in a position which
            // is not contravariant by `Never`.
            if self.type_system.is_top(t) {
                return self.bottom_type;
            }
        }

        if ctx.type_alias(t).is_some() {
            return self.instantiated_type_alias(t, variance);
        }
        match *ctx.ty(t) {
            TypeKind::Interface { .. } => self.interface_type(t, variance),
            TypeKind::Function(_) => self.function_type(t, variance),
            _ => t,
        }
    }

    /// `_functionType(type, variance)`.
    fn function_type(&self, t: TypeId, variance: Variance) -> TypeId {
        let ctx = self.type_system.ctx;
        let TypeKind::Function(f) = *ctx.ty(t) else {
            unreachable!()
        };
        let new_return_type = self.process(f.ret, variance);

        let new_parameters: Vec<FnParam> = ctx
            .list(f.params)
            .iter()
            .map(|parameter| FnParam {
                ty: self.process(parameter.ty, combine(variance, Variance::Contravariant)),
                ..*parameter
            })
            .collect();

        // Dart: `FunctionTypeImpl(...)` without an alias.
        ctx.function_type(
            ctx.list(f.type_params),
            &new_parameters,
            new_return_type,
            f.nullability,
            None,
        )
    }

    /// `_instantiatedTypeAlias(type, alias, variance)`.
    fn instantiated_type_alias(&self, t: TypeId, variance: Variance) -> TypeId {
        let ctx = self.type_system.ctx;
        let alias = *ctx.alias(ctx.type_alias(t).unwrap());
        let alias_element = alias.element;
        let alias_arguments = ctx.list(alias.args);

        let type_parameters = &ctx.get(alias_element).type_params;
        debug_assert_eq!(type_parameters.len(), alias_arguments.len());

        let mut new_type_arguments = Vec::with_capacity(type_parameters.len());
        for (i, &type_parameter) in type_parameters.iter().enumerate() {
            new_type_arguments.push(self.process(
                alias_arguments[i],
                combine(ctx.type_parameter_variance(type_parameter), variance),
            ));
        }

        ctx.instantiate_type_alias(alias_element, &new_type_arguments, alias.nullability)
    }

    /// `_interfaceType(type, variance)`.
    fn interface_type(&self, t: TypeId, variance: Variance) -> TypeId {
        let ctx = self.type_system.ctx;
        let TypeKind::Interface {
            element,
            args,
            nullability,
            ..
        } = *ctx.ty(t)
        else {
            unreachable!()
        };
        let type_parameters = ctx.interface_type_parameters(element);
        if type_parameters.is_empty() {
            return t;
        }

        let type_arguments = ctx.list(args);
        debug_assert_eq!(type_parameters.len(), type_arguments.len());

        let new_type_arguments: Vec<TypeId> = type_arguments
            .iter()
            .map(|&type_argument| self.process(type_argument, variance))
            .collect();

        // Dart: `InterfaceTypeImpl(...)` without an alias.
        ctx.interface_type(element, &new_type_arguments, nullability)
    }
}

/// `ReplaceTopBottomVisitor.run(topType, bottomType, typeSystem, type)`.
///
/// Runs an instance of the visitor on the given [t] and returns the
/// resulting type.
pub fn run(
    type_system: TypeSystem<'_>,
    top_type: TypeId,
    bottom_type: TypeId,
    t: TypeId,
) -> TypeId {
    let visitor = ReplaceTopBottomVisitor {
        type_system,
        top_type,
        bottom_type,
    };
    visitor.process(t, Variance::Covariant)
}
