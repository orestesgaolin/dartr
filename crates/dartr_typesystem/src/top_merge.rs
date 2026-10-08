// Dart source: pkg/analyzer/lib/src/dart/element/top_merge.dart

//! `TopMergeHelper` (NNBD_TOP_MERGE).
//!
//! Where Dart throws (`StateError` or `_TopMergeStateError`), the Rust
//! functions return `None`. Dart's `_TopMergeStateError` "should never
//! happen, because we should never attempt `NNBD_TOP_MERGE` for types that
//! are not subtypes of each other, and already NORM(ed)".

use dartr_element::{
    EId, FnParam, NamedType, Nullability, ParameterKind, TypeId, TypeKind, TypeParameterElement,
};

use crate::type_algebra::MapSubstitution;
use crate::type_ext::{
    TypeExt, is_named, is_optional_positional, is_required_named, is_required_positional,
};
use crate::type_system::TypeSystem;

/// `TopMergeHelper`.
pub struct TopMergeHelper<'a> {
    pub type_system: TypeSystem<'a>,
}

/// `_MergeTypeParametersResult`.
struct MergeTypeParametersResult {
    type_parameters: Vec<EId<TypeParameterElement>>,
    a_substitution: MapSubstitution,
    b_substitution: MapSubstitution,
}

impl<'a> TopMergeHelper<'a> {
    pub fn new(type_system: TypeSystem<'a>) -> Self {
        TopMergeHelper { type_system }
    }

    /// `topMerge(T, S)`; `None` where Dart throws.
    ///
    /// Merges two types into a single type.
    /// Compute the canonical representation of [T].
    ///
    /// https://github.com/dart-lang/language/
    /// See `accepted/future-releases/nnbd/feature-specification.md`
    /// See `#classes-defined-in-opted-in-libraries`
    pub fn top_merge(&self, t: TypeId, s: TypeId) -> Option<TypeId> {
        let ts = self.type_system;
        let ctx = ts.ctx;
        let t_nullability = ctx.nullability_suffix(t);
        let s_nullability = ctx.nullability_suffix(s);
        let t_kind = *ctx.ty(t);
        let s_kind = *ctx.ty(s);

        // NNBD_TOP_MERGE(Object?, Object?) = Object?
        let t_is_object_question =
            t_nullability == Nullability::Question && ctx.is_dart_core_object(t);
        let s_is_object_question =
            s_nullability == Nullability::Question && ctx.is_dart_core_object(s);
        if t_is_object_question && s_is_object_question {
            return Some(t);
        }

        // NNBD_TOP_MERGE(dynamic, dynamic) = dynamic
        let t_is_dynamic = matches!(t_kind, TypeKind::Dynamic);
        let s_is_dynamic = matches!(s_kind, TypeKind::Dynamic);
        if t_is_dynamic && s_is_dynamic {
            return Some(TypeId::DYNAMIC);
        }

        if matches!(t_kind, TypeKind::Invalid) || matches!(s_kind, TypeKind::Invalid) {
            return Some(TypeId::INVALID);
        }

        if matches!(t_kind, TypeKind::Never(Nullability::None))
            && matches!(s_kind, TypeKind::Never(Nullability::None))
        {
            return Some(TypeId::NEVER);
        }

        // NNBD_TOP_MERGE(void, void) = void
        let t_is_void = matches!(t_kind, TypeKind::Void);
        let s_is_void = matches!(s_kind, TypeKind::Void);
        if t_is_void && s_is_void {
            return Some(TypeId::VOID);
        }

        // NNBD_TOP_MERGE(Object?, void) = Object?
        // NNBD_TOP_MERGE(void, Object?) = Object?
        if t_is_object_question && s_is_void || t_is_void && s_is_object_question {
            return Some(ts.object_question());
        }

        // NNBD_TOP_MERGE(dynamic, void) = Object?
        // NNBD_TOP_MERGE(void, dynamic) = Object?
        if t_is_dynamic && s_is_void || t_is_void && s_is_dynamic {
            return Some(ts.object_question());
        }

        // NNBD_TOP_MERGE(Object?, dynamic) = Object?
        // NNBD_TOP_MERGE(dynamic, Object?) = Object?
        if t_is_object_question && s_is_dynamic {
            return Some(t);
        }
        if t_is_dynamic && s_is_object_question {
            return Some(s);
        }

        // Merge nullabilities.
        let t_is_question = t_nullability == Nullability::Question;
        let s_is_question = s_nullability == Nullability::Question;
        if t_is_question && s_is_question {
            let t_none = ctx.with_nullability(t, Nullability::None);
            let s_none = ctx.with_nullability(s, Nullability::None);
            let r_none = self.top_merge(t_none, s_none)?;
            return Some(ctx.with_nullability(r_none, Nullability::Question));
        } else if t_is_question || s_is_question {
            // Dart: throw StateError('$T_nullability vs $S_nullability').
            return None;
        }

        debug_assert_eq!(t_nullability, Nullability::None);
        debug_assert_eq!(s_nullability, Nullability::None);

        // And for all other types, recursively apply the transformation over
        // the structure of the type.
        //
        // For example: NNBD_TOP_MERGE(C<T>, C<S>) = C<NNBD_TOP_MERGE(T, S)>
        //
        // The NNBD_TOP_MERGE of two types is not defined for types which are not
        // otherwise structurally equal.

        match (t_kind, s_kind) {
            (TypeKind::Interface { .. }, TypeKind::Interface { .. }) => self.interface_types(t, s),
            (TypeKind::Function(_), TypeKind::Function(_)) => self.function_types(t, s),
            (TypeKind::Record { .. }, TypeKind::Record { .. }) => self.record_types(t, s),
            (
                TypeKind::TypeParameter {
                    param: t_element, ..
                },
                TypeKind::TypeParameter {
                    param: s_element, ..
                },
            ) => {
                // Dart: `T.element == S.element` (elements compare by identity).
                if t_element == s_element {
                    Some(t)
                } else {
                    // Dart: throw _TopMergeStateError('Not the same type parameters').
                    None
                }
            }
            // Dart: throw _TopMergeStateError('Unexpected pair').
            _ => None,
        }
    }

