// Dart source: pkg/_fe_analyzer_shared/test/type_inference/variable_bindings_test.dart

//! Port of the `VariableBinder` tests. The Dart harness objects (`_Node`,
//! `_VariableElement`) become `Copy` handles: a node is a [`Node`] value and
//! a variable is an index ([`Var`]) into a shared arena of [`Element`]s.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use dartr_flow::type_analyzer::{JoinedPatternVariableInconsistency, TypeAnalyzerErrorsBase};
use dartr_flow::variable_bindings::{VariableBinder, VariableBinderErrors};
use dartr_type_analyzer::variable_bindings::VariableBinderState;

use indexmap::IndexMap;

type Inconsistency = JoinedPatternVariableInconsistency;

/// Dart `_Node`: the `id` is `int?`, printed as `null` when absent. `uid`
/// keeps two nodes with the same `id` distinct (Dart object identity).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Node {
    uid: u32,
    id: Option<u32>,
}

impl Node {
    fn display(self) -> String {
        match self.id {
            Some(id) => id.to_string(),
            None => "null".to_string(),
        }
    }
}

/// The key of a join (Dart `Object key`): the `_Or` node, or the key of the
/// shared case scope.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Key {
    Or(Node),
    Scope(u32),
}

impl From<Node> for Key {
    fn from(node: Node) -> Key {
        Key::Or(node)
    }
}

/// Dart `_VariableElement` handle.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Var(usize);

/// Dart `_VariableBindElement` and `_VariableJoinElement`.
#[derive(Debug)]
enum Element {
    Bind(String),
    Join {
        components: Vec<Var>,
        inconsistency: Inconsistency,
    },
}

type Arena = Rc<RefCell<Vec<Element>>>;

fn inconsistency_name(i: Inconsistency) -> &'static str {
    match i {
        Inconsistency::None => "none",
        Inconsistency::LogicalOr => "logicalOr",
        Inconsistency::SharedCaseAbsent => "sharedCaseAbsent",
        Inconsistency::SharedCaseHasLabel => "sharedCaseHasLabel",
        Inconsistency::DifferentFinalityOrType => "differentFinalityOrType",
    }
}

fn element_to_string(arena: &Arena, var: Var) -> String {
    element_to_string_inner(&arena.borrow(), var)
}

fn element_to_string_inner(arena: &[Element], var: Var) -> String {
    match &arena[var.0] {
        Element::Bind(id) => id.clone(),
        Element::Join {
            components,
            inconsistency,
        } => {
            let mut parts = Vec::new();
            if *inconsistency != Inconsistency::None {
                parts.push(format!("notConsistent:{}", inconsistency_name(*inconsistency)));
            }
            let list: Vec<String> = components
                .iter()
                .map(|c| element_to_string_inner(arena, *c))
                .collect();
            parts.push(format!("[{}]", list.join(", ")));
            parts.join(" ")
        }
    }
}

/// Dart `_Errors`.
struct Errors {
    arena: Arena,
    errors: Vec<String>,
}

impl TypeAnalyzerErrorsBase for Errors {
    fn assert_in_error_recovery(&mut self) {
        panic!("Unexpected error assertInErrorRecovery");
    }
}

impl VariableBinderErrors for Errors {
    type Node = Node;
    type Variable = Var;
    type Name = &'static str;

    fn duplicate_variable_pattern(&mut self, name: &'static str, original: Var, duplicate: Var) {
        let message = format!(
            "duplicateVariablePattern(name: {name}, original: {}, duplicate: {})",
            element_to_string(&self.arena, original),
            element_to_string(&self.arena, duplicate),
        );
        self.errors.push(message);
    }

    fn logical_or_pattern_branch_missing_variable(
        &mut self,
        node: Node,
        has_in_left: bool,
        name: &'static str,
        variable: Var,
    ) {
        let message = format!(
            "logicalOrPatternBranchMissingVariable(node: {}, hasInLeft: {has_in_left}, \
             name: {name}, variable: {})",
            node.display(),
            element_to_string(&self.arena, variable),
        );
        self.errors.push(message);
    }
}

/// Dart `_VariableBinder`.
struct Binder {
    arena: Arena,
}

impl Binder {
    fn inconsistency_of(&self, var: Var) -> Inconsistency {
        match &self.arena.borrow()[var.0] {
            Element::Bind(_) => Inconsistency::None,
            Element::Join { inconsistency, .. } => *inconsistency,
        }
    }
}

impl VariableBinder for Binder {
    type Node = Node;
    type Variable = Var;
    type Key = Key;

