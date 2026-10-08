// Dart source: pkg/analyzer/lib/src/summary2/detach_nodes.dart (copy instead of detach)

//! Deep copy of a subtree from one [`Ast`] into another.
//!
//! The linker keeps expressions of a library cycle (constant initializers,
//! default values, annotations) in one `Ast` of the cycle, as detached
//! subtrees (Dart `detach_nodes.dart` detaches the nodes from their unit).
//! Here the nodes cannot move between arenas, so they are copied:
//!
//! - Tokens: the tokens from the begin token to the end token of the root
//!   (following `next`) are copied in order and linked with `next` /
//!   `previous`, so [`Ast::begin_token`], [`Ast::end_token`], token
//!   iteration and [`crate::to_source::to_source`] work on the copy. A token
//!   that a node references but that is outside this range (for example a
//!   comment token of a `Comment` node) is copied on its own. Each copy
//!   keeps the type, flags, offset and length, and has its own lexeme (see
//!   [`dartr_syntax::Tokens::push_copy`]), because the destination does not
//!   have the source text of the file. `end_group`, the replaced token of a
//!   replacement token, and `before_synthetic` point to the copy of their
//!   target when the target is copied, else they are none. Preceding
//!   comments are not copied.
//! - Nodes: generated per kind (`generated/copy.rs`): clone the node
//!   struct, map the token fields, copy the child nodes and node lists, keep
//!   the other fields, then [`Ast::add`]. The new root has no parent.
//!
//! The destination can hold many copied subtrees (from many source `Ast`s).

use std::collections::HashMap;

use dartr_syntax::{TokenId, TokenType};

use crate::arena::{Ast, Id, NodeId, NodeList, TokenList};

/// Copies subtrees of [`AstCopier::src`] into [`AstCopier::dst`].
pub struct AstCopier<'a> {
    pub(crate) src: &'a Ast,
    pub(crate) dst: &'a mut Ast,
    /// Source token -> copied token, for the subtree that is copied now.
    tokens: HashMap<TokenId, TokenId>,
}

impl<'a> AstCopier<'a> {
    pub fn new(src: &'a Ast, dst: &'a mut Ast) -> AstCopier<'a> {
        AstCopier {
            src,
            dst,
            tokens: HashMap::new(),
        }
    }

    /// Copies the subtree [node] of the source into the destination and
    /// returns the new root (its parent is none). Each call copies its own
    /// tokens: two copied subtrees do not share tokens.
    pub fn copy(&mut self, node: impl Into<NodeId>) -> NodeId {
        let node = node.into();
        self.tokens.clear();
        self.copy_token_range(node);
        let root = crate::generated::copy::copy_node(self, node);
        self.link_groups();
        root
    }

    /// Copies the tokens from the begin token to the end token of [node]
    /// and links them with `next` / `previous`.
    fn copy_token_range(&mut self, node: NodeId) {
        let src = &self.src.tokens;
        let end = self.src.end_token(node);
        let mut t = self.src.begin_token(node);
        let mut previous: Option<TokenId> = None;
        loop {
            let c = self.dst.tokens.push_copy(src, t);
            self.tokens.insert(t, c);
            if let Some(p) = previous {
                self.dst.tokens.set_next(p, c);
            }
            previous = Some(c);
            let next = src.next(t);
            if t == end || next.is_none() || src.ty(t) == TokenType::EOF {
                break;
            }
            t = next;
        }
    }

    /// Sets `end_group` and `before_synthetic` of the copied tokens to the
    /// copies of their targets (none when a target is not copied).
    fn link_groups(&mut self) {
        for (&s, &c) in &self.tokens {
            let from = self.src.tokens.get(s);
            let end_group = remap(&self.tokens, from.end_group);
            let before_synthetic = remap(&self.tokens, from.before_synthetic);
            let to = self.dst.tokens.get_mut(c);
            to.end_group = end_group;
            to.before_synthetic = before_synthetic;
        }
    }

    /// The copy of token [t] (copied on its own if it is not in the token
    /// range of the root).
    pub(crate) fn token(&mut self, t: TokenId) -> TokenId {
        if let Some(&c) = self.tokens.get(&t) {
            return c;
        }
        let c = self.dst.tokens.push_copy(&self.src.tokens, t);
        self.tokens.insert(t, c);
        c
    }

    /// The copy of the token list [list].
    pub(crate) fn token_list(&mut self, list: TokenList) -> TokenList {
        let items: Vec<TokenId> = self.src.token_list(list).to_vec();
        let copies: Vec<TokenId> = items.into_iter().map(|t| self.token(t)).collect();
        self.dst.new_token_list(copies)
    }

    /// The copy of the child node [id].
    pub(crate) fn node<T: ?Sized>(&mut self, id: Id<T>) -> Id<T> {
        Id::from_raw(crate::generated::copy::copy_node(self, id.raw()))
    }

    /// The copy of the node list [list] (a new list in the destination).
    pub(crate) fn list<T: ?Sized>(&mut self, list: NodeList<T>) -> NodeList<T> {
        let items: Vec<NodeId> = self.src.list_raw(list).to_vec();
        let copies: Vec<Id<T>> = items
            .into_iter()
            .map(|n| Id::from_raw(crate::generated::copy::copy_node(self, n)))
            .collect();
        self.dst.new_list(copies)
    }
}

fn remap(map: &HashMap<TokenId, TokenId>, t: TokenId) -> TokenId {
    if t.is_none() {
        return t;
    }
    map.get(&t).copied().unwrap_or(TokenId::NONE)
}

/// Copies the subtree [node] of [src] into [dst] and returns the new root
/// (its parent is none). See [`AstCopier`].
pub fn copy_subtree(src: &Ast, node: impl Into<NodeId>, dst: &mut Ast) -> NodeId {
    AstCopier::new(src, dst).copy(node)
}
