// Dart source: pkg/_fe_analyzer_shared/test/mini_ir.dart

//! A miniature string-based internal representation ("IR") of Dart code
//! suitable for use in unit testing.
//!
//! The type analyzer pushes and pops entries of a conceptual stack (see the
//! docs of `TypeAnalyzer`); the mini-AST builds a string for each entry, so
//! that tests can check the structure that the analyzer produced.

use std::fmt;

/// An entry of the IR stack.
#[derive(Clone, Debug)]
pub struct IrNode {
    /// The string form of the IR.
    pub ir: String,
    /// The test source location that produced this node.
    pub location: String,
    /// The kind of the node.
    pub kind: Kind,
}

impl fmt::Display for IrNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {} ({})", self.kind, self.ir, self.location)
    }
}

/// The kinds of stack entries (see the Dart docs of `Kind` for a
/// description of each).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A `case` head (pattern and guard) or `default`.
    CaseHead,
    /// The merged case heads of a switch statement case.
    CaseHeads,
    /// A collection element.
    CollectionElement,
    /// An expression.
    Expression,
    /// A case of a switch expression.
    ExpressionCase,
    /// A label.
    Label,
    /// An element of a map pattern.
    MapPatternElement,
    /// A pattern.
    Pattern,
    /// A statement.
    Statement,
    /// A case of a switch statement.
    StatementCase,
    /// A type.
    Type,
    /// A variable.
    Variable,
    /// A list of variables.
    Variables,
}

/// The builder of the IR stack (Dart `MiniIRBuilder`).
#[derive(Default)]
pub struct MiniIrBuilder {
    label_counter: usize,
    pop_limit: usize,
    stack: Vec<IrNode>,
    tmp_counter: usize,
}

/// A label (Dart `MiniIRLabel`).
#[derive(Default)]
pub struct MiniIrLabel {
    name: Option<String>,
}

/// A temporary variable (Dart `MiniIRTmp`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MiniIrTmp {
    name: String,
    value: String,
    /// The test source location that allocated this temporary.
    pub location: String,
}

impl MiniIrBuilder {
    /// Creates an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocates a new label.
    pub fn allocate_label(&self) -> MiniIrLabel {
        MiniIrLabel::default()
    }

    /// Pops the top expression and allocates a temporary holding it.
    pub fn allocate_tmp(&mut self, location: &str) -> MiniIrTmp {
        let name = format!("t{}", self.tmp_counter);
        self.tmp_counter += 1;
        let value = self.pop(Kind::Expression).ir;
        MiniIrTmp {
            name,
            value,
            location: location.to_string(),
        }
    }

    /// Pops one entry for each of `input_kinds`, and pushes
    /// `name(inputs...)` with kind `output_kind`. The last `names.len()`
    /// inputs are prefixed with `name: `.
    pub fn apply(
        &mut self,
        name: &str,
        input_kinds: &[Kind],
        output_kind: Kind,
        location: &str,
        names: &[&str],
    ) {
        let mut args: Vec<String> = self
            .pop_list(input_kinds.len(), input_kinds)
            .into_iter()
            .map(|ir_node| ir_node.ir)
            .collect();
        let args_len = args.len();
        for i in 1..=names.len() {
            args[args_len - i] = format!("{}: {}", names[names.len() - i], args[args_len - i]);
        }
        self.push(IrNode {
            ir: format!("{name}({})", args.join(", ")),
            kind: output_kind,
            location: location.to_string(),
        });
    }

    /// Pushes the atom `name` with kind `kind`.
    pub fn atom(&mut self, name: &str, kind: Kind, location: &str) {
        self.push(IrNode {
            ir: name.to_string(),
            kind,
            location: location.to_string(),
        });
    }

    /// Checks that the top of the stack has kind `expected_kind` and IR
    /// `expected_ir`.
    pub fn check(&self, expected_ir: &str, expected_kind: Kind, location: &str) {
        let last = self.stack.last().expect("empty IR stack");
        assert_eq!(last.kind, expected_kind, "at {location}");
        assert_eq!(last.ir, expected_ir, "at {location}");
    }

