// Dart source: pkg/analyzer/test/src/dart/element/inheritance_manager3_test.dart

//! Port of `inheritance_manager3_test.dart`. Each test keeps the Dart source
//! of the original test; `support::SourceTest` builds the declarations with
//! the real parser and the `test_support` builder (see `support/mod.rs`).
//! Tests whose source needs inference (implicit types, inherited
//! covariance) build it with the real linker (`SourceTest::linked`).

mod support;

use dartr_element::{
    Ctx, EId, ElemRef, InterfaceElement, Nullability, TypeId, TypeKind, TypeParameterElement,
};
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{GetMemberOptions, Name};
use dartr_typesystem::member;

/// Dart `list.single`.
fn single<T: Copy + std::fmt::Debug>(list: Vec<T>) -> T {
    assert_eq!(list.len(), 1, "single: {list:?}");
    list[0]
}

/// `(type as TypeParameterType).element`.
fn type_parameter_of(ctx: &Ctx<'_>, t: TypeId) -> EId<TypeParameterElement> {
    match *ctx.ty(t) {
        TypeKind::TypeParameter { param, .. } => param,
        _ => panic!("not a type parameter type"),
    }
}

/// `(type as FunctionType).typeParameters`.
fn function_type_parameters(ctx: &Ctx<'_>, t: TypeId) -> Vec<EId<TypeParameterElement>> {
    match *ctx.ty(t) {
        TypeKind::Function(f) => ctx.list(f.type_params).to_vec(),
        _ => panic!("not a function type"),
    }
}

/// Dart: `InheritanceManager3NameTest`.
mod name_test {
    #[allow(unused_imports)]
    use super::support::*;
    #[allow(unused_imports)]
    use super::*;

    /// Dart: `test_equals`.
    #[test]
    fn test_equals() {
        let r = SourceTest::new("");
        let ctx = r.ctx();
        let name = |n: &str| Name::new(&ctx, None, n);
        assert_eq!(name("foo"), name("foo"));
        assert_eq!(name("foo"), name("foo=").for_getter(&ctx));
        assert_eq!(name("foo="), name("foo="));
        assert_eq!(name("foo="), name("foo").for_setter(&ctx));
        assert_eq!(
            name("foo="),
            name("foo")
                .for_setter(&ctx)
                .for_setter(&ctx)
                .for_setter(&ctx)
        );
    }

    /// Dart: `test_forGetter`.
    #[test]
    fn test_for_getter() {
        let r = SourceTest::new("");
        let ctx = r.ctx();
        let name = Name::new(&ctx, None, "foo");
        assert_eq!(name.for_getter(&ctx).text(&ctx), "foo");
        assert_eq!(name, name.for_getter(&ctx));
    }

    /// Dart: `test_forGetter_fromSetter`.
    #[test]
    fn test_for_getter_from_setter() {
        let r = SourceTest::new("");
        let ctx = r.ctx();
        let name = Name::new(&ctx, None, "foo=");
        assert_eq!(name.for_getter(&ctx).text(&ctx), "foo");
    }

    /// Dart: `test_forSetter`.
    #[test]
    fn test_for_setter() {
        let r = SourceTest::new("");
        let ctx = r.ctx();
        let name = Name::new(&ctx, None, "foo=");
        assert_eq!(name.for_setter(&ctx).text(&ctx), "foo=");
        assert_eq!(name, name.for_setter(&ctx));
    }

    /// Dart: `test_forSetter_fromGetter`.
    #[test]
    fn test_for_setter_from_getter() {
        let r = SourceTest::new("");
        let ctx = r.ctx();
        let name = Name::new(&ctx, None, "foo");
        assert_eq!(name.for_setter(&ctx).text(&ctx), "foo=");
    }

    /// Dart: `test_name_getter`.
    #[test]
    fn test_name_getter() {
        let r = SourceTest::new("");
        let ctx = r.ctx();
        assert_eq!(Name::new(&ctx, None, "foo").text(&ctx), "foo");
    }

    /// Dart: `test_name_setter`.
    #[test]
    fn test_name_setter() {
        let r = SourceTest::new("");
        let ctx = r.ctx();
        assert_eq!(Name::new(&ctx, None, "foo=").text(&ctx), "foo=");
    }
}

/// Dart: `InheritanceManager3Test`.
mod inheritance_manager3_test {
    #[allow(unused_imports)]
    use super::support::*;
    #[allow(unused_imports)]
    use super::*;

    /// Dart: `test_getInherited_closestSuper`.
    #[test]
    fn test_get_inherited_closest_super() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {
  void foo() {}
}

class X extends B {
  void foo() {}
}
"#,
        );
        r.assert_get_inherited(r#"X"#, r#"foo"#, Some(r#"B.foo: void Function()"#));
    }

    /// Dart: `test_getInherited_interfaces`.
    #[test]
    fn test_get_inherited_interfaces() {
        let r = SourceTest::new(
            r#"abstract class I {
  void foo();
}

abstract class J {
  void foo();
}

class X implements I, J {
  void foo() {}
}
"#,
        );
        r.assert_get_inherited(r#"X"#, r#"foo"#, Some(r#"I.foo: void Function()"#));
    }

    /// Dart: `test_getInherited_mixin`.
    #[test]
    fn test_get_inherited_mixin() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

mixin M {
  void foo() {}
}

class X extends A with M {
  void foo() {}
}
"#,
        );
        r.assert_get_inherited(r#"X"#, r#"foo"#, Some(r#"M.foo: void Function()"#));
    }

    /// Dart: `test_getInherited_preferImplemented`.
    #[test]
    fn test_get_inherited_prefer_implemented() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class I {
  void foo() {}
}

