// Dart source: pkg/analyzer/lib/src/dart/ast/ast.dart (AstNodeImpl, NodeListImpl, ChildEntities)

//! The node arena of one compilation unit.
//!
//! # Design
//!
//! - One [`Ast`] per compilation unit owns the token arena
//!   ([`dartr_syntax::Tokens`]) and all nodes. A node is referenced by a
//!   [`NodeId`] (a `u32`, dense, in creation order) or a typed [`Id<T>`]
//!   (same layout). `T` is a node struct (`Id<MethodInvocation>`) or a node
//!   category (`Id<Expression>`): an empty enum per abstract Dart interface.
//!   [`Id::upcast`] is checked at compile time (`T: SubtypeOf<U>`, from the
//!   Dart supertypes), [`Ast::cast`] at run time (Dart `is` / `as`).
//! - Nodes are stored per kind (`Vec<MethodInvocation>`, ...); the arena
//!   keeps the kind, the index in that vector, and the parent of each node.
//!   `ast[id]` gives the node struct. Node fields are the Dart properties in
//!   `childEntities` order: `TokenId` / `Option<TokenId>`, `Id<T>` /
//!   `Option<Id<T>>`, [`NodeList<T>`] (a range in a shared pool) and
//!   [`TokenList`].
//! - The AST builder creates nodes bottom-up with [`Ast::add`], which sets
//!   the parent of the children (Dart `_becomeParentOf`).
//! - The resolver keeps static types, elements and other resolution data in
//!   side tables indexed by [`NodeId`] ([`NodeMap`], or its own `Vec`s), so
//!   the node structs stay syntax only and the AST can be shared read-only.
//! - Rewrites (Dart `AstRewriter`, `replaceChild`): make the new node with
//!   [`Ast::add`] (it adopts the children of the old node), then
//!   [`Ast::replace_with`] / [`Ast::replace_child`] puts it in the slot of
//!   the old node, with a run-time check of the slot type like the Dart
//!   cast. [`crate::AstVisitorMut`] reads the children before it visits them,
//!   so a visitor can replace the node it visits.

use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroU32;

use dartr_syntax::{TokenId, Tokens};

use crate::generated::nodes::{Annotation, Comment, NodeKind, Stores};

/// Index of a node in an [`Ast`] (untyped). Nodes are numbered in the order
/// of creation, so a side table of a resolver can be a `Vec` indexed by
/// [`NodeId::index`] (see [`NodeMap`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct NodeId(NonZeroU32);

impl NodeId {
    #[inline(always)]
    pub fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }

    #[inline(always)]
    pub fn from_index(index: usize) -> NodeId {
        NodeId(NonZeroU32::new(index as u32 + 1).unwrap())
    }
}

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.index())
    }
}

/// A node category or node class: a concrete node struct (for example
/// [`crate::MethodInvocation`]) or an abstract Dart interface (for example
/// [`crate::Expression`], an empty enum).
pub trait NodeType {
    /// The Dart name.
    const NAME: &'static str;
    /// Dart `node is T`.
    fn test(kind: NodeKind) -> bool;
}

/// A concrete node struct.
pub trait Concrete: NodeType + Sized {
    const KIND: NodeKind;
    fn store(stores: &Stores) -> &Vec<Self>;
    fn store_mut(stores: &mut Stores) -> &mut Vec<Self>;
}

/// `T: SubtypeOf<U>`: every `T` is a `U` (Dart `T implements U`). Used by
/// [`Id::upcast`].
pub trait SubtypeOf<U: ?Sized> {}

impl<T: ?Sized> SubtypeOf<T> for T {}

/// A typed node id: `Id<MethodInvocation>`, `Id<Expression>`. Same layout
/// as [`NodeId`].
#[repr(transparent)]
pub struct Id<T: ?Sized> {
    raw: NodeId,
    _t: PhantomData<fn() -> T>,
}

impl<T: ?Sized> Id<T> {
    #[inline(always)]
    pub fn raw(self) -> NodeId {
        self.raw
    }

    /// An id of type `T` without a check. The kind of [raw] must be a `T`.
    #[inline(always)]
    pub fn from_raw(raw: NodeId) -> Id<T> {
        Id {
            raw,
            _t: PhantomData,
        }
    }

    /// Converts to a supertype (no check at run time).
    #[inline(always)]
    pub fn upcast<U: ?Sized>(self) -> Id<U>
    where
        T: SubtypeOf<U>,
    {
        Id::from_raw(self.raw)
    }
}