    /// `_functionTypes(T, S)`.
    fn function_types(&self, t: TypeId, s: TypeId) -> Option<TypeId> {
        let ts = self.type_system;
        let ctx = ts.ctx;
        let (TypeKind::Function(tf), TypeKind::Function(sf)) = (*ctx.ty(t), *ctx.ty(s)) else {
            unreachable!();
        };
        let t_type_parameters = ctx.list(tf.type_params);
        let s_type_parameters = ctx.list(sf.type_params);
        if t_type_parameters.len() != s_type_parameters.len() {
            // 'Different number of type parameters'
            return None;
        }

        let r_type_parameters: Vec<EId<TypeParameterElement>>;
        let mut substitutions: Option<(MapSubstitution, MapSubstitution)> = None;

        if !t_type_parameters.is_empty() {
            // 'Unable to merge type parameters'
            let merged = self.type_parameters(t_type_parameters, s_type_parameters)?;
            r_type_parameters = merged.type_parameters;
            substitutions = Some((merged.a_substitution, merged.b_substitution));
        } else {
            r_type_parameters = Vec::new();
        }

        let merge_types = |t: TypeId, s: TypeId| -> Option<TypeId> {
            let (t, s) = match &substitutions {
                Some((t_substitution, s_substitution)) => (
                    t_substitution.substitute_type(&ctx, t),
                    s_substitution.substitute_type(&ctx, s),
                ),
                None => (t, s),
            };
            self.top_merge(t, s)
        };

        let r_return_type = merge_types(tf.ret, sf.ret)?;

        let t_parameters = ctx.list(tf.params);
        let s_parameters = ctx.list(sf.params);
        if t_parameters.len() != s_parameters.len() {
            // 'Different number of formal parameters'
            return None;
        }

        let mut r_parameters: Vec<FnParam> = Vec::with_capacity(t_parameters.len());
        for (t_parameter, s_parameter) in t_parameters.iter().zip(s_parameters) {
            // 'Different formal parameter kinds'
            let r_kind = parameter_kind(t_parameter.kind, s_parameter.kind)?;

            if is_named(t_parameter.kind) && t_parameter.name != s_parameter.name {
                // 'Different named parameter names'
                return None;
            }

            // Given two corresponding parameters of type `T1` and `T2`, where at least
            // one of the parameters is covariant:
            let t_is_covariant = t_parameter.covariant;
            let s_is_covariant = s_parameter.covariant;
            let r_is_covariant = t_is_covariant || s_is_covariant;
            let r_type = if r_is_covariant {
                let t1 = t_parameter.ty;
                let t2 = s_parameter.ty;
                let t1_is_subtype = ts.is_subtype_of(t1, t2);
                let t2_is_subtype = ts.is_subtype_of(t2, t1);
                if t1_is_subtype && t2_is_subtype {
                    // if `T1 <: T2` and `T2 <: T1`, then the result is
                    // `NNBD_TOP_MERGE(T1, T2)`, and it is covariant.
                    merge_types(t_parameter.ty, s_parameter.ty)?
                } else if t1_is_subtype {
                    // otherwise, if `T1 <: T2`, then the result is
                    // `T2` and it is covariant.
                    t2
                } else {
                    // otherwise, the result is `T1` and it is covariant.
                    t1
                }
            } else {
                merge_types(t_parameter.ty, s_parameter.ty)?
            };

            // `T_parameter.copyWith(type: R_type, kind: R_kind)`: the
            // covariance of `T_parameter` stays (as in Dart).
            r_parameters.push(FnParam {
                ty: r_type,
                kind: r_kind,
                ..*t_parameter
            });
        }

        Some(ctx.function_type(
            &r_type_parameters,
            &r_parameters,
            r_return_type,
            Nullability::None,
            None,
        ))
    }

