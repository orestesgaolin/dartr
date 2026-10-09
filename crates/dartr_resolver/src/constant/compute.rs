// Dart source: pkg/analyzer/lib/src/dart/constant/compute.dart
// (with pkg/_fe_analyzer_shared/lib/src/util/dependency_walker.dart)

//! [`compute_constants`]: computes the values of constants in dependency
//! order (Tarjan's strongly connected components, Dart `DependencyWalker`);
//! constants in a cycle get `recursive_compile_time_constant`.

use indexmap::IndexMap;

use crate::constant::evaluation::{ConstantEvaluationEngine, ConstantTarget};

/// Dart `computeConstants(constants:)`.
pub fn compute_constants(engine: &ConstantEvaluationEngine<'_>, constants: &[ConstantTarget]) {
    let mut walker = ConstantWalker {
        engine,
        node_map: IndexMap::new(),
        nodes: Vec::new(),
    };
    for &constant in constants {
        let node = walker.get_node(constant);
        walker.walk(node);
    }
}

/// Dart `_ConstantNode` (`graph.Node`).
struct ConstantNode {
    constant: ConstantTarget,
    index: u32,
    low_link: u32,
    dependencies: Option<Vec<usize>>,
}

/// Dart `_ConstantWalker`.
struct ConstantWalker<'e, 'a> {
    engine: &'e ConstantEvaluationEngine<'a>,
    node_map: IndexMap<ConstantTarget, usize>,
    nodes: Vec<ConstantNode>,
}

impl ConstantWalker<'_, '_> {
    fn get_node(&mut self, constant: ConstantTarget) -> usize {
        if let Some(&n) = self.node_map.get(&constant) {
            return n;
        }
        let n = self.nodes.len();
        self.nodes.push(ConstantNode {
            constant,
            index: 0,
            low_link: 0,
            dependencies: None,
        });
        self.node_map.insert(constant, n);
        n
    }

    fn is_evaluated(&self, node: usize) -> bool {
        self.engine.is_constant_evaluated(self.nodes[node].constant)
    }

    /// Dart `Node.getDependencies` / `_computeDependencies`.
    fn get_dependencies(&mut self, node: usize) -> Vec<usize> {
        if let Some(d) = &self.nodes[node].dependencies {
            return d.clone();
        }
        let mut targets = Vec::new();
        self.engine
            .compute_dependencies(self.nodes[node].constant, &mut |t| targets.push(t));
        let dependencies: Vec<usize> = targets.into_iter().map(|t| self.get_node(t)).collect();
        self.nodes[node].dependencies = Some(dependencies.clone());
        dependencies
    }

    /// Dart `evaluate(node)`.
    fn evaluate(&self, node: usize) {
        self.engine
            .compute_constant_value(self.nodes[node].constant);
    }

    /// Dart `evaluateScc(scc)`.
    fn evaluate_scc(&self, scc: &[usize]) {
        for &node in scc {
            let constant = self.nodes[node].constant;
            if let ConstantTarget::Element(e) = constant
                && e.tag() == dartr_element::Tag::Constructor
            {
                self.engine.set_not_cycle_free(e);
            }
            self.engine.generate_cycle_error(constant);
        }
    }

    /// Dart `DependencyWalker.walk(startingPoint)`.
    fn walk(&mut self, starting_point: usize) {
        if self.is_evaluated(starting_point) {
            return;
        }
        let mut index = 1;
        let mut stack = Vec::new();
        self.strong_connect(starting_point, &mut index, &mut stack);
    }

    fn strong_connect(&mut self, node: usize, index: &mut u32, stack: &mut Vec<usize>) {
        let mut has_trivial_cycle = false;

        // Assign the current node an index and add it to the stack. We
        // haven't seen any of its dependencies yet, so set its lowLink to
        // its index, indicating that so far it is the only node in its
        // strongly connected component.
        self.nodes[node].index = *index;
        self.nodes[node].low_link = *index;
        *index += 1;
        stack.push(node);

        // Consider the node's dependencies one at a time.
        for dependency in self.get_dependencies(node) {
            // If the dependency has already been evaluated, it can't be part
            // of this node's strongly connected component, so we can skip
            // it.
            if self.is_evaluated(dependency) {
                continue;
            }
            if node == dependency {
                // If a node includes itself as a dependency, there is no
                // need to explore the dependency further.
                has_trivial_cycle = true;
            } else if self.nodes[dependency].index == 0 {
                // The dependency hasn't been seen yet, so recurse on it.
                self.strong_connect(dependency, index, stack);
                if self.nodes[dependency].low_link < self.nodes[node].low_link {
                    self.nodes[node].low_link = self.nodes[dependency].low_link;
                }
            } else if self.nodes[dependency].index < self.nodes[node].low_link {
                // The dependency has already been seen, so it is part of the
                // current node's strongly connected component.
                self.nodes[node].low_link = self.nodes[dependency].index;
            }
        }

        // If the current node's lowLink is the same as its index, then we
        // have finished visiting a strongly connected component, so pop the
        // stack and evaluate it before moving on.
        if self.nodes[node].low_link == self.nodes[node].index {
            if stack.last() == Some(&node) {
                stack.pop();
                if has_trivial_cycle {
                    self.evaluate_scc(&[node]);
                } else {
                    self.evaluate(node);
                }
            } else {
                let mut scc = Vec::new();
                loop {
                    let other_node = stack.pop().expect("node on the stack");
                    scc.push(other_node);
                    if other_node == node {
                        break;
                    }
                }
                self.evaluate_scc(&scc);
            }
        }
    }
}