impl<T: ?Sized> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for Id<T> {}

impl<T: ?Sized> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T: ?Sized> Eq for Id<T> {}

impl<T: ?Sized> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state)
    }
}

impl<T: ?Sized + NodeType> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:?}", T::NAME, self.raw)
    }
}

impl<T: ?Sized> From<Id<T>> for NodeId {
    #[inline(always)]
    fn from(id: Id<T>) -> NodeId {
        id.raw
    }
}

/// A node list (Dart `NodeList<E>`): a range in the list pool of the
/// [`Ast`]. The owner of the list is the parent of its elements.
///
/// The elements can be replaced in place ([`Ast::replace_child`]). A list
/// does not grow: to add or remove elements, make a new list with
/// [`Ast::new_list`] and store it in the field (the old range stays unused).
#[repr(C)]
pub struct NodeList<T: ?Sized> {
    start: u32,
    len: u32,
    _t: PhantomData<fn() -> T>,
}

impl<T: ?Sized> NodeList<T> {
    pub const EMPTY: NodeList<T> = NodeList {
        start: 0,
        len: 0,
        _t: PhantomData,
    };

    #[inline(always)]
    pub fn len(self) -> usize {
        self.len as usize
    }

    #[inline(always)]
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// The same range with another element type (no check).
    #[inline(always)]
    pub fn cast<U: ?Sized>(self) -> NodeList<U> {
        NodeList {
            start: self.start,
            len: self.len,
            _t: PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn slots(self, lists: &[NodeId]) -> &[NodeId] {
        &lists[self.start as usize..(self.start + self.len) as usize]
    }

    #[inline(always)]
    pub(crate) fn slots_mut(self, lists: &mut [NodeId]) -> &mut [NodeId] {
        &mut lists[self.start as usize..(self.start + self.len) as usize]
    }
}

impl<T: ?Sized> Clone for NodeList<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for NodeList<T> {}

impl<T: ?Sized> Default for NodeList<T> {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl<T: ?Sized> PartialEq for NodeList<T> {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.len == other.len
    }
}

impl<T: ?Sized> fmt::Debug for NodeList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeList[{}..+{}]", self.start, self.len)
    }
}

/// A list of tokens (Dart `List<Token>`, for example `Comment.tokens`): a
/// range in the token list pool of the [`Ast`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TokenList {
    start: u32,
    len: u32,
}

impl TokenList {
    pub fn len(self) -> usize {
        self.len as usize
    }

    pub fn is_empty(self) -> bool {
        self.len == 0
    }
}

/// A child entity (Dart `SyntacticEntity` in `childEntities`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Entity {
    Node(NodeId),
    Token(TokenId),
}

/// The AST of one compilation unit: the token arena and the node arena.
///
/// Nodes are created with [`Ast::add`] (which sets the parent of the
/// children), read with `ast[id]` / [`Ast::get`], and changed with
/// [`Ast::get_mut`] + [`Ast::adopt_children`], [`Ast::replace_child`],
/// [`Ast::replace_with`] and [`Ast::remove_child`].
#[derive(Clone, Debug, Default)]
pub struct Ast {
    /// The tokens of the unit (the scanner output, changed by the parser).
    pub tokens: Tokens,
    pub(crate) kinds: Vec<NodeKind>,
    pub(crate) slots: Vec<u32>,
    pub(crate) parents: Vec<Option<NodeId>>,
    pub(crate) stores: Stores,
    pub(crate) lists: Vec<NodeId>,
    pub(crate) token_lists: Vec<TokenId>,
    /// A reusable buffer for [`Ast::adopt_children`].
    scratch: Vec<NodeId>,
}

impl Ast {
    pub fn new(tokens: Tokens) -> Ast {
        Ast {
            tokens,
            ..Ast::default()
        }
    }

    /// The number of nodes (the bound of [`NodeId::index`]).
    #[inline]
    pub fn node_count(&self) -> usize {
        self.kinds.len()
    }

    /// Adds [node] and makes it the parent of its children (Dart
    /// constructor + `_becomeParentOf`).
    pub fn add<T: Concrete>(&mut self, node: T) -> Id<T> {
        let store = T::store_mut(&mut self.stores);
        let slot = store.len() as u32;
        store.push(node);
        let id = NodeId::from_index(self.kinds.len());
        self.kinds.push(T::KIND);
        self.slots.push(slot);
        self.parents.push(None);
        self.adopt_children(id);
        Id::from_raw(id)
    }

