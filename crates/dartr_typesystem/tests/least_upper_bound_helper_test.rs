// Dart source: pkg/analyzer/test/src/dart/element/least_upper_bound_helper_test.dart

//! Port of `least_upper_bound_helper_test.dart`: `PathToObjectTest` and
//! `SuperinterfaceSetTest`.

use dartr_element::TypeId;
use dartr_typesystem::TypeExt;
use dartr_typesystem::least_upper_bound::InterfaceLeastUpperBoundHelper;
use dartr_typesystem::test_support::*;

fn classes(headers: &[&str]) -> Vec<ClassSpec> {
    headers.iter().map(|h| ClassSpec::new(h)).collect()
}

mod path_to_object_test {
    use super::*;

    /// `_toType(type)`.
    fn to_type(t: &TypeSystemTest, input: &str) -> i64 {
        let ty = t.parse_interface_type(input);
        InterfaceLeastUpperBoundHelper::compute_longest_inheritance_path_to_object(&t.ctx(), ty)
    }

    #[test]
    fn class_mixins1() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class X extends A with M1"]),
            mixins: strs(&["mixin M1"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "M1"), 2);
        assert_eq!(to_type(&t, "A"), 2);

        // class _X&A&M1 extends A implements M1 {}
        //    length: 2
        // class X extends _X&A&M1 {}
        //    length: 3
        assert_eq!(to_type(&t, "X"), 4);
    }

    #[test]
    fn class_mixins2() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class X extends A with M1, M2"]),
            mixins: strs(&["mixin M1", "mixin M2"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "M1"), 2);
        assert_eq!(to_type(&t, "M2"), 2);
        assert_eq!(to_type(&t, "A"), 2);

        // class _X&A&M1 extends A implements M1 {}
        //    length: 2
        // class _X&A&M1&M2 extends _X&A&M1 implements M2 {}
        //    length: 3
        // class X extends _X&A&M1&M2 {}
        //    length: 4
        assert_eq!(to_type(&t, "X"), 5);
    }

    #[test]
    fn class_mixins_longer_via_second_mixin() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class I1",
                "class I2 extends I1",
                "class I3 extends I2",
                "class A",
                "class X extends A with M1, M2",
            ]),
            mixins: strs(&["mixin M1", "mixin M2 implements I3"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "I1"), 2);
        assert_eq!(to_type(&t, "I2"), 3);
        assert_eq!(to_type(&t, "I3"), 4);
        assert_eq!(to_type(&t, "M1"), 2);
        assert_eq!(to_type(&t, "M2"), 5);
        assert_eq!(to_type(&t, "A"), 2);

        // class _X&A&M1 extends A implements M1 {}
        //    length: 2
        // class _X&A&M1&M2 extends _X&A&M1 implements M2 {}
        //    length: 5 = max(1 + _X&A&M1, 1 + M2)
        // class X extends _X&A&M1&M2 {}
        //    length: 6
        assert_eq!(to_type(&t, "X"), 7);
    }

    #[test]
    fn class_multiple_interface_paths() {
        //
        //   Object?
        //     |
        //   Object
        //     |
        //     A
        //    / \
        //   B   C
        //   |   |
        //   |   D
        //    \ /
        //     E
        //
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A",
                "class B implements A",
                "class C implements A",
                "class D implements C",
                "class E implements B, D",
            ]),
            ..LibrarySpec::test()
        });
        // assertion: even though the longest path to Object for typeB is 2, and
        // typeE implements typeB, the longest path for typeE is 4 since it also
        // implements typeD
        assert_eq!(to_type(&t, "B"), 3);
        assert_eq!(to_type(&t, "E"), 5);
    }

    #[test]
    fn class_multiple_superclass_paths() {
        //
        //   Object?
        //     |
        //   Object
        //     |
        //     A
        //    / \
        //   B   C
        //   |   |
        //   |   D
        //    \ /
        //     E
        //
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A",
                "class B extends A",
                "class C extends A",
                "class D extends C",
                "class E extends B implements D",
            ]),
            ..LibrarySpec::test()
        });
        // assertion: even though the longest path to Object for typeB is 2, and
        // typeE extends typeB, the longest path for typeE is 4 since it also
        // implements typeD
        assert_eq!(to_type(&t, "B"), 3);
        assert_eq!(to_type(&t, "E"), 5);
    }

    #[test]
    fn class_null() {
        let t = TypeSystemTest::new();
        assert_eq!(to_type(&t, "Null"), 1);
    }

    #[test]
    fn class_object() {
        let t = TypeSystemTest::new();
        assert_eq!(to_type(&t, "Object?"), 0);
        assert_eq!(to_type(&t, "Object"), 1);
    }

    #[test]
    fn class_recursion() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A extends B", "class B extends A"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "A"), 2);
    }

    #[test]
    fn class_single_interface_path() {
        //
        //   Object?
        //     |
        //   Object
        //     |
        //     A
        //     |
        //     B
        //     |
        //     C
        //
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B implements A", "class C implements B"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "A"), 2);
        assert_eq!(to_type(&t, "B"), 3);
        assert_eq!(to_type(&t, "C"), 4);
    }

    #[test]
    fn class_single_superclass_path() {
        //
        //   Object?
        //     |
        //   Object
        //     |
        //     A
        //     |
        //     B
        //     |
        //     C
        //
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B extends A", "class C extends B"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "A"), 2);
        assert_eq!(to_type(&t, "B"), 3);
        assert_eq!(to_type(&t, "C"), 4);
    }

    #[test]
    fn mixin_constraints_interfaces_all_same() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B", "class I", "class J"]),
            mixins: strs(&["mixin M on A, B implements I, J"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "A"), 2);
        assert_eq!(to_type(&t, "B"), 2);
        assert_eq!(to_type(&t, "I"), 2);
        assert_eq!(to_type(&t, "J"), 2);
        // The interface of M is:
        // class _M&A&A implements A, B, I, J {}
        assert_eq!(to_type(&t, "M"), 3);
    }

    #[test]
    fn mixin_longer_constraint_1() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A1",
                "class A extends A1",
                "class B",
                "class I",
                "class J",
            ]),
            mixins: strs(&["mixin M on A, B implements I, J"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "A"), 3);
        assert_eq!(to_type(&t, "B"), 2);
        assert_eq!(to_type(&t, "I"), 2);
        assert_eq!(to_type(&t, "J"), 2);
        // The interface of M is:
        // class _M&A&A implements A, B, I, J {}
        assert_eq!(to_type(&t, "M"), 4);
    }

    #[test]
    fn mixin_longer_constraint_2() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A",
                "class B1",
                "class B implements B1",
                "class I",
                "class J",
            ]),
            mixins: strs(&["mixin M on A, B implements I, J"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "A"), 2);
        assert_eq!(to_type(&t, "B"), 3);
        assert_eq!(to_type(&t, "I"), 2);
        assert_eq!(to_type(&t, "J"), 2);
        // The interface of M is:
        // class _M&A&A implements A, B, I, J {}
        assert_eq!(to_type(&t, "M"), 4);
    }

    #[test]
    fn mixin_longer_interface_1() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A",
                "class B",
                "class I1",
                "class I implements I1",
                "class J",
            ]),
            mixins: strs(&["mixin M on A, B implements I, J"]),
            ..LibrarySpec::test()
        });
        assert_eq!(to_type(&t, "A"), 2);
        assert_eq!(to_type(&t, "B"), 2);
        assert_eq!(to_type(&t, "I"), 3);
        assert_eq!(to_type(&t, "J"), 2);
        // The interface of M is:
        // class _M&A&A implements A, B, I, J {}
        assert_eq!(to_type(&t, "M"), 4);
    }
}

