// Dart source: pkg/analyzer/lib/src/dart/element/runtime_type_equality.dart

//! `RuntimeTypeEqualityHelper` and `RuntimeTypeEqualityVisitor`
//! (nnbd/feature-specification.md#runtime-type-equality-operator).
//!
//! The Dart visitor dispatches on `T1` (`T1.acceptWithArgument(this, T2)`);
//! here [`RuntimeTypeEqualityVisitor::visit`] matches on the kind of `T1`.

use dartr_element::{EId, Nullability, TypeId, TypeKind, TypeParameterElement};

use crate::type_algebra::MapSubstitution;
use crate::type_ext::{TypeExt, is_named, is_optional, is_positional};
use crate::type_system::TypeSystem;

/// `RuntimeTypeEqualityHelper`.
pub struct RuntimeTypeEqualityHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

impl<'a> RuntimeTypeEqualityHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        RuntimeTypeEqualityHelper { type_system }
    }

    /// `equal(T1, T2)`: returns whether runtime types [T1] and [T2] are
    /// equal.
    pub fn equal(&self, t1: TypeId, t2: TypeId) -> bool {
        let n1 = self.type_system.normalize(t1);
        let n2 = self.type_system.normalize(t2);
        RuntimeTypeEqualityVisitor::new(self.type_system).visit(n1, n2)
    }
}

/// `RuntimeTypeEqualityVisitor`.
pub struct RuntimeTypeEqualityVisitor<'a> {
    type_system: TypeSystem<'a>,
}

/// `_TypeParametersResult`.
struct TypeParametersResult {
    t1_substitution: MapSubstitution,
    t2_substitution: MapSubstitution,
}