    /// Makes [id] the parent of each of its child nodes. Call this after a
    /// change of node fields through [`Ast::get_mut`] (Dart setters call
    /// `_becomeParentOf`).
    pub fn adopt_children(&mut self, id: impl Into<NodeId>) {
        let id = id.into();
        let mut children = std::mem::take(&mut self.scratch);
        children.clear();
        self.for_each_child(id, &mut |c| children.push(c));
        for &c in &children {
            self.parents[c.index()] = Some(id);
        }
        self.scratch = children;
    }

    /// Changes a node: runs [f] on it, then [`Ast::adopt_children`].
    pub fn modify<T: Concrete, R>(&mut self, id: Id<T>, f: impl FnOnce(&mut T) -> R) -> R {
        let r = f(self.get_mut(id));
        self.adopt_children(id);
        r
    }

    /// Makes a node list of [items]. The parent of the items is set when
    /// the owner node is added ([`Ast::add`]) or adopts its children.
    pub fn new_list<T: ?Sized>(&mut self, items: impl IntoIterator<Item = Id<T>>) -> NodeList<T> {
        let start = self.lists.len() as u32;
        self.lists.extend(items.into_iter().map(|i| i.raw()));
        NodeList {
            start,
            len: self.lists.len() as u32 - start,
            _t: PhantomData,
        }
    }

    pub fn new_token_list(&mut self, items: impl IntoIterator<Item = TokenId>) -> TokenList {
        let start = self.token_lists.len() as u32;
        self.token_lists.extend(items);
        TokenList {
            start,
            len: self.token_lists.len() as u32 - start,
        }
    }

    #[inline]
    pub fn kind(&self, id: impl Into<NodeId>) -> NodeKind {
        self.kinds[id.into().index()]
    }

    #[inline]
    pub(crate) fn slot(&self, id: NodeId) -> usize {
        self.slots[id.index()] as usize
    }

    /// Dart `AstNode.parent`.
    #[inline]
    pub fn parent(&self, id: impl Into<NodeId>) -> Option<NodeId> {
        self.parents[id.into().index()]
    }

    /// Sets the parent link only (Dart `_parent = ...`), for example to
    /// detach a node (Dart `detachFromParent`).
    pub fn set_parent(&mut self, id: impl Into<NodeId>, parent: Option<NodeId>) {
        self.parents[id.into().index()] = parent;
    }

    /// Dart `AstNode.root`.
    pub fn root(&self, id: impl Into<NodeId>) -> NodeId {
        let mut id = id.into();
        while let Some(p) = self.parent(id) {
            id = p;
        }
        id
    }

    /// Dart `node is T`.
    #[inline]
    pub fn is<T: ?Sized + NodeType>(&self, id: impl Into<NodeId>) -> bool {
        T::test(self.kind(id))
    }

    /// Dart `node as T?` with a check: `None` if the node is not a `T`.
    #[inline]
    pub fn cast<T: ?Sized + NodeType>(&self, id: impl Into<NodeId>) -> Option<Id<T>> {
        let id = id.into();
        if T::test(self.kind(id)) {
            Some(Id::from_raw(id))
        } else {
            None
        }
    }

    #[inline]
    pub fn get<T: Concrete>(&self, id: Id<T>) -> &T {
        debug_assert_eq!(self.kind(id), T::KIND);
        &T::store(&self.stores)[self.slot(id.raw())]
    }

    /// A mutable node. After a change of child fields, call
    /// [`Ast::adopt_children`] (or use [`Ast::modify`]).
    #[inline]
    pub fn get_mut<T: Concrete>(&mut self, id: Id<T>) -> &mut T {
        debug_assert_eq!(self.kind(id), T::KIND);
        let slot = self.slot(id.raw());
        &mut T::store_mut(&mut self.stores)[slot]
    }

    /// Dart `thisOrAncestorOfType<T>()`.
    pub fn this_or_ancestor_of_type<T: ?Sized + NodeType>(
        &self,
        id: impl Into<NodeId>,
    ) -> Option<Id<T>> {
        let mut node = Some(id.into());
        while let Some(n) = node {
            if let Some(t) = self.cast::<T>(n) {
                return Some(t);
            }
            node = self.parent(n);
        }
        None
    }

