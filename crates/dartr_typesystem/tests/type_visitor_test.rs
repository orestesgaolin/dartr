// Dart source: pkg/analyzer/test/src/dart/element/type_visitor_test.dart

//! `RecursiveTypeVisitorTest`.

use dartr_element::{Ctx, DisplayOptions, TypeId};
use dartr_typesystem::TypeExt;
use dartr_typesystem::test_support::*;
use dartr_typesystem::type_visitor::{RecursiveTypeVisitor, super_visit_interface_type};

/// `_MockRecursiveVisitor`.
struct MockRecursiveVisitor<'a> {
    ctx: Ctx<'a>,
    /// Dart `Set<DartType>`: the set uses `==` (structural equality); see
    /// [`MockRecursiveVisitor::contains`].
    visited_types: Vec<TypeId>,
    stop_on_type: Option<TypeId>,
}

impl<'a> MockRecursiveVisitor<'a> {
    fn new(ctx: Ctx<'a>) -> Self {
        MockRecursiveVisitor {
            ctx,
            visited_types: Vec::new(),
            stop_on_type: None,
        }
    }

    fn display(&self, t: TypeId) -> String {
        dartr_element::type_display_string_with(&self.ctx, t, DisplayOptions::default())
    }

    // Dart: `Set.contains` uses `==`.
    fn contains(&self, t: TypeId) -> bool {
        self.visited_types.iter().any(|&v| self.ctx.dart_eq(v, t))
    }

    fn add(&mut self, t: TypeId) {
        if !self.contains(t) {
            self.visited_types.push(t);
        }
    }

    // Dart: `type != stopOnType` uses `==`.
    fn is_stop_type(&self, t: TypeId) -> bool {
        self.stop_on_type.is_some_and(|s| self.ctx.dart_eq(t, s))
    }

    fn assert_not_visited_type(&self, t: TypeId) {
        assert!(!self.contains(t), "visited: {}", self.display(t));
    }

    fn assert_not_visited_types(&self, types: &[TypeId]) {
        for &t in types {
            self.assert_not_visited_type(t);
        }
    }

    fn assert_visited_type(&self, t: TypeId) {
        assert!(self.contains(t), "not visited: {}", self.display(t));
    }

    fn assert_visited_types(&self, types: &[TypeId]) {
        for &t in types {
            self.assert_visited_type(t);
        }
    }
}

impl<'a> RecursiveTypeVisitor<'a> for MockRecursiveVisitor<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }

    // `super(includeTypeAliasArguments: false)`.
    fn include_type_alias_arguments(&self) -> bool {
        false
    }

    fn visit_dart_type(&mut self, t: TypeId) -> bool {
        self.add(t);
        !self.is_stop_type(t)
    }

    fn visit_interface_type(&mut self, t: TypeId) -> bool {
        self.add(t);
        if self.is_stop_type(t) {
            return false;
        }
        super_visit_interface_type(self, t)
    }
}

const COMPLEX: &str = "dynamic Function<T extends int, K extends String>(\
num, double, {void c, Object d})";

#[test]
fn calls_default_behavior() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    assert!(visitor.visit(t.parse_type("int")));
    visitor.assert_visited_type(t.parse_type("int"));
}

#[test]
fn function_type_complex() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_function_type(COMPLEX);
    assert!(visitor.visit(ty));
    visitor.assert_visited_types(&[
        t.parse_type("dynamic"),
        t.parse_type("int"),
        t.parse_type("String"),
        t.parse_type("num"),
        t.parse_type("double"),
        t.parse_type("void"),
        t.parse_type("Object"),
    ]);
}

#[test]
fn function_type_positional_parameter() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_function_type("dynamic Function([int])");
    assert!(visitor.visit(ty));
    visitor.assert_visited_type(t.parse_type("int"));
}

#[test]
fn function_type_return_type() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_function_type("int Function()");
    assert!(visitor.visit(ty));
    visitor.assert_visited_type(t.parse_type("int"));
}

#[test]
fn function_type_type_formal_bound() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_function_type("dynamic Function<T extends int>()");
    assert!(visitor.visit(ty));
    visitor.assert_visited_types(&[t.parse_type("dynamic"), t.parse_type("int")]);
}

#[test]
fn function_type_type_formal_no_bound() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_function_type("dynamic Function<T>()");
    assert!(visitor.visit(ty));
    visitor.assert_visited_type(t.parse_type("dynamic"));
}

#[test]
fn interface_type_type_parameter() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_type("List<int>");
    assert!(visitor.visit(ty));
    visitor.assert_visited_type(t.parse_type("int"));
}

#[test]
fn interface_type_type_parameters() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_type("Map<int, String>");
    assert!(visitor.visit(ty));
    visitor.assert_visited_types(&[t.parse_type("int"), t.parse_type("String")]);
}

#[test]
fn interface_type_type_parameters_nested() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_type("List<List<int>>");
    assert!(visitor.visit(ty));
    visitor.assert_visited_type(t.parse_type("int"));
}

#[test]
fn record_type_named() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_record_type("({int f1, double f2})");
    assert!(visitor.visit(ty));
    visitor.assert_visited_type(t.parse_type("int"));
    visitor.assert_visited_type(t.parse_type("double"));
}

#[test]
fn record_type_named_dollar_identifier() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_record_type("({int $1})");
    assert!(visitor.visit(ty));
    visitor.assert_visited_type(t.parse_type("int"));
}

#[test]
fn record_type_positional() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_record_type("(int, double)");
    assert!(visitor.visit(ty));
    visitor.assert_visited_type(t.parse_type("int"));
    visitor.assert_visited_type(t.parse_type("double"));
}

#[test]
fn stop_visiting_first() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_function_type(COMPLEX);
    visitor.stop_on_type = Some(t.parse_type("dynamic"));
    assert!(!visitor.visit(ty));
    visitor.assert_not_visited_types(&[
        t.parse_type("int"),
        t.parse_type("String"),
        t.parse_type("num"),
        t.parse_type("double"),
        t.parse_type("void"),
        t.parse_type("Object"),
    ]);
}

#[test]
fn stop_visiting_halfway() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_function_type(COMPLEX);
    visitor.stop_on_type = Some(t.parse_type("int"));
    assert!(!visitor.visit(ty));
    visitor.assert_not_visited_types(&[
        t.parse_type("String"),
        t.parse_type("void"),
        t.parse_type("Object"),
    ]);
}

#[test]
fn stop_visiting_nested() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let outer_list = t.parse_type("List<Map<int, String>>");
    visitor.stop_on_type = Some(t.parse_type("int"));
    assert!(!visitor.visit(outer_list));
    visitor.assert_not_visited_type(t.parse_type("String"));
}

#[test]
fn stop_visiting_nested_parent() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let outer_type = t.parse_type("Map<List<int>, List<String>>");
    visitor.stop_on_type = Some(t.parse_type("int"));
    assert!(!visitor.visit(outer_type));
    visitor.assert_not_visited_type(t.parse_type("String"));
}

#[test]
fn stop_visiting_type_parameters() {
    let t = TypeSystemTest::new();
    let mut visitor = MockRecursiveVisitor::new(t.ctx());
    let ty = t.parse_type("Map<int, String>");
    visitor.stop_on_type = Some(t.parse_type("int"));
    assert!(!visitor.visit(ty));
    visitor.assert_visited_type(t.parse_type("int"));
    visitor.assert_not_visited_type(t.parse_type("String"));
}
