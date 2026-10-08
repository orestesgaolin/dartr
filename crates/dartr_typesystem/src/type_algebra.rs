// Dart source: pkg/analyzer/lib/src/dart/element/type_algebra.dart

//! Substitution of type parameters (`Substitution`, `MapSubstitution`,
//! `FreshTypeParameters`, `getFreshTypeParameters`, `replaceTypeParameters`,
//! `uniteNullabilities`).
//!
//! Dart returns the same type object when a substitution changes nothing
//! (the `useCounter` logic). With interned types an unchanged type has the
//! same [`TypeId`]; the use counters are still ported, because a generic
//! function type whose fresh formals are the only change must keep its
//! original formals (and so its original `TypeId`).
//!
//! The function types built by the linker (`FunctionTypeBuilder`,
//! `NamedTypeBuilder`, `RecordTypeBuilder`) are not part of the Rust type
//! model, so their visit methods are not ported.

use dartr_element::{
    AliasId, AliasRef, Ctx, EId, FnParam, NamedType, Nullability, TypeId, TypeKind,
    TypeParameterElement,
};
use indexmap::IndexMap;

use crate::type_ext::TypeExt;

/// `uniteNullabilities(a, b)`.
pub fn unite_nullabilities(a: Nullability, b: Nullability) -> Nullability {
    if a == Nullability::Question || b == Nullability::Question {
        return Nullability::Question;
    }
    Nullability::None
}

/// Dart `Substitution`: maps type parameters to types.
pub trait Substitution {
    /// `getSubstitute(parameter, upperBound)`.
    fn get_substitute(
        &self,
        ctx: &Ctx<'_>,
        parameter: EId<TypeParameterElement>,
        upper_bound: bool,
    ) -> Option<TypeId>;

    /// Whether this is `Substitution.empty` (`_NullSubstitution`), whose
    /// `substituteType` returns the type unchanged.
    fn is_empty_substitution(&self) -> bool {
        false
    }

    /// `substituteType(type, contravariant: contravariant)`.
    fn substitute_type_with(&self, ctx: &Ctx<'_>, t: TypeId, contravariant: bool) -> TypeId
    where
        Self: Sized,
    {
        substitute_type(ctx, self, t, contravariant)
    }
}

/// `Substitution.substituteType(type, contravariant:)` for any
/// substitution.
pub fn substitute_type(
    ctx: &Ctx<'_>,
    substitution: &dyn Substitution,
    t: TypeId,
    contravariant: bool,
) -> TypeId {
    if substitution.is_empty_substitution() {
        return t;
    }
    let mut substitutor = TypeSubstitutor {
        ctx: *ctx,
        top: substitution,
        envs: vec![Env {
            map: Vec::new(),
            covariant_context: true,
            use_counter: 0,
        }],
    };
    if contravariant {
        substitutor.invert_variance();
    }
    substitutor.visit(t)
}

/// `MapSubstitution` (`_MapSubstitution`, and `_NullSubstitution` when the
/// map is empty). The map keeps Dart's insertion order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapSubstitution {
    pub map: IndexMap<EId<TypeParameterElement>, TypeId>,
}

impl MapSubstitution {
    /// `Substitution.empty`.
    pub fn empty() -> MapSubstitution {
        MapSubstitution::default()
    }

    /// `Substitution.fromMap(map)`.
    pub fn from_map(map: IndexMap<EId<TypeParameterElement>, TypeId>) -> MapSubstitution {
        MapSubstitution { map }
    }

    /// `Substitution.fromPairs2(parameters, types)`.
    pub fn from_pairs(parameters: &[EId<TypeParameterElement>], types: &[TypeId]) -> MapSubstitution {
        assert_eq!(parameters.len(), types.len());
        MapSubstitution {
            map: parameters.iter().copied().zip(types.iter().copied()).collect(),
        }
    }

    /// `Substitution.fromInterfaceType(type)`: the type parameters of the
    /// class of [t] to the type arguments of [t].
    pub fn from_interface_type(ctx: &Ctx<'_>, t: TypeId) -> MapSubstitution {
        let TypeKind::Interface { element, args, .. } = *ctx.ty(t) else {
            panic!("not an interface type");
        };
        let args = ctx.list(args);
        if args.is_empty() {
            return MapSubstitution::empty();
        }
        MapSubstitution::from_pairs(ctx.interface_type_parameters(element), args)
    }

