// Dart source: pkg/analyzer/test/src/dart/element/upper_lower_bound_test.dart

//! Port of `upper_lower_bound_test.dart`: one `mod` per test class
//! (`BoundsHelperPredicatesTest`, `LowerBoundTest`,
//! `UpperBound_FunctionTypes_Test`, `UpperBound_InterfaceTypes_Test`,
//! `UpperBound_RecordTypes_Test`, `UpperBoundTest`); the helpers of
//! `_BoundsTestBase` are the free functions below.

#![allow(non_snake_case)]

use dartr_element::{Nullability, TypeId};
use dartr_typesystem::TypeExt;
use dartr_typesystem::test_support::*;

// ------------------------------------------------------------ _BoundsTestBase

fn classes(headers: &[&str]) -> Vec<ClassSpec> {
    headers.iter().map(|h| ClassSpec::new(h)).collect()
}

fn _assert_bottom(t: &TypeSystemTest, ty: TypeId) {
    if !t.ctx().is_bottom(ty) {
        panic!("isBottom must be true: {}", t.display(ty));
    }
}

fn _assert_not_bottom(t: &TypeSystemTest, ty: TypeId) {
    if t.ctx().is_bottom(ty) {
        panic!("isBottom must be false: {}", t.display(ty));
    }
}

fn _assert_not_null(t: &TypeSystemTest, ty: TypeId) {
    if t.type_system().is_null(ty) {
        panic!("isNull must be false: {}", t.display(ty));
    }
}

fn _assert_not_object(t: &TypeSystemTest, ty: TypeId) {
    if t.type_system().is_object(ty) {
        panic!("isObject must be false: {}", t.display(ty));
    }
}

fn _assert_not_special(t: &TypeSystemTest, ty: TypeId) {
    _assert_not_bottom(t, ty);
    _assert_not_null(t, ty);
    _assert_not_object(t, ty);
    _assert_not_top(t, ty);
}

fn _assert_not_top(t: &TypeSystemTest, ty: TypeId) {
    if t.type_system().is_top(ty) {
        panic!("isTop must be false: {}", t.display(ty));
    }
}

fn _assert_null(t: &TypeSystemTest, ty: TypeId) {
    if !t.type_system().is_null(ty) {
        panic!("isNull must be true: {}", t.display(ty));
    }
}

fn _assert_nullability(t: &TypeSystemTest, ty: TypeId, expected: Nullability) {
    if t.ctx().nullability_suffix(ty) != expected {
        panic!("Expected {expected:?} in {}", t.display(ty));
    }
}

fn _assert_nullability_none(t: &TypeSystemTest, ty: TypeId) {
    _assert_nullability(t, ty, Nullability::None);
}

fn _assert_nullability_question(t: &TypeSystemTest, ty: TypeId) {
    _assert_nullability(t, ty, Nullability::Question);
}

fn _assert_object(t: &TypeSystemTest, ty: TypeId) {
    if !t.type_system().is_object(ty) {
        panic!("isObject must be true: {}", t.display(ty));
    }
}

fn _assert_top(t: &TypeSystemTest, ty: TypeId) {
    if !t.type_system().is_top(ty) {
        panic!("isTop must be true: {}", t.display(ty));
    }
}

/// `expect(result, expected, reason: ...)` with Dart `==`.
fn expect_type(t: &TypeSystemTest, result: TypeId, expected: TypeId) {
    // Dart: ==
    assert!(
        t.ctx().dart_eq(result, expected),
        "\nexpected: {}\nactual: {}\n",
        t.display(expected),
        t.display(result)
    );
}

fn _check_least_upper_bound(t: &TypeSystemTest, T1: TypeId, T2: TypeId, expected: TypeId) {
    let ts = t.type_system();

    let result = ts.least_upper_bound(T1, T2);
    expect_type(t, result, expected);

    // Check that the result is an upper bound.
    assert!(ts.is_subtype_of(T1, result));
    assert!(ts.is_subtype_of(T2, result));

    // Check for symmetry.
    let result = ts.least_upper_bound(T2, T1);
    expect_type(t, result, expected);
}

fn _check_least_upper_bound2(t: &TypeSystemTest, T1: &str, T2: &str, expected: &str) {
    _check_least_upper_bound(
        t,
        t.parse_type(T1),
        t.parse_type(T2),
        t.parse_type(expected),
    );
}

// ------------------------------------------------- BoundsHelperPredicatesTest

mod bounds_helper_predicates_test {
    use std::cell::RefCell;

    use super::*;