class X extends A implements I {
  void foo() {}
}
"#,
        );
        r.assert_get_inherited(r#"X"#, r#"foo"#, Some(r#"A.foo: void Function()"#));
    }

    /// Dart: `test_getInheritedConcreteMap_accessor_extends`.
    #[test]
    fn test_get_inherited_concrete_map_accessor_extends() {
        let r = SourceTest::new(
            r#"class A {
  int get foo => 0;
}

class B extends A {}
"#,
        );
        r.assert_inherited_concrete_map(
            r#"B"#,
            r#"A.foo: int Function()
"#,
        );
    }

    /// Dart: `test_getInheritedConcreteMap_accessor_implements`.
    #[test]
    fn test_get_inherited_concrete_map_accessor_implements() {
        let r = SourceTest::new(
            r#"class A {
  int get foo => 0;
}

abstract class B implements A {}
"#,
        );
        r.assert_inherited_concrete_map(r#"B"#, r#""#);
    }

    /// Dart: `test_getInheritedConcreteMap_accessor_with`.
    #[test]
    fn test_get_inherited_concrete_map_accessor_with() {
        let r = SourceTest::new(
            r#"mixin A {
  int get foo => 0;
}

class B extends Object with A {}
"#,
        );
        r.assert_inherited_concrete_map(
            r#"B"#,
            r#"A.foo: int Function()
"#,
        );
    }

    /// Dart: `test_getInheritedConcreteMap_ignoresDeclarationInClass`.
    #[test]
    fn test_get_inherited_concrete_map_ignores_declaration_in_class() {
        let r = SourceTest::new(
            r#"class A {}

class B extends A {
  void f() {}
}
"#,
        );
        r.assert_inherited_concrete_map(r#"B"#, r#""#);
    }

    /// Dart: `test_getInheritedConcreteMap_implicitExtends`.
    #[test]
    fn test_get_inherited_concrete_map_implicit_extends() {
        let r = SourceTest::new(
            r#"class A {}
"#,
        );
        r.assert_inherited_concrete_map(r#"A"#, r#""#);
    }

    /// Dart: `test_getInheritedConcreteMap_method_extends`.
    #[test]
    fn test_get_inherited_concrete_map_method_extends() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {}
"#,
        );
        r.assert_inherited_concrete_map(
            r#"B"#,
            r#"A.foo: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedConcreteMap_method_extends_abstract`.
    #[test]
    fn test_get_inherited_concrete_map_method_extends_abstract() {
        let r = SourceTest::new(
            r#"abstract class A {
  void foo();
}

class B extends A {}
"#,
        );
        r.assert_inherited_concrete_map(r#"B"#, r#""#);
    }

    /// Dart: `test_getInheritedConcreteMap_method_extends_invalidForImplements`.
    #[test]
    fn test_get_inherited_concrete_map_method_extends_invalid_for_implements() {
        let r = SourceTest::new(
            r#"abstract class I {
  void foo(int x, {int y});
  void bar(String s);
}

class A {
  void foo(int x) {}
}

class C extends A implements I {}
"#,
        );
        r.assert_inherited_concrete_map(
            r#"C"#,
            r#"A.foo: void Function(int)
"#,
        );
    }

    /// Dart: `test_getInheritedConcreteMap_method_implements`.
    #[test]
    fn test_get_inherited_concrete_map_method_implements() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

abstract class B implements A {}
"#,
        );
        r.assert_inherited_concrete_map(r#"B"#, r#""#);
    }

    /// Dart: `test_getInheritedConcreteMap_method_with`.
    #[test]
    fn test_get_inherited_concrete_map_method_with() {
        let r = SourceTest::new(
            r#"mixin A {
  void foo() {}
}

class B extends Object with A {}
"#,
        );
        r.assert_inherited_concrete_map(
            r#"B"#,
            r#"A.foo: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedConcreteMap_method_with2`.
    #[test]
    fn test_get_inherited_concrete_map_method_with2() {
        let r = SourceTest::new(
            r#"mixin A {
  void foo() {}
}

mixin B {
  void bar() {}
}

class C extends Object with A, B {}
"#,
        );
        r.assert_inherited_concrete_map(
            r#"C"#,
            r#"A.foo: void Function()
B.bar: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedConcreteMap_providesInheritedMemberEvenIfShadowedInClass`.
    #[test]
    fn test_get_inherited_concrete_map_provides_inherited_member_even_if_shadowed_in_class() {
        let r = SourceTest::new(
            r#"class A {
  void f() {}
}

class B extends A {
  void f() {}
}
"#,
        );
        r.assert_inherited_concrete_map(
            r#"B"#,
            r#"A.f: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_accessor_extends`.
    #[test]
    fn test_get_inherited_map_accessor_extends() {
        let r = SourceTest::new(
            r#"class A {
  int get foo => 0;
}

class B extends A {}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.foo: int Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_accessor_implements`.
    #[test]
    fn test_get_inherited_map_accessor_implements() {
        let r = SourceTest::new(
            r#"class A {
  int get foo => 0;
}

abstract class B implements A {}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.foo: int Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_accessor_with`.
    #[test]
    fn test_get_inherited_map_accessor_with() {
        let r = SourceTest::new(
            r#"mixin A {
  int get foo => 0;
}

class B extends Object with A {}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.foo: int Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_closestSuper`.
    #[test]
    fn test_get_inherited_map_closest_super() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {
  void foo() {}
}

class X extends B {}
"#,
        );
        r.assert_inherited_map(
            r#"X"#,
            r#"B.foo: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_field_extends`.
    #[test]
    fn test_get_inherited_map_field_extends() {
        let r = SourceTest::new(
            r#"class A {
  int foo;
}

class B extends A {}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.foo: int Function()
A.foo=: void Function(int)
"#,
        );
    }

    /// Dart: `test_getInheritedMap_field_implements`.
    #[test]
    fn test_get_inherited_map_field_implements() {
        let r = SourceTest::new(
            r#"class A {
  int foo;
}

abstract class B implements A {}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.foo: int Function()
A.foo=: void Function(int)
"#,
        );
    }

    /// Dart: `test_getInheritedMap_field_with`.
    #[test]
    fn test_get_inherited_map_field_with() {
        let r = SourceTest::new(
            r#"mixin A {
  int foo;
}

class B extends Object with A {}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.foo: int Function()
A.foo=: void Function(int)
"#,
        );
    }

    /// Dart: `test_getInheritedMap_ignoresDeclarationInClass`.
    #[test]
    fn test_get_inherited_map_ignores_declaration_in_class() {
        let r = SourceTest::new(
            r#"class A {}

class B extends A {
  void f() {}
}
"#,
        );
        r.assert_inherited_map(r#"B"#, r#""#);
    }

    /// Dart: `test_getInheritedMap_implicitExtendsObject`.
    #[test]
    fn test_get_inherited_map_implicit_extends_object() {
        let r = SourceTest::new(
            r#"class A {}
"#,
        );
        r.assert_inherited_map(r#"A"#, r#""#);
    }

    /// Dart: `test_getInheritedMap_method_extends`.
    #[test]
    fn test_get_inherited_map_method_extends() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.foo: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_method_implements`.
    #[test]
    fn test_get_inherited_map_method_implements() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

abstract class B implements A {}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.foo: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_method_with`.
    #[test]
    fn test_get_inherited_map_method_with() {
        let r = SourceTest::new(
            r#"mixin A {
  void foo() {}
}

class B extends Object with A {}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.foo: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_preferImplemented`.
    #[test]
    fn test_get_inherited_map_prefer_implemented() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class I {
  void foo() {}
}

class X extends A implements I {
  void foo() {}
}
"#,
        );
        r.assert_inherited_map(
            r#"X"#,
            r#"A.foo: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_providesInheritedMemberEvenIfShadowedInClass`.
    #[test]
    fn test_get_inherited_map_provides_inherited_member_even_if_shadowed_in_class() {
        let r = SourceTest::new(
            r#"class A {
  void f() {}
}

class B extends A {
  void f() {}
}
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"A.f: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_topMerge_method`.
    #[test]
    fn test_get_inherited_map_top_merge_method() {
        let r = SourceTest::with_files(&[
            (
                r#"package:test/a.dart"#,
                r#"class A {
  void foo({int a}) {}
}
"#,
            ),
            (
                r#"package:test/test.dart"#,
                r#"import 'a.dart';

class B {
  void foo({required int? a}) {}
}

class C implements A, B {
  void foo({int? a}) {}
}
"#,
            ),
        ]);
        r.assert_inherited_map(r#"C"#, r#""#);
    }

    /// Dart: `test_getInheritedMap_union_conflict`.
    #[test]
    fn test_get_inherited_map_union_conflict() {
        let r = SourceTest::new(
            r#"abstract class I {
  int foo();
  void bar();
}

abstract class J {
  double foo();
  void bar();
}

abstract class A implements I, J {}
"#,
        );
        r.assert_inherited_map(
            r#"A"#,
            r#"I.bar: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_union_differentNames`.
    #[test]
    fn test_get_inherited_map_union_different_names() {
        let r = SourceTest::new(
            r#"abstract class I {
  int foo();
}

abstract class J {
  double bar();
}

abstract class A implements I, J {}
"#,
        );
        r.assert_inherited_map(
            r#"A"#,
            r#"I.foo: int Function()
J.bar: double Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_union_multipleSubtypes_2_getters`.
    #[test]
    fn test_get_inherited_map_union_multiple_subtypes_2_getters() {
        let r = SourceTest::new(
            r#"abstract class I {
  int get foo;
}

abstract class J {
  int get foo;
}

abstract class A implements I, J {}
"#,
        );
        r.assert_inherited_map(
            r#"A"#,
            r#"I.foo: int Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_union_multipleSubtypes_2_methods`.
    #[test]
    fn test_get_inherited_map_union_multiple_subtypes_2_methods() {
        let r = SourceTest::new(
            r#"abstract class I {
  void foo();
}

abstract class J {
  void foo();
}

abstract class A implements I, J {}
"#,
        );
        r.assert_inherited_map(
            r#"A"#,
            r#"I.foo: void Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_union_multipleSubtypes_2_setters`.
    #[test]
    fn test_get_inherited_map_union_multiple_subtypes_2_setters() {
        let r = SourceTest::new(
            r#"abstract class I {
  void set foo(num _);
}

abstract class J {
  void set foo(int _);
}

abstract class A implements I, J {}
abstract class B implements J, I {}
"#,
        );
        r.assert_inherited_map(
            r#"A"#,
            r#"I.foo=: void Function(num)
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"I.foo=: void Function(num)
"#,
        );
    }

    /// Dart: `test_getInheritedMap_union_multipleSubtypes_3_getters`.
    #[test]
    fn test_get_inherited_map_union_multiple_subtypes_3_getters() {
        let r = SourceTest::new(
            r#"class A {}
class B extends A {}
class C extends B {}

abstract class I1 {
  A get foo;
}

abstract class I2 {
  B get foo;
}

abstract class I3 {
  C get foo;
}

abstract class D implements I1, I2, I3 {}
abstract class E implements I3, I2, I1 {}
"#,
        );
        r.assert_inherited_map(
            r#"D"#,
            r#"I3.foo: C Function()
"#,
        );
        r.assert_inherited_map(
            r#"E"#,
            r#"I3.foo: C Function()
"#,
        );
    }

    /// Dart: `test_getInheritedMap_union_multipleSubtypes_3_methods`.
    #[test]
    fn test_get_inherited_map_union_multiple_subtypes_3_methods() {
        let r = SourceTest::new(
            r#"class A {}
class B extends A {}
class C extends B {}

abstract class I1 {
  void foo(A _);
}

abstract class I2 {
  void foo(B _);
}

abstract class I3 {
  void foo(C _);
}

abstract class D implements I1, I2, I3 {}
abstract class E implements I3, I2, I1 {}
"#,
        );
        r.assert_inherited_map(
            r#"D"#,
            r#"I1.foo: void Function(A)
"#,
        );
    }

    /// Dart: `test_getInheritedMap_union_multipleSubtypes_3_setters`.
    #[test]
    fn test_get_inherited_map_union_multiple_subtypes_3_setters() {
        let r = SourceTest::new(
            r#"class A {}
class B extends A {}
class C extends B {}

abstract class I1 {
  set foo(A _);
}

abstract class I2 {
  set foo(B _);
}

abstract class I3 {
  set foo(C _);
}

abstract class D implements I1, I2, I3 {}
abstract class E implements I3, I2, I1 {}
"#,
        );
        r.assert_inherited_map(
            r#"D"#,
            r#"I1.foo=: void Function(A)
"#,
        );
        r.assert_inherited_map(
            r#"E"#,
            r#"I1.foo=: void Function(A)
"#,
        );
    }

    /// Dart: `test_getInheritedMap_union_oneSubtype_2_methods`.
    #[test]
    fn test_get_inherited_map_union_one_subtype_2_methods() {
        let r = SourceTest::new(
            r#"abstract class I1 {
  int foo();
}

abstract class I2 {
  int foo([int _]);
}

abstract class A implements I1, I2 {}
abstract class B implements I2, I1 {}
"#,
        );
        r.assert_inherited_map(
            r#"A"#,
            r#"I2.foo: int Function([int])
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"I2.foo: int Function([int])
"#,
        );
    }

    /// Dart: `test_getInheritedMap_union_oneSubtype_3_methods`.
    #[test]
    fn test_get_inherited_map_union_one_subtype_3_methods() {
        let r = SourceTest::new(
            r#"abstract class I1 {
  int foo();
}

abstract class I2 {
  int foo([int _]);
}

abstract class I3 {
  int foo([int _, int __]);
}

abstract class A implements I1, I2, I3 {}
abstract class B implements I3, I2, I1 {}
"#,
        );
        r.assert_inherited_map(
            r#"A"#,
            r#"I3.foo: int Function([int, int])
"#,
        );
        r.assert_inherited_map(
            r#"B"#,
            r#"I3.foo: int Function([int, int])
"#,
        );
    }

    /// Dart: `test_getMember`.
    #[test]
    fn test_get_member() {
        let r = SourceTest::new(
            r#"abstract class I1 {
  void f(int i);
}

abstract class I2 {
  void f(Object o);
}

abstract class C implements I1, I2 {}
"#,
        );
        r.assert_get_member(
            r#"C"#,
            r#"f"#,
            Some(r#"I2.f: void Function(Object)"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_concrete`.
    #[test]
    fn test_get_member_concrete() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}
"#,
        );
        r.assert_get_member(
            r#"A"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            true,
            false,
        );
    }

    /// Dart: `test_getMember_concrete_abstract`.
    #[test]
    fn test_get_member_concrete_abstract() {
        let r = SourceTest::new(
            r#"abstract class A {
  void foo();
}
"#,
        );
        r.assert_get_member(r#"A"#, r#"foo"#, None, true, false);
    }

    /// Dart: `test_getMember_concrete_fromMixedClass`.
    #[test]
    fn test_get_member_concrete_from_mixed_class() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class X with A {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            true,
            false,
        );
    }

    /// Dart: `test_getMember_concrete_fromMixedClass2`.
    #[test]
    fn test_get_member_concrete_from_mixed_class2() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B = Object with A;

class X with B {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            true,
            false,
        );
    }

    /// Dart: `test_getMember_concrete_fromMixedClass_skipObject`.
    #[test]
    fn test_get_member_concrete_from_mixed_class_skip_object() {
        let r = SourceTest::new(
            r#"class A {
  String toString() => 'A';
}

class B {}

class X extends A with B {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"toString"#,
            Some(r#"A.toString: String Function()"#),
            true,
            false,
        );
    }

    /// Dart: `test_getMember_concrete_fromMixin`.
    #[test]
    fn test_get_member_concrete_from_mixin() {
        let r = SourceTest::new(
            r#"mixin M {
  void foo() {}
}

class X with M {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"M.foo: void Function()"#),
            true,
            false,
        );
    }

    /// Dart: `test_getMember_concrete_fromSuper`.
    #[test]
    fn test_get_member_concrete_from_super() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {}

abstract class C extends B {}
"#,
        );
        r.assert_get_member(
            r#"B"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            true,
            false,
        );
        r.assert_get_member(
            r#"C"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            true,
            false,
        );
    }

    /// Dart: `test_getMember_concrete_missing`.
    #[test]
    fn test_get_member_concrete_missing() {
        let r = SourceTest::new(
            r#"abstract class A {}
"#,
        );
        r.assert_get_member(r#"A"#, r#"foo"#, None, true, false);
    }

    /// Dart: `test_getMember_concrete_noSuchMethod`.
    #[test]
    fn test_get_member_concrete_no_such_method() {
        let r = SourceTest::linked(
            r#"class A {
  void foo() {}
}

class B implements A {
  noSuchMethod(_) {}
}

abstract class C extends B {}
"#,
        );
        r.assert_get_member(
            r#"B"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            true,
            false,
        );
        r.assert_get_member(
            r#"C"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            true,
            false,
        );
    }

    /// Dart: `test_getMember_concrete_noSuchMethod_mixin`.
    #[test]
    fn test_get_member_concrete_no_such_method_mixin() {
        let r = SourceTest::linked(
            r#"class A {
  void foo();

  noSuchMethod(_) {}
}

abstract class B extends Object with A {}
"#,
        );
        r.assert_get_member(r#"B"#, r#"foo"#, None, true, false);
    }

    /// Dart: `test_getMember_concrete_noSuchMethod_moreSpecificSignature`.
    #[test]
    fn test_get_member_concrete_no_such_method_more_specific_signature() {
        let r = SourceTest::linked(
            r#"class A {
  void foo() {}
}

class B implements A {
  noSuchMethod(_) {}
}

class C extends B {
  void foo([int a]);
}
"#,
        );
        r.assert_get_member(
            r#"C"#,
            r#"foo"#,
            Some(r#"C.foo: void Function([int])"#),
            true,
            false,
        );
    }

    /// Dart: `test_getMember_fromGenericClass_method_returnType`.
    #[test]
    fn test_get_member_from_generic_class_method_return_type() {
        let r = SourceTest::new(
            r#"
abstract class B<E> {
  T foo<T>();
}
"#,
        );
        let ctx = r.ctx();
        let b = r.class_or_mixin("B");
        let foo = r.manager().get_member(b, r.name("foo")).unwrap();
        let t = single(member::type_parameters(&ctx, foo));
        let return_type = member::return_type(&ctx, foo);
        // Check that the return type uses the same `T` as `<T>`.
        assert_eq!(type_parameter_of(&ctx, return_type), t);
    }

    /// Dart: `test_getMember_fromGenericSuper_method_bound`.
    #[test]
    fn test_get_member_from_generic_super_method_bound() {
        let r = SourceTest::new(
            r#"
abstract class Foo<TF> {}
class Bar implements Foo<Bar> {}
abstract class A<XA> {
  T foo<T extends Foo<T>>() => throw '';
}
abstract class B<XB> extends A<XB> {}
"#,
        );
        let ctx = r.ctx();
        let check_textends_foo_t = |t: EId<TypeParameterElement>| {
            let bound = ctx.type_parameter_bound(t).unwrap();
            let other_t = type_parameter_of(&ctx, single(ctx.type_arguments(bound).to_vec()));
            // Dart: same(t)
            assert_eq!(other_t, t);
        };
        let x = r.type_parameter("XB");
        let type_x = ctx.type_parameter_type(x, Nullability::None);
        let class = r.class_or_mixin("B");
        let type_class = ctx.interface_type(class, &[type_x], Nullability::None);
        let foo = r
            .manager()
            .get_member3(type_class, r.name("foo"), GetMemberOptions::default())
            .unwrap();
        let foo2 = r.manager().get_member(class, r.name("foo")).unwrap();
        check_textends_foo_t(single(function_type_parameters(
            &ctx,
            member::type_(&ctx, foo),
        )));
        check_textends_foo_t(single(function_type_parameters(
            &ctx,
            member::type_(&ctx, foo2),
        )));
        check_textends_foo_t(single(member::type_parameters(&ctx, foo2)));
        check_textends_foo_t(single(member::type_parameters(&ctx, foo)));
    }

    /// Dart: `test_getMember_fromGenericSuper_method_bound2`.
    #[test]
    fn test_get_member_from_generic_super_method_bound2() {
        let r = SourceTest::new(
            r#"
abstract class Foo<T> {}
class Bar implements Foo<Bar> {}
abstract class A<X> {
  T foo<T extends Foo<T>>() => throw '';
}
abstract class B<X> extends A<X> {}
typedef C<V> = B<List<V>>;
abstract class D<XD> extends C<XD> {}
"#,
        );
        let ctx = r.ctx();
        let check_textends_foo_t = |t: EId<TypeParameterElement>| {
            let bound = ctx.type_parameter_bound(t).unwrap();
            let other_t = type_parameter_of(&ctx, single(ctx.type_arguments(bound).to_vec()));
            // Dart: same(t)
            assert_eq!(other_t, t);
        };
        let x = r.type_parameter("XD");
        let type_x = ctx.type_parameter_type(x, Nullability::None);
        let class = r.class_or_mixin("D");
        let type_class = ctx.interface_type(class, &[type_x], Nullability::None);
        let foo = r
            .manager()
            .get_member3(type_class, r.name("foo"), GetMemberOptions::default())
            .unwrap();
        let foo2 = r.manager().get_member(class, r.name("foo")).unwrap();
        check_textends_foo_t(single(function_type_parameters(
            &ctx,
            member::type_(&ctx, foo),
        )));
        check_textends_foo_t(single(function_type_parameters(
            &ctx,
            member::type_(&ctx, foo2),
        )));
        check_textends_foo_t(single(member::type_parameters(&ctx, foo2)));
        check_textends_foo_t(single(member::type_parameters(&ctx, foo)));
    }

    /// Dart: `test_getMember_fromGenericSuper_method_returnType`.
    #[test]
    fn test_get_member_from_generic_super_method_return_type() {
        let r = SourceTest::new(
            r#"
abstract class A<E> {
  T foo<T>();
}

abstract class B<E> extends A<E> {}
"#,
        );
        let ctx = r.ctx();
        let b = r.class_or_mixin("B");
        let foo = r.manager().get_member(b, r.name("foo")).unwrap();
        let t = single(member::type_parameters(&ctx, foo));
        let return_type = member::return_type(&ctx, foo);
        // Check that the return type uses the same `T` as `<T>`.
        assert_eq!(type_parameter_of(&ctx, return_type), t);
    }

    /// Dart: `test_getMember_fromNotGenericSuper_method_returnType`.
    #[test]
    fn test_get_member_from_not_generic_super_method_return_type() {
        let r = SourceTest::new(
            r#"
abstract class A {
  T foo<T>();
}

abstract class B extends A {}
"#,
        );
        let ctx = r.ctx();
        let b = r.class_or_mixin("B");
        let foo = r.manager().get_member(b, r.name("foo")).unwrap();
        let t = single(member::type_parameters(&ctx, foo));
        let return_type = member::return_type(&ctx, foo);
        // Check that the return type uses the same `T` as `<T>`.
        assert_eq!(type_parameter_of(&ctx, return_type), t);
    }

    /// Dart: `test_getMember_method_covariantAfterSubstitutedParameter_merged`.
    #[test]
    fn test_get_member_method_covariant_after_substituted_parameter_merged() {
        let r = SourceTest::new(
            r#"
class A<T> {
  void foo<U>(covariant Object a, U b, int c) {}
}

class B extends A<int> implements C {}

class C {
  void foo<U>(Object a, U b, covariant Object c) {}
}
"#,
        );
        let ctx = r.ctx();
        let member = r
            .manager()
            .get_member_with(
                r.class_or_mixin("B"),
                r.name("foo"),
                GetMemberOptions {
                    concrete: true,
                    ..Default::default()
                },
            )
            .unwrap();
        let parameters = member::formal_parameters(&ctx, member);
        assert!(member::is_covariant(&ctx, parameters[0]));
        assert!(!member::is_covariant(&ctx, parameters[1]));
        assert!(member::is_covariant(&ctx, parameters[2]));
    }

    /// Dart: `test_getMember_method_covariantByDeclaration_inherited`.
    #[test]
    fn test_get_member_method_covariant_by_declaration_inherited() {
        let r = SourceTest::linked(
            r#"
abstract class A {
  void foo(covariant num a);
}

abstract class B extends A {
  void foo(int a);
}
"#,
        );
        let ctx = r.ctx();
        let member = r
            .manager()
            .get_member(r.class_or_mixin("B"), r.name("foo"))
            .unwrap();
        // TODO(scheglov): It would be nice to use `_assertGetMember`.
        // But we need a way to check covariance.
        // Maybe check the element display string, not the type.
        assert!(member::is_covariant(
            &ctx,
            member::formal_parameters(&ctx, member)[0]
        ));
    }

    /// Dart: `test_getMember_method_covariantByDeclaration_merged`.
    #[ignore = "FailingTest in Dart: The baseElement and the element associated with the declaration  are not the same"]
    #[test]
    fn test_get_member_method_covariant_by_declaration_merged() {
        let r = SourceTest::new(
            r#"
class A {
  void foo(covariant num a) {}
}

class B {
  void foo(int a) {}
}

class C extends B implements A {}
"#,
        );
        let ctx = r.ctx();
        let member = r
            .manager()
            .get_member_with(
                r.class_or_mixin("C"),
                r.name("foo"),
                GetMemberOptions {
                    concrete: true,
                    ..Default::default()
                },
            )
            .unwrap();
        // TODO(scheglov): It would be nice to use `_assertGetMember`.
        // Dart: same(result.findElement.method('foo', of: 'B'))
        assert_eq!(
            ElemRef::Base(member::base_element(&ctx, member)),
            r.method("B", "foo")
        );
        assert!(member::is_covariant(
            &ctx,
            member::formal_parameters(&ctx, member)[0]
        ));
    }

    /// Dart: `test_getMember_mixin_notMerge_replace`.
    #[test]
    fn test_get_member_mixin_not_merge_replace() {
        let r = SourceTest::new(
            r#"class A<T> {
  T foo() => throw 0;
}

mixin M<T> {
  T foo() => throw 1;
}

class X extends A<dynamic> with M<Object?> {}
class Y extends A<Object?> with M<dynamic> {}
"#,
        );
        r.assert_get_member2(r#"X"#, r#"foo"#, Some(r#"M.foo: Object? Function()"#));
        r.assert_get_member2(r#"Y"#, r#"foo"#, Some(r#"M.foo: dynamic Function()"#));
    }

    /// Dart: `test_getMember_optIn_inheritsOptIn`.
    #[test]
    fn test_get_member_opt_in_inherits_opt_in() {
        let r = SourceTest::with_files(&[
            (
                r#"package:test/a.dart"#,
                r#"class A {
  int foo(int a, int? b) => 0;
}
"#,
            ),
            (
                r#"package:test/test.dart"#,
                r#"import 'a.dart';
class B extends A {
  int? bar(int a) => 0;
}
"#,
            ),
        ]);
        r.assert_get_member(
            r#"B"#,
            r#"foo"#,
            Some(r#"A.foo: int Function(int, int?)"#),
            false,
            false,
        );
        r.assert_get_member(
            r#"B"#,
            r#"bar"#,
            Some(r#"B.bar: int? Function(int)"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_optIn_topMerge_getter_existing`.
    #[test]
    fn test_get_member_opt_in_top_merge_getter_existing() {
        let r = SourceTest::new(
            r#"class A {
  dynamic get foo => 0;
}

class B {
  Object? get foo => 0;
}

class X extends A implements B {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"B.foo: Object? Function()"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_optIn_topMerge_getter_synthetic`.
    #[test]
    fn test_get_member_opt_in_top_merge_getter_synthetic() {
        let r = SourceTest::new(
            r#"abstract class A {
  Future<void> get foo;
}

abstract class B {
  Future<dynamic> get foo;
}

abstract class X extends A implements B {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"X.foo: Future<Object?> Function()"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_optIn_topMerge_method_existing`.
    #[test]
    fn test_get_member_opt_in_top_merge_method_existing() {
        let r = SourceTest::new(
            r#"class A {
  dynamic foo() {}
}

class B {
  Object? foo() {}
}

class X extends A implements B {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"B.foo: Object? Function()"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_optIn_topMerge_method_synthetic`.
    #[test]
    fn test_get_member_opt_in_top_merge_method_synthetic() {
        let r = SourceTest::new(
            r#"class A {
  Object? foo(dynamic x) {}
}

class B {
  dynamic foo(Object? x) {}
}

class X extends A implements B {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"X.foo: Object? Function(Object?)"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_optIn_topMerge_setter_existing`.
    #[test]
    fn test_get_member_opt_in_top_merge_setter_existing() {
        let r = SourceTest::new(
            r#"class A {
  set foo(dynamic _) {}
}

class B {
  set foo(Object? _) {}
}

class X extends A implements B {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo="#,
            Some(r#"B.foo=: void Function(Object?)"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_optIn_topMerge_setter_synthetic`.
    #[test]
    fn test_get_member_opt_in_top_merge_setter_synthetic() {
        let r = SourceTest::new(
            r#"abstract class A {
  set foo(Future<void> _);
}

abstract class B {
  set foo(Future<dynamic> _);
}

abstract class X extends A implements B {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo="#,
            Some(r#"X.foo=: void Function(Future<Object?>)"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_preferLatest_mixin`.
    #[test]
    fn test_get_member_prefer_latest_mixin() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

mixin M1 {
  void foo() {}
}

mixin M2 {
  void foo() {}
}

abstract class I {
  void foo();
}

class X extends A with M1, M2 implements I {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"M2.foo: void Function()"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_preferLatest_superclass`.
    #[test]
    fn test_get_member_prefer_latest_superclass() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {
  void foo() {}
}

abstract class I {
  void foo();
}

class X extends B implements I {}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"B.foo: void Function()"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_preferLatest_this`.
    #[test]
    fn test_get_member_prefer_latest_this() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

abstract class I {
  void foo();
}

class X extends A implements I {
  void foo() {}
}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"X.foo: void Function()"#),
            false,
            false,
        );
    }

    /// Dart: `test_getMember_setter_covariantByDeclaration_inherited`.
    #[test]
    fn test_get_member_setter_covariant_by_declaration_inherited() {
        let r = SourceTest::linked(
            r#"
abstract class A {
  set foo(covariant num a);
}

abstract class B extends A {
  set foo(int a);
}
"#,
        );
        let ctx = r.ctx();
        let member = r
            .manager()
            .get_member(r.class_or_mixin("B"), r.name("foo="))
            .unwrap();
        // TODO(scheglov): It would be nice to use `_assertGetMember`.
        // But we need a way to check covariance.
        // Maybe check the element display string, not the type.
        assert!(member::is_covariant(
            &ctx,
            member::formal_parameters(&ctx, member)[0]
        ));
    }

    /// Dart: `test_getMember_setter_covariantByDeclaration_merged`.
    #[ignore = "FailingTest in Dart: The baseElement and the element associated with the declaration  are not the same"]
    #[test]
    fn test_get_member_setter_covariant_by_declaration_merged() {
        let r = SourceTest::new(
            r#"
class A {
  set foo(covariant num a) {}
}

class B {
  set foo(int a) {}
}

class C extends B implements A {}
"#,
        );
        let ctx = r.ctx();
        let member = r
            .manager()
            .get_member_with(
                r.class_or_mixin("C"),
                r.name("foo="),
                GetMemberOptions {
                    concrete: true,
                    ..Default::default()
                },
            )
            .unwrap();
        // TODO(scheglov): It would be nice to use `_assertGetMember`.
        // Dart: same(result.findElement.setter('foo', of: 'B'))
        assert_eq!(
            ElemRef::Base(member::base_element(&ctx, member)),
            r.setter("B", "foo")
        );
        assert!(member::is_covariant(
            &ctx,
            member::formal_parameters(&ctx, member)[0]
        ));
    }

    /// Dart: `test_getMember_super_abstract`.
    #[test]
    fn test_get_member_super_abstract() {
        let r = SourceTest::linked(
            r#"abstract class A {
  void foo();
}

class B extends A {
  noSuchMethod(_) {}
}
"#,
        );
        r.assert_get_member(r#"B"#, r#"foo"#, None, false, true);
    }

    /// Dart: `test_getMember_super_forMixin_interface`.
    #[test]
    fn test_get_member_super_for_mixin_interface() {
        let r = SourceTest::new(
            r#"abstract class A {
  void foo();
}

mixin M implements A {}
"#,
        );
        r.assert_get_member(r#"M"#, r#"foo"#, None, false, true);
    }

    /// Dart: `test_getMember_super_forMixin_superclassConstraint`.
    #[test]
    fn test_get_member_super_for_mixin_superclass_constraint() {
        let r = SourceTest::new(
            r#"abstract class A {
  void foo();
}

mixin M on A {}
"#,
        );
        r.assert_get_member(
            r#"M"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            false,
            true,
        );
    }

    /// Dart: `test_getMember_super_forObject`.
    #[test]
    fn test_get_member_super_for_object() {
        let r = SourceTest::new(
            r#"
class A {}
"#,
        );
        let object = r.ctx().tp.object_element().upcast::<InterfaceElement>();
        let member = r.manager().get_member_with(
            object,
            r.name("hashCode"),
            GetMemberOptions {
                for_super: true,
                ..Default::default()
            },
        );
        assert_eq!(member, None);
    }

    /// Dart: `test_getMember_super_fromMixin`.
    #[test]
    fn test_get_member_super_from_mixin() {
        let r = SourceTest::new(
            r#"mixin M {
  void foo() {}
}

class X extends Object with M {
  void foo() {}
}
"#,
        );
        r.assert_get_member(
            r#"X"#,
            r#"foo"#,
            Some(r#"M.foo: void Function()"#),
            false,
            true,
        );
    }

    /// Dart: `test_getMember_super_fromSuper`.
    #[test]
    fn test_get_member_super_from_super() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {
  void foo() {}
}
"#,
        );
        r.assert_get_member(
            r#"B"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            false,
            true,
        );
    }

    /// Dart: `test_getMember_super_missing`.
    #[test]
    fn test_get_member_super_missing() {
        let r = SourceTest::new(
            r#"class A {}

class B extends A {}
"#,
        );
        r.assert_get_member(r#"B"#, r#"foo"#, None, false, true);
    }

    /// Dart: `test_getMember_super_noSuchMember`.
    #[test]
    fn test_get_member_super_no_such_member() {
        let r = SourceTest::linked(
            r#"class A {
  void foo();
  noSuchMethod(_) {}
}

class B extends A {
  void foo() {}
}
"#,
        );
        r.assert_get_member(
            r#"B"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            false,
            true,
        );
    }

    /// Dart: `test_getOverridden_doesNotShadowIfDirectlyOverriddenByAnotherPath`.
    #[test]
    fn test_get_overridden_does_not_shadow_if_directly_overridden_by_another_path() {
        let r = SourceTest::new(
            r#"class A {
  void m() {}
}
class B extends A {
  void m() {}
}
class C extends B implements A {
  void m() {}
}
"#,
        );
        r.assert_get_overridden4(
            r#"C"#,
            r#"m"#,
            Some(
                r#"A.m: void Function()
B.m: void Function()
"#,
            ),
        );
    }

    /// Dart: `test_getOverridden_shadowsTransitiveOverrides`.
    #[test]
    fn test_get_overridden_shadows_transitive_overrides() {
        let r = SourceTest::new(
            r#"class A {
  void m() {}
}
class B extends A {
  void m() {}
}
class C extends B {
  void m() {}
}
"#,
        );
        r.assert_get_overridden4(
            r#"C"#,
            r#"m"#,
            Some(
                r#"B.m: void Function()
"#,
            ),
        );
    }
}

/// Dart: `InheritanceManager3Test_elements`.
mod elements_test {
    #[allow(unused_imports)]
    use super::support::*;
    #[allow(unused_imports)]
    use super::*;

    /// Dart: `test_interface_candidatesConflict`.
    #[test]
    fn test_interface_candidates_conflict() {
        let r = SourceTest::new(
            r#"mixin A {
  void foo(int _);
}

abstract class B {
  void foo(String _);
}

abstract class C extends Object with A implements B {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: false,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"overridden
  foo
    <testLibrary>::@mixin::A::@method::foo
    <testLibrary>::@class::B::@method::foo
superImplemented
conflicts
  CandidatesConflict
    <testLibrary>::@mixin::A::@method::foo
    <testLibrary>::@class::B::@method::foo
"#,
        );
    }

    /// Dart: `test_interface_candidatesConflict_interfaceInAugmentation`.
    #[ignore = "SkippedTest in Dart (augmentations)"]
    #[test]
    fn test_interface_candidates_conflict_interface_in_augmentation() {
        // MANUAL
        /* Dart:
        //     var a = newFile('$testPackageLibPath/a.dart', r'''
        // part 'b.dart';
        //
        // mixin A {
        //   void foo(int _);
        // }
        //
        // abstract class B {
        //   void foo(String _);
        // }
        //
        // abstract class C extends Object with A {}
        // ''');
        //
        //     newFile('$testPackageLibPath/b.dart', r'''
        // part of 'a.dart';
        //
        // augment abstract class C implements B {}
        // ''');
        //
        //     var library = await buildFileLibrary(a);
        //
        //     var element = library.getClass('C')!;
        //     assertInterfaceText(element, r'''
        // overridden
        //   foo
        //     package:test/a.dart::<fragment>::@mixin::A::@method::foo
        //     package:test/a.dart::<fragment>::@class::B::@method::foo
        // superImplemented
        // conflicts
        //   CandidatesConflict
        //     package:test/a.dart::<fragment>::@mixin::A::@method::foo
        //     package:test/a.dart::<fragment>::@class::B::@method::foo
        // ''');
         */
        todo!("manual port");
    }

    /// Dart: `test_interface_getterMethodConflict`.
    #[test]
    fn test_interface_getter_method_conflict() {
        let r = SourceTest::new(
            r#"abstract class A {
  int get foo;
}

abstract class B {
  int foo();
}

abstract class C implements A, B {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: false,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"overridden
  foo
    <testLibrary>::@class::A::@getter::foo
    <testLibrary>::@class::B::@method::foo
superImplemented
conflicts
  GetterMethodConflict
    getter: <testLibrary>::@class::A::@getter::foo
    method: <testLibrary>::@class::B::@method::foo
"#,
        );
    }

    /// Dart: `test_interface_getterMethodConflict_declares`.
    #[test]
    fn test_interface_getter_method_conflict_declares() {
        let r = SourceTest::new(
            r#"abstract class A {
  int get foo;
}

abstract class B {
  int foo();
}

abstract class C implements A, B {
  int foo() => 0;
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: false,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@class::C::@method::foo
declared
  foo: <testLibrary>::@class::C::@method::foo
implemented
  foo: <testLibrary>::@class::C::@method::foo
overridden
  foo
    <testLibrary>::@class::A::@getter::foo
    <testLibrary>::@class::B::@method::foo
superImplemented
conflicts
  GetterMethodConflict
    getter: <testLibrary>::@class::A::@getter::foo
    method: <testLibrary>::@class::B::@method::foo
"#,
        );
    }
}

/// Dart: `InheritanceManager3Test_ExtensionType`.
mod extension_type_test {
    #[allow(unused_imports)]
    use super::support::*;
    #[allow(unused_imports)]
    use super::*;

    /// Dart: `test_declareGetter`.
    #[test]
    fn test_declare_getter() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  int get foo => 0;
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"A"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::A::@getter::foo
  it: <testLibrary>::@extensionType::A::@getter::it
declared
  foo: <testLibrary>::@extensionType::A::@getter::foo
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareGetter_implementClass_precludeGetter`.
    #[test]
    fn test_declare_getter_implement_class_preclude_getter() {
        let r = SourceTest::new(
            r#"class A {
  int get foo => 0;
}

class B extends A {}

extension type C(B it) implements A {
  int get foo => 0;
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@getter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@getter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@getter::foo
inheritedMap
  foo: <testLibrary>::@class::A::@getter::foo
"#,
        );
    }

    /// Dart: `test_declareGetter_implementClass_precludeMethod`.
    #[test]
    fn test_declare_getter_implement_class_preclude_method() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {}

extension type C(B it) implements A {
  int get foo => 0;
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@getter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@getter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@method::foo
inheritedMap
  foo: <testLibrary>::@class::A::@method::foo
"#,
        );
    }

    /// Dart: `test_declareGetter_implementClass_withSetter`.
    #[test]
    fn test_declare_getter_implement_class_with_setter() {
        let r = SourceTest::linked(
            r#"class A {
  set foo(_) {}
}

class B extends A {}

extension type C(B it) implements A {
  int get foo => 0;
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@getter::foo
  foo=: <testLibrary>::@class::A::@setter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@getter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo=
    <testLibrary>::@class::A::@setter::foo
inheritedMap
  foo=: <testLibrary>::@class::A::@setter::foo
"#,
        );
    }

    /// Dart: `test_declareGetter_static`.
    #[test]
    fn test_declare_getter_static() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  static int get foo => 0;
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"A"#),
            config,
            r#"map
  it: <testLibrary>::@extensionType::A::@getter::it
declared
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareMethod`.
    #[test]
    fn test_declare_method() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"A"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::A::@method::foo
  it: <testLibrary>::@extensionType::A::@getter::it
declared
  foo: <testLibrary>::@extensionType::A::@method::foo
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareMethod_implementClass_implementExtensionType_wouldConflict`.
    #[test]
    fn test_declare_method_implement_class_implement_extension_type_would_conflict() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

extension type B(A it) {
  void foo() {}
}

extension type C(A it) implements A, B {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::B::@method::foo
    <testLibrary>::@class::A::@method::foo
  it
    <testLibrary>::@extensionType::B::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
"#,
        );
    }

    /// Dart: `test_declareMethod_implementClass_method2_wouldConflict`.
    #[test]
    fn test_declare_method_implement_class_method2_would_conflict() {
        let r = SourceTest::new(
            r#"class A {
  int foo() => 0;
}

class B {
  String foo() => '0';
}

extension type C(Object it) implements A, B {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@method::foo
    <testLibrary>::@class::B::@method::foo
"#,
        );
    }

    /// Dart: `test_declareMethod_implementClass_noPreclude`.
    #[test]
    fn test_declare_method_implement_class_no_preclude() {
        let r = SourceTest::new(
            r#"class A {}

class B extends A {
  void foo() {}
}

extension type C(B it) implements A {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
"#,
        );
    }

    /// Dart: `test_declareMethod_implementClass_precludeGetter`.
    #[test]
    fn test_declare_method_implement_class_preclude_getter() {
        let r = SourceTest::new(
            r#"class A {
  int get foo => 0;
}

class B extends A {
  void bar() {}
}

extension type C(B it) implements A {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@getter::foo
inheritedMap
  foo: <testLibrary>::@class::A::@getter::foo
"#,
        );
    }

    /// Dart: `test_declareMethod_implementClass_precludeMethod`.
    #[test]
    fn test_declare_method_implement_class_preclude_method() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {
  void bar() {}
}

extension type C(B it) implements A {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@method::foo
inheritedMap
  foo: <testLibrary>::@class::A::@method::foo
"#,
        );
    }

    /// Dart: `test_declareMethod_implementClass_precludeSetter`.
    #[test]
    fn test_declare_method_implement_class_preclude_setter() {
        let r = SourceTest::linked(
            r#"class A {
  set foo(_) {}
}

class B extends A {}

extension type C(B it) implements A {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo=
    <testLibrary>::@class::A::@setter::foo
inheritedMap
  foo=: <testLibrary>::@class::A::@setter::foo
"#,
        );
    }

    /// Dart: `test_declareMethod_implementExtensionType_method2_wouldConflict`.
    #[test]
    fn test_declare_method_implement_extension_type_method2_would_conflict() {
        let r = SourceTest::new(
            r#"extension type A1(int it) {
  void foo() {}
}

extension type A2(int it) {
  void foo() {}
}

extension type B(int it) implements A1, A2 {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::A1::@method::foo
    <testLibrary>::@extensionType::A2::@method::foo
  it
    <testLibrary>::@extensionType::A1::@getter::it
    <testLibrary>::@extensionType::A2::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::A1::@method::foo
  it: <testLibrary>::@extensionType::A1::@getter::it
"#,
        );
    }

    /// Dart: `test_declareMethod_implementExtensionType_precludeGetter`.
    #[test]
    fn test_declare_method_implement_extension_type_preclude_getter() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  int get foo => 0;
}

extension type B(int it) implements A {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::A::@getter::foo
  it
    <testLibrary>::@extensionType::A::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::A::@getter::foo
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareMethod_implementExtensionType_precludeMethod`.
    #[test]
    fn test_declare_method_implement_extension_type_preclude_method() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  void foo() {}
}

extension type B(int it) implements A {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::A::@method::foo
  it
    <testLibrary>::@extensionType::A::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::A::@method::foo
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareMethod_implementExtensionType_precludeSetter`.
    #[test]
    fn test_declare_method_implement_extension_type_preclude_setter() {
        let r = SourceTest::linked(
            r#"extension type A(int it) {
  set foo(_) {}
}

extension type B(int it) implements A {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo=
    <testLibrary>::@extensionType::A::@setter::foo
  it
    <testLibrary>::@extensionType::A::@getter::it
inheritedMap
  foo=: <testLibrary>::@extensionType::A::@setter::foo
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareMethod_static`.
    #[test]
    fn test_declare_method_static() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  static void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"A"#),
            config,
            r#"map
  it: <testLibrary>::@extensionType::A::@getter::it
declared
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareSetter`.
    #[test]
    fn test_declare_setter() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  set foo(int _) {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"A"#),
            config,
            r#"map
  foo=: <testLibrary>::@extensionType::A::@setter::foo
  it: <testLibrary>::@extensionType::A::@getter::it
declared
  foo=: <testLibrary>::@extensionType::A::@setter::foo
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareSetter_implementClass_withGetter`.
    #[test]
    fn test_declare_setter_implement_class_with_getter() {
        let r = SourceTest::linked(
            r#"class A {
  int get foo => 0;
}

class B extends A {}

extension type C(B it) implements A {
  set foo(_) {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@class::A::@getter::foo
  foo=: <testLibrary>::@extensionType::C::@setter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo=: <testLibrary>::@extensionType::C::@setter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@getter::foo
inheritedMap
  foo: <testLibrary>::@class::A::@getter::foo
"#,
        );
    }

    /// Dart: `test_declareSetter_implementClass_withMethod`.
    #[test]
    fn test_declare_setter_implement_class_with_method() {
        let r = SourceTest::linked(
            r#"class A {
  void foo() {}
}

class B extends A {}

extension type C(B it) implements A {
  set foo(_) {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo=: <testLibrary>::@extensionType::C::@setter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo=: <testLibrary>::@extensionType::C::@setter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@method::foo
inheritedMap
  foo: <testLibrary>::@class::A::@method::foo
"#,
        );
    }

    /// Dart: `test_declareSetter_implementExtensionType_withGetter`.
    #[test]
    fn test_declare_setter_implement_extension_type_with_getter() {
        let r = SourceTest::linked(
            r#"extension type A(int it) {
  int get foo => 0;
}

extension type B(int it) implements A {
  set foo(_) {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::A::@getter::foo
  foo=: <testLibrary>::@extensionType::B::@setter::foo
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  foo=: <testLibrary>::@extensionType::B::@setter::foo
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::A::@getter::foo
  it
    <testLibrary>::@extensionType::A::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::A::@getter::foo
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareSetter_implementExtensionType_withMethod`.
    #[test]
    fn test_declare_setter_implement_extension_type_with_method() {
        let r = SourceTest::linked(
            r#"extension type A(int it) {
  void foo() {}
}

extension type B(int it) implements A {
  set foo(_) {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo=: <testLibrary>::@extensionType::B::@setter::foo
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  foo=: <testLibrary>::@extensionType::B::@setter::foo
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::A::@method::foo
  it
    <testLibrary>::@extensionType::A::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::A::@method::foo
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_declareSetter_static`.
    #[test]
    fn test_declare_setter_static() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  static set foo(int _) {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"A"#),
            config,
            r#"map
  it: <testLibrary>::@extensionType::A::@getter::it
declared
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_getOverridden`.
    #[test]
    fn test_get_overridden() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

extension type B(A it) implements A {
  void foo() {}
}
"#,
        );
        let _config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        assert_eq!(
            get_overridden_text(&r, r.class_or_mixin(r#"B"#), r#"foo"#),
            r#"<null>
"#
        );
    }

    /// Dart: `test_noDeclaration_implementClass_generic_method`.
    #[test]
    fn test_no_declaration_implement_class_generic_method() {
        let r = SourceTest::new(
            r#"class A<T> {
  void foo(T a) {}
}

class B extends A<int> {}

extension type C(B it) implements A<int> {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: SubstitutedMethodElementImpl
    baseElement: <testLibrary>::@class::A::@method::foo
    substitution: {T: int}
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    SubstitutedMethodElementImpl
      baseElement: <testLibrary>::@class::A::@method::foo
      substitution: {T: int}
inheritedMap
  foo: SubstitutedMethodElementImpl
    baseElement: <testLibrary>::@class::A::@method::foo
    substitution: {T: int}
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_implementExtensionType_hasConflict_methods`.
    #[test]
    fn test_no_declaration_implement_class_implement_extension_type_has_conflict_methods() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

extension type B(A it) {
  void foo() {}
}

extension type C(A it) implements A, B {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::B::@method::foo
    <testLibrary>::@class::A::@method::foo
  it
    <testLibrary>::@extensionType::B::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
conflicts
  HasNonExtensionAndExtensionMemberConflict
    nonExtension
      <testLibrary>::@class::A::@method::foo
    extension
      <testLibrary>::@extensionType::B::@method::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_implementExtensionType_hasConflict_setters`.
    #[test]
    fn test_no_declaration_implement_class_implement_extension_type_has_conflict_setters() {
        let r = SourceTest::new(
            r#"class A {
  set foo(int _) {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A, B {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo=
    <testLibrary>::@extensionType::B::@setter::foo
    <testLibrary>::@class::A::@setter::foo
  it
    <testLibrary>::@extensionType::B::@getter::it
inheritedMap
  foo=: <testLibrary>::@extensionType::B::@setter::foo
  it: <testLibrary>::@extensionType::B::@getter::it
conflicts
  HasNonExtensionAndExtensionMemberConflict
    nonExtension
      <testLibrary>::@class::A::@setter::foo
    extension
      <testLibrary>::@extensionType::B::@setter::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_implementExtensionType_noConflict_methodPrecludesSetters`.
    #[test]
    fn test_no_declaration_implement_class_implement_extension_type_no_conflict_method_precludes_setters()
     {
        let r = SourceTest::new(
            r#"class A {
  set foo(int _) {}
}

extension type B(A it) {
  set foo(int _) {}
}

extension type C(A it) implements A, B {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo: <testLibrary>::@extensionType::C::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo=
    <testLibrary>::@extensionType::B::@setter::foo
    <testLibrary>::@class::A::@setter::foo
  it
    <testLibrary>::@extensionType::B::@getter::it
inheritedMap
  foo=: <testLibrary>::@extensionType::B::@setter::foo
  it: <testLibrary>::@extensionType::B::@getter::it
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_implementExtensionType_noConflict_setterPrecludesMethods`.
    #[test]
    fn test_no_declaration_implement_class_implement_extension_type_no_conflict_setter_precludes_methods()
     {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

extension type B(A it) {
  void foo() {}
}

extension type C(A it) implements A, B {
  set foo(int _) {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo=: <testLibrary>::@extensionType::C::@setter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  foo=: <testLibrary>::@extensionType::C::@setter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::B::@method::foo
    <testLibrary>::@class::A::@method::foo
  it
    <testLibrary>::@extensionType::B::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_method`.
    #[test]
    fn test_no_declaration_implement_class_method() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B extends A {}

extension type C(B it) implements A {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@class::A::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@method::foo
inheritedMap
  foo: <testLibrary>::@class::A::@method::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_method2_hasConflict`.
    #[test]
    fn test_no_declaration_implement_class_method2_has_conflict() {
        let r = SourceTest::new(
            r#"class A {
  int foo() => 0;
}

class B {
  String foo() => '0';
}

extension type C(Object it) implements A, B {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@method::foo
    <testLibrary>::@class::B::@method::foo
conflicts
  CandidatesConflict
    <testLibrary>::@class::A::@method::foo
    <testLibrary>::@class::B::@method::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_method2_noConflict`.
    #[test]
    fn test_no_declaration_implement_class_method2_no_conflict() {
        let r = SourceTest::new(
            r#"class A {
  int foo() => 0;
}

class B {
  num foo() => 0;
}

extension type C(Object it) implements A, B {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@class::A::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@method::foo
    <testLibrary>::@class::B::@method::foo
inheritedMap
  foo: <testLibrary>::@class::A::@method::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_method2_noConflict2`.
    #[test]
    fn test_no_declaration_implement_class_method2_no_conflict2() {
        let r = SourceTest::new(
            r#"class A {
  int foo() => 0;
}

class B1 extends A {}

class B2 extends A {}

abstract class C implements B1, B2 {}

extension type D(C it) implements B1, B2 {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"D"#),
            config,
            r#"map
  foo: <testLibrary>::@class::A::@method::foo
  it: <testLibrary>::@extensionType::D::@getter::it
declared
  it: <testLibrary>::@extensionType::D::@getter::it
redeclared
  foo
    <testLibrary>::@class::A::@method::foo
inheritedMap
  foo: <testLibrary>::@class::A::@method::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_setter`.
    #[test]
    fn test_no_declaration_implement_class_setter() {
        let r = SourceTest::new(
            r#"class A {
  set foo(int _) {}
}

class B extends A {}

extension type C(B it) implements A {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo=: <testLibrary>::@class::A::@setter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo=
    <testLibrary>::@class::A::@setter::foo
inheritedMap
  foo=: <testLibrary>::@class::A::@setter::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementClass_setter_combinedSignature`.
    #[test]
    fn test_no_declaration_implement_class_setter_combined_signature() {
        let r = SourceTest::new(
            r#"abstract class A {
  void set foo((int, int) Function(Object?, dynamic) f);
}

abstract class B {
  void set foo((int, int) Function(dynamic, Object?) f);
}

class C implements A, B {
  void set foo((int, int) Function(dynamic, dynamic) f) {}
}

extension type E(C it) implements A, B {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"E"#),
            config,
            r#"map
  foo=: <testLibrary>::@extensionType::E::@setter::foo
  it: <testLibrary>::@extensionType::E::@getter::it
declared
  it: <testLibrary>::@extensionType::E::@getter::it
redeclared
  foo=
    <testLibrary>::@class::A::@setter::foo
    <testLibrary>::@class::B::@setter::foo
inheritedMap
  foo=: <testLibrary>::@extensionType::E::@setter::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementExtensionType_generic_method`.
    #[test]
    fn test_no_declaration_implement_extension_type_generic_method() {
        let r = SourceTest::new(
            r#"extension type A<T>(T it) {
  void foo(T a) {}
}

extension type B(int it) implements A<int> {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo: SubstitutedMethodElementImpl
    baseElement: <testLibrary>::@extensionType::A::@method::foo
    substitution: {T: int}
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo
    SubstitutedMethodElementImpl
      baseElement: <testLibrary>::@extensionType::A::@method::foo
      substitution: {T: int}
  it
    SubstitutedGetterElementImpl
      baseElement: <testLibrary>::@extensionType::A::@getter::it
      substitution: {T: int}
inheritedMap
  foo: SubstitutedMethodElementImpl
    baseElement: <testLibrary>::@extensionType::A::@method::foo
    substitution: {T: int}
  it: SubstitutedGetterElementImpl
    baseElement: <testLibrary>::@extensionType::A::@getter::it
    substitution: {T: int}
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementExtensionType_method`.
    #[test]
    fn test_no_declaration_implement_extension_type_method() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  void foo() {}
}

extension type B(int it) implements A {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::A::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::A::@method::foo
  it
    <testLibrary>::@extensionType::A::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::A::@method::foo
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementExtensionType_method2_hasConflict`.
    #[test]
    fn test_no_declaration_implement_extension_type_method2_has_conflict() {
        let r = SourceTest::new(
            r#"extension type A1(int it) {
  void foo() {}
}

extension type A2(int it) {
  void foo() {}
}

extension type B(int it) implements A1, A2 {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::A1::@method::foo
    <testLibrary>::@extensionType::A2::@method::foo
  it
    <testLibrary>::@extensionType::A1::@getter::it
    <testLibrary>::@extensionType::A2::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::A1::@method::foo
  it: <testLibrary>::@extensionType::A1::@getter::it
conflicts
  NotUniqueExtensionMemberConflict
    <testLibrary>::@extensionType::A1::@method::foo
    <testLibrary>::@extensionType::A2::@method::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementExtensionType_method2_noConflict_setterPrecludes`.
    #[test]
    fn test_no_declaration_implement_extension_type_method2_no_conflict_setter_precludes() {
        let r = SourceTest::new(
            r#"extension type A1(int it) {
  void foo() {}
}

extension type A2(int it) {
  void foo() {}
}

extension type B(int it) implements A1, A2 {
  set foo(int _) {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo=: <testLibrary>::@extensionType::B::@setter::foo
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  foo=: <testLibrary>::@extensionType::B::@setter::foo
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::A1::@method::foo
    <testLibrary>::@extensionType::A2::@method::foo
  it
    <testLibrary>::@extensionType::A1::@getter::it
    <testLibrary>::@extensionType::A2::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::A1::@method::foo
  it: <testLibrary>::@extensionType::A1::@getter::it
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementExtensionType_method2_noConflict_unique`.
    #[test]
    fn test_no_declaration_implement_extension_type_method2_no_conflict_unique() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  void foo() {}
}

extension type B1(int it) implements A {}

extension type B2(int it) implements A {}

extension type C(int it) implements B1, B2 {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::A::@method::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo
    <testLibrary>::@extensionType::A::@method::foo
  it
    <testLibrary>::@extensionType::B1::@getter::it
    <testLibrary>::@extensionType::B2::@getter::it
inheritedMap
  foo: <testLibrary>::@extensionType::A::@method::foo
  it: <testLibrary>::@extensionType::B1::@getter::it
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementExtensionType_setter2_hasConflict`.
    #[test]
    fn test_no_declaration_implement_extension_type_setter2_has_conflict() {
        let r = SourceTest::new(
            r#"extension type A1(int it) {
  set foo(int _) {}
}

extension type A2(int it) {
  set foo(int _) {}
}

extension type B(int it) implements A1, A2 {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo=
    <testLibrary>::@extensionType::A1::@setter::foo
    <testLibrary>::@extensionType::A2::@setter::foo
  it
    <testLibrary>::@extensionType::A1::@getter::it
    <testLibrary>::@extensionType::A2::@getter::it
inheritedMap
  foo=: <testLibrary>::@extensionType::A1::@setter::foo
  it: <testLibrary>::@extensionType::A1::@getter::it
conflicts
  NotUniqueExtensionMemberConflict
    <testLibrary>::@extensionType::A1::@setter::foo
    <testLibrary>::@extensionType::A2::@setter::foo
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementExtensionType_setter2_noConflict_methodPrecludes`.
    #[test]
    fn test_no_declaration_implement_extension_type_setter2_no_conflict_method_precludes() {
        let r = SourceTest::new(
            r#"extension type A1(int it) {
  set foo(int _) {}
}

extension type A2(int it) {
  set foo(int _) {}
}

extension type B(int it) implements A1, A2 {
  void foo() {}
}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"B"#),
            config,
            r#"map
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
declared
  foo: <testLibrary>::@extensionType::B::@method::foo
  it: <testLibrary>::@extensionType::B::@getter::it
redeclared
  foo=
    <testLibrary>::@extensionType::A1::@setter::foo
    <testLibrary>::@extensionType::A2::@setter::foo
  it
    <testLibrary>::@extensionType::A1::@getter::it
    <testLibrary>::@extensionType::A2::@getter::it
inheritedMap
  foo=: <testLibrary>::@extensionType::A1::@setter::foo
  it: <testLibrary>::@extensionType::A1::@getter::it
"#,
        );
    }

    /// Dart: `test_noDeclaration_implementExtensionType_setter2_noConflict_unique`.
    #[test]
    fn test_no_declaration_implement_extension_type_setter2_no_conflict_unique() {
        let r = SourceTest::new(
            r#"extension type A(int it) {
  set foo(int _) {}
}

extension type B1(int it) implements A {}

extension type B2(int it) implements A {}

extension type C(int it) implements B1, B2 {}
"#,
        );
        let config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        r.assert_interface_text(
            r.class_or_mixin(r#"C"#),
            config,
            r#"map
  foo=: <testLibrary>::@extensionType::A::@setter::foo
  it: <testLibrary>::@extensionType::C::@getter::it
declared
  it: <testLibrary>::@extensionType::C::@getter::it
redeclared
  foo=
    <testLibrary>::@extensionType::A::@setter::foo
  it
    <testLibrary>::@extensionType::B1::@getter::it
    <testLibrary>::@extensionType::B2::@getter::it
inheritedMap
  foo=: <testLibrary>::@extensionType::A::@setter::foo
  it: <testLibrary>::@extensionType::B1::@getter::it
"#,
        );
    }

    /// Dart: `test_withObjectMembers`.
    #[test]
    fn test_with_object_members() {
        let r = SourceTest::new(
            r#"extension type A(int it) {}
"#,
        );
        let mut config = InterfacePrinterConfiguration {
            without_identical_implemented: true,
            ..Default::default()
        };
        config.with_object_members = true;
        r.assert_interface_text(
            r.class_or_mixin(r#"A"#),
            config,
            r#"map
  it: <testLibrary>::@extensionType::A::@getter::it
declared
  it: <testLibrary>::@extensionType::A::@getter::it
"#,
        );
    }
}

/// Rust-only variants of tests that need inference: the Dart source with the
/// inferred types written out. Not part of the Dart test count.
mod explicit_type_variants {
    #[allow(unused_imports)]
    use super::support::*;

    /// The Dart test `test_get_member_concrete_no_such_method` with the type that override inference gives
    /// `noSuchMethod(_)` (`dynamic Function(Invocation)`), so the
    /// `noSuchMethod` forwarder rules run before the linker exists.
    #[test]
    fn test_get_member_concrete_no_such_method() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B implements A {
  dynamic noSuchMethod(Invocation _) {}
}

abstract class C extends B {}
"#,
        );
        r.assert_get_member(
            r#"B"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            true,
            false,
        );
        r.assert_get_member(
            r#"C"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            true,
            false,
        );
    }

    /// The Dart test `test_get_member_concrete_no_such_method_mixin` with the type that override inference gives
    /// `noSuchMethod(_)` (`dynamic Function(Invocation)`), so the
    /// `noSuchMethod` forwarder rules run before the linker exists.
    #[test]
    fn test_get_member_concrete_no_such_method_mixin() {
        let r = SourceTest::new(
            r#"class A {
  void foo();

  dynamic noSuchMethod(Invocation _) {}
}

abstract class B extends Object with A {}
"#,
        );
        r.assert_get_member(r#"B"#, r#"foo"#, None, true, false);
    }

    /// The Dart test `test_get_member_concrete_no_such_method_more_specific_signature` with the type that override inference gives
    /// `noSuchMethod(_)` (`dynamic Function(Invocation)`), so the
    /// `noSuchMethod` forwarder rules run before the linker exists.
    #[test]
    fn test_get_member_concrete_no_such_method_more_specific_signature() {
        let r = SourceTest::new(
            r#"class A {
  void foo() {}
}

class B implements A {
  dynamic noSuchMethod(Invocation _) {}
}

class C extends B {
  void foo([int a]);
}
"#,
        );
        r.assert_get_member(
            r#"C"#,
            r#"foo"#,
            Some(r#"C.foo: void Function([int])"#),
            true,
            false,
        );
    }

    /// The Dart test `test_get_member_super_abstract` with the type that override inference gives
    /// `noSuchMethod(_)` (`dynamic Function(Invocation)`), so the
    /// `noSuchMethod` forwarder rules run before the linker exists.
    #[test]
    fn test_get_member_super_abstract() {
        let r = SourceTest::new(
            r#"abstract class A {
  void foo();
}

class B extends A {
  dynamic noSuchMethod(Invocation _) {}
}
"#,
        );
        r.assert_get_member(r#"B"#, r#"foo"#, None, false, true);
    }

    /// The Dart test `test_get_member_super_no_such_member` with the type that override inference gives
    /// `noSuchMethod(_)` (`dynamic Function(Invocation)`), so the
    /// `noSuchMethod` forwarder rules run before the linker exists.
    #[test]
    fn test_get_member_super_no_such_member() {
        let r = SourceTest::new(
            r#"class A {
  void foo();
  dynamic noSuchMethod(Invocation _) {}
}

class B extends A {
  void foo() {}
}
"#,
        );
        r.assert_get_member(
            r#"B"#,
            r#"foo"#,
            Some(r#"A.foo: void Function()"#),
            false,
            true,
        );
    }
}