impl<'a> RuntimeTypeEqualityVisitor<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        RuntimeTypeEqualityVisitor { type_system }
    }

    /// `T1.acceptWithArgument(this, T2)`.
    pub fn visit(&self, t1: TypeId, t2: TypeId) -> bool {
        match *self.type_system.ctx.ty(t1) {
            // Dart: identical(T1, T2)
            TypeKind::Dynamic => t1 == t2,
            TypeKind::Function(_) => self.visit_function_type(t1, t2),
            TypeKind::Interface { .. } => self.visit_interface_type(t1, t2),
            // Dart: identical(T1, T2)
            TypeKind::Invalid => t1 == t2,
            TypeKind::Never(_) => self.visit_never_type(t1, t2),
            TypeKind::Record { .. } => self.visit_record_type(t1, t2),
            TypeKind::TypeParameter { .. } => self.visit_type_parameter_type(t1, t2),
            // Dart: identical(T1, T2)
            TypeKind::Void => t1 == t2,
            // Dart: UnknownInferredType.acceptWithArgument throws.
            TypeKind::Unknown => panic!("Should not happen outside inference."),
        }
    }

    /// `visitFunctionType(T1, T2)`.
    fn visit_function_type(&self, t1: TypeId, t2: TypeId) -> bool {
        let ts = self.type_system;
        let ctx = ts.ctx;
        let TypeKind::Function(f1) = *ctx.ty(t1) else {
            unreachable!();
        };
        let TypeKind::Function(f2) = *ctx.ty(t2) else {
            return false;
        };

        let Some(type_parameters) =
            self.type_parameters(ctx.list(f1.type_params), ctx.list(f2.type_params))
        else {
            return false;
        };

        if ts.is_nullable(t1) != ts.is_nullable(t2) {
            // The nullabilities are different.
            return false;
        }

        let equal = |t1: TypeId, t2: TypeId| -> bool {
            let t1 = type_parameters.t1_substitution.substitute_type(&ctx, t1);
            let t2 = type_parameters.t2_substitution.substitute_type(&ctx, t2);
            self.visit(t1, t2)
        };

        if !equal(f1.ret, f2.ret) {
            return false;
        }

        let t1_formal_parameters = ctx.list(f1.params);
        let t2_formal_parameters = ctx.list(f2.params);
        if t1_formal_parameters.len() != t2_formal_parameters.len() {
            return false;
        }

        for (t1_parameter, t2_parameter) in t1_formal_parameters.iter().zip(t2_formal_parameters) {
            if is_positional(t1_parameter.kind) != is_positional(t2_parameter.kind) {
                return false;
            }
            if is_optional(t1_parameter.kind) != is_optional(t2_parameter.kind) {
                return false;
            }

            if is_named(t1_parameter.kind) && t1_parameter.name != t2_parameter.name {
                return false;
            }

            if !equal(t1_parameter.ty, t2_parameter.ty) {
                return false;
            }
        }

        true
    }

    /// `visitInterfaceType(T1, T2)`.
    fn visit_interface_type(&self, t1: TypeId, t2: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        let TypeKind::Interface {
            element: e1,
            args: args1,
            ..
        } = *ctx.ty(t1)
        else {
            unreachable!();
        };
        if let TypeKind::Interface {
            element: e2,
            args: args2,
            ..
        } = *ctx.ty(t2)
            // Dart: identical(T1.element, T2.element)
            && e1 == e2
            && self.compatible_nullability(t1, t2)
        {
            let t1_type_arguments = ctx.list(args1);
            let t2_type_arguments = ctx.list(args2);
            if t1_type_arguments.len() == t2_type_arguments.len() {
                for (&t1_type_argument, &t2_type_argument) in
                    t1_type_arguments.iter().zip(t2_type_arguments)
                {
                    if !self.visit(t1_type_argument, t2_type_argument) {
                        return false;
                    }
                }
                return true;
            }
        }
        false
    }

    /// `visitNeverType(T1, T2)`.
    fn visit_never_type(&self, t1: TypeId, t2: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        // Note, that all types are normalized before this visitor.
        // So, `Never?` never happens, it is already `Null`.
        debug_assert_ne!(ctx.nullability_suffix(t1), Nullability::Question);
        matches!(ctx.ty(t2), TypeKind::Never(_)) && self.compatible_nullability(t1, t2)
    }

    /// `visitRecordType(T1, T2)`.
    fn visit_record_type(&self, t1: TypeId, t2: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        let (
            TypeKind::Record {
                positional: positional1,
                named: named1,
                ..
            },
            TypeKind::Record {
                positional: positional2,
                named: named2,
                ..
            },
        ) = (*ctx.ty(t1), *ctx.ty(t2))
        else {
            return false;
        };

        if !self.compatible_nullability(t1, t2) {
            return false;
        }

        let positional1 = ctx.list(positional1);
        let positional2 = ctx.list(positional2);
        if positional1.len() != positional2.len() {
            return false;
        }

        let named1 = ctx.list(named1);
        let named2 = ctx.list(named2);
        if named1.len() != named2.len() {
            return false;
        }

        for (&field1, &field2) in positional1.iter().zip(positional2) {
            if !self.visit(field1, field2) {
                return false;
            }
        }

        for (field1, field2) in named1.iter().zip(named2) {
            if field1.name != field2.name {
                return false;
            }
            if !self.visit(field1.ty, field2.ty) {
                return false;
            }
        }

        true
    }

    /// `visitTypeParameterType(T1, T2)`.
    fn visit_type_parameter_type(&self, t1: TypeId, t2: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        let TypeKind::TypeParameter { param: e1, .. } = *ctx.ty(t1) else {
            unreachable!();
        };
        match *ctx.ty(t2) {
            TypeKind::TypeParameter { param: e2, .. } => {
                // Dart: identical(T1.element, T2.element)
                self.compatible_nullability(t1, t2) && e1 == e2
            }
            _ => false,
        }
    }

    /// `_compatibleNullability(T1, T2)`.
    fn compatible_nullability(&self, t1: TypeId, t2: TypeId) -> bool {
        let ctx = self.type_system.ctx;
        ctx.nullability_suffix(t1) == ctx.nullability_suffix(t2)
    }

    /// `_typeParameters(T1_parameters, T2_parameters)`: determines if the
    /// two lists of type parameters are equal. If they are, returns the
    /// substitutions necessary to demonstrate their equality. If they
    /// aren't, returns `None`.
    fn type_parameters(
        &self,
        t1_parameters: &[EId<TypeParameterElement>],
        t2_parameters: &[EId<TypeParameterElement>],
    ) -> Option<TypeParametersResult> {
        let ctx = self.type_system.ctx;
        if t1_parameters.len() != t2_parameters.len() {
            return None;
        }

        let mut new_types = Vec::with_capacity(t1_parameters.len());
        for &t1_parameter in t1_parameters {
            let new_parameter = ctx.fresh_copy(t1_parameter);
            new_types.push(ctx.type_parameter_type(new_parameter, Nullability::None));
        }

        let t1_substitution = MapSubstitution::from_pairs(t1_parameters, &new_types);
        let t2_substitution = MapSubstitution::from_pairs(t2_parameters, &new_types);
        for (&t1_parameter, &t2_parameter) in t1_parameters.iter().zip(t2_parameters) {
            let t1_bound = ctx.type_parameter_bound(t1_parameter);
            let t2_bound = ctx.type_parameter_bound(t2_parameter);
            match (t1_bound, t2_bound) {
                (None, None) => {
                    // OK, no bound.
                }
                (Some(t1_bound), Some(t2_bound)) => {
                    let t1_bound = t1_substitution.substitute_type(&ctx, t1_bound);
                    let t2_bound = t2_substitution.substitute_type(&ctx, t2_bound);
                    if !self.visit(t1_bound, t2_bound) {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(TypeParametersResult {
            t1_substitution,
            t2_substitution,
        })
    }
}