    /// The test class with its static maps `_isMoreBottomChecked` and
    /// `_isMoreTopChecked` (each map is used by one test only, so a map per
    /// test is the same).
    struct P<'t> {
        t: &'t TypeSystemTest,
        is_more_bottom_checked: RefCell<Vec<String>>,
        is_more_top_checked: RefCell<Vec<String>>,
    }

    impl<'t> P<'t> {
        fn new(t: &'t TypeSystemTest) -> Self {
            P {
                t,
                is_more_bottom_checked: RefCell::new(Vec::new()),
                is_more_top_checked: RefCell::new(Vec::new()),
            }
        }

        fn is_bottom(&self, ty: TypeId) {
            assert!(self.t.ctx().is_bottom(ty), "{}", self.t.display(ty));
        }

        fn is_more_bottom(&self, T: TypeId, S: TypeId) {
            self._assert_is_bottom_or_null(T);
            self._assert_is_bottom_or_null(S);

            let str = format!("{} vs {}", self.t.display(T), self.t.display(S));
            Self::_check_unique_type_str(&self.is_more_bottom_checked, &str);

            assert!(self.t.type_system().is_more_bottom(T, S), "{str}");
        }

        fn is_more_top(&self, T: TypeId, S: TypeId) {
            self._assert_is_top_or_object(T);
            self._assert_is_top_or_object(S);

            let str = format!("{} vs {}", self.t.display(T), self.t.display(S));
            Self::_check_unique_type_str(&self.is_more_top_checked, &str);

            assert!(self.t.type_system().is_more_top(T, S), "{str}");
        }

        fn is_not_bottom(&self, ty: TypeId) {
            assert!(!self.t.ctx().is_bottom(ty), "{}", self.t.display(ty));
        }

        fn is_not_more_bottom(&self, T: TypeId, S: TypeId) {
            self._assert_is_bottom_or_null(T);
            self._assert_is_bottom_or_null(S);

            let str = format!("{} vs {}", self.t.display(T), self.t.display(S));
            Self::_check_unique_type_str(&self.is_more_bottom_checked, &str);

            assert!(!self.t.type_system().is_more_bottom(T, S), "{str}");
        }

        fn is_not_more_top(&self, T: TypeId, S: TypeId) {
            self._assert_is_top_or_object(T);
            self._assert_is_top_or_object(S);

            let str = format!("{} vs {}", self.t.display(T), self.t.display(S));
            Self::_check_unique_type_str(&self.is_more_top_checked, &str);

            assert!(!self.t.type_system().is_more_top(T, S), "{str}");
        }

        fn is_not_null(&self, ty: TypeId) {
            assert!(!self.t.type_system().is_null(ty), "{}", self.t.display(ty));
        }

        fn is_not_object(&self, ty: TypeId) {
            assert!(
                !self.t.type_system().is_object(ty),
                "{}",
                self.t.display(ty)
            );
        }

        fn is_not_top(&self, ty: TypeId) {
            assert!(!self.t.type_system().is_top(ty), "{}", self.t.display(ty));
        }

        fn is_null(&self, ty: TypeId) {
            assert!(self.t.type_system().is_null(ty), "{}", self.t.display(ty));
        }

        fn is_object(&self, ty: TypeId) {
            assert!(self.t.type_system().is_object(ty), "{}", self.t.display(ty));
        }

        fn is_top(&self, ty: TypeId) {
            assert!(self.t.type_system().is_top(ty), "{}", self.t.display(ty));
        }

        /// [TypeSystemImpl.isMoreBottom] can be used only for `BOTTOM` or `NULL`
        /// types. No need to check other types.
        fn _assert_is_bottom_or_null(&self, ty: TypeId) {
            assert!(
                self.t.ctx().is_bottom(ty) || self.t.type_system().is_null(ty),
                "{}",
                self.t.display(ty)
            );
        }

        /// [TypeSystemImpl.isMoreTop] can be used only for `TOP` or `OBJECT`
        /// types. No need to check other types.
        fn _assert_is_top_or_object(&self, ty: TypeId) {
            assert!(
                self.t.type_system().is_top(ty) || self.t.type_system().is_object(ty),
                "{}",
                self.t.display(ty)
            );
        }

        fn _check_unique_type_str(map: &RefCell<Vec<String>>, str: &str) {
            let mut map = map.borrow_mut();
            if map.iter().any(|s| s == str) {
                panic!("Not unique: {str}");
            }
            map.push(str.to_string());
        }
    }

    #[test]
    fn is_bottom() {
        let t = TypeSystemTest::new();
        let p = P::new(&t);
        let pt = |s: &str| t.parse_type(s);

        // BOTTOM(Never) is true
        p.is_bottom(pt("Never"));
        p.is_not_bottom(pt("Never?"));

        // BOTTOM(X&T) is true iff BOTTOM(T)
        t.with_type_parameter_scope("T extends Object?", |scope| {
            p.is_bottom(scope.parse_type("T & Never"));
            p.is_not_bottom(scope.parse_type("(T & Never)?"));
            p.is_not_bottom(scope.parse_type("T & Never?"));
            p.is_not_bottom(scope.parse_type("(T & Never?)?"));
        });

        // BOTTOM(X extends T) is true iff BOTTOM(T)
        t.with_type_parameter_scope("T extends Never", |scope| {
            p.is_bottom(scope.parse_type("T"));
            p.is_not_bottom(scope.parse_type("T?"));
        });

        t.with_type_parameter_scope("T extends Never?", |scope| {
            p.is_not_bottom(scope.parse_type("T"));
            p.is_not_bottom(scope.parse_type("T?"));
        });

        // BOTTOM(T) is false otherwise
        p.is_not_bottom(pt("dynamic"));
        p.is_not_bottom(pt("InvalidType"));
        p.is_not_bottom(pt("void"));

        p.is_not_bottom(pt("Object"));
        p.is_not_bottom(pt("Object?"));

        p.is_not_bottom(pt("int"));
        p.is_not_bottom(pt("int?"));

        t.with_type_parameter_scope("T extends num", |scope| {
            p.is_not_bottom(scope.parse_type("T"));
            p.is_not_bottom(scope.parse_type("T?"));
            p.is_not_bottom(scope.parse_type("T & int"));
            p.is_not_bottom(scope.parse_type("(T & int)?"));
        });
    }

    #[test]
    fn is_more_bottom() {
        let t = TypeSystemTest::new();
        let p = P::new(&t);
        let pt = |s: &str| t.parse_type(s);

        // MOREBOTTOM(Never, T) = true
        p.is_more_bottom(pt("Never"), pt("Never"));
        p.is_more_bottom(pt("Never"), pt("Never?"));

        p.is_more_bottom(pt("Never"), pt("Null"));

        // MOREBOTTOM(T, Never) = false
        p.is_not_more_bottom(pt("Never?"), pt("Never"));

        p.is_not_more_bottom(pt("Null"), pt("Never"));

        // MOREBOTTOM(Null, T) = true
        p.is_more_bottom(pt("Null"), pt("Never?"));

        p.is_more_bottom(pt("Null"), pt("Null"));

        // MOREBOTTOM(T, Null) = false
        p.is_not_more_bottom(pt("Never?"), pt("Null"));

        // MOREBOTTOM(X&T, Y&S) = MOREBOTTOM(T, S)
        t.with_type_parameter_scope("T extends Object?, S extends Object?", |scope| {
            p.is_more_bottom(
                scope.parse_type("T & Never"),
                scope.parse_type("(S & Never)?"),
            );
        });

        // MOREBOTTOM(X&T, S) = true
        t.with_type_parameter_scope("T extends Object?, S extends Never", |scope| {
            p.is_more_bottom(scope.parse_type("T & Never"), scope.parse_type("S"));
        });

        // MOREBOTTOM(T, X&S) = false
        t.with_type_parameter_scope("T extends Never, S extends Object?", |scope| {
            p.is_not_more_bottom(scope.parse_type("T"), scope.parse_type("S & Never"));
        });

        // MOREBOTTOM(X extends T, Y extends S) = MOREBOTTOM(T, S)
        t.with_type_parameter_scope("T extends Never, S extends Never", |scope| {
            p.is_more_bottom(scope.parse_type("T"), scope.parse_type("S?"));
        });
    }

    #[test]
    fn is_more_top() {
        let t = TypeSystemTest::new();
        let p = P::new(&t);
        let pt = |s: &str| t.parse_type(s);

        // MORETOP(void, T) = true
        p.is_more_top(pt("void"), pt("void"));
        p.is_more_top(pt("void"), pt("dynamic"));
        p.is_more_top(pt("void"), pt("InvalidType"));
        p.is_more_top(pt("void"), pt("Object"));
        p.is_more_top(pt("void"), pt("Object?"));
        p.is_more_top(pt("void"), pt("FutureOr<Object>"));
        p.is_more_top(pt("void"), pt("FutureOr<Object?>"));

        // MORETOP(T, void) = false
        p.is_not_more_top(pt("dynamic"), pt("void"));
        p.is_not_more_top(pt("InvalidType"), pt("void"));
        p.is_not_more_top(pt("Object"), pt("void"));
        p.is_not_more_top(pt("Object?"), pt("void"));
        p.is_not_more_top(pt("FutureOr<Object>"), pt("void"));
        p.is_not_more_top(pt("FutureOr<Object?>"), pt("void"));

        // MORETOP(dynamic, T) = true
        p.is_more_top(pt("dynamic"), pt("dynamic"));
        p.is_more_top(pt("dynamic"), pt("Object"));
        p.is_more_top(pt("dynamic"), pt("Object?"));
        p.is_more_top(pt("dynamic"), pt("FutureOr<Object>"));
        p.is_more_top(pt("dynamic"), pt("FutureOr<Object?>"));

        // MORETOP(parseType('InvalidType'), T) = true
        p.is_more_top(pt("InvalidType"), pt("dynamic"));
        p.is_more_top(pt("InvalidType"), pt("Object"));
        p.is_more_top(pt("InvalidType"), pt("Object?"));
        p.is_more_top(pt("InvalidType"), pt("FutureOr<Object>"));
        p.is_more_top(pt("InvalidType"), pt("FutureOr<Object?>"));

        // MORETOP(T, dynamic) = false
        p.is_not_more_top(pt("Object"), pt("dynamic"));
        p.is_not_more_top(pt("Object?"), pt("dynamic"));
        p.is_not_more_top(pt("FutureOr<Object>"), pt("dynamic"));
        p.is_not_more_top(pt("FutureOr<Object?>"), pt("dynamic"));

        // MORETOP(T, parseType('InvalidType')) = false
        p.is_not_more_top(pt("Object"), pt("InvalidType"));
        p.is_not_more_top(pt("Object?"), pt("InvalidType"));
        p.is_not_more_top(pt("FutureOr<Object>"), pt("InvalidType"));
        p.is_not_more_top(pt("FutureOr<Object?>"), pt("InvalidType"));

        // MORETOP(Object, T) = true
        p.is_more_top(pt("Object"), pt("Object"));
        p.is_more_top(pt("Object"), pt("Object?"));
        p.is_more_top(pt("Object"), pt("FutureOr<Object>"));
        p.is_more_top(pt("Object"), pt("FutureOr<Object>?"));

        // MORETOP(T, Object) = false
        p.is_not_more_top(pt("Object?"), pt("Object"));
        p.is_not_more_top(pt("FutureOr<Object>"), pt("Object"));
        p.is_not_more_top(pt("FutureOr<Object>?"), pt("Object"));

        // MORETOP(T?, S?) = MORETOP(T, S)
        p.is_more_top(pt("Object?"), pt("Object?"));
        p.is_more_top(pt("FutureOr<void>?"), pt("FutureOr<void>?"));
        p.is_more_top(pt("FutureOr<void>?"), pt("FutureOr<dynamic>?"));
        p.is_more_top(pt("FutureOr<void>?"), pt("FutureOr<InvalidType>?"));
        p.is_more_top(pt("FutureOr<void>?"), pt("FutureOr<Object>?"));

        // MORETOP(T, S?) = true
        p.is_more_top(pt("FutureOr<Object>"), pt("FutureOr<void>?"));
        p.is_more_top(pt("FutureOr<Object>"), pt("FutureOr<dynamic>?"));
        p.is_more_top(pt("FutureOr<Object>"), pt("FutureOr<InvalidType>?"));
        p.is_more_top(pt("FutureOr<Object>"), pt("FutureOr<Object>?"));

        // MORETOP(T?, S) = false
        p.is_not_more_top(pt("FutureOr<void>?"), pt("FutureOr<Object>"));
        p.is_not_more_top(pt("FutureOr<dynamic>?"), pt("FutureOr<Object>"));
        p.is_not_more_top(pt("FutureOr<InvalidType>?"), pt("FutureOr<Object>"));
        p.is_not_more_top(pt("FutureOr<Object>?"), pt("FutureOr<Object>"));

        // MORETOP(FutureOr<T>, FutureOr<S>) = MORETOP(T, S)
        p.is_more_top(pt("FutureOr<void>"), pt("FutureOr<void>"));
        p.is_more_top(pt("FutureOr<void>"), pt("FutureOr<dynamic>"));
        p.is_more_top(pt("FutureOr<void>"), pt("FutureOr<InvalidType>"));
        p.is_more_top(pt("FutureOr<void>"), pt("FutureOr<Object>"));
        p.is_not_more_top(pt("FutureOr<dynamic>"), pt("FutureOr<void>"));
        p.is_not_more_top(pt("FutureOr<InvalidType>"), pt("FutureOr<void>"));
        p.is_not_more_top(pt("FutureOr<Object>"), pt("FutureOr<void>"));
    }

    #[test]
    fn is_null() {
        let t = TypeSystemTest::new();
        let p = P::new(&t);
        let pt = |s: &str| t.parse_type(s);

        // NULL(Null) is true
        p.is_null(pt("Null"));

        // NULL(T?) is true iff NULL(T) or BOTTOM(T)
        p.is_null(pt("Never?"));
        t.with_type_parameter_scope("T extends Never", |scope| {
            p.is_null(scope.parse_type("T?"));
        });

        // NULL(T) is false otherwise
        p.is_not_null(pt("dynamic"));
        p.is_not_null(pt("InvalidType"));
        p.is_not_null(pt("void"));

        p.is_not_null(pt("Object"));
        p.is_not_null(pt("Object?"));

        p.is_not_null(pt("int"));
        p.is_not_null(pt("int?"));

        p.is_not_null(pt("FutureOr<Null>"));

        p.is_not_null(pt("FutureOr<Null>?"));
    }

    #[test]
    fn is_object() {
        let t = TypeSystemTest::new();
        let p = P::new(&t);
        let pt = |s: &str| t.parse_type(s);

        // OBJECT(Object) is true
        p.is_object(pt("Object"));
        p.is_not_object(pt("Object?"));

        // OBJECT(FutureOr<T>) is OBJECT(T)
        p.is_object(pt("FutureOr<Object>"));
        p.is_not_object(pt("FutureOr<Object?>"));

        p.is_not_object(pt("FutureOr<Object>?"));
        p.is_not_object(pt("FutureOr<Object?>?"));

        // OBJECT(T) is false otherwise
        p.is_not_object(pt("dynamic"));
        p.is_not_object(pt("InvalidType"));
        p.is_not_object(pt("void"));
        p.is_not_object(pt("int"));
    }

    #[test]
    fn is_top() {
        let t = TypeSystemTest::new();
        let p = P::new(&t);
        let pt = |s: &str| t.parse_type(s);

        // TOP(T?) is true iff TOP(T) or OBJECT(T)
        p.is_top(pt("Object?"));
        p.is_top(pt("FutureOr<dynamic>?"));
        p.is_top(pt("FutureOr<InvalidType>?"));
        p.is_top(pt("FutureOr<void>?"));

        p.is_top(pt("FutureOr<Object>?"));
        p.is_top(pt("FutureOr<Object?>?"));

        p.is_not_top(pt("FutureOr<int>?"));
        p.is_not_top(pt("FutureOr<int?>?"));

        // TOP(dynamic) is true
        p.is_top(pt("dynamic"));
        p.is_top(pt("InvalidType"));
        assert!(t.type_system().is_top(TypeId::UNKNOWN));

        // TOP(void) is true
        p.is_top(pt("void"));

        // TOP(FutureOr<T>) is TOP(T)
        p.is_top(pt("FutureOr<dynamic>"));
        p.is_top(pt("FutureOr<InvalidType>"));
        p.is_top(pt("FutureOr<void>"));

        p.is_not_top(pt("FutureOr<Object>"));
        p.is_top(pt("FutureOr<Object?>"));

        // TOP(T) is false otherwise
        p.is_not_top(pt("Object"));

        p.is_not_top(pt("int"));
        p.is_not_top(pt("int?"));

        p.is_not_top(pt("Never"));
        p.is_not_top(pt("Never?"));
    }
}

