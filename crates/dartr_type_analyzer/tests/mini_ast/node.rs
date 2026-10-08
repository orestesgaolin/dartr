// Dart source: pkg/_fe_analyzer_shared/test/mini_ast.dart (classes `Node`,
// `Var`, `_PropertyElement`)

//! Minimal mini-AST handles used by [`MiniAstOperations`]: [`Node`],
//! [`Var`] and [`PropertyElement`].
//!
//! Dart `Node` and `Var` are objects with identity. Here they are `Copy`
//! handles (a `u32` id) into thread-local tables, so they can be used as the
//! `AstNode` and `Variable` associated types of the operations traits
//! (`Copy + Eq + Hash + Debug`). Equality is identity, as in Dart.
//!
//! These types are deliberately small; the full mini-AST (expressions,
//! statements, patterns) is added on top of them.
//!
//! [`MiniAstOperations`]: super::operations::MiniAstOperations

use std::cell::{Cell, RefCell};
use std::fmt;

use dartr_flow::flow_analysis_operations::PropertyNonPromotabilityReason;

use super::mini_types::Type;

thread_local! {
    /// `Node._nextId`.
    static NEXT_NODE_ID: Cell<u32> = const { Cell::new(0) };

    /// The data of the variables, indexed by [`Var`] id.
    static VARS: RefCell<Vec<VarData>> = const { RefCell::new(Vec::new()) };
}

/// Representation of an expression or statement in the pseudo-Dart language
/// used for flow analysis testing (Dart class `Node`; here only its id).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Node {
    /// `id`: unique per thread, in creation order.
    pub id: u32,
}

impl Node {
    /// `Node.placeholder()`: a new node with a fresh id.
    pub fn placeholder() -> Node {
        Node { id: next_node_id() }
    }
}

/// Allocates the next node id (`Node._nextId++`). [`Var`]s take their ids
/// from the same counter, as Dart `Var` extends `Node`.
fn next_node_id() -> u32 {
    NEXT_NODE_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    })
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node#{}", self.id)
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Node#{}", self.id)
    }
}

/// The data of a [`Var`].
#[derive(Clone, Debug)]
pub struct VarData {
    /// `name`.
    pub name: String,
    /// `isFinal` (mutable in Dart).
    pub is_final: bool,
    /// The type of the variable, or `None` if it is not yet known.
    pub ty: Option<Type>,
    /// Identifier for this variable in IR. This allows distinct variables
    /// with the same name to be distinguished.
    pub identity: String,
    /// The node of the variable (Dart `Var extends Node`).
    pub node: Node,
}

/// A variable of the mini-AST (Dart class `Var`).
///
/// A `Copy` handle (index into a thread-local table); compared by identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Var(u32);

impl Var {
    /// `Var(name, {isFinal = false, identity})`.
    pub fn new(name: &str) -> Var {
        Self::with_options(name, false, None)
    }

    /// `Var(name, isFinal: ..., identity: ...)`.
    pub fn with_options(name: &str, is_final: bool, identity: Option<&str>) -> Var {
        let node = Node { id: next_node_id() };
        VARS.with(|vars| {
            let mut vars = vars.borrow_mut();
            vars.push(VarData {
                name: name.to_string(),
                is_final,
                ty: None,
                identity: identity.unwrap_or(name).to_string(),
                node,
            });
            Var((vars.len() - 1) as u32)
        })
    }

    /// A copy of the data of this variable.
    pub fn data(self) -> VarData {
        VARS.with(|vars| vars.borrow()[self.0 as usize].clone())
    }

    fn update(self, f: impl FnOnce(&mut VarData)) {
        VARS.with(|vars| f(&mut vars.borrow_mut()[self.0 as usize]));
    }

    /// `name`.
    pub fn name(self) -> String {
        self.data().name
    }

    /// `isFinal`.
    pub fn is_final(self) -> bool {
        self.data().is_final
    }

    /// Sets `isFinal`.
    pub fn set_is_final(self, is_final: bool) {
        self.update(|d| d.is_final = is_final);
    }

    /// `identity`.
    pub fn identity(self) -> String {
        self.data().identity
    }

    /// The string that should be used to check variables in a set.
    pub fn string_to_check_variables(self) -> String {
        self.identity()
    }

    /// The node of this variable (Dart `Var extends Node`).
    pub fn node(self) -> Node {
        self.data().node
    }

    /// Gets the type if known; otherwise panics (Dart throws `'Type not yet
    /// known'`).
    pub fn ty(self) -> Type {
        self.data().ty.expect("Type not yet known")
    }

    /// The type, or `None` if it is not yet known (Dart `_type`).
    pub fn ty_or_null(self) -> Option<Type> {
        self.data().ty
    }

    /// Sets the type; panics if it is already set (Dart throws `'Type
    /// already set'`).
    pub fn set_ty(self, ty: Type) {
        self.update(|d| {
            if d.ty.is_some() {
                panic!("Type already set");
            }
            d.ty = Some(ty);
        });
    }
}

impl fmt::Debug for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Var({})", self.name())
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

/// A property of the mini-AST (Dart class `_PropertyElement`), used as the
/// `PropertyMember` of the operations.
#[derive(Clone, Debug)]
pub struct PropertyElement {
    /// The type of the property.
    pub ty: Type,
    /// The name of the property (used by `toString`).
    pub name: String,
    /// Whether the property is promotable.
    pub is_promotable: bool,
    /// The reason the property is not promotable, if applicable and relevant
    /// to the test.
    ///
    /// If the property is promotable ([`is_promotable`](Self::is_promotable)
    /// is `true`), this value is always `None`.
    ///
    /// Otherwise the value *may* be a reason for the property not being
    /// promotable, but it may also still be `None` if the reason is not
    /// relevant to the test.
    pub why_not_promotable: Option<PropertyNonPromotabilityReason>,
}

impl PropertyElement {
    /// `_PropertyElement(type, name, {isPromotable, whyNotPromotable})`.
    pub fn new(
        ty: Type,
        name: &str,
        is_promotable: bool,
        why_not_promotable: Option<PropertyNonPromotabilityReason>,
    ) -> Self {
        if is_promotable {
            assert!(why_not_promotable.is_none());
        }
        PropertyElement {
            ty,
            name: name.to_string(),
            is_promotable,
            why_not_promotable,
        }
    }
}

impl fmt::Display for PropertyElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.ty, self.name)
    }
}