mod superinterface_set_test {
    use super::*;

    /// `_superInterfaces(type)`.
    fn super_interfaces(t: &TypeSystemTest, ty: TypeId) -> Vec<TypeId> {
        let helper = InterfaceLeastUpperBoundHelper::new(t.type_system());
        helper.compute_superinterface_set(ty).items().to_vec()
    }

    /// `expect(actual, unorderedEquals(expected))` with Dart `==`.
    fn assert_unordered_equals(t: &TypeSystemTest, actual: &[TypeId], expected: &[TypeId]) {
        let ctx = t.ctx();
        let show = |list: &[TypeId]| list.iter().map(|&x| t.display(x)).collect::<Vec<_>>();
        let mut unmatched: Vec<TypeId> = actual.to_vec();
        for &e in expected {
            // Dart: ==
            match unmatched.iter().position(|&a| ctx.dart_eq(a, e)) {
                Some(i) => {
                    unmatched.remove(i);
                }
                None => panic!(
                    "Expected: unordered {:?}\n  Actual: {:?}\n  missing: {}",
                    show(expected),
                    show(actual),
                    t.display(e)
                ),
            }
        }
        assert!(
            unmatched.is_empty(),
            "Expected: unordered {:?}\n  Actual: {:?}\n  unexpected: {:?}",
            show(expected),
            show(actual),
            show(&unmatched)
        );
    }