// ------------------------------------------------------------- LowerBoundTest

mod lower_bound_test {
    use super::*;

    fn _check_greatest_lower_bound(
        t: &TypeSystemTest,
        T1: TypeId,
        T2: TypeId,
        expected: TypeId,
        check_subtype: bool,
    ) {
        let ts = t.type_system();

        let result = ts.greatest_lower_bound(T1, T2);
        expect_type(t, result, expected);

        // Check that the result is a lower bound.
        if check_subtype {
            assert!(ts.is_subtype_of(result, T1));
            assert!(ts.is_subtype_of(result, T2));
        }

        // Check for symmetry.
        let result = ts.greatest_lower_bound(T2, T1);
        expect_type(t, result, expected);
    }

    /// `_checkGreatestLowerBound(T1, T2, expected)` (`checkSubtype: true`).
    fn glb(t: &TypeSystemTest, T1: TypeId, T2: TypeId, expected: TypeId) {
        _check_greatest_lower_bound(t, T1, T2, expected, true);
    }

    /// `_checkGreatestLowerBound` with three function type specs.
    fn glb_f(t: &TypeSystemTest, T1: &str, T2: &str, expected: &str) {
        glb(
            t,
            t.parse_function_type(T1),
            t.parse_function_type(T2),
            t.parse_function_type(expected),
        );
    }

    fn _check_greatest_lower_bound2(t: &TypeSystemTest, T1: &str, T2: &str, expected: &str) {
        glb(
            t,
            t.parse_type(T1),
            t.parse_type(T2),
            t.parse_type(expected),
        );
    }