    /// Pops the body, iterable and (unless `tmp` is given) the variable of a
    /// for-in loop, and pushes the loop.
    pub fn for_in(&mut self, tmp: Option<&MiniIrTmp>, location: &str, is_asynchronous: bool) {
        let name = if is_asynchronous {
            "forIn_async"
        } else {
            "forIn"
        };
        let body = self.pop(Kind::Statement);
        let iterable = self.pop(Kind::Expression);
        let variable = match tmp {
            None => self.pop(Kind::Variable).ir,
            Some(tmp) => tmp.name.clone(),
        };
        self.push(IrNode {
            ir: format!("{name}({variable}, {}, {})", iterable.ir, body.ir),
            kind: Kind::Statement,
            location: location.to_string(),
        });
    }

    /// The first half of Dart `guard`: returns the state to pass to
    /// [`guard_end`](Self::guard_end). Dart `guard` runs a callback that
    /// visits a node; here the caller visits the node between
    /// `guard_begin` and `guard_end`, because the visit needs the harness
    /// that owns this builder.
    pub fn guard_begin(&mut self) -> (usize, usize) {
        let previous_stack_depth = self.stack.len();
        let previous_pop_limit = self.pop_limit;
        self.pop_limit = previous_stack_depth;
        (previous_stack_depth, previous_pop_limit)
    }

    /// The second half of Dart `guard`: checks that the visit pushed
    /// exactly one entry.
    pub fn guard_end(&mut self, previous: (usize, usize), node_description: &dyn Fn() -> String) {
        let (previous_stack_depth, previous_pop_limit) = previous;
        let stack_delta = self.stack.len() as isize - previous_stack_depth as isize;
        if stack_delta != 1 {
            panic!(
                "Stack delta of {stack_delta} while visiting {}\nStack: {self}",
                node_description()
            );
        }
        self.pop_limit = previous_pop_limit;
    }

    /// Pops an expression `e` and pushes `let(tmp, value, if(==(tmp, null),
    /// null, e))`.
    pub fn if_not_null(&mut self, tmp: &MiniIrTmp, location: &str) {
        let e = self.pop(Kind::Expression).ir;
        self.push(IrNode {
            ir: format!("if(==({}, null), null, {e})", tmp.name),
            kind: Kind::Expression,
            location: location.to_string(),
        });
        self.let_(tmp, location);
    }

    /// Pops `ifNull` and `ifNotNull` expressions and pushes the null test.
    pub fn if_null(&mut self, tmp: &MiniIrTmp, location: &str) {
        let if_null = self.pop(Kind::Expression);
        let if_not_null = self.pop(Kind::Expression);
        self.push(IrNode {
            ir: format!(
                "if(==({}, null), {}, {})",
                tmp.name, if_null.ir, if_not_null.ir
            ),
            kind: Kind::Expression,
            location: location.to_string(),
        });
        self.let_(tmp, location);
    }

    /// Pushes an index get.
    pub fn index_get(&mut self, location: &str) {
        self.apply(
            "[]",
            &[Kind::Expression, Kind::Expression],
            Kind::Expression,
            location,
            &[],
        );
    }

    /// Pushes an index set.
    pub fn index_set(
        &mut self,
        receiver_tmp: Option<&MiniIrTmp>,
        index_tmp: Option<&MiniIrTmp>,
        location: &str,
    ) {
        let value = self.pop(Kind::Expression).ir;
        let index = match index_tmp {
            None => self.pop(Kind::Expression).ir,
            Some(tmp) => tmp.name.clone(),
        };
        let receiver = match receiver_tmp {
            None => self.pop(Kind::Expression).ir,
            Some(tmp) => tmp.name.clone(),
        };
        self.push(IrNode {
            ir: format!("[]=({receiver}, {index}, {value})"),
            kind: Kind::Expression,
            location: location.to_string(),
        });
    }