    /// Dart `thisOrAncestorMatching(predicate)`.
    pub fn this_or_ancestor_matching(
        &self,
        id: impl Into<NodeId>,
        mut predicate: impl FnMut(&Ast, NodeId) -> bool,
    ) -> Option<NodeId> {
        let mut node = Some(id.into());
        while let Some(n) = node {
            if predicate(self, n) {
                return Some(n);
            }
            node = self.parent(n);
        }
        None
    }

    /// The elements of a node list.
    #[inline]
    pub fn list<T: ?Sized>(&self, list: NodeList<T>) -> &[Id<T>] {
        let raw = list.slots(&self.lists);
        // SAFETY: `Id<T>` is `repr(transparent)` over `NodeId`.
        unsafe { std::slice::from_raw_parts(raw.as_ptr() as *const Id<T>, raw.len()) }
    }

    /// The elements of a node list as untyped ids.
    #[inline]
    pub fn list_raw<T: ?Sized>(&self, list: NodeList<T>) -> &[NodeId] {
        list.slots(&self.lists)
    }

    #[inline]
    pub fn token_list(&self, list: TokenList) -> &[TokenId] {
        &self.token_lists[list.start as usize..(list.start + list.len) as usize]
    }

    /// Dart `NodeList.beginToken`.
    pub fn list_begin_token<T: ?Sized>(&self, list: NodeList<T>) -> Option<TokenId> {
        self.list_raw(list).first().map(|&n| self.begin_token(n))
    }

    /// Dart `NodeList.endToken`.
    pub fn list_end_token<T: ?Sized>(&self, list: NodeList<T>) -> Option<TokenId> {
        self.list_raw(list).last().map(|&n| self.end_token(n))
    }

    /// Dart `AstNode.offset`.
    pub fn offset(&self, id: impl Into<NodeId>) -> u32 {
        let id = id.into();
        if self.kind(id) == NodeKind::CompilationUnit {
            return 0;
        }
        self.tokens.offset(self.begin_token(id))
    }

    /// Dart `AstNode.end`.
    pub fn end(&self, id: impl Into<NodeId>) -> u32 {
        self.tokens.get(self.end_token(id)).end()
    }

    /// Dart `AstNode.length`.
    pub fn length(&self, id: impl Into<NodeId>) -> u32 {
        let id = id.into();
        self.end(id) - self.offset(id)
    }

    /// Dart `AstNode.childEntities`: the properties in order, sorted by
    /// offset when they are out of order (recovery), with the sort of the
    /// Dart VM (`List.sort`).
    pub fn child_entities(&self, id: impl Into<NodeId>) -> Vec<Entity> {
        let id = id.into();
        let mut out = Vec::new();
        self.child_entities_unsorted(id, &mut out);
        let offsets: Vec<u32> = out.iter().map(|e| self.entity_offset(*e)).collect();
        if offsets.windows(2).any(|w| w[0] > w[1]) {
            let mut pairs: Vec<(u32, Entity)> = offsets.into_iter().zip(out).collect();
            crate::sort::dart_sort(&mut pairs, |a, b| a.0 as i64 - b.0 as i64);
            out = pairs.into_iter().map(|p| p.1).collect();
        }
        out
    }

    /// The offset of a child entity.
    pub fn entity_offset(&self, e: Entity) -> u32 {
        match e {
            Entity::Node(n) => self.offset(n),
            Entity::Token(t) => self.tokens.offset(t),
        }
    }

