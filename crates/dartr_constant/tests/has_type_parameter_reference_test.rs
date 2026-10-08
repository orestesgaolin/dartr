// Dart source: pkg/analyzer/test/src/dart/constant/has_type_parameter_reference_test.dart
//
// A port of `HasTypeParameterReferenceTest` (types built by hand in place of
// `parseType`), and checks of `has_invalid_type` (no Dart test exists).

#![allow(non_snake_case)]

mod support;

use dartr_constant::{has_invalid_type, has_type_parameter_reference};
use dartr_element::{
    Ctx, EId, FnParam, FunctionTypeData, Nullability, ParameterKind, TypeId, TypeKind,
    TypeParameterElement,
};
use support::{World, build_world};

struct Types<'w> {
    world: &'w World,
}

impl<'w> Types<'w> {
    fn ctx(&self) -> Ctx<'w> {
        self.world.ctx()
    }

    fn interface(
        &self,
        element: EId<dartr_element::ClassElement>,
        args: &[TypeId],
        n: Nullability,
    ) -> TypeId {
        let ctx = self.ctx();
        let args = ctx.intern_list(args);
        ctx.intern(TypeKind::Interface {
            element: element.upcast(),
            args,
            nullability: n,
            alias: None,
        })
    }

    fn int(&self) -> TypeId {
        self.ctx().tp.int_type()
    }

    fn t(&self, n: Nullability) -> TypeId {
        self.ctx().intern(TypeKind::TypeParameter {
            param: self.world.scope_t,
            nullability: n,
            promoted_bound: None,
            alias: None,
        })
    }

    fn function(
        &self,
        type_params: &[EId<TypeParameterElement>],
        params: &[TypeId],
        ret: TypeId,
    ) -> TypeId {
        let ctx = self.ctx();
        let params: Vec<FnParam> = params
            .iter()
            .map(|&ty| FnParam {
                name: None,
                kind: ParameterKind::Required,
                ty,
                covariant: false,
                element: None,
            })
            .collect();
        ctx.intern(TypeKind::Function(FunctionTypeData {
            type_params: ctx.intern_list(type_params),
            params: ctx.intern_list(&params),
            required_positional: params.len() as u16,
            ret,
            nullability: Nullability::None,
            alias: None,
        }))
    }
}

fn check(world: &World, ty: TypeId, expected: bool) {
    assert_eq!(has_type_parameter_reference(&world.ctx(), ty), expected);
}

#[test]
fn test_dynamic() {
    let world = build_world();
    check(&world, TypeId::DYNAMIC, false);
}

#[test]
fn test_functionType() {
    let world = build_world();
    let t = Types { world: &world };
    // void Function()
    check(&world, t.function(&[], &[], TypeId::VOID), false);
    // T Function()
    check(&world, t.function(&[], &[], t.t(Nullability::None)), true);
    // void Function(T)
    check(
        &world,
        t.function(&[], &[t.t(Nullability::None)], TypeId::VOID),
        true,
    );
    // void Function<S extends T>()
    check(
        &world,
        t.function(&[world.scope_s], &[], TypeId::VOID),
        true,
    );
}

#[test]
fn test_interfaceType() {
    let world = build_world();
    let t = Types { world: &world };
    let tp = &world.tp;
    check(&world, t.int(), false);
    check(
        &world,
        t.interface(tp.int_element(), &[], Nullability::Question),
        false,
    );
    let tt = t.t(Nullability::None);
    check(
        &world,
        t.interface(tp.list_element(), &[tt], Nullability::None),
        true,
    );
    check(
        &world,
        t.interface(tp.map_element(), &[tt, t.int()], Nullability::None),
        true,
    );
    check(
        &world,
        t.interface(tp.map_element(), &[t.int(), tt], Nullability::None),
        true,
    );
}

#[test]
fn test_typeParameter() {
    let world = build_world();
    let t = Types { world: &world };
    check(&world, t.t(Nullability::None), true);
    check(&world, t.t(Nullability::Question), true);
}

#[test]
fn test_void() {
    let world = build_world();
    check(&world, TypeId::VOID, false);
}

#[test]
fn has_invalid_type_finds_nested_invalid_types() {
    let world = build_world();
    let t = Types { world: &world };
    let ctx = world.ctx();
    let tp = &world.tp;
    assert!(has_invalid_type(&ctx, TypeId::INVALID));
    assert!(!has_invalid_type(&ctx, t.int()));
    assert!(has_invalid_type(
        &ctx,
        t.interface(tp.list_element(), &[TypeId::INVALID], Nullability::None)
    ));
    assert!(has_invalid_type(
        &ctx,
        t.function(&[], &[TypeId::INVALID], TypeId::VOID)
    ));
    assert!(has_invalid_type(
        &ctx,
        t.function(&[], &[], TypeId::INVALID)
    ));
    // The bound of a type parameter type is not visited.
    assert!(!has_invalid_type(&ctx, t.t(Nullability::None)));
}