    /// Wraps the top statement in `labeled(...)` if the label was referred
    /// to.
    pub fn labeled(&mut self, label: &MiniIrLabel, location: &str) {
        if let Some(name) = &label.name {
            let statement = self.pop(Kind::Statement);
            self.push(IrNode {
                ir: format!("labeled({name}, {statement})"),
                kind: Kind::Statement,
                location: location.to_string(),
            });
        }
    }

    /// Pops an expression `e` and pushes `let(tmp, value, e)` (Dart `let`).
    pub fn let_(&mut self, tmp: &MiniIrTmp, location: &str) {
        let e = self.pop(Kind::Expression).ir;
        self.push(IrNode {
            ir: format!("let({}, {}, {e})", tmp.name, tmp.value),
            kind: Kind::Expression,
            location: location.to_string(),
        });
    }

    /// Pushes a property get.
    pub fn property_get(&mut self, property_name: &str, location: &str) {
        self.apply(
            &format!("get_{property_name}"),
            &[Kind::Expression],
            Kind::Expression,
            location,
            &[],
        );
    }

    /// Pushes a property set.
    pub fn property_set(
        &mut self,
        receiver_tmp: Option<&MiniIrTmp>,
        property_name: &str,
        location: &str,
    ) {
        let value = self.pop(Kind::Expression).ir;
        let receiver = match receiver_tmp {
            None => self.pop(Kind::Expression).ir,
            Some(tmp) => tmp.name.clone(),
        };
        self.push(IrNode {
            ir: format!("set_{property_name}({receiver}, {value})"),
            kind: Kind::Expression,
            location: location.to_string(),
        });
    }

    /// Pushes a read of `tmp`.
    pub fn read_tmp(&mut self, tmp: &MiniIrTmp, location: &str) {
        self.push(IrNode {
            ir: tmp.name.clone(),
            kind: Kind::Expression,
            location: location.to_string(),
        });
    }

    /// Pushes a reference to `label` (allocating its name).
    pub fn refer_to_label(&mut self, label: &mut MiniIrLabel, location: &str) {
        let name = label
            .name
            .get_or_insert_with(|| {
                let name = format!("L{}", self.label_counter);
                self.label_counter += 1;
                name
            })
            .clone();
        self.push(IrNode {
            ir: name,
            kind: Kind::Label,
            location: location.to_string(),
        });
    }

    /// Pushes a variable get.
    pub fn variable_get(&mut self, name: &str, location: &str) {
        self.atom(name, Kind::Expression, location);
    }

    /// Pushes a variable set.
    pub fn variable_set(&mut self, name: &str, location: &str) {
        self.apply(
            &format!("{name}="),
            &[Kind::Expression],
            Kind::Expression,
            location,
            &[],
        );
    }

    fn pop(&mut self, expected_kind: Kind) -> IrNode {
        assert!(
            self.stack.len() > self.pop_limit,
            "IR stack underflow (pop limit {})",
            self.pop_limit
        );
        let ir_node = self.stack.pop().expect("IR stack");
        assert_eq!(ir_node.kind, expected_kind, "unexpected kind of {ir_node}");
        ir_node
    }

    fn pop_list(&mut self, count: usize, expected_kinds: &[Kind]) -> Vec<IrNode> {
        assert!(
            self.stack.len() >= count && self.stack.len() - count >= self.pop_limit,
            "IR stack underflow: need {count}, stack: {self}"
        );
        let new_length = self.stack.len() - count;
        let result = self.stack.split_off(new_length);
        let kinds: Vec<Kind> = result.iter().map(|n| n.kind).collect();
        assert_eq!(kinds, expected_kinds, "unexpected kinds in {result:?}");
        result
    }

    fn push(&mut self, node: IrNode) {
        self.stack.push(node);
    }
}

impl fmt::Display for MiniIrBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<&str> = self.stack.iter().map(|n| n.ir.as_str()).collect();
        write!(f, "{}", parts.join(", "))
    }
}