    /// Whether this is `Substitution.empty`.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// `andThen(after)`: applies this substitution, then [after].
    pub fn and_then(&self, ctx: &Ctx<'_>, after: &MapSubstitution) -> MapSubstitution {
        if self.is_empty() {
            // _NullSubstitution.andThen returns this.
            return self.clone();
        }
        if after.is_empty() {
            return self.clone();
        }
        MapSubstitution {
            map: self
                .map
                .iter()
                .map(|(&k, &v)| (k, after.substitute_type(ctx, v)))
                .collect(),
        }
    }

    /// `substituteType(type)`.
    pub fn substitute_type(&self, ctx: &Ctx<'_>, t: TypeId) -> TypeId {
        substitute_type(ctx, self, t, false)
    }

    /// `substituteType(type, contravariant: contravariant)`.
    pub fn substitute_type_contravariant(&self, ctx: &Ctx<'_>, t: TypeId, contravariant: bool) -> TypeId {
        substitute_type(ctx, self, t, contravariant)
    }

    /// `mapInterfaceTypes(types)`.
    pub fn map_types(&self, ctx: &Ctx<'_>, types: &[TypeId]) -> Vec<TypeId> {
        types.iter().map(|&t| self.substitute_type(ctx, t)).collect()
    }
}

impl Substitution for MapSubstitution {
    fn get_substitute(
        &self,
        ctx: &Ctx<'_>,
        parameter: EId<TypeParameterElement>,
        _upper_bound: bool,
    ) -> Option<TypeId> {
        if self.map.is_empty() {
            // _NullSubstitution.getSubstitute
            return Some(ctx.type_parameter_type(parameter, Nullability::None));
        }
        self.map.get(&parameter).copied()
    }

    fn is_empty_substitution(&self) -> bool {
        self.map.is_empty()
    }
}

/// `_CombinedSubstitution` / `Substitution.combine(first, second)`:
/// variables of [first] win; neither is applied to the results of the
/// other.
pub struct CombinedSubstitution<'s> {
    pub first: &'s dyn Substitution,
    pub second: &'s dyn Substitution,
}

impl Substitution for CombinedSubstitution<'_> {
    fn get_substitute(
        &self,
        ctx: &Ctx<'_>,
        parameter: EId<TypeParameterElement>,
        upper_bound: bool,
    ) -> Option<TypeId> {
        self.first
            .get_substitute(ctx, parameter, upper_bound)
            .or_else(|| self.second.get_substitute(ctx, parameter, upper_bound))
    }

    fn is_empty_substitution(&self) -> bool {
        // `combine` returns the other substitution when one is empty.
        self.first.is_empty_substitution() && self.second.is_empty_substitution()
    }
}

/// `Substitution.combine(first, second)`.
pub fn combine<'s>(
    first: &'s dyn Substitution,
    second: &'s dyn Substitution,
) -> CombinedSubstitution<'s> {
    if first.is_empty_substitution() {
        // Dart returns [second]; a combination with an empty first part
        // behaves the same because the empty part is consulted first only
        // through `getSubstitute`, which `combine` never reaches for it.
        return CombinedSubstitution {
            first: second,
            second,
        };
    }
    if second.is_empty_substitution() {
        return CombinedSubstitution { first, second: first };
    }
    CombinedSubstitution { first, second }
}

/// A substitution given by a function (`_TypeVariableEliminator` and
/// similar subclasses of `Substitution` that override `getSubstitute`).
pub struct FnSubstitution<F>(pub F)
where
    F: Fn(EId<TypeParameterElement>, bool) -> Option<TypeId>;

impl<F> Substitution for FnSubstitution<F>
where
    F: Fn(EId<TypeParameterElement>, bool) -> Option<TypeId>,
{
    fn get_substitute(
        &self,
        _ctx: &Ctx<'_>,
        parameter: EId<TypeParameterElement>,
        upper_bound: bool,
    ) -> Option<TypeId> {
        (self.0)(parameter, upper_bound)
    }
}

/// `FreshTypeParameters`.
#[derive(Clone, Debug)]
pub struct FreshTypeParameters {
    pub fresh_type_parameters: Vec<EId<TypeParameterElement>>,
    pub substitution: MapSubstitution,
}

impl FreshTypeParameters {
    /// `applyToFunctionType(type)`.
    pub fn apply_to_function_type(&self, ctx: &Ctx<'_>, t: TypeId) -> TypeId {
        let TypeKind::Function(f) = *ctx.ty(t) else {
            panic!("not a function type");
        };
        let params: Vec<FnParam> = ctx
            .list(f.params)
            .iter()
            .map(|p| FnParam {
                ty: self.substitute(ctx, p.ty),
                ..*p
            })
            .collect();
        let ret = self.substitute(ctx, f.ret);
        ctx.function_type(&self.fresh_type_parameters, &params, ret, f.nullability, None)
    }