    #[test]
    fn bottom_any() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_bottom(&t, T1);
            _assert_not_bottom(&t, T2);
            glb(&t, T1, T2, T1);
        };

        check(pt("Never"), pt("Object"));
        check(pt("Never"), pt("Object?"));

        check(pt("Never"), pt("int"));
        check(pt("Never"), pt("int?"));

        check(pt("Never"), pt("List<int>"));
        check(pt("Never"), pt("List<int>?"));

        check(pt("Never"), pt("FutureOr<int>"));
        check(pt("Never"), pt("FutureOr<int>?"));

        t.with_type_parameter_scope("T extends Never", |scope| {
            let T = scope.parse_type("T");
            check(T, pt("int"));
            check(T, pt("int?"));
        });

        t.with_type_parameter_scope("T extends Object?", |scope| {
            let T = scope.parse_type("T & Never");
            check(T, pt("int"));
            check(T, pt("int?"));
        });
    }

    #[test]
    fn bottom_bottom() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_bottom(&t, T1);
            _assert_bottom(&t, T2);
            glb(&t, T1, T2, T1);
        };

        t.with_type_parameter_scope("T extends Never", |scope| {
            check(pt("Never"), scope.parse_type("T"));
        });

        t.with_type_parameter_scope("T extends Object?", |scope| {
            check(pt("Never"), scope.parse_type("T & Never"));
        });
    }

    #[test]
    fn function_type2_parameters_conflicts() {
        let t = TypeSystemTest::new();
        glb(
            &t,
            t.parse_function_type("void Function(int a)"),
            t.parse_function_type("void Function({int a})"),
            t.parse_type("Never"),
        );

        glb(
            &t,
            t.parse_function_type("void Function([int a])"),
            t.parse_function_type("void Function({int a})"),
            t.parse_type("Never"),
        );
    }

    #[test]
    fn function_type2_parameters_named() {
        let t = TypeSystemTest::new();
        glb_f(&t, "void Function()", "void Function()", "void Function()");

        {
            glb_f(
                &t,
                "void Function({int a})",
                "void Function({int a})",
                "void Function({int a})",
            );

            glb_f(
                &t,
                "void Function({int a})",
                "void Function({required int a})",
                "void Function({int a})",
            );

            glb_f(
                &t,
                "void Function({required int a})",
                "void Function({required int a})",
                "void Function({required int a})",
            );
        }

        {
            glb_f(
                &t,
                "void Function({int a, int b})",
                "void Function({int a, int c})",
                "void Function({int a, int b, int c})",
            );

            glb_f(
                &t,
                "void Function({int a, required int b})",
                "void Function({int a, required int c})",
                "void Function({int a, int b, int c})",
            );
        }

        {
            glb_f(
                &t,
                "void Function({int a})",
                "void Function({num a})",
                "void Function({num a})",
            );

            glb_f(
                &t,
                "void Function({int a})",
                "void Function({double a})",
                "void Function({num a})",
            );

            glb_f(
                &t,
                "void Function({int a})",
                "void Function({double? a})",
                "void Function({num? a})",
            );

            glb_f(
                &t,
                "void Function({int a})",
                "void Function({double a})",
                "void Function({num a})",
            );
        }
    }

    #[test]
    fn function_type2_parameters_positional() {
        let t = TypeSystemTest::new();
        glb_f(&t, "void Function()", "void Function()", "void Function()");

        glb_f(
            &t,
            "void Function(int)",
            "void Function(int)",
            "void Function(int)",
        );

        glb_f(
            &t,
            "void Function(int)",
            "void Function(num)",
            "void Function(num)",
        );

        glb_f(
            &t,
            "void Function(int)",
            "void Function(double)",
            "void Function(num)",
        );

        glb_f(
            &t,
            "void Function(int)",
            "void Function(double?)",
            "void Function(num?)",
        );

        glb_f(
            &t,
            "void Function(int)",
            "void Function(double)",
            "void Function(num)",
        );

        {
            glb_f(
                &t,
                "void Function(int)",
                "void Function([int])",
                "void Function([int])",
            );

            glb_f(
                &t,
                "void Function(int)",
                "void Function()",
                "void Function([int])",
            );

            glb_f(
                &t,
                "void Function([int])",
                "void Function([int])",
                "void Function([int])",
            );

            glb_f(
                &t,
                "void Function([int])",
                "void Function()",
                "void Function([int])",
            );
        }
    }

    #[test]
    fn function_type2_return_type() {
        let t = TypeSystemTest::new();
        glb_f(&t, "int Function()", "int Function()", "int Function()");

        glb_f(&t, "int Function()", "num Function()", "int Function()");

        glb_f(&t, "int Function()", "void Function()", "int Function()");

        glb_f(&t, "int Function()", "Never Function()", "Never Function()");
    }

    #[test]
    fn function_type2_type_parameters() {
        let t = TypeSystemTest::new();
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_nullability_none(&t, T1);
            _assert_nullability_none(&t, T2);

            _check_greatest_lower_bound(&t, T1, T2, expected, false);
        };

        check(
            t.parse_function_type("void Function<T>()"),
            t.parse_function_type("void Function()"),
            t.parse_type("Never"),
        );

        check(
            t.parse_function_type("void Function<T extends int>()"),
            t.parse_function_type("void Function<T extends num>()"),
            t.parse_type("Never"),
        );

        check(
            t.parse_function_type("T Function<T extends num>()"),
            t.parse_function_type("U Function<U extends num>()"),
            t.parse_function_type("R Function<R extends num>()"),
        );
    }

    #[test]
    fn function_type_interface_type() {
        let t = TypeSystemTest::new();
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            glb(&t, T1, T2, expected);
        };

        check(
            t.parse_function_type("void Function()"),
            t.parse_type("int"),
            t.parse_type("Never"),
        );
    }

    #[test]
    fn function_type_interface_type_function() {
        let t = TypeSystemTest::new();
        let check = |T1: TypeId| {
            _assert_nullability_none(&t, T1);
            glb(&t, T1, t.parse_type("Function"), T1);
        };

        check(t.parse_function_type("void Function()"));

        check(t.parse_function_type("int Function(num?)"));
    }

    #[test]
    fn future_or() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let future_or_function = |str: &str| {
            let result = t.parse_interface_type(str);
            assert_eq!(t.display(result), str);
            result
        };

        // DOWN(FutureOr<T1>, FutureOr<T2>) = FutureOr<S>, S = DOWN(T1, T2)
        glb(
            &t,
            pt("FutureOr<int>"),
            pt("FutureOr<num>"),
            pt("FutureOr<int>"),
        );
        glb(
            &t,
            future_or_function("FutureOr<void Function(int)>"),
            future_or_function("FutureOr<void Function(double)>"),
            future_or_function("FutureOr<void Function(num)>"),
        );

        // DOWN(FutureOr<T1>, Future<T2>) = Future<S>, S = DOWN(T1, T2)
        // DOWN(Future<T1>, FutureOr<T2>) = Future<S>, S = DOWN(T1, T2)
        glb(
            &t,
            pt("FutureOr<num>"),
            pt("Future<int>"),
            pt("Future<int>"),
        );
        glb(
            &t,
            pt("FutureOr<int>"),
            pt("Future<num>"),
            pt("Future<int>"),
        );

        // DOWN(FutureOr<T1>, T2) = S, S = DOWN(T1, T2)
        // DOWN(T1, FutureOr<T2>) = S, S = DOWN(T1, T2)
        glb(&t, pt("FutureOr<num>"), pt("int"), pt("int"));
        glb(&t, pt("FutureOr<int>"), pt("num"), pt("int"));
    }

    #[test]
    fn identical() {
        let t = TypeSystemTest::new();
        let check = |ty: TypeId| glb(&t, ty, ty, ty);

        check(t.parse_type("int"));
        check(t.parse_type("int?"));
        check(t.parse_type("List<int>"));
    }

    #[test]
    fn interface_type2() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_nullability_none(&t, T1);
            _assert_nullability_none(&t, T2);

            glb(&t, T1, T2, expected);
        };

        check(pt("int"), pt("int"), pt("int"));
        check(pt("num"), pt("int"), pt("int"));
        check(pt("double"), pt("int"), pt("Never"));

        check(pt("List<int>"), pt("List<int>"), pt("List<int>"));
        check(pt("List<num>"), pt("List<int>"), pt("List<int>"));
        check(pt("List<double>"), pt("List<int>"), pt("Never"));
    }

    #[test]
    fn interface_type2_interfaces() {
        // class A
        // class B implements A
        // class C implements B
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B implements A", "class C implements B"]),
            ..LibrarySpec::test()
        });
        glb(
            &t,
            t.parse_interface_type("A"),
            t.parse_interface_type("C"),
            t.parse_interface_type("C"),
        );
    }

    #[test]
    fn interface_type2_mixins() {
        // class A
        // class B
        // class C
        // class D extends A with B, C
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&[
                "class A",
                "class B",
                "class C",
                "class D extends A with B, C",
            ]),
            ..LibrarySpec::test()
        });
        let pi = |s: &str| t.parse_interface_type(s);
        glb(&t, pi("A"), pi("D"), pi("D"));
        glb(&t, pi("B"), pi("D"), pi("D"));
        glb(&t, pi("C"), pi("D"), pi("D"));
    }

    #[test]
    fn interface_type2_super_type() {
        // class A
        // class B extends A
        // class C extends B
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(&["class A", "class B extends A", "class C extends B"]),
            ..LibrarySpec::test()
        });
        glb(
            &t,
            t.parse_interface_type("A"),
            t.parse_interface_type("C"),
            t.parse_interface_type("C"),
        );
    }

    #[test]
    fn none_question() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_nullability_none(&t, T1);
            _assert_nullability_question(&t, T2);

            _assert_not_special(&t, T1);
            _assert_not_special(&t, T2);

            glb(&t, T1, T2, expected);
        };

        check(pt("int"), pt("int?"), pt("int"));

        check(pt("num"), pt("int?"), pt("int"));
        check(pt("int"), pt("num?"), pt("int"));

        check(pt("double"), pt("int?"), pt("Never"));
        check(pt("int"), pt("double?"), pt("Never"));
    }

    #[test]
    fn null_any() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T2: TypeId, expected: TypeId| {
            _assert_not_bottom(&t, T2);
            _assert_not_null(&t, T2);
            _assert_not_top(&t, T2);

            glb(&t, pt("Null"), T2, expected);
        };

        let check_null = |T2: TypeId| check(T2, pt("Null"));

        let check_never = |T2: TypeId| check(T2, pt("Never"));

        check_null(pt("FutureOr<Null>"));

        check_null(pt("FutureOr<Null>?"));

        check_never(pt("Object"));

        check_never(pt("int"));
        check_null(pt("int?"));

        check_never(pt("List<int>"));
        check_null(pt("List<int>?"));

        check_never(pt("List<int?>"));
        check_null(pt("List<int?>?"));
    }

    #[test]
    fn null_null() {
        let t = TypeSystemTest::new();
        let check = |T1: TypeId, T2: TypeId| {
            _assert_null(&t, T1);
            _assert_null(&t, T2);

            _assert_not_bottom(&t, T1);
            _assert_not_bottom(&t, T2);

            glb(&t, T1, T2, T1);
        };

        check(t.parse_type("Null"), t.parse_type("Null"));
    }

    #[test]
    fn object_any() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T2: TypeId, expected: TypeId| {
            _assert_not_object(&t, T2);

            glb(&t, pt("Object"), T2, expected);
        };

        let check_never = |T2: TypeId| check(T2, pt("Never"));

        check(pt("int"), pt("int"));
        check(pt("int?"), pt("int"));

        check(pt("FutureOr<int>"), pt("FutureOr<int>"));
        check(pt("FutureOr<int>?"), pt("FutureOr<int>"));
        check(pt("FutureOr<int>"), pt("FutureOr<int>"));

        check_never(pt("FutureOr<int?>"));
        check_never(pt("FutureOr<int?>?"));
        check_never(pt("FutureOr<int?>"));

        t.with_type_parameter_scope("T extends Object", |scope| {
            check(scope.parse_type("T"), scope.parse_type("T"));
            check(scope.parse_type("T?"), scope.parse_type("T"));
        });

        t.with_type_parameter_scope("T extends Object?", |scope| {
            check(scope.parse_type("T"), scope.parse_type("T & Object"));
            check(scope.parse_type("T?"), scope.parse_type("T & Object"));
        });

        t.with_type_parameter_scope("T extends FutureOr<Object?>", |scope| {
            check_never(scope.parse_type("T"));
            check_never(scope.parse_type("T?"));
        });
    }

    #[test]
    fn object_object() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_object(&t, T1);
            _assert_object(&t, T2);

            glb(&t, T1, T2, T1);
        };

        check(pt("FutureOr<Object>"), pt("Object"));

        check(pt("FutureOr<FutureOr<Object>>"), pt("FutureOr<Object>"));
    }

    #[test]
    fn question_question() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_nullability_question(&t, T1);
            _assert_nullability_question(&t, T2);

            _assert_not_special(&t, T1);
            _assert_not_special(&t, T2);

            glb(&t, T1, T2, expected);
        };

        check(pt("int?"), pt("int?"), pt("int?"));

        check(pt("num?"), pt("int?"), pt("int?"));
        check(pt("int?"), pt("num?"), pt("int?"));

        check(pt("double?"), pt("int?"), pt("Never?"));
        check(pt("int?"), pt("double?"), pt("Never?"));
    }

    #[test]
    fn record_type2_different_shape() {
        let t = TypeSystemTest::new();
        let check = |T1: &str, T2: &str| {
            _check_greatest_lower_bound2(&t, T1, T2, "Never");
        };

        check("(int,)", "(int, String)");
        check("(int,)", "({int $1})");

        check("({int f1, String f2})", "({int f1})");
        check("({int f1})", "({int f2})");
    }

    #[test]
    fn record_type2_same_shape_named() {
        let t = TypeSystemTest::new();
        _check_greatest_lower_bound2(&t, "({int f1})", "({int f1})", "({int f1})");

        _check_greatest_lower_bound2(&t, "({int f1})", "({num f1})", "({int f1})");

        _check_greatest_lower_bound2(&t, "({int f1})", "({double f1})", "({Never f1})");

        _check_greatest_lower_bound2(
            &t,
            "({int f1, double f2})",
            "({double f1, int f2})",
            "({Never f1, Never f2})",
        );
    }

    #[test]
    fn record_type2_same_shape_positional() {
        let t = TypeSystemTest::new();
        _check_greatest_lower_bound2(&t, "(int,)", "(int,)", "(int,)");
        _check_greatest_lower_bound2(&t, "(int,)", "(num,)", "(int,)");
        _check_greatest_lower_bound2(&t, "(int,)", "(double,)", "(Never,)");

        _check_greatest_lower_bound2(&t, "(int, String)", "(int, String)", "(int, String)");

        _check_greatest_lower_bound2(&t, "(int, double)", "(double, int)", "(Never, Never)");
    }

    #[test]
    fn record_type_and_not() {
        let t = TypeSystemTest::new();
        _check_greatest_lower_bound2(&t, "(int,)", "int", "Never");
        _check_greatest_lower_bound2(&t, "(int,)", "void Function()", "Never");
    }

    #[test]
    fn record_type_dart_core_record() {
        let t = TypeSystemTest::new();
        let check = |T: &str| {
            _check_greatest_lower_bound2(&t, T, "Record", T);
        };

        check("(int, String)");
        check("({int f1, String f2})");
    }

    #[test]
    fn self_() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            let types = [
                t.parse_type("dynamic"),
                t.parse_type("InvalidType"),
                t.parse_type("void"),
                t.parse_type("Never"),
                scope.parse_type("T"),
                t.parse_type("int"),
                t.parse_function_type("void Function()"),
            ];

            for ty in types {
                glb(&t, ty, ty, ty);
            }
        });
    }

    #[test]
    fn top_any() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let pf = |s: &str| t.parse_function_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_top(&t, T1);
            _assert_not_top(&t, T2);
            glb(&t, T1, T2, T2);
        };

        check(pt("void"), pt("Object"));
        check(pt("void"), pt("int"));
        check(pt("void"), pt("int?"));
        check(pt("void"), pt("List<int>"));
        check(pt("void"), pt("FutureOr<int>"));
        check(pt("void"), pt("Never"));
        check(pt("void"), pf("void Function()"));
        check(pt("void"), pt("(int, int)"));

        check(pt("dynamic"), pt("Object"));
        check(pt("dynamic"), pt("int"));
        check(pt("dynamic"), pt("int?"));
        check(pt("dynamic"), pt("List<int>"));
        check(pt("dynamic"), pt("FutureOr<int>"));
        check(pt("dynamic"), pt("Never"));
        check(pt("dynamic"), pf("void Function()"));
        check(pt("dynamic"), pt("(int, int)"));

        check(pt("InvalidType"), pt("Object"));
        check(pt("InvalidType"), pt("int"));
        check(pt("InvalidType"), pt("int?"));
        check(pt("InvalidType"), pt("List<int>"));
        check(pt("InvalidType"), pt("FutureOr<int>"));
        check(pt("InvalidType"), pt("Never"));
        check(pt("InvalidType"), pf("void Function()"));
        check(pt("InvalidType"), pt("(int, int)"));

        check(pt("Object?"), pt("Object"));
        check(pt("Object?"), pt("int"));
        check(pt("Object?"), pt("int?"));
        check(pt("Object?"), pt("List<int>"));
        check(pt("Object?"), pt("FutureOr<int>"));
        check(pt("Object?"), pt("Never"));
        check(pt("Object?"), pf("void Function()"));
        check(pt("Object?"), pt("(int, int)"));

        check(pt("FutureOr<void>"), pt("int"));
        check(pt("FutureOr<void>?"), pt("int"));
    }

    #[test]
    fn top_top() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_top(&t, T1);
            _assert_top(&t, T2);
            glb(&t, T1, T2, T2);
        };

        check(pt("void"), pt("dynamic"));
        check(pt("void"), pt("InvalidType"));
        check(pt("void"), pt("Object?"));
        check(pt("void"), pt("FutureOr<void>"));
        check(pt("void"), pt("FutureOr<dynamic>"));
        check(pt("void"), pt("FutureOr<InvalidType>"));
        check(pt("void"), pt("FutureOr<Object?>"));

        check(pt("dynamic"), pt("Object?"));
        check(pt("dynamic"), pt("FutureOr<void>"));
        check(pt("dynamic"), pt("FutureOr<dynamic>"));
        check(pt("dynamic"), pt("FutureOr<Object?>"));

        check(pt("InvalidType"), pt("Object?"));
        check(pt("InvalidType"), pt("FutureOr<void>"));
        check(pt("InvalidType"), pt("FutureOr<dynamic>"));
        check(pt("InvalidType"), pt("FutureOr<Object?>"));

        check(pt("Object?"), pt("FutureOr<void>?"));
        check(pt("Object?"), pt("FutureOr<dynamic>?"));
        check(pt("Object?"), pt("FutureOr<InvalidType>?"));
        check(pt("Object?"), pt("FutureOr<Object>?"));
        check(pt("Object?"), pt("FutureOr<Object?>?"));

        check(pt("FutureOr<void>"), pt("Object?"));
        check(pt("FutureOr<dynamic>"), pt("Object?"));
        check(pt("FutureOr<InvalidType>"), pt("Object?"));
        check(pt("FutureOr<Object?>"), pt("Object?"));

        check(pt("FutureOr<void>"), pt("FutureOr<dynamic>"));
        check(pt("FutureOr<void>"), pt("FutureOr<InvalidType>"));
        check(pt("FutureOr<void>"), pt("FutureOr<Object?>"));
        check(pt("FutureOr<dynamic>"), pt("FutureOr<Object?>"));
        check(pt("FutureOr<InvalidType>"), pt("FutureOr<Object?>"));
    }

    #[test]
    fn type_parameter() {
        let t = TypeSystemTest::new();
        let check = |bound: Option<&str>, T2: TypeId| {
            let spec = match bound {
                None => "T".to_string(),
                Some(bound) => format!("T extends {bound}"),
            };
            t.with_type_parameter_scope(&spec, |scope| {
                glb(&t, scope.parse_type("T"), T2, t.parse_type("Never"));
            });
        };

        check(None, t.parse_function_type("void Function()"));
        check(None, t.parse_type("int"));
        check(Some("num"), t.parse_type("int"));
    }
}