    fn join_pattern_variables(
        &mut self,
        key: Key,
        components: Vec<Var>,
        inconsistency: Inconsistency,
    ) -> Var {
        let mut flat = Vec::new();
        for &variable in &components {
            let join_components = match &self.arena.borrow()[variable.0] {
                Element::Join { components, .. } if matches!(key, Key::Or(_)) => {
                    Some(components.clone())
                }
                _ => None,
            };
            match join_components {
                Some(join_components) => flat.extend(join_components),
                None => flat.push(variable),
            }
        }
        let inconsistency = inconsistency.max_with_all(
            components.iter().map(|&c| self.inconsistency_of(c)),
        );
        let mut arena = self.arena.borrow_mut();
        arena.push(Element::Join {
            components: flat,
            inconsistency,
        });
        Var(arena.len() - 1)
    }
}

/// Dart `_Node` subclasses.
enum Pattern {
    And(Box<Pattern>, Box<Pattern>),
    Empty,
    Or(Box<Pattern>, Box<Pattern>, Option<u32>),
    Var(&'static str, u32),
}

fn and(left: Pattern, right: Pattern) -> Pattern {
    Pattern::And(Box::new(left), Box::new(right))
}

fn or(left: Pattern, right: Pattern) -> Pattern {
    Pattern::Or(Box::new(left), Box::new(right), None)
}

fn or_id(left: Pattern, right: Pattern, id: u32) -> Pattern {
    Pattern::Or(Box::new(left), Box::new(right), Some(id))
}

fn var(name: &'static str, id: u32) -> Pattern {
    Pattern::Var(name, id)
}

/// Dart `_Harness`.
struct Harness {
    arena: Arena,
    errors: Errors,
    binder: Binder,
    state: VariableBinderState<&'static str, Var, Key>,
    next_uid: u32,
}

impl Harness {
    fn new() -> Harness {
        let arena: Arena = Rc::new(RefCell::new(Vec::new()));
        Harness {
            errors: Errors {
                arena: arena.clone(),
                errors: Vec::new(),
            },
            binder: Binder {
                arena: arena.clone(),
            },
            arena,
            state: VariableBinderState::new(),
            next_uid: 0,
        }
    }

    fn visit(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::And(left, right) => {
                self.visit(left);
                self.visit(right);
            }
            Pattern::Empty => {}
            Pattern::Or(left, right, id) => {
                let node = Node {
                    uid: self.next_uid,
                    id: *id,
                };
                self.next_uid += 1;
                self.state.logical_or_pattern_start();
                self.visit(left);
                self.state.logical_or_pattern_finish_left();
                self.visit(right);
                self.state
                    .logical_or_pattern_finish(&mut self.binder, Some(&mut self.errors), node);
            }
            Pattern::Var(name, id) => {
                let element = {
                    let mut arena = self.arena.borrow_mut();
                    arena.push(Element::Bind(id.to_string()));
                    Var(arena.len() - 1)
                };
                self.state.add(Some(&mut self.errors), name, element);
            }
        }
    }

    fn run_pattern(
        &mut self,
        pattern: Pattern,
        expect_errors: &[&str],
        expected_variables: &[&str],
    ) {
        self.state.case_pattern_start();
        self.visit(&pattern);
        let variables = self.state.case_pattern_finish(None);
        self.state.finish();
        self.assert_variables(&variables, expected_variables);
        assert_eq!(self.errors.errors, expect_errors);
    }

    fn run_switch_statement_shared_body(
        &mut self,
        shared_case_scope_key: u32,
        case_patterns: Vec<Pattern>,
        has_default_first: bool,
        has_default_last: bool,
        expect_errors: &[&str],
        expected_variables: &[&str],
    ) {
        assert!(!(has_default_first && has_default_last));
        let key = Key::Scope(shared_case_scope_key);
        self.state.switch_statement_shared_case_scope_start(key);
        if has_default_first {
            self.state.switch_statement_shared_case_scope_empty(&key);
        }
        for case_pattern in &case_patterns {
            self.state.case_pattern_start();
            self.visit(case_pattern);
            self.state.case_pattern_finish(Some(&key));
        }
        if has_default_last {
            self.state.switch_statement_shared_case_scope_empty(&key);
        }
        let variables = self
            .state
            .switch_statement_shared_case_scope_finish(&mut self.binder, key);
        self.state.finish();
        self.assert_variables(&variables, expected_variables);
        assert_eq!(self.errors.errors, expect_errors);
    }