    /// `substitute(type)`.
    pub fn substitute(&self, ctx: &Ctx<'_>, t: TypeId) -> TypeId {
        self.substitution.substitute_type(ctx, t)
    }
}

/// `getFreshTypeParameters(typeParameters)`.
pub fn get_fresh_type_parameters(
    ctx: &Ctx<'_>,
    type_parameters: &[EId<TypeParameterElement>],
) -> FreshTypeParameters {
    // `freshCopy()` plus `if (!isLegacyCovariant) fresh.variance = variance`:
    // `variance` is set at creation in Rust.
    let fresh: Vec<EId<TypeParameterElement>> = type_parameters
        .iter()
        .map(|&p| ctx.fresh_copy_with_variance(p))
        .collect();
    let mut map = IndexMap::new();
    for (i, &p) in type_parameters.iter().enumerate() {
        map.insert(p, ctx.type_parameter_type(fresh[i], Nullability::None));
    }
    let substitution = MapSubstitution::from_map(map);
    for (i, &p) in type_parameters.iter().enumerate() {
        if let Some(bound) = ctx.type_parameter_bound(p) {
            let new_bound = substitution.substitute_type(ctx, bound);
            ctx.get(fresh[i]).bound.set(Some(new_bound));
        }
    }
    FreshTypeParameters {
        fresh_type_parameters: fresh,
        substitution,
    }
}

/// `replaceTypeParameters(type, newTypeParameters)`: the formals of the
/// generic function type [t] replaced by [new_type_parameters] in the
/// parameters and the return type.
pub fn replace_type_parameters(
    ctx: &Ctx<'_>,
    t: TypeId,
    new_type_parameters: &[EId<TypeParameterElement>],
) -> TypeId {
    let TypeKind::Function(f) = *ctx.ty(t) else {
        panic!("not a function type");
    };
    let type_params = ctx.list(f.type_params);
    assert_eq!(new_type_parameters.len(), type_params.len());
    if new_type_parameters.is_empty() {
        return t;
    }
    let args: Vec<TypeId> = new_type_parameters
        .iter()
        .map(|&p| ctx.type_parameter_type(p, Nullability::None))
        .collect();
    let substitution = MapSubstitution::from_pairs(type_params, &args);
    let params: Vec<FnParam> = ctx
        .list(f.params)
        .iter()
        .map(|p| FnParam {
            ty: substitution.substitute_type(ctx, p.ty),
            ..*p
        })
        .collect();
    let ret = substitution.substitute_type(ctx, f.ret);
    ctx.function_type(new_type_parameters, &params, ret, f.nullability, None)
}

/// One environment of `_TypeSubstitutor`: the top one is `_TopSubstitutor`
/// (it asks the substitution), the inner ones are
/// `_FreshTypeParametersSubstitutor` (fresh formals of generic function
/// types).
struct Env {
    /// Fresh formals of this environment (empty for the top one).
    map: Vec<(EId<TypeParameterElement>, TypeId)>,
    covariant_context: bool,
    use_counter: u32,
}

/// `_TypeSubstitutor` with its chain of environments (`outer`).
struct TypeSubstitutor<'s, 'a> {
    ctx: Ctx<'a>,
    top: &'s dyn Substitution,
    envs: Vec<Env>,
}

impl TypeSubstitutor<'_, '_> {
    fn current(&mut self) -> &mut Env {
        self.envs.last_mut().unwrap()
    }

    fn invert_variance(&mut self) {
        let env = self.current();
        env.covariant_context = !env.covariant_context;
    }

    /// `lookup(parameter, upperBound)` of the environment [index].
    fn lookup(
        &self,
        index: usize,
        parameter: EId<TypeParameterElement>,
        upper_bound: bool,
    ) -> Option<TypeId> {
        if index == 0 {
            self.top.get_substitute(&self.ctx, parameter, upper_bound)
        } else {
            self.envs[index]
                .map
                .iter()
                .find(|(p, _)| *p == parameter)
                .map(|&(_, t)| t)
        }
    }