    /// The child nodes in Dart `visitChildren` order.
    pub fn children(&self, id: impl Into<NodeId>) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.for_each_child(id, &mut |c| out.push(c));
        out
    }

    /// Dart `AstNode.visitChildren`.
    pub fn visit_children<V: crate::AstVisitor>(&self, id: impl Into<NodeId>, v: &mut V) {
        self.for_each_child(id, &mut |c| self.accept(c, v));
    }

    /// Dart `AstNode.visitChildren` for an [`crate::AstVisitorMut`]: the
    /// children are read first, then visited.
    pub fn visit_children_mut<V: crate::AstVisitorMut>(
        &mut self,
        id: impl Into<NodeId>,
        v: &mut V,
    ) {
        for c in self.children(id) {
            self.accept_mut(c, v);
        }
    }

    /// Dart `AstNode.visitChildren` for a [`crate::GeneralizingAstVisitor`].
    pub fn visit_children_generalizing<V: crate::GeneralizingAstVisitor>(
        &self,
        id: impl Into<NodeId>,
        v: &mut V,
    ) {
        self.for_each_child(id, &mut |c| self.accept_generalizing(c, v));
    }

    /// Dart `AstNode.replaceChild`: replaces [old], a child of [parent], with
    /// [new] and makes [parent] the parent of [new]. Panics like Dart (an
    /// `ArgumentError`, or a failed cast if [new] does not fit the slot).
    pub fn replace_child(
        &mut self,
        parent: impl Into<NodeId>,
        old: impl Into<NodeId>,
        new: impl Into<NodeId>,
    ) {
        let (parent, old, new) = (parent.into(), old.into(), new.into());
        if old == new {
            return;
        }
        if !self.replace_child_raw(parent, old, new) {
            panic!("The old node is not a child of this node.");
        }
        self.parents[new.index()] = Some(parent);
    }

    /// Dart `AstNode.replaceWith`: replaces [old] with [new] in the parent of
    /// [old].
    pub fn replace_with(&mut self, old: impl Into<NodeId>, new: impl Into<NodeId>) {
        let (old, new) = (old.into(), new.into());
        if old == new {
            return;
        }
        let parent = self
            .parent(old)
            .unwrap_or_else(|| panic!("The node is not a child of another node."));
        self.replace_child(parent, old, new);
    }

    /// Dart `AstNode.removeChild`: clears the nullable slot of [parent] that
    /// holds [old]. Errors like Dart for required slots and list elements.
    pub fn remove_child(
        &mut self,
        parent: impl Into<NodeId>,
        old: impl Into<NodeId>,
    ) -> Result<(), String> {
        self.remove_child_raw(parent.into(), old.into())
    }

    /// Dart `AstNode.removeFromParent`.
    pub fn remove_from_parent(&mut self, id: impl Into<NodeId>) -> Result<(), String> {
        let id = id.into();
        let parent = self
            .parent(id)
            .ok_or_else(|| "The node is not a child of another node.".to_string())?;
        self.remove_child_raw(parent, id)
    }

    /// Replaces element [index] of [list], owned by [owner] (Dart
    /// `NodeList.operator []=`).
    pub fn list_set<T: ?Sized>(
        &mut self,
        owner: impl Into<NodeId>,
        list: NodeList<T>,
        index: usize,
        new: Id<T>,
    ) {
        list.slots_mut(&mut self.lists)[index] = new.raw();
        self.parents[new.raw().index()] = Some(owner.into());
    }

    /// Dart `AstNodeImpl._containsOffset`: whether [id] contains the range
    /// `start..end`. An insertion point between this node and an adjacent
    /// identifier of another node is not contained.
    pub fn contains_offset(&self, id: impl Into<NodeId>, start: u32, end: u32) -> bool {
        let id = id.into();
        let begin_token = self.begin_token(id);
        let offset = self.tokens.offset(begin_token);
        let end_token = self.end_token(id);
        let node_end = self.tokens.get(end_token).end();
        if start == end {
            if start == offset {
                if let Some(previous) = self.tokens.previous(begin_token).get() {
                    let p = self.tokens.get(previous);
                    if start == p.end() && p.is_identifier() {
                        return false;
                    }
                }
            }
            if start == node_end {
                if let Some(next) = self.tokens.next(end_token).get() {
                    let n = self.tokens.get(next);
                    if start == n.offset && n.is_identifier() {
                        return false;
                    }
                }
            }
        }
        offset <= start && node_end >= end
    }

    /// Dart `NodeListImpl._elementContainingRange` (binary search).
    pub fn element_containing_range<T: ?Sized>(
        &self,
        list: NodeList<T>,
        start: u32,
        end: u32,
    ) -> Option<NodeId> {
        let elements = self.list_raw(list);
        let mut left: isize = 0;
        let mut right: isize = elements.len() as isize - 1;
        while left <= right {
            let middle = left + (right - left) / 2;
            let candidate = elements[middle as usize];
            if self.contains_offset(candidate, start, end) {
                return Some(candidate);
            }
            if end <= self.offset(candidate) {
                right = middle - 1;
            } else if self.end(candidate) <= start {
                left = middle + 1;
            } else {
                return None;
            }
        }
        None
    }

    /// Dart `CompilationUnit.nodeCovering`: the smallest node under [root]
    /// whose range includes `offset..offset + length`.
    pub fn node_covering(
        &self,
        root: impl Into<NodeId>,
        offset: u32,
        length: u32,
    ) -> Option<NodeId> {
        let root = root.into();
        let end = offset + length;
        if end > self.end(root) {
            return None;
        }
        let mut previous = root;
        while let Some(current) = self.child_containing_range(previous, offset, end) {
            previous = current;
        }
        Some(previous)
    }

    /// Begin token of an annotated node: Dart `_AnnotatedNodeMixin.beginToken`
    /// without the fallback.
    pub(crate) fn annotated_begin_token(
        &self,
        comment: Option<Id<Comment>>,
        metadata: NodeList<Annotation>,
    ) -> Option<TokenId> {
        crate::token::lexically_first(
            &self.tokens,
            &[
                comment.map(|c| self.begin_token(c)),
                self.list_begin_token(metadata),
            ],
        )
    }

    /// Dart `_AnnotatedNodeMixin._visitCommentAndAnnotations`.
    pub(crate) fn visit_comment_and_annotations(
        &self,
        comment: Option<Id<Comment>>,
        metadata: NodeList<Annotation>,
        f: &mut dyn FnMut(NodeId),
    ) {
        let items = self.list_raw(metadata);
        let comment_first = match (comment, items.first()) {
            (Some(c), Some(&a)) => self.offset(c) < self.offset(a),
            _ => true,
        };
        if comment_first {
            if let Some(c) = comment {
                f(c.raw());
            }
            for &a in items {
                f(a);
            }
        } else {
            let mut sorted: Vec<NodeId> = comment
                .map(|c| c.raw())
                .into_iter()
                .chain(items.iter().copied())
                .collect();
            crate::sort::dart_sort(&mut sorted, |a, b| {
                self.offset(*a) as i64 - self.offset(*b) as i64
            });
            for n in sorted {
                f(n);
            }
        }
    }

    /// Dart `CompilationUnitImpl.visitChildren` for the directives and
    /// declarations.
    pub(crate) fn visit_directives_and_declarations<D: ?Sized, M: ?Sized>(
        &self,
        directives: NodeList<D>,
        declarations: NodeList<M>,
        f: &mut dyn FnMut(NodeId),
    ) {
        let d = self.list_raw(directives);
        let m = self.list_raw(declarations);
        let in_order = match (d.last(), m.first()) {
            (Some(&a), Some(&b)) => self.offset(a) < self.offset(b),
            _ => true,
        };
        if in_order {
            for &n in d.iter().chain(m) {
                f(n);
            }
        } else {
            let mut sorted: Vec<NodeId> = d.iter().chain(m).copied().collect();
            crate::sort::dart_sort(&mut sorted, |a, b| {
                self.offset(*a) as i64 - self.offset(*b) as i64
            });
            for n in sorted {
                f(n);
            }
        }
    }
}