    fn assert_variables(&self, variables: &IndexMap<&'static str, Var>, expected: &[&str]) {
        // Dart compares `Set<String>`: order does not matter.
        let actual: BTreeSet<String> = variables
            .iter()
            .map(|(name, &v)| format!("{name}: {}", element_to_string(&self.arena, v)))
            .collect();
        let expected: BTreeSet<String> = expected.iter().map(|s| s.to_string()).collect();
        assert_eq!(actual, expected);
    }
}

mod duplicate_variable {
    use super::*;

    #[test]
    fn in_logical_and() {
        Harness::new().run_pattern(
            and(var("x", 1), var("x", 2)),
            &["duplicateVariablePattern(name: x, original: 1, duplicate: 2)"],
            &["x: 1"],
        );
    }
}

mod logical_or {
    use super::*;

    mod variable_should_be_present_in_both_branches {
        use super::*;

        #[test]
        fn both_have() {
            Harness::new().run_pattern(
                or(var("x", 1), var("x", 2)),
                &[],
                &["x: [1, 2]"],
            );
        }

        #[test]
        fn left_has() {
            Harness::new().run_pattern(
                or_id(var("x", 1), Pattern::Empty, 2),
                &["logicalOrPatternBranchMissingVariable(node: 2, \
                   hasInLeft: true, name: x, variable: 1)"],
                &["x: notConsistent:logicalOr [1]"],
            );
        }

        #[test]
        fn right_has() {
            Harness::new().run_pattern(
                or_id(Pattern::Empty, var("x", 1), 2),
                &["logicalOrPatternBranchMissingVariable(node: 2, \
                   hasInLeft: false, name: x, variable: 1)"],
                &["x: notConsistent:logicalOr [1]"],
            );
        }
    }
}

mod switch_statement {
    use super::*;

    fn run(
        case_patterns: Vec<Pattern>,
        has_default_first: bool,
        has_default_last: bool,
        expect_errors: &[&str],
        expected_variables: &[&str],
    ) {
        Harness::new().run_switch_statement_shared_body(
            0,
            case_patterns,
            has_default_first,
            has_default_last,
            expect_errors,
            expected_variables,
        );
    }

    #[test]
    fn both_have() {
        run(
            vec![var("x", 1), var("x", 2)],
            false,
            false,
            &[],
            &["x: [1, 2]"],
        );
    }

    #[test]
    fn first_has() {
        run(
            vec![var("x", 1), Pattern::Empty],
            false,
            false,
            &[],
            &["x: notConsistent:sharedCaseAbsent [1]"],
        );
    }

    #[test]
    fn second_has() {
        run(
            vec![Pattern::Empty, var("x", 1)],
            false,
            false,
            &[],
            &["x: notConsistent:sharedCaseAbsent [1]"],
        );
    }

    #[test]
    fn partial_intersection() {
        run(
            vec![and(var("x", 1), var("y", 2)), var("x", 3)],
            false,
            false,
            &[],
            &["x: [1, 3]", "y: notConsistent:sharedCaseAbsent [2]"],
        );
    }

    mod has_default {
        use super::*;

        #[test]
        fn first() {
            // `hasDefaultFirst` does not happen normally.
            run(
                vec![var("x", 1)],
                true,
                false,
                &[],
                &["x: notConsistent:sharedCaseHasLabel [1]"],
            );
        }

        #[test]
        fn last() {
            run(
                vec![var("x", 1)],
                false,
                true,
                &[],
                &["x: notConsistent:sharedCaseHasLabel [1]"],
            );
        }
    }

    mod with_logical_or {
        use super::*;

        #[test]
        fn both_have() {
            run(
                vec![or(var("x", 1), var("x", 2)), var("x", 3)],
                false,
                false,
                &[],
                &["x: [[1, 2], 3]"],
            );
        }

        #[test]
        fn both_have_inconsistent() {
            run(
                vec![or(var("x", 1), Pattern::Empty), var("x", 2)],
                false,
                false,
                &["logicalOrPatternBranchMissingVariable(node: null, \
                   hasInLeft: true, name: x, variable: 1)"],
                &["x: notConsistent:logicalOr [notConsistent:logicalOr [1], 2]"],
            );
        }

        #[test]
        fn first_has() {
            run(
                vec![or(var("x", 1), var("x", 2)), Pattern::Empty],
                false,
                false,
                &[],
                &["x: notConsistent:sharedCaseAbsent [[1, 2]]"],
            );
        }

        #[test]
        fn second_has() {
            run(
                vec![Pattern::Empty, or(var("x", 1), var("x", 2))],
                false,
                false,
                &[],
                &["x: notConsistent:sharedCaseAbsent [[1, 2]]"],
            );
        }
    }
}