// ----------------------------------------------- UpperBound_FunctionTypes_Test

mod upper_bound_function_types_test {
    use super::*;

    /// `_checkLeastUpperBound` with three function type specs.
    fn lub_f(t: &TypeSystemTest, T1: &str, T2: &str, expected: &str) {
        _check_least_upper_bound(
            t,
            t.parse_function_type(T1),
            t.parse_function_type(T2),
            t.parse_function_type(expected),
        );
    }

    #[test]
    fn nested2_up_parameter_type() {
        let t = TypeSystemTest::new();
        let T1 = t.parse_function_type("void Function(void Function(String, int, int))");
        assert_eq!(
            t.display(T1),
            "void Function(void Function(String, int, int))"
        );

        let T2 = t.parse_function_type("void Function(void Function(int, double, num))");
        assert_eq!(
            t.display(T2),
            "void Function(void Function(int, double, num))"
        );

        let expected = t.parse_function_type("void Function(void Function(Object, num, num))");
        assert_eq!(
            t.display(expected),
            "void Function(void Function(Object, num, num))"
        );

        _check_least_upper_bound(&t, T1, T2, expected);
    }

    #[test]
    fn nested3_down_parameter_types() {
        let t = TypeSystemTest::new();
        let T1 =
            t.parse_function_type("void Function(void Function(void Function(String, int, int)))");
        assert_eq!(
            t.display(T1),
            "void Function(void Function(void Function(String, int, int)))"
        );

        let T2 =
            t.parse_function_type("void Function(void Function(void Function(int, double, num)))");
        assert_eq!(
            t.display(T2),
            "void Function(void Function(void Function(int, double, num)))"
        );

        let expected =
            t.parse_function_type("void Function(void Function(void Function(Never, Never, int)))");
        assert_eq!(
            t.display(expected),
            "void Function(void Function(void Function(Never, Never, int)))"
        );

        _check_least_upper_bound(&t, T1, T2, expected);
    }

    #[test]
    fn parameters_fuzzy_arrows() {
        let t = TypeSystemTest::new();
        let T1 = t.parse_function_type("void Function(dynamic)");

        let T2 = t.parse_function_type("void Function(int)");

        let expected = t.parse_function_type("void Function(int)");

        _check_least_upper_bound(&t, T1, T2, expected);
    }

    #[test]
    fn parameters_optional_named() {
        let t = TypeSystemTest::new();
        lub_f(
            &t,
            "void Function({int a})",
            "void Function()",
            "void Function()",
        );

        lub_f(
            &t,
            "void Function({int a})",
            "void Function({int b})",
            "void Function()",
        );

        lub_f(
            &t,
            "void Function({int a})",
            "void Function({int a})",
            "void Function({int a})",
        );

        lub_f(
            &t,
            "void Function({int a})",
            "void Function({int? a})",
            "void Function({int a})",
        );

        lub_f(
            &t,
            "void Function({int a, double b})",
            "void Function({int a})",
            "void Function({int a})",
        );
    }

    #[test]
    fn parameters_optional_positional() {
        let t = TypeSystemTest::new();
        lub_f(
            &t,
            "void Function([int])",
            "void Function()",
            "void Function()",
        );

        lub_f(
            &t,
            "void Function([int, double])",
            "void Function([int])",
            "void Function([int])",
        );

        lub_f(
            &t,
            "void Function([int])",
            "void Function([int])",
            "void Function([int])",
        );

        lub_f(
            &t,
            "void Function([int])",
            "void Function([int?])",
            "void Function([int])",
        );

        lub_f(
            &t,
            "void Function([int])",
            "void Function([double])",
            "void Function([Never])",
        );

        lub_f(
            &t,
            "void Function([int])",
            "void Function([num])",
            "void Function([int])",
        );

        lub_f(
            &t,
            "void Function([double, num])",
            "void Function([num, int])",
            "void Function([double, int])",
        );
    }

    #[test]
    fn parameters_required_named() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound(
            &t,
            t.parse_function_type("void Function(int a)"),
            t.parse_function_type("void Function({required int a})"),
            t.parse_type("Function"),
        );

        _check_least_upper_bound(
            &t,
            t.parse_function_type("void Function([int a])"),
            t.parse_function_type("void Function({required int a})"),
            t.parse_type("Function"),
        );

        _check_least_upper_bound(
            &t,
            t.parse_function_type("void Function({int b})"),
            t.parse_function_type("void Function({required int a})"),
            t.parse_type("Function"),
        );

        lub_f(
            &t,
            "void Function({int a})",
            "void Function({required int a})",
            "void Function({required int a})",
        );

        lub_f(
            &t,
            "void Function({int a, required int b})",
            "void Function({required int b})",
            "void Function({required int b})",
        );

        lub_f(
            &t,
            "void Function({required int a})",
            "void Function({required num a})",
            "void Function({required int a})",
        );
    }

    #[test]
    fn parameters_required_positional() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound(
            &t,
            t.parse_function_type("void Function(int)"),
            t.parse_function_type("void Function()"),
            t.parse_type("Function"),
        );

        lub_f(
            &t,
            "void Function(int)",
            "void Function(int)",
            "void Function(int)",
        );

        lub_f(
            &t,
            "void Function(int)",
            "void Function(int?)",
            "void Function(int)",
        );

        lub_f(
            &t,
            "void Function(int)",
            "void Function(double)",
            "void Function(Never)",
        );

        lub_f(
            &t,
            "void Function(int)",
            "void Function(num)",
            "void Function(int)",
        );

        lub_f(
            &t,
            "void Function(double, num)",
            "void Function(num, int)",
            "void Function(double, int)",
        );
    }

    #[test]
    fn parameters_required_positional_different_arity() {
        let t = TypeSystemTest::new();
        let T1 = t.parse_function_type("void Function(int, int)");

        let T2 = t.parse_function_type("void Function(int, int, int)");

        _check_least_upper_bound(&t, T1, T2, t.type_provider().function_type());
    }

    #[test]
    fn return_type() {
        let t = TypeSystemTest::new();
        lub_f(&t, "int Function()", "int Function()", "int Function()");
        lub_f(&t, "int Function()", "int? Function()", "int? Function()");

        lub_f(&t, "int Function()", "num Function()", "num Function()");
        lub_f(&t, "int? Function()", "num Function()", "num? Function()");

        lub_f(
            &t,
            "int Function()",
            "dynamic Function()",
            "dynamic Function()",
        );
        lub_f(
            &t,
            "int Function()",
            "InvalidType Function()",
            "InvalidType Function()",
        );
        lub_f(&t, "int Function()", "Never Function()", "int Function()");
    }

    #[test]
    fn same_type_with_named() {
        let t = TypeSystemTest::new();
        let T1 = t.parse_function_type("int Function(String, int, num, {num n})");

        let T2 = t.parse_function_type("int Function(String, int, num, {num n})");

        let expected = t.parse_function_type("int Function(String, int, num, {num n})");

        _check_least_upper_bound(&t, T1, T2, expected);
    }

    #[test]
    fn same_type_with_optional() {
        let t = TypeSystemTest::new();
        let T1 = t.parse_function_type("int Function(String, int, num, [double])");

        let T2 = t.parse_function_type("int Function(String, int, num, [double])");

        let expected = t.parse_function_type("int Function(String, int, num, [double])");

        _check_least_upper_bound(&t, T1, T2, expected);
    }

    #[test]
    fn type_parameters() {
        let t = TypeSystemTest::new();
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_nullability_none(&t, T1);
            _assert_nullability_none(&t, T2);

            _check_least_upper_bound(&t, T1, T2, expected);
        };

        check(
            t.parse_function_type("void Function<T>()"),
            t.parse_function_type("void Function()"),
            t.parse_type("Function"),
        );

        check(
            t.parse_function_type("void Function<T extends int>()"),
            t.parse_function_type("void Function<T extends num>()"),
            t.parse_type("Function"),
        );

        {
            let T1 = t.parse_function_type("T Function<T extends num>()");
            let T2 = t.parse_function_type("U Function<U extends num>()");
            {
                let result = t.type_system().least_upper_bound(T1, T2);
                let result_str = t.display(result);
                assert_eq!(result_str, "T Function<T extends num>()");
            }
            {
                let result = t.type_system().least_upper_bound(T2, T1);
                let result_str = t.display(result);
                assert_eq!(result_str, "U Function<U extends num>()");
            }
        }
    }

    #[test]
    fn unrelated() {
        let t = TypeSystemTest::new();
        let T1 = t.parse_function_type("int Function()");

        _check_least_upper_bound(&t, T1, t.parse_type("int"), t.parse_type("Object"));
        _check_least_upper_bound(&t, T1, t.parse_type("int?"), t.parse_type("Object?"));

        _check_least_upper_bound(
            &t,
            T1,
            t.parse_type("FutureOr<Function?>"),
            t.parse_type("Object?"),
        );
    }
}

// ---------------------------------------------- UpperBound_InterfaceTypes_Test

mod upper_bound_interface_types_test {
    use super::*;