    /// `_interfaceTypes(T, S)`.
    fn interface_types(&self, t: TypeId, s: TypeId) -> Option<TypeId> {
        let ctx = self.type_system.ctx;
        let (
            TypeKind::Interface {
                element: t_element,
                args: t_args,
                ..
            },
            TypeKind::Interface {
                element: s_element,
                args: s_args,
                ..
            },
        ) = (*ctx.ty(t), *ctx.ty(s))
        else {
            unreachable!();
        };
        // Dart: `T.element != S.element` (elements compare by identity).
        if t_element != s_element {
            // 'Different class elements'
            return None;
        }

        let t_arguments = ctx.list(t_args);
        let s_arguments = ctx.list(s_args);
        if t_arguments.is_empty() {
            Some(t)
        } else {
            let arguments = t_arguments
                .iter()
                .zip(s_arguments)
                .map(|(&t_argument, &s_argument)| self.top_merge(t_argument, s_argument))
                .collect::<Option<Vec<TypeId>>>()?;
            Some(ctx.instantiate_interface(t_element, &arguments, Nullability::None))
        }
    }

    /// `_recordTypes(T1, T2)`.
    fn record_types(&self, t1: TypeId, t2: TypeId) -> Option<TypeId> {
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
            unreachable!();
        };

        let positional1 = ctx.list(positional1);
        let positional2 = ctx.list(positional2);
        if positional1.len() != positional2.len() {
            // 'Different number of position fields'
            return None;
        }

        let mut positional_fields = Vec::with_capacity(positional1.len());
        for (&field1, &field2) in positional1.iter().zip(positional2) {
            positional_fields.push(self.top_merge(field1, field2)?);
        }

        let named1 = ctx.list(named1);
        let named2 = ctx.list(named2);
        if named1.len() != named2.len() {
            // 'Different number of named fields'
            return None;
        }

        let mut named_fields = Vec::with_capacity(named1.len());
        for (field1, field2) in named1.iter().zip(named2) {
            if field1.name != field2.name {
                // 'Different named field names'
                return None;
            }
            let ty = self.top_merge(field1.ty, field2.ty)?;
            named_fields.push(NamedType {
                name: field1.name,
                ty,
            });
        }

        Some(ctx.record_type(&positional_fields, &named_fields, Nullability::None, None))
    }

    /// `_typeParameters(aParameters, bParameters)`.
    fn type_parameters(
        &self,
        a_parameters: &[EId<TypeParameterElement>],
        b_parameters: &[EId<TypeParameterElement>],
    ) -> Option<MergeTypeParametersResult> {
        let ctx = self.type_system.ctx;
        if a_parameters.len() != b_parameters.len() {
            return None;
        }

        let mut new_parameters = Vec::with_capacity(a_parameters.len());
        let mut new_types = Vec::with_capacity(a_parameters.len());
        for &a_parameter in a_parameters {
            let new_parameter = ctx.fresh_copy(a_parameter);
            new_parameters.push(new_parameter);
            new_types.push(ctx.type_parameter_type(new_parameter, Nullability::None));
        }

        let a_substitution = MapSubstitution::from_pairs(a_parameters, &new_types);
        let b_substitution = MapSubstitution::from_pairs(b_parameters, &new_types);
        for i in 0..a_parameters.len() {
            let a = a_parameters[i];
            let b = b_parameters[i];

            let a_bound = ctx.type_parameter_bound(a);
            let b_bound = ctx.type_parameter_bound(b);
            match (a_bound, b_bound) {
                (None, None) => {
                    // OK, no bound.
                }
                (Some(a_bound), Some(b_bound)) => {
                    let a_bound = a_substitution.substitute_type(&ctx, a_bound);
                    let b_bound = b_substitution.substitute_type(&ctx, b_bound);
                    let new_bound = self.top_merge(a_bound, b_bound)?;
                    ctx.get(new_parameters[i]).bound.set(Some(new_bound));
                }
                _ => return None,
            }
        }

        Some(MergeTypeParametersResult {
            type_parameters: new_parameters,
            a_substitution,
            b_substitution,
        })
    }
}

/// `_parameterKind(T, S)`.
fn parameter_kind(t: ParameterKind, s: ParameterKind) -> Option<ParameterKind> {
    if is_required_positional(t) && is_required_positional(s) {
        return Some(ParameterKind::Required);
    }

    if is_optional_positional(t) && is_optional_positional(s) {
        return Some(ParameterKind::Positional);
    }

    if is_required_named(t) && is_required_named(s) {
        return Some(ParameterKind::NamedRequired);
    }

    if is_optional_named(t) && is_optional_named(s) {
        return Some(ParameterKind::Named);
    }

    None
}

/// `isOptionalNamed`.
fn is_optional_named(kind: ParameterKind) -> bool {
    matches!(kind, ParameterKind::Named)
}