impl<T: Concrete> std::ops::Index<Id<T>> for Ast {
    type Output = T;

    #[inline]
    fn index(&self, id: Id<T>) -> &T {
        self.get(id)
    }
}

impl<T: Concrete> std::ops::IndexMut<Id<T>> for Ast {
    #[inline]
    fn index_mut(&mut self, id: Id<T>) -> &mut T {
        self.get_mut(id)
    }
}

/// A side table with one optional value per node, for data that later
/// phases attach to nodes (static types, elements, ...). Dart keeps these in
/// fields of the node classes (`staticType`, `element`, ...).
#[derive(Clone, Debug)]
pub struct NodeMap<V> {
    values: Vec<Option<V>>,
}

impl<V> Default for NodeMap<V> {
    fn default() -> Self {
        NodeMap { values: Vec::new() }
    }
}

impl<V> NodeMap<V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: impl Into<NodeId>) -> Option<&V> {
        self.values.get(id.into().index()).and_then(|v| v.as_ref())
    }

    pub fn insert(&mut self, id: impl Into<NodeId>, value: V) -> Option<V> {
        let i = id.into().index();
        if i >= self.values.len() {
            self.values.resize_with(i + 1, || None);
        }
        self.values[i].replace(value)
    }

    pub fn remove(&mut self, id: impl Into<NodeId>) -> Option<V> {
        let i = id.into().index();
        self.values.get_mut(i).and_then(|v| v.take())
    }

    /// The entries in node order.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &V)> {
        self.values
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|v| (NodeId::from_index(i), v)))
    }
}