    fn build(classes_: &[&str], mixins: &[&str]) -> TypeSystemTest {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(classes_),
            mixins: strs(mixins),
            ..LibrarySpec::test()
        });
        t
    }

    /// `_checkLeastUpperBound` with three interface type specs.
    fn lub_i(t: &TypeSystemTest, T1: &str, T2: &str, expected: &str) {
        _check_least_upper_bound(
            t,
            t.parse_interface_type(T1),
            t.parse_interface_type(T2),
            t.parse_interface_type(expected),
        );
    }

    /// The local `assertLUB` of the nullability tests.
    fn assert_lub(t: &TypeSystemTest, type1: TypeId, type2: TypeId, expected: TypeId) {
        let ts = t.type_system();
        // Dart: ==
        expect_type(t, ts.least_upper_bound(type1, type2), expected);
        expect_type(t, ts.least_upper_bound(type2, type1), expected);
    }

    #[test]
    fn direct_interface() {
        // class A
        // class B implements A
        // class C implements B
        let t = build(
            &["class A", "class B implements A", "class C implements B"],
            &[],
        );
        lub_i(&t, "B", "C", "B");
    }

    #[test]
    fn direct_superclass() {
        // class A
        // class B extends A
        // class C extends B
        let t = build(&["class A", "class B extends A", "class C extends B"], &[]);
        lub_i(&t, "B", "C", "B");
    }

    #[test]
    fn direct_superclass_nullability() {
        let t = build(&["class A", "class B extends A"], &[]);
        let a_question = t.parse_interface_type("A?");
        let a_none = t.parse_interface_type("A");
        let b_none_question = t.parse_interface_type("B?");
        let b_none_none = t.parse_interface_type("B");

        assert_lub(&t, b_none_question, a_question, a_question);
        assert_lub(&t, b_none_question, a_none, a_question);

        assert_lub(&t, b_none_none, a_question, a_question);
        assert_lub(&t, b_none_none, a_none, a_none);
    }

    #[test]
    fn implementations_of_comparable() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "String", "num", "Object");
    }

    #[test]
    fn mixin_and_class_constraint_and_interface() {
        let t = build(&["class A", "class B implements A"], &["mixin M on A"]);
        lub_i(&t, "B", "M", "A");
    }

    #[test]
    fn mixin_and_class_object() {
        let t = build(&["class A"], &["mixin M"]);
        _check_least_upper_bound(
            &t,
            t.parse_interface_type("A"),
            t.parse_interface_type("M"),
            t.parse_type("Object"),
        );
    }

    #[test]
    fn mixin_and_class_shared_interface() {
        let t = build(
            &["class A", "class B implements A"],
            &["mixin M implements A"],
        );
        lub_i(&t, "B", "M", "A");
    }

    #[test]
    fn same_element_nullability() {
        let t = build(&["class A"], &[]);
        let a_question = t.parse_interface_type("A?");
        let a_none = t.parse_interface_type("A");

        assert_lub(&t, a_question, a_question, a_question);
        assert_lub(&t, a_question, a_none, a_question);

        assert_lub(&t, a_none, a_question, a_question);
        assert_lub(&t, a_none, a_none, a_none);
    }

    #[test]
    fn shared_mixin1() {
        // mixin M {}
        // class B with M {}
        // class C with M {}
        let t = build(&["class B with M", "class C with M"], &["mixin M"]);
        lub_i(&t, "B", "C", "M");
    }

    #[test]
    fn shared_mixin2() {
        // mixin M1 {}
        // mixin M2 {}
        // mixin M3 {}
        // class A with M1, M2 {}
        // class B with M1, M3 {}
        let t = build(
            &["class A with M1, M2", "class B with M1, M3"],
            &["mixin M1", "mixin M2", "mixin M3"],
        );
        lub_i(&t, "A", "B", "M1");
    }

    #[test]
    fn shared_mixin3() {
        // mixin M1 {}
        // mixin M2 {}
        // mixin M3 {}
        // class A with M2, M1 {}
        // class B with M3, M1 {}
        let t = build(
            &["class A with M2, M1", "class B with M3, M1"],
            &["mixin M1", "mixin M2", "mixin M3"],
        );
        lub_i(&t, "A", "B", "M1");
    }

    #[test]
    fn shared_superclass1() {
        // class A {}
        // class B extends A {}
        // class C extends A {}
        let t = build(&["class A", "class B extends A", "class C extends A"], &[]);
        lub_i(&t, "B", "C", "A");
    }

    #[test]
    fn shared_superclass1_nullability() {
        let t = build(&["class A", "class B extends A", "class C extends A"], &[]);
        let a_question = t.parse_interface_type("A?");
        let a_none = t.parse_interface_type("A");
        let b_none_question = t.parse_interface_type("B?");
        let b_none_none = t.parse_interface_type("B");
        let c_none_question = t.parse_interface_type("C?");
        let c_none_none = t.parse_interface_type("C");

        assert_lub(&t, b_none_question, c_none_question, a_question);
        assert_lub(&t, b_none_question, c_none_none, a_question);

        assert_lub(&t, b_none_none, c_none_question, a_question);
        assert_lub(&t, b_none_none, c_none_none, a_none);
    }

    #[test]
    fn shared_superclass2() {
        // class A {}
        // class B extends A {}
        // class C extends A {}
        // class D extends C {}
        let t = build(
            &[
                "class A",
                "class B extends A",
                "class C extends A",
                "class D extends C",
            ],
            &[],
        );
        lub_i(&t, "B", "D", "A");
    }

    #[test]
    fn shared_superclass3() {
        // class A {}
        // class B extends A {}
        // class C extends B {}
        // class D extends B {}
        let t = build(
            &[
                "class A",
                "class B extends A",
                "class C extends B",
                "class D extends B",
            ],
            &[],
        );
        lub_i(&t, "C", "D", "B");
    }

    #[test]
    fn shared_superclass4() {
        // class A {}
        // class A2 {}
        // class A3 {}
        // class B extends A implements A2 {}
        // class C extends A implement A3 {}
        let t = build(
            &[
                "class A",
                "class A2",
                "class A3",
                "class B extends A implements A2",
                "class C extends A implements A3",
            ],
            &[],
        );
        lub_i(&t, "B", "C", "A");
    }

    #[test]
    fn shared_superinterface1() {
        // class A {}
        // class B implements A {}
        // class C implements A {}
        let t = build(
            &["class A", "class B implements A", "class C implements A"],
            &[],
        );
        lub_i(&t, "B", "C", "A");
    }

    #[test]
    fn shared_superinterface2() {
        // class A {}
        // class B implements A {}
        // class C implements A {}
        // class D implements C {}
        let t = build(
            &[
                "class A",
                "class B implements A",
                "class C implements A",
                "class D implements C",
            ],
            &[],
        );
        lub_i(&t, "B", "D", "A");
    }

    #[test]
    fn shared_superinterface3() {
        // class A {}
        // class B implements A {}
        // class C implements B {}
        // class D implements B {}
        let t = build(
            &[
                "class A",
                "class B implements A",
                "class C implements B",
                "class D implements B",
            ],
            &[],
        );
        lub_i(&t, "C", "D", "B");
    }

    #[test]
    fn shared_superinterface4() {
        // class A {}
        // class A2 {}
        // class A3 {}
        // class B implements A, A2 {}
        // class C implements A, A3 {}
        let t = build(
            &[
                "class A",
                "class A2",
                "class A3",
                "class B implements A, A2",
                "class C implements A, A3",
            ],
            &[],
        );
        lub_i(&t, "B", "C", "A");
    }
}

// ------------------------------------------------- UpperBound_RecordTypes_Test

mod upper_bound_record_types_test {
    use super::*;

    #[test]
    fn different_shape() {
        let t = TypeSystemTest::new();
        let check = |T1: &str, T2: &str| {
            _check_least_upper_bound2(&t, T1, T2, "Record");
        };

        check("(int,)", "(int, String)");
        check("(int,)", "({int $1})");

        check("({int f1, String f2})", "({int f1})");
        check("({int f1})", "({int f2})");
    }

    #[test]
    fn never() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "(int,)", "Never", "(int,)");
    }

    #[test]
    fn record_and_not() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "(int,)", "int", "Object");
        _check_least_upper_bound2(&t, "(int,)", "void Function()", "Object");
    }

    #[test]
    fn record_dart_core_record() {
        let t = TypeSystemTest::new();
        let check = |T1: &str| {
            _check_least_upper_bound2(&t, T1, "Record", "Record");
        };

        check("(int, String)");
        check("({int f1, String f2})");
    }

    #[test]
    fn same_shape_named() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "({int f1})", "({int f1})", "({int f1})");

        _check_least_upper_bound2(&t, "({int f1})", "({num f1})", "({num f1})");

        _check_least_upper_bound2(&t, "({int f1})", "({double f1})", "({num f1})");

        _check_least_upper_bound2(
            &t,
            "({int f1, double f2})",
            "({double f1, int f2})",
            "({num f1, num f2})",
        );
    }

    #[test]
    fn same_shape_positional() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "(int,)", "(int,)", "(int,)");
        _check_least_upper_bound2(&t, "(int,)", "(num,)", "(num,)");
        _check_least_upper_bound2(&t, "(int,)", "(double,)", "(num,)");

        _check_least_upper_bound2(&t, "(int, String)", "(int, String)", "(int, String)");

        _check_least_upper_bound2(&t, "(int, double)", "(double, int)", "(num, num)");
    }

    #[test]
    fn top() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "(int,)", "dynamic", "dynamic");
        _check_least_upper_bound2(&t, "(int,)", "Object?", "Object?");
    }
}

// -------------------------------------------------------------- UpperBoundTest

mod upper_bound_test {
    use super::*;

