// Dart source: pkg/analyzer/lib/src/dart/element/type.dart (`operator ==` of
// FunctionTypeImpl, InterfaceTypeImpl, NeverTypeImpl, RecordTypeImpl,
// TypeParameterTypeImpl, VoidTypeImpl, DynamicTypeImpl, InvalidTypeImpl;
// FunctionTypeImpl.relateTypeFormals and _equalParameters),
// type_schema.dart (UnknownInferredType ==)

//! Dart `==` on types (design §1.4). `TypeId ==` is Dart `identical`.
//!
//! Dart `==` ignores aliases, parameter names of positional parameters and
//! parameter elements, and compares generic function types up to renaming
//! of their type formals. Dart does the renaming with fresh type parameters
//! (`relateTypeFormals` + `instantiate`); this port compares under a
//! bijection between the formals of the two types instead, which gives the
//! same answer without creating elements:
//!
//! - `relateTypeFormals(f1, f2, (t, s) => t == s)` checks, for each pair of
//!   formals, that the bounds are equal after both lists are mapped to the
//!   same fresh variables (a missing bound is `dynamic`). With a bijection,
//!   a type parameter type of formal `i` of `f1` equals a type parameter type
//!   of formal `i` of `f2`.
//! - `instantiate(fresh)` drops the formals and substitutes; the comparison
//!   of the return types and parameters then runs under the same bijection.

use dartr_element::{Ctx, EId, TypeId, TypeKind, TypeParameterElement};

use crate::type_ext::{TypeExt, is_named, is_optional, is_positional};

/// Dart `left == right` for types.
pub fn dart_eq(ctx: &Ctx<'_>, a: TypeId, b: TypeId) -> bool {
    Eq { ctx, pairs: Vec::new() }.eq(a, b)
}

/// Dart `TypeImpl.equalArrays`.
pub fn dart_eq_lists(ctx: &Ctx<'_>, a: &[TypeId], b: &[TypeId]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| dart_eq(ctx, x, y))
}

struct Eq<'c, 'a> {
    ctx: &'c Ctx<'a>,
    /// Related type formals of enclosing generic function types: `(p1, p2)`
    /// means "the fresh variable of this pair".
    pairs: Vec<(EId<TypeParameterElement>, EId<TypeParameterElement>)>,
}

impl Eq<'_, '_> {
    fn same_param(&self, p1: EId<TypeParameterElement>, p2: EId<TypeParameterElement>) -> bool {
        // The innermost binding wins (formals shadow outer ones).
        for &(x, y) in self.pairs.iter().rev() {
            if x == p1 || y == p2 {
                return x == p1 && y == p2;
            }
        }
        p1 == p2
    }

    fn eq(&mut self, a: TypeId, b: TypeId) -> bool {
        // Dart: identical(other, this) (only valid without renaming).
        if a == b && self.pairs.is_empty() {
            return true;
        }
        let ctx = self.ctx;
        match (*ctx.ty(a), *ctx.ty(b)) {
            (TypeKind::Dynamic, TypeKind::Dynamic) => true,
            (TypeKind::Void, TypeKind::Void) => true,
            // InvalidTypeImpl and UnknownInferredType: `identical`.
            (TypeKind::Invalid, TypeKind::Invalid) => true,
            (TypeKind::Unknown, TypeKind::Unknown) => true,
            (TypeKind::Never(n1), TypeKind::Never(n2)) => n1 == n2,
            (
                TypeKind::Interface {
                    element: e1,
                    args: a1,
                    nullability: n1,
                    ..
                },
                TypeKind::Interface {
                    element: e2,
                    args: a2,
                    nullability: n2,
                    ..
                },
            ) => {
                if e1 != e2 || n1 != n2 {
                    return false;
                }
                let (a1, a2) = (ctx.list(a1), ctx.list(a2));
                a1.len() == a2.len() && a1.iter().zip(a2).all(|(&x, &y)| self.eq(x, y))
            }
            (
                TypeKind::TypeParameter {
                    param: p1,
                    nullability: n1,
                    promoted_bound: b1,
                    ..
                },
                TypeKind::TypeParameter {
                    param: p2,
                    nullability: n2,
                    promoted_bound: b2,
                    ..
                },
            ) => {
                if !self.same_param(p1, p2) || n1 != n2 {
                    return false;
                }
                match (b1, b2) {
                    (None, None) => true,
                    (Some(x), Some(y)) => self.eq(x, y),
                    _ => false,
                }
            }
            (
                TypeKind::Record {
                    positional: p1,
                    named: m1,
                    nullability: n1,
                    ..
                },
                TypeKind::Record {
                    positional: p2,
                    named: m2,
                    nullability: n2,
                    ..
                },
            ) => {
                if n1 != n2 {
                    return false;
                }
                let (p1, p2) = (ctx.list(p1), ctx.list(p2));
                if p1.len() != p2.len() || !p1.iter().zip(p2).all(|(&x, &y)| self.eq(x, y)) {
                    return false;
                }
                let (m1, m2) = (ctx.list(m1), ctx.list(m2));
                m1.len() == m2.len()
                    && m1
                        .iter()
                        .zip(m2)
                        .all(|(x, y)| x.name == y.name && self.eq(x.ty, y.ty))
            }
            (TypeKind::Function(f1), TypeKind::Function(f2)) => {
                if f1.nullability != f2.nullability {
                    return false;
                }
                let (t1, t2) = (ctx.list(f1.type_params), ctx.list(f2.type_params));
                if t1.len() != t2.len() {
                    return false;
                }
                let mark = self.pairs.len();
                let mut result = true;
                if !t1.is_empty() {
                    // relateTypeFormals: bounds are compared after mapping
                    // the formals 0..=i to the same fresh variables.
                    for i in 0..t1.len() {
                        self.pairs.push((t1[i], t2[i]));
                        let bound1 = ctx.type_parameter_bound(t1[i]).unwrap_or(TypeId::DYNAMIC);
                        let bound2 = ctx.type_parameter_bound(t2[i]).unwrap_or(TypeId::DYNAMIC);
                        // Dart: relation(bound2, bound1) is `==`, which is
                        // symmetric; the arguments keep the orientation of
                        // `pairs` (left from `a`, right from `b`).
                        if !self.eq(bound1, bound2) {
                            result = false;
                            break;
                        }
                    }
                }
                if result {
                    result = self.eq(f1.ret, f2.ret) && self.equal_parameters(f1, f2);
                }
                self.pairs.truncate(mark);
                result
            }
            _ => false,
        }
    }

    /// `FunctionTypeImpl._equalParameters`.
    fn equal_parameters(
        &mut self,
        f1: dartr_element::FunctionTypeData,
        f2: dartr_element::FunctionTypeData,
    ) -> bool {
        let ctx = self.ctx;
        let (first, second) = (ctx.list(f1.params), ctx.list(f2.params));
        if first.len() != second.len() {
            return false;
        }
        for (p1, p2) in first.iter().zip(second) {
            if is_positional(p1.kind) != is_positional(p2.kind) {
                return false;
            }
            if is_optional(p1.kind) != is_optional(p2.kind) {
                return false;
            }
            if !self.eq(p1.ty, p2.ty) {
                return false;
            }
            if is_named(p1.kind) && p1.name != p2.name {
                return false;
            }
        }
        true
    }
}
