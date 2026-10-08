// Dart source: pkg/analyzer/lib/src/dart/element/least_greatest_closure.dart

//! `LeastGreatestClosureHelper` and `PatternGreatestClosureHelper`: the
//! least and greatest closures of a type with respect to type parameters.

use dartr_element::{Ctx, EId, TypeId, TypeKind, TypeParameterElement};

use crate::replacement_visitor::{
    ReplacementVisitor, super_visit_function_type, super_visit_type_parameter_type,
};
use crate::type_algebra::unite_nullabilities;
use crate::type_ext::TypeExt;
use crate::type_system::TypeSystem;

/// `LeastGreatestClosureHelper`.
pub struct LeastGreatestClosureHelper<'a> {
    pub type_system: TypeSystem<'a>,
    pub top_type: TypeId,
    pub top_function_type: TypeId,
    pub bottom_type: TypeId,
    /// Dart: `Set<TypeParameterElementImpl>` (identity set). Small, so a
    /// `Vec` with `contains`.
    pub elimination_targets: Vec<EId<TypeParameterElement>>,
    /// Dart: `late final bool _isLeastClosure`.
    is_least_closure: bool,
    is_covariant: bool,
}

impl<'a> LeastGreatestClosureHelper<'a> {
    pub fn new(
        type_system: TypeSystem<'a>,
        top_type: TypeId,
        top_function_type: TypeId,
        bottom_type: TypeId,
        elimination_targets: Vec<EId<TypeParameterElement>>,
    ) -> Self {
        LeastGreatestClosureHelper {
            type_system,
            top_type,
            top_function_type,
            bottom_type,
            elimination_targets,
            is_least_closure: false,
            is_covariant: true,
        }
    }

    /// `_functionReplacement`.
    fn function_replacement(&self) -> TypeId {
        if self.is_least_closure && self.is_covariant
            || (!self.is_least_closure && !self.is_covariant)
        {
            self.bottom_type
        } else {
            self.top_function_type
        }
    }

    /// `_typeParameterReplacement`.
    fn type_parameter_replacement(&self) -> TypeId {
        if self.is_least_closure && self.is_covariant
            || (!self.is_least_closure && !self.is_covariant)
        {
            self.bottom_type
        } else {
            self.top_type
        }
    }

    /// `eliminateToGreatest(type)`: returns a supertype of [t] for all
    /// values of `eliminationTargets`.
    pub fn eliminate_to_greatest(&mut self, t: TypeId) -> TypeId {
        self.is_covariant = true;
        self.is_least_closure = false;
        self.visit(t).unwrap_or(t)
    }

    /// `eliminateToLeast(type)`: returns a subtype of [t] for all values of
    /// `eliminationTargets`.
    pub fn eliminate_to_least(&mut self, t: TypeId) -> TypeId {
        self.is_covariant = true;
        self.is_least_closure = true;
        self.visit(t).unwrap_or(t)
    }
}

impl<'a> ReplacementVisitor<'a> for LeastGreatestClosureHelper<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.type_system.ctx
    }

    fn change_variance(&mut self) {
        self.is_covariant = !self.is_covariant;
    }

    fn visit_function_type(&mut self, t: TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        let TypeKind::Function(node) = *ctx.ty(t) else {
            unreachable!()
        };
        // - if `S` is
        //   `T Function<X0 extends B0, ...., Xk extends Bk>(T0 x0, ...., Tn xn,
        //       [Tn+1 xn+1, ..., Tm xm])`
        //   or `T Function<X0 extends B0, ...., Xk extends Bk>(T0 x0, ...., Tn xn,
        //       {Tn+1 xn+1, ..., Tm xm})`
        //   and `L` contains any free type variables from any of the `Bi`:
        //  - The least closure of `S` with respect to `L` is `Never`
        //  - The greatest closure of `S` with respect to `L` is `Function`
        for &type_parameter in ctx.list(node.type_params) {
            if let Some(bound) = ctx.type_parameter_bound(type_parameter)
                && ctx.references_any(bound, &self.elimination_targets)
            {
                return Some(self.function_replacement());
            }
        }

        super_visit_function_type(self, t)
    }

    fn visit_type_parameter_type(&mut self, t: TypeId) -> Option<TypeId> {
        let ctx = self.ctx();
        let TypeKind::TypeParameter { param, .. } = *ctx.ty(t) else {
            unreachable!()
        };
        // Dart: identity set `contains` (element identity).
        if self.elimination_targets.contains(&param) {
            let replacement = self.type_parameter_replacement();
            return Some(ctx.with_nullability(
                replacement,
                unite_nullabilities(
                    ctx.nullability_suffix(replacement),
                    ctx.nullability_suffix(t),
                ),
            ));
        }
        super_visit_type_parameter_type(self, t)
    }
}

/// `PatternGreatestClosureHelper`.
pub struct PatternGreatestClosureHelper<'a> {
    pub ctx: Ctx<'a>,
    pub top_type: TypeId,
    pub bottom_type: TypeId,
    is_covariant: bool,
}

impl<'a> PatternGreatestClosureHelper<'a> {
    pub fn new(ctx: Ctx<'a>, top_type: TypeId, bottom_type: TypeId) -> Self {
        PatternGreatestClosureHelper {
            ctx,
            top_type,
            bottom_type,
            is_covariant: true,
        }
    }

    /// `eliminateToGreatest(type)`: returns a supertype of [t] for all
    /// values of type parameters.
    pub fn eliminate_to_greatest(&mut self, t: TypeId) -> TypeId {
        self.is_covariant = true;
        self.visit(t).unwrap_or(t)
    }
}

impl<'a> ReplacementVisitor<'a> for PatternGreatestClosureHelper<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }

    fn change_variance(&mut self) {
        self.is_covariant = !self.is_covariant;
    }

    fn visit_type_parameter_type(&mut self, t: TypeId) -> Option<TypeId> {
        let ctx = self.ctx;
        let replacement = if self.is_covariant {
            self.top_type
        } else {
            self.bottom_type
        };
        Some(ctx.with_nullability(
            replacement,
            unite_nullabilities(
                ctx.nullability_suffix(replacement),
                ctx.nullability_suffix(t),
            ),
        ))
    }
}