    /// `getSubstitute(parameter)`.
    fn get_substitute(&mut self, parameter: EId<TypeParameterElement>) -> Option<TypeId> {
        // Dart: `environment.lookup(parameter, covariantContext)` with the
        // covariantContext of the visitor (the innermost environment).
        let covariant = self.envs.last().unwrap().covariant_context;
        let mut index = self.envs.len();
        while index > 0 {
            index -= 1;
            if let Some(replacement) = self.lookup(index, parameter, covariant) {
                // bumpCountersUntil(environment)
                for env in &mut self.envs[index..] {
                    env.use_counter += 1;
                }
                return Some(replacement);
            }
        }
        None
    }

    /// `_FreshTypeParametersSubstitutor.freshTypeParameters(elements)` on the
    /// new innermost environment.
    fn fresh_type_parameters(
        &mut self,
        elements: &[EId<TypeParameterElement>],
    ) -> Vec<EId<TypeParameterElement>> {
        let ctx = self.ctx;
        let fresh: Vec<EId<TypeParameterElement>> = elements
            .iter()
            .map(|&e| {
                let fresh = ctx.fresh_copy_with_variance(e);
                let fresh_type = ctx.type_parameter_type(fresh, Nullability::None);
                self.current().map.push((e, fresh_type));
                fresh
            })
            .collect();
        for (i, &e) in elements.iter().enumerate() {
            if let Some(bound) = ctx.type_parameter_bound(e) {
                let new_bound = self.visit(bound);
                ctx.get(fresh[i]).bound.set(Some(new_bound));
            }
        }
        fresh
    }

    fn map_alias(&mut self, alias: Option<AliasId>) -> Option<AliasId> {
        let alias = alias?;
        let a = *self.ctx.alias(alias);
        let args: Vec<TypeId> = self.ctx.list(a.args).to_vec();
        let args = self.map_list(&args);
        let args = self.ctx.intern_list(&args);
        Some(self.ctx.intern_alias(AliasRef { args, ..a }))
    }

    fn map_list(&mut self, types: &[TypeId]) -> Vec<TypeId> {
        types.iter().map(|&t| self.visit(t)).collect()
    }

    fn visit(&mut self, t: TypeId) -> TypeId {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Dynamic
            | TypeKind::Void
            | TypeKind::Invalid
            | TypeKind::Unknown
            | TypeKind::Never(_) => t,
            TypeKind::Interface {
                element,
                args,
                nullability,
                alias,
            } => {
                if args.is_empty() && alias.is_none() {
                    return t;
                }
                let before = self.current().use_counter;
                let args = self.map_list(ctx.list(args));
                let alias = self.map_alias(alias);
                if self.current().use_counter == before {
                    return t;
                }
                ctx.interface_type_with_alias(element, &args, nullability, alias)
            }
            TypeKind::Record {
                positional,
                named,
                nullability,
                alias,
            } => {
                let before = self.current().use_counter;
                let positional = self.map_list(ctx.list(positional));
                let named: Vec<NamedType> = ctx
                    .list(named)
                    .iter()
                    .map(|f| NamedType {
                        name: f.name,
                        ty: self.visit(f.ty),
                    })
                    .collect();
                let alias = self.map_alias(alias);
                if self.current().use_counter == before {
                    return t;
                }
                ctx.record_type(&positional, &named, nullability, alias)
            }
            TypeKind::TypeParameter { param, nullability, .. } => {
                let Some(argument) = self.get_substitute(param) else {
                    return t;
                };
                let n = unite_nullabilities(nullability, ctx.nullability_suffix(argument));
                ctx.with_nullability(argument, n)
            }
            TypeKind::Function(f) => {
                let before = self.current().use_counter;
                let mut type_formals: Vec<EId<TypeParameterElement>> =
                    ctx.list(f.type_params).to_vec();
                let inner = !type_formals.is_empty();
                if inner {
                    // newInnerEnvironment(): copies the covariantContext.
                    let covariant_context = self.envs.last().unwrap().covariant_context;
                    self.envs.push(Env {
                        map: Vec::new(),
                        covariant_context,
                        use_counter: 0,
                    });
                    type_formals = self.fresh_type_parameters(&type_formals);
                }
                // Invert the variance when translating parameters.
                self.invert_variance();
                let params: Vec<FnParam> = ctx
                    .list(f.params)
                    .iter()
                    .map(|p| FnParam {
                        ty: self.visit(p.ty),
                        ..*p
                    })
                    .collect();
                self.invert_variance();
                let ret = self.visit(f.ret);
                if inner {
                    self.envs.pop();
                }
                let alias = self.map_alias(f.alias);
                if self.current().use_counter == before {
                    return t;
                }
                ctx.function_type(&type_formals, &params, ret, f.nullability, alias)
            }
        }
    }
}