    #[test]
    fn generic_interface_path() {
        //
        //  A
        //  | implements
        //  B<T>
        //  | implements
        //  C<T>
        //
        //  D
        //
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A",
                "class B<T> implements A",
                "class C<T> implements B<T>",
                "class D",
            ]),
            ..LibrarySpec::test()
        });
        let inst_a = t.parse_interface_type("A");

        // A
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_a),
            &[t.parse_type("Object?"), t.parse_type("Object")],
        );

        // B<D>
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, t.parse_interface_type("B<D>")),
            &[t.parse_type("Object?"), t.parse_type("Object"), inst_a],
        );

        // C<D>
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, t.parse_interface_type("C<D>")),
            &[
                t.parse_type("Object?"),
                t.parse_type("Object"),
                inst_a,
                t.parse_interface_type("B<D>"),
            ],
        );
    }

    #[test]
    fn generic_superclass_path() {
        //
        //  A
        //  |
        //  B<T>
        //  |
        //  C<T>
        //
        //  D
        //
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A",
                "class B<T> extends A",
                "class C<T> extends B<T>",
                "class D",
            ]),
            ..LibrarySpec::test()
        });
        let inst_a = t.parse_interface_type("A");

        // A
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_a),
            &[t.parse_type("Object?"), t.parse_type("Object")],
        );

        // B<D>
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, t.parse_interface_type("B<D>")),
            &[t.parse_type("Object?"), t.parse_type("Object"), inst_a],
        );

        // C<D>
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, t.parse_interface_type("C<D>")),
            &[
                t.parse_type("Object?"),
                t.parse_type("Object"),
                inst_a,
                t.parse_interface_type("B<D>"),
            ],
        );
    }

    #[test]
    fn mixin_constraints() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B implements A", "class C"]),
            mixins: strs(&["mixin M on B, C"]),
            ..LibrarySpec::test()
        });
        let inst_a = t.parse_interface_type("A");
        let inst_b = t.parse_interface_type("B");
        let inst_c = t.parse_interface_type("C");
        let inst_m = t.parse_interface_type("M");

        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_m),
            &[
                t.parse_type("Object?"),
                t.parse_type("Object"),
                inst_a,
                inst_b,
                inst_c,
            ],
        );
    }

    #[test]
    fn mixin_constraints_object() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            mixins: strs(&["mixin M"]),
            ..LibrarySpec::test()
        });
        let inst_m = t.parse_interface_type("M");

        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_m),
            &[t.parse_type("Object?"), t.parse_type("Object")],
        );
    }

    #[test]
    fn mixin_interfaces() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B implements A", "class C"]),
            mixins: strs(&["mixin M implements B, C"]),
            ..LibrarySpec::test()
        });
        let inst_a = t.parse_interface_type("A");
        let inst_b = t.parse_interface_type("B");
        let inst_c = t.parse_interface_type("C");
        let inst_m = t.parse_interface_type("M");

        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_m),
            &[
                t.parse_type("Object?"),
                t.parse_type("Object"),
                inst_a,
                inst_b,
                inst_c,
            ],
        );
    }

    #[test]
    fn multiple_interface_paths() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A",
                "class B implements A",
                "class C implements A",
                "class D implements C",
                "class E implements B, D",
            ]),
            ..LibrarySpec::test()
        });
        let inst_a = t.parse_interface_type("A");
        let inst_b = t.parse_interface_type("B");
        let inst_c = t.parse_interface_type("C");
        let inst_d = t.parse_interface_type("D");
        let inst_e = t.parse_interface_type("E");

        // D
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_d),
            &[t.parse_type("Object?"), t.parse_type("Object"), inst_a, inst_c],
        );

        // E
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_e),
            &[
                t.parse_type("Object?"),
                t.parse_type("Object"),
                inst_a,
                inst_b,
                inst_c,
                inst_d,
            ],
        );
    }

    #[test]
    fn multiple_superclass_paths() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A",
                "class B extends A",
                "class C extends A",
                "class D extends C",
                "class E extends B implements D",
            ]),
            ..LibrarySpec::test()
        });
        let inst_a = t.parse_interface_type("A");
        let inst_b = t.parse_interface_type("B");
        let inst_c = t.parse_interface_type("C");
        let inst_d = t.parse_interface_type("D");
        let inst_e = t.parse_interface_type("E");

        // D
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_d),
            &[t.parse_type("Object?"), t.parse_type("Object"), inst_a, inst_c],
        );

        // E
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_e),
            &[
                t.parse_type("Object?"),
                t.parse_type("Object"),
                inst_a,
                inst_b,
                inst_c,
                inst_d,
            ],
        );
    }

    #[test]
    fn recursion() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B extends A"]),
            ..LibrarySpec::test()
        });
        let class_a = t.class_element("A");
        let inst_a = t.parse_interface_type("A");
        let inst_b = t.parse_interface_type("B");

        // classA.supertype = instB;
        t.ctx()
            .interface(class_a.upcast())
            .supertype
            .set(Some(inst_b));

        assert_unordered_equals(&t, &super_interfaces(&t, inst_b), &[inst_a, inst_b]);

        assert_unordered_equals(&t, &super_interfaces(&t, inst_a), &[inst_a, inst_b]);
    }

    #[test]
    fn single_interface_path() {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B implements A", "class C implements B"]),
            ..LibrarySpec::test()
        });
        let inst_a = t.parse_interface_type("A");
        let inst_b = t.parse_interface_type("B");
        let inst_c = t.parse_interface_type("C");

        // A
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_a),
            &[t.parse_type("Object?"), t.parse_type("Object")],
        );

        // B
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_b),
            &[t.parse_type("Object?"), t.parse_type("Object"), inst_a],
        );

        // C
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_c),
            &[t.parse_type("Object?"), t.parse_type("Object"), inst_a, inst_b],
        );
    }

    #[test]
    fn single_superclass_path() {
        //
        //  A
        //  |
        //  B
        //  |
        //  C
        //
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B extends A", "class C extends B"]),
            ..LibrarySpec::test()
        });
        let inst_a = t.parse_interface_type("A");
        let inst_b = t.parse_interface_type("B");
        let inst_c = t.parse_interface_type("C");

        // A
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_a),
            &[t.parse_type("Object?"), t.parse_type("Object")],
        );

        // B
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_b),
            &[t.parse_type("Object?"), t.parse_type("Object"), inst_a],
        );

        // C
        assert_unordered_equals(
            &t,
            &super_interfaces(&t, inst_c),
            &[t.parse_type("Object?"), t.parse_type("Object"), inst_a, inst_b],
        );
    }
}