    fn build_extension_types(specs: &[&str]) -> TypeSystemTest {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            extension_types: strs(specs),
            ..LibrarySpec::test()
        });
        t
    }

    fn build_classes(specs: &[&str]) -> TypeSystemTest {
        let mut t = TypeSystemTest::new();
        t.build_test_library(LibrarySpec {
            classes: classes(specs),
            ..LibrarySpec::test()
        });
        t
    }

    #[test]
    fn bottom_any() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_bottom(&t, T1);
            _assert_not_bottom(&t, T2);
            _check_least_upper_bound(&t, T1, T2, T2);
        };

        check(pt("Never"), pt("dynamic"));
        check(pt("Never"), pt("InvalidType"));

        check(pt("Never"), pt("Object"));
        check(pt("Never"), pt("Object?"));

        check(pt("Never"), pt("int"));
        check(pt("Never"), pt("int?"));

        check(pt("Never"), pt("List<int>"));
        check(pt("Never"), pt("List<int>?"));

        check(pt("Never"), pt("FutureOr<int>"));
        check(pt("Never"), pt("FutureOr<int>?"));

        check(pt("Never"), t.parse_function_type("void Function()"));
        check(pt("Never"), t.parse_function_type("void Function()?"));

        {
            t.with_type_parameter_scope("T", |scope| {
                check(pt("Never"), scope.parse_type("T"));
                check(pt("Never"), scope.parse_type("T?"));
            });
        }

        {
            t.with_type_parameter_scope("T extends Never", |scope| {
                let T = scope.parse_type("T");
                check(T, pt("int"));
                check(T, pt("int?"));
            });
        }

        {
            t.with_type_parameter_scope("T extends Object?", |scope| {
                let T = scope.parse_type("T & Never");
                check(T, pt("int"));
                check(T, pt("int?"));
            });
        }
    }

    #[test]
    fn bottom_bottom() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_bottom(&t, T1);
            _assert_bottom(&t, T2);
            _check_least_upper_bound(&t, T1, T2, T2);
        };

        t.with_type_parameter_scope("T extends Never", |scope| {
            check(pt("Never"), scope.parse_type("T"));
        });

        t.with_type_parameter_scope("T extends Object?", |scope| {
            check(pt("Never"), scope.parse_type("T & Never"));
        });
    }

    #[test]
    fn extension_type_implement_extension_type_implicit_object_question() {
        // extension type A(Object?) {}
        // extension type B(Object?) implements A {}
        // extension type C(Object?) implements A {}
        let t = build_extension_types(&[
            "extension type A(Object? it)",
            "extension type B(Object? it) implements A",
            "extension type C(Object? it) implements A",
        ]);
        _check_least_upper_bound(
            &t,
            t.parse_interface_type("B"),
            t.parse_interface_type("C"),
            t.parse_interface_type("A"),
        );
    }

    #[test]
    fn extension_type_no_type_parameters_interfaces() {
        // extension type A(int) implements int {}
        // extension type B(double) implements double {}
        let t = build_extension_types(&[
            "extension type A(int it) implements int",
            "extension type B(double it) implements double",
        ]);
        _check_least_upper_bound(
            &t,
            t.parse_interface_type("A"),
            t.parse_interface_type("B"),
            t.parse_type("num"),
        );
    }

    #[test]
    fn extension_type_no_type_parameters_no_interfaces() {
        // extension type A(int) {}
        // extension type B(double) {}
        let t = build_extension_types(&["extension type A(int it)", "extension type B(double it)"]);
        _check_least_upper_bound(
            &t,
            t.parse_interface_type("A"),
            t.parse_interface_type("B"),
            t.parse_type("Object?"),
        );
    }

    #[test]
    fn extension_type_with_type_parameters_object_none() {
        let t = build_extension_types(&[
            "extension type A<T>(T it) implements Object?",
            "extension type B<T>(T it) implements Object?",
        ]);
        _check_least_upper_bound(
            &t,
            t.parse_interface_type("A<String>"),
            t.parse_interface_type("B<num>"),
            t.parse_type("Object"),
        );
    }

    #[test]
    fn extension_type_with_type_parameters_with_interfaces() {
        let t = build_extension_types(&[
            "extension type E<T>(T it) implements Object?",
            "extension type A<T1 extends String>(T1 it) implements E<T1>, String",
            "extension type B<T2 extends int>(T2 it) implements E<T2?>, num",
        ]);
        // A<T1> implements E<T1>, String
        // B<T2> implements E<T2?>, num
        _check_least_upper_bound(
            &t,
            t.parse_interface_type("A<String>"),
            t.parse_interface_type("B<num>"),
            t.parse_type("Object"),
        );
    }

    #[test]
    fn function_type_interface_type() {
        let t = TypeSystemTest::new();
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _check_least_upper_bound(&t, T1, T2, expected);
        };

        check(
            t.parse_function_type("void Function()"),
            t.parse_type("int"),
            t.parse_type("Object"),
        );
    }

    #[test]
    fn function_type_interface_type_function() {
        let t = TypeSystemTest::new();
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _check_least_upper_bound(&t, T1, T2, expected);
        };

        let check_none = |T1: TypeId| {
            _assert_nullability_none(&t, T1);
            check(T1, t.parse_type("Function"), t.parse_type("Function"));
        };

        check_none(t.parse_function_type("void Function()"));

        check_none(t.parse_function_type("int Function(num?)"));

        check(
            t.parse_function_type("void Function()?"),
            t.parse_type("Function"),
            t.parse_type("Function?"),
        );
    }

    /// `UP(Future<T1>, FutureOr<T2>) = FutureOr<T3> where T3 = UP(T1, T2)`
    /// `UP(FutureOr<T1>, Future<T2>) = FutureOr<T3> where T3 = UP(T1, T2)`
    #[test]
    fn future_or_future() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "Future<int>", "FutureOr<double>", "FutureOr<num>");

        _check_least_upper_bound2(&t, "Future<int>", "FutureOr<String>", "FutureOr<Object>");
    }

    /// `UP(FutureOr<T1>, FutureOr<T2>) = FutureOr<T3> where T3 = UP(T1, T2)`
    #[test]
    fn future_or_future_or() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "FutureOr<int>", "FutureOr<double>", "FutureOr<num>");

        _check_least_upper_bound2(&t, "FutureOr<int>", "FutureOr<String>", "FutureOr<Object>");
    }

    /// `UP(T1, FutureOr<T2>) = FutureOr<T3> where T3 = UP(T1, T2)`
    /// `UP(FutureOr<T1>, T2) = FutureOr<T3> where T3 = UP(T1, T2)`
    #[test]
    fn future_or_other() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "FutureOr<int>", "double", "FutureOr<num>");

        _check_least_upper_bound2(&t, "FutureOr<int>", "String", "FutureOr<Object>");
    }

    #[test]
    fn identical() {
        let t = TypeSystemTest::new();
        let check = |ty: TypeId| _check_least_upper_bound(&t, ty, ty, ty);

        check(t.parse_type("int"));
        check(t.parse_type("int?"));
        check(t.parse_type("List<int>"));
    }

    #[test]
    fn interface_type_function_type() {
        let t = build_classes(&["class A"]);
        _check_least_upper_bound(
            &t,
            t.parse_interface_type("A"),
            t.parse_function_type("void Function()"),
            t.parse_type("Object"),
        );
    }

    #[test]
    fn none_question() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_nullability_none(&t, T1);
            _assert_nullability_question(&t, T2);

            _assert_not_special(&t, T1);
            _assert_not_special(&t, T2);

            _check_least_upper_bound(&t, T1, T2, expected);
        };

        check(pt("double"), pt("int?"), pt("num?"));
        check(pt("num"), pt("double?"), pt("num?"));
        check(pt("num"), pt("int?"), pt("num?"));
    }

    #[test]
    fn null_any() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_null(&t, T1);
            _assert_not_null(&t, T2);

            _assert_not_top(&t, T1);
            _assert_not_top(&t, T2);

            _assert_not_bottom(&t, T1);
            _assert_not_bottom(&t, T2);

            _check_least_upper_bound(&t, T1, T2, expected);
        };

        check(pt("Null"), pt("Object"), pt("Object?"));

        check(pt("Null"), pt("int"), pt("int?"));
        check(pt("Null"), pt("int?"), pt("int?"));

        check(pt("Null"), pt("List<int>"), pt("List<int>?"));
        check(pt("Null"), pt("List<int>?"), pt("List<int>?"));

        check(pt("Null"), pt("FutureOr<int>"), pt("FutureOr<int>?"));
        check(pt("Null"), pt("FutureOr<int>?"), pt("FutureOr<int>?"));

        check(pt("Null"), pt("FutureOr<int?>"), pt("FutureOr<int?>"));
        check(pt("Null"), pt("FutureOr<int?>?"), pt("FutureOr<int?>?"));

        check(
            pt("Null"),
            t.parse_function_type("int Function()"),
            t.parse_function_type("int Function()?"),
        );
    }

    #[test]
    fn null_null() {
        let t = TypeSystemTest::new();
        let check = |T1: TypeId, T2: TypeId| {
            _assert_null(&t, T1);
            _assert_null(&t, T2);

            _assert_not_bottom(&t, T1);
            _assert_not_bottom(&t, T2);

            _check_least_upper_bound(&t, T1, T2, T2);
        };

        check(t.parse_type("Null"), t.parse_type("Null"));
    }

    #[test]
    fn object_any() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_object(&t, T1);
            _assert_not_object(&t, T2);

            _check_least_upper_bound(&t, T1, T2, expected);
        };

        check(pt("Object"), pt("int"), pt("Object"));
        check(pt("Object"), pt("int?"), pt("Object?"));

        check(pt("Object"), pt("FutureOr<int?>"), pt("Object?"));

        check(pt("FutureOr<Object>"), pt("int"), pt("FutureOr<Object>"));
        check(pt("FutureOr<Object>"), pt("int?"), pt("FutureOr<Object>?"));
    }

    #[test]
    fn object_object() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_object(&t, T1);
            _assert_object(&t, T2);

            _check_least_upper_bound(&t, T1, T2, T2);
        };

        check(pt("FutureOr<Object>"), pt("Object"));

        check(pt("FutureOr<FutureOr<Object>>"), pt("FutureOr<Object>"));
    }

    #[test]
    fn question_question() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_nullability_question(&t, T1);
            _assert_nullability_question(&t, T2);

            _assert_not_special(&t, T1);
            _assert_not_special(&t, T2);

            _check_least_upper_bound(&t, T1, T2, expected);
        };

        check(pt("double?"), pt("int?"), pt("num?"));
        check(pt("num?"), pt("double?"), pt("num?"));
        check(pt("num?"), pt("int?"), pt("num?"));
    }

    #[test]
    fn top_any() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_top(&t, T1);
            _assert_not_top(&t, T2);
            _check_least_upper_bound(&t, T1, T2, T1);
        };

        let check2 = |T1: TypeId| {
            check(T1, pt("Object"));
            check(T1, pt("int"));
            check(T1, pt("int?"));
            check(T1, pt("List<int>"));
            check(T1, pt("FutureOr<int>"));
            check(T1, t.parse_function_type("void Function()"));

            t.with_type_parameter_scope("T", |scope| {
                check(T1, scope.parse_type("T"));
                check(T1, scope.parse_type("T?"));
            });
        };

        check2(pt("void"));
        check2(pt("dynamic"));
        check2(pt("InvalidType"));
        check2(pt("Object?"));

        check2(pt("FutureOr<void>"));
        check2(pt("FutureOr<void>?"));
    }

    #[test]
    fn top_top() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId| {
            _assert_top(&t, T1);
            _assert_top(&t, T2);
            _check_least_upper_bound(&t, T1, T2, T1);
        };

        check(pt("void"), pt("dynamic"));
        check(pt("void"), pt("InvalidType"));
        check(pt("void"), pt("Object?"));
        check(pt("void"), pt("FutureOr<void>"));
        check(pt("void"), pt("FutureOr<dynamic>"));
        check(pt("void"), pt("FutureOr<InvalidType>"));
        check(pt("void"), pt("FutureOr<Object?>"));

        check(pt("dynamic"), pt("Object?"));
        check(pt("dynamic"), pt("FutureOr<void>"));
        check(pt("dynamic"), pt("FutureOr<dynamic>"));
        check(pt("dynamic"), pt("FutureOr<Object?>"));

        check(pt("InvalidType"), pt("Object?"));
        check(pt("InvalidType"), pt("FutureOr<void>"));
        check(pt("InvalidType"), pt("FutureOr<dynamic>"));
        check(pt("InvalidType"), pt("FutureOr<Object?>"));

        check(pt("Object?"), pt("FutureOr<void>?"));
        check(pt("Object?"), pt("FutureOr<dynamic>?"));
        check(pt("Object?"), pt("FutureOr<InvalidType>?"));
        check(pt("Object?"), pt("FutureOr<Object>?"));
        check(pt("Object?"), pt("FutureOr<Object?>?"));

        check(pt("FutureOr<void>"), pt("Object?"));
        check(pt("FutureOr<dynamic>"), pt("Object?"));
        check(pt("FutureOr<InvalidType>"), pt("Object?"));
        check(pt("FutureOr<Object?>"), pt("Object?"));

        check(pt("FutureOr<void>"), pt("FutureOr<dynamic>"));
        check(pt("FutureOr<void>"), pt("FutureOr<InvalidType>"));
        check(pt("FutureOr<void>"), pt("FutureOr<Object?>"));
        check(pt("FutureOr<dynamic>"), pt("FutureOr<Object?>"));
        check(pt("FutureOr<InvalidType>"), pt("FutureOr<Object?>"));
    }

    #[test]
    fn type_parameter_bound() {
        let t = TypeSystemTest::new();
        let pt = |s: &str| t.parse_type(s);
        let check = |T1: TypeId, T2: TypeId, expected: TypeId| {
            _assert_nullability_none(&t, T1);
            _assert_nullability_none(&t, T2);

            _assert_not_special(&t, T1);
            _assert_not_special(&t, T2);

            _check_least_upper_bound(&t, T1, T2, expected);
        };

        t.with_type_parameter_scope("T extends int", |scope| {
            check(scope.parse_type_parameter_type("T"), pt("num"), pt("num"));
        });

        t.with_type_parameter_scope("T extends int, U extends num", |scope| {
            check(
                scope.parse_type_parameter_type("T"),
                scope.parse_type_parameter_type("U"),
                pt("num"),
            );
        });

        t.with_type_parameter_scope("T extends int, U extends num?", |scope| {
            check(
                scope.parse_type_parameter_type("T"),
                scope.parse_type_parameter_type("U"),
                pt("num?"),
            );
        });

        t.with_type_parameter_scope("T extends int?, U extends num", |scope| {
            check(
                scope.parse_type_parameter_type("T"),
                scope.parse_type_parameter_type("U"),
                pt("num?"),
            );
        });

        t.with_type_parameter_scope("T extends num, U extends T", |scope| {
            let T = scope.parse_type_parameter_type("T");
            check(T, scope.parse_type_parameter_type("U"), T);
        });
    }

    #[test]
    fn type_parameter_f_bounded() {
        // class A<T> {}
        let t = build_classes(&["class A<T>"]);

        t.with_type_parameter_scope("S, U", |scope| {
            let ctx = t.ctx();
            let S = scope.type_parameter("S");
            ctx.get(S).bound.set(Some(scope.parse_type("A<S>")));

            let U = scope.type_parameter("U");
            ctx.get(U).bound.set(Some(scope.parse_type("A<U>")));

            _check_least_upper_bound(
                &t,
                scope.parse_type("S"),
                scope.parse_type("U"),
                t.parse_type("A<Object?>"),
            );
        });
    }

    #[test]
    fn type_parameter_function_bounded() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Function", |scope| {
            _check_least_upper_bound(
                &t,
                scope.parse_type("T"),
                t.parse_function_type("void Function()"),
                t.type_provider().function_type(),
            );
        });
    }

    #[test]
    fn type_parameter_function_no_bound() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            _check_least_upper_bound(
                &t,
                scope.parse_type("T"),
                t.parse_function_type("void Function()"),
                t.parse_type("Object?"),
            );
        });
    }

    #[test]
    fn type_parameter_greatest_closure_function_bounded() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends void Function(T)", |scope| {
            _check_least_upper_bound(
                &t,
                scope.parse_type("T"),
                t.parse_function_type("void Function(Null)"),
                t.parse_function_type("void Function(Never)"),
            );
        });
    }

    #[test]
    fn type_parameter_greatest_closure_function_promoted() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            _check_least_upper_bound(
                &t,
                scope.parse_type("T & void Function(T)"),
                t.parse_function_type("void Function(Null)"),
                t.parse_function_type("void Function(Never)"),
            );
        });
    }

    #[test]
    fn type_parameter_interface_bounded() {
        let t = build_classes(&["class A", "class B extends A", "class C extends A"]);
        t.with_type_parameter_scope("T extends B", |scope| {
            _check_least_upper_bound(
                &t,
                scope.parse_type("T"),
                t.parse_interface_type("C"),
                t.parse_interface_type("A"),
            );
        });
    }

    #[test]
    fn type_parameter_interface_bounded_object_question() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T extends Object?", |scope| {
            _check_least_upper_bound(
                &t,
                scope.parse_type("T"),
                t.parse_type("int"),
                t.parse_type("Object?"),
            );
        });
    }

    #[test]
    fn type_parameter_interface_no_bound() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("T", |scope| {
            _check_least_upper_bound(
                &t,
                scope.parse_type("T"),
                t.parse_type("int"),
                t.parse_type("Object?"),
            );
        });
    }

    #[test]
    fn type_parameter_intersection_basic() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("X extends num?, Y extends X", |scope| {
            let X_none = scope.parse_type("X");
            let Y_none = scope.parse_type("Y");
            let X_none_promoted = scope.parse_type("X & num");

            // `UP(X & num, Y) == X`, because `Y <: X`.
            _check_least_upper_bound(&t, X_none_promoted, Y_none, X_none);

            // `UP(X & num, num?) == num?`, because `X <: num?`.
            _check_least_upper_bound(
                &t,
                X_none_promoted,
                t.parse_type("num?"),
                t.parse_type("num?"),
            );

            // `UP(X & num, String) == Object`.
            _check_least_upper_bound(
                &t,
                X_none_promoted,
                t.parse_type("String"),
                t.parse_type("Object"),
            );
        });
    }

    #[test]
    fn type_parameter_intersection_fbounded() {
        // `X`, `class C<X> {}`, `Y extends C<Y>?`, `Y & C<Y>`.
        let t = build_classes(&["class C<X>"]);
        t.with_type_parameter_scope("Y", |scope| {
            let Y = scope.type_parameter("Y");
            t.ctx().get(Y).bound.set(Some(scope.parse_type("C<Y>?")));

            // `UP(Y & C<Y>, C<Never>) == C<Object?>`.
            _check_least_upper_bound(
                &t,
                scope.parse_type("Y & C<Y>"),
                t.parse_interface_type("C<Never>"),
                t.parse_interface_type("C<Object?>"),
            );
        });
    }

    #[test]
    fn type_parameter_intersection_null() {
        let t = TypeSystemTest::new();
        t.with_type_parameter_scope("X", |scope| {
            // UP(X & num?, Null) == num?
            _check_least_upper_bound(
                &t,
                scope.parse_type("X & num?"),
                t.parse_type("Null"),
                t.parse_type("num?"),
            );

            // UP(X & num, Null) == num?
            _check_least_upper_bound(
                &t,
                scope.parse_type("X & num"),
                t.parse_type("Null"),
                t.parse_type("num?"),
            );
        });
    }

    /// `_checkLeastUpperBound` with three interface type specs.
    fn lub_i(t: &TypeSystemTest, T1: &str, T2: &str, expected: &str) {
        _check_least_upper_bound(
            t,
            t.parse_interface_type(T1),
            t.parse_interface_type(T2),
            t.parse_interface_type(expected),
        );
    }

    #[test]
    fn type_parameters_contravariant_different() {
        // class A<in T>
        let t = build_classes(&["class A<in T>"]);
        lub_i(&t, "A<int>", "A<num>", "A<int>");
    }

    #[test]
    fn type_parameters_contravariant_same() {
        // class A<in T>
        let t = build_classes(&["class A<in T>"]);
        lub_i(&t, "A<num>", "A<num>", "A<num>");
    }

    #[test]
    fn type_parameters_covariant_different() {
        // class A<out T>
        let t = build_classes(&["class A<out T>"]);
        lub_i(&t, "A<int>", "A<num>", "A<num>");
    }

    #[test]
    fn type_parameters_covariant_same() {
        // class A<out T>
        let t = build_classes(&["class A<out T>"]);
        lub_i(&t, "A<num>", "A<num>", "A<num>");
    }

    #[test]
    fn type_parameters_invariant_object() {
        // class A<inout T>
        let t = build_classes(&["class A<inout T>"]);
        _check_least_upper_bound(
            &t,
            t.parse_interface_type("A<num>"),
            t.parse_interface_type("A<int>"),
            t.parse_type("Object"),
        );
    }

    #[test]
    fn type_parameters_invariant_same() {
        // class A<inout T>
        let t = build_classes(&["class A<inout T>"]);
        lub_i(&t, "A<num>", "A<num>", "A<num>");
    }

    #[test]
    fn type_parameters_multi_basic() {
        // class A<out T, inout U, in V>
        let t = build_classes(&["class A<out T, inout U, in V>"]);
        lub_i(
            &t,
            "A<num, num, num>",
            "A<int, num, int>",
            "A<num, num, int>",
        );
    }

    #[test]
    fn type_parameters_multi_object_interface() {
        // class A<out T, inout U, in V>
        let t = build_classes(&["class A<out T, inout U, in V>"]);
        _check_least_upper_bound(
            &t,
            t.parse_interface_type("A<num, String, num>"),
            t.parse_interface_type("A<int, num, int>"),
            t.parse_type("Object"),
        );
    }

    #[test]
    fn type_parameters_multi_object_type() {
        // class A<out T, inout U, in V>
        let t = build_classes(&["class A<out T, inout U, in V>"]);
        lub_i(
            &t,
            "A<String, num, num>",
            "A<int, num, int>",
            "A<Object, num, int>",
        );
    }

    /// Check least upper bound of the same class with different type parameters.
    #[test]
    fn type_parameters_no_variance_different() {
        let t = TypeSystemTest::new();
        _check_least_upper_bound2(&t, "List<int>", "List<double>", "List<num>");
    }

    #[test]
    fn type_parameters_no_variance_same() {
        let t = TypeSystemTest::new();
        let list_of_int = t.parse_type("List<int>");
        _check_least_upper_bound(&t, list_of_int, list_of_int, list_of_int);
    }
}
