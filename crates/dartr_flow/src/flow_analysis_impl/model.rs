// Dart source: pkg/_fe_analyzer_shared/lib/src/flow_analysis/flow_analysis.dart
// (classes `FlowModel`, `FlowModelHelper`, `NonPromotionHistory`,
// `PromotionInfo`, `PromotionModel`, `Reachability`, `SsaNode`,
// `_PropertySsaNode`, `_DemotionResult`, `ExpressionInfo`, `_NullInfo`,
// `_Reference`, `_PropertyReference`, `TrivialVariableReference`)

//! The flow analysis data model (unit A9).
//!
//! # Identity
//!
//! The Dart code relies on object identity (`identical`, and `==` on classes
//! that don't override it) to detect when nothing changed. Here every
//! immutable Dart object is an [`Rc`] newtype ([`FlowModel`],
//! [`PromotionModel`], [`SsaNode`], [`Reachability`], [`ExpressionInfo`]),
//! and `identical` is `ptr_eq`. Type lists (`List<SharedTypeView>`) are
//! `Rc<[_]>`; in Dart every empty list used here is `const []` (so all empty
//! lists are identical), which [`list_identical`] reproduces.
//!
//! # Type parameters
//!
//! All types are generic over one parameter `F:` [`FlowTypes`], which
//! bundles the client's operations and node types.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::hash::Hash;
use std::ops::Deref;
use std::rc::Rc;

use indexmap::IndexMap;

use crate::flow_analysis::{NonPromotionReason, PromotionKey};
use crate::flow_analysis_operations::{FlowAnalysisOperations, FlowAnalysisTypeOperations};
use crate::flow_link::{FlowLink, FlowLinkReader, LinkRef, link_identical};
use crate::promotion_key_store::PromotionKeyStore;
use crate::shared_type::{SharedTypeOperations, SharedTypeView};
use crate::type_analyzer::TypeAnalyzerOptions;

/// The client types that flow analysis is generic over.
///
/// Dart `_FlowAnalysisImpl<Node, Statement extends Node, Expression extends
/// Node, Variable>` plus the operations. The Dart subtype relations
/// `Statement extends Node` and the test `node is Expression` (used by
/// `_write`) become [`statement_to_node`](Self::statement_to_node) and
/// [`is_expression`](Self::is_expression).
pub trait FlowTypes {
    /// The client's operations (gives `Variable`, the type, the property
    /// member and the name).
    type Ops: FlowAnalysisOperations;
    /// The client's AST node (Dart `Node`).
    type Node: Copy + Eq + Hash + fmt::Debug + 'static;
    /// The client's statement node (Dart `Statement extends Node`).
    type Statement: Copy + Eq + Hash + fmt::Debug + 'static;
    /// The client's expression node (Dart `Expression extends Node`).
    type Expression: Copy + Eq + Hash + fmt::Debug + 'static;

    /// Dart: the upcast `Statement` → `Node`.
    fn statement_to_node(statement: Self::Statement) -> Self::Node;

    /// Dart: `node is Expression`.
    fn is_expression(node: Self::Node) -> bool;
}

/// The client's type structure of `F`.
pub type TypeOfF<F> = <<F as FlowTypes>::Ops as SharedTypeOperations>::Type;
/// A [`SharedTypeView`] of `F`.
pub type TypeViewF<F> = SharedTypeView<TypeOfF<F>>;
/// The client's variable of `F`.
pub type VariableF<F> = <<F as FlowTypes>::Ops as FlowAnalysisOperations>::Variable;
/// The client's name of `F`.
pub type NameF<F> = <<F as FlowTypes>::Ops as SharedTypeOperations>::Name;
/// The client's property member of `F`.
pub type PropertyMemberF<F> = <<F as FlowTypes>::Ops as FlowAnalysisOperations>::PropertyMember;
/// A [`NonPromotionReason`] of `F`.
pub type NonPromotionReasonF<F> =
    NonPromotionReason<VariableF<F>, <F as FlowTypes>::Node, PropertyMemberF<F>, NameF<F>>;
/// An immutable list of types (Dart `List<SharedTypeView>`).
pub type TypeList<F> = Rc<[TypeViewF<F>]>;

/// Creates an empty type list (Dart `const []`).
pub fn empty_type_list<F: FlowTypes>() -> TypeList<F> {
    Rc::from(Vec::new())
}

/// Dart `identical(a, b)` for type lists. All empty lists are identical
/// (in Dart they are all `const []`).
pub fn list_identical<T>(a: &Rc<[T]>, b: &Rc<[T]>) -> bool {
    Rc::ptr_eq(a, b) || (a.is_empty() && b.is_empty())
}

macro_rules! rc_newtype {
    ($(#[$m:meta])* $name:ident, $data:ident) => {
        $(#[$m])*
        pub struct $name<F: FlowTypes>(Rc<$data<F>>);

        impl<F: FlowTypes> Clone for $name<F> {
            fn clone(&self) -> Self {
                $name(self.0.clone())
            }
        }

        impl<F: FlowTypes> Deref for $name<F> {
            type Target = $data<F>;
            fn deref(&self) -> &$data<F> {
                &self.0
            }
        }

        impl<F: FlowTypes> $name<F> {
            /// Dart `identical(this, other)`.
            pub fn ptr_eq(&self, other: &Self) -> bool {
                Rc::ptr_eq(&self.0, &other.0)
            }
        }
    };
}

/// Dart `identical(a, b)` (or `==` for a class that doesn't override it)
/// for two optional objects.
pub fn opt_ptr_eq<T: PtrEq>(a: Option<&T>, b: Option<&T>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.ptr_eq_dyn(b),
        _ => false,
    }
}

/// Helper trait for [`opt_ptr_eq`].
pub trait PtrEq {
    /// Dart `identical(this, other)`.
    fn ptr_eq_dyn(&self, other: &Self) -> bool;
}

impl<F: FlowTypes> PtrEq for SsaNode<F> {
    fn ptr_eq_dyn(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl<F: FlowTypes> PtrEq for FlowModel<F> {
    fn ptr_eq_dyn(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl<F: FlowTypes> PtrEq for PromotionModel<F> {
    fn ptr_eq_dyn(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

// ------------------------------------------------------------ FlowModelHelper

/// Convenience methods used by [`FlowModel`] and reference methods to access
/// variables in the flow analysis implementation (Dart mixin
/// `FlowModelHelper`, `@visibleForTesting`).
pub trait FlowModelHelper<F: FlowTypes> {
    /// [`FlowLinkReader`] object for efficiently looking up
    /// [`PromotionModel`] objects in [`FlowModel::promotion_info`]
    /// structures, or for computing the difference between two such
    /// structures. (Dart field of the mixin; interior mutability because the
    /// reader caches its current state.)
    fn reader(&self) -> &RefCell<FlowLinkReader<PromotionModel<F>>>;

    /// Returns the client's representation of the type `bool`.
    fn bool_type(&self) -> TypeViewF<F>;

    /// The [`PromotionKeyStore`], which tracks the unique integer assigned
    /// to everything in the control flow that might be promotable.
    fn promotion_key_store(&self) -> &PromotionKeyStore<VariableF<F>>;

    /// Language features enables affecting the behavior of flow analysis.
    fn type_analyzer_options(&self) -> &TypeAnalyzerOptions;

    /// The operations, used to access types and check subtyping.
    fn type_operations(&self) -> &F::Ops;

    /// Whether the variable of `variable_key` was declared with the `final`
    /// modifier and the `inference-update-4` feature flag is enabled.
    fn is_final(&self, variable_key: PromotionKey) -> bool;

    /// Determines whether a promotion from type `previous_type` to
    /// `new_type` is allowed to occur, given the current configuration of
    /// flow analysis.
    ///
    /// Caller is required to ensure that `new_type <: previous_type`.
    fn is_valid_promotion_step(&self, previous_type: TypeViewF<F>, new_type: TypeViewF<F>) -> bool;
}

// --------------------------------------------------------------- Reachability

/// Immutable data structure modeling the reachability of the given point in
/// the source code. Reachability is tracked relative to checkpoints occurring
/// previously along the control flow path leading up to the current point in
/// the program. A given point is said to be "locally reachable" if it is
/// reachable from the most recent checkpoint, and "overall reachable" if it
/// is reachable from the top of the function.
#[derive(Clone)]
pub struct Reachability(Rc<ReachabilityData>);

/// The fields of a [`Reachability`].
pub struct ReachabilityData {
    /// Reachability of the checkpoint this reachability is relative to, or
    /// `None` if there is no checkpoint. Reachabilities form a tree
    /// structure that mimics the control flow of the code being analyzed, so
    /// this is called the "parent".
    pub parent: Option<Reachability>,

    /// Whether this point in the source code is considered reachable from
    /// the most recent checkpoint.
    pub locally_reachable: bool,

    /// Whether this point in the source code is considered reachable from
    /// the beginning of the function being analyzed.
    pub overall_reachable: bool,

    /// The number of `parent` links between this node and
    /// [`Reachability::initial`].
    pub depth: usize,
}

impl Deref for Reachability {
    type Target = ReachabilityData;
    fn deref(&self) -> &ReachabilityData {
        &self.0
    }
}

thread_local! {
    static INITIAL_REACHABILITY: Reachability = Reachability(Rc::new(ReachabilityData {
        parent: None,
        locally_reachable: true,
        overall_reachable: true,
        depth: 0,
    }));
}

impl Reachability {
    /// Model of the initial reachability state of the function being
    /// analyzed (Dart `static const initial`; one instance per thread).
    pub fn initial() -> Reachability {
        INITIAL_REACHABILITY.with(|r| r.clone())
    }

    fn new(parent: Option<Reachability>, locally_reachable: bool, overall_reachable: bool) -> Self {
        let depth = parent.as_ref().map_or(0, |p| p.depth + 1);
        debug_assert_eq!(
            overall_reachable,
            locally_reachable && parent.as_ref().is_none_or(|p| p.overall_reachable)
        );
        Reachability(Rc::new(ReachabilityData {
            parent,
            locally_reachable,
            overall_reachable,
            depth,
        }))
    }

    /// Dart `identical(this, other)`.
    pub fn ptr_eq(&self, other: &Reachability) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Dart `identical(a, b)` for optional reachabilities.
    pub fn opt_ptr_eq(a: Option<&Reachability>, b: Option<&Reachability>) -> bool {
        match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a.ptr_eq(b),
            _ => false,
        }
    }

    /// Updates `this` reachability to account for the reachability of
    /// `base`.
    ///
    /// This is the reachability component of the algorithm in
    /// [`FlowModel::rebase_forward`].
    pub fn rebase_forward(&self, base: &Reachability) -> Reachability {
        // If [base] is not reachable, then the result is not reachable.
        if !base.locally_reachable {
            return base.clone();
        }
        // If any of the reachability nodes between `this` and its common
        // ancestor with [base] are locally unreachable, that means that there
        // was an exit in the flow control path from the point at which `this`
        // and [base] diverged up to the current point of `this`; therefore we
        // want to mark [base] as unreachable.
        let ancestor = Reachability::common_ancestor(Some(self), Some(base));
        let mut node = Some(self.clone());
        while let Some(s) = node {
            if Reachability::opt_ptr_eq(Some(&s), ancestor.as_ref()) {
                break;
            }
            if !s.locally_reachable {
                return base.set_unreachable();
            }
            node = s.parent.clone();
        }
        // Otherwise, the result is as reachable as [base] was.
        base.clone()
    }

    /// Returns a reachability with the same checkpoint as `this`, but where
    /// the current point in the program is considered locally unreachable.
    pub fn set_unreachable(&self) -> Reachability {
        if !self.locally_reachable {
            return self.clone();
        }
        Reachability::new(self.parent.clone(), false, false)
    }

    /// Returns a new reachability whose checkpoint is the current point of
    /// execution. This models flow control within a control flow split, e.g.
    /// inside an `if` statement.
    pub fn split(&self) -> Reachability {
        Reachability::new(Some(self.clone()), true, self.overall_reachable)
    }

    /// Returns a reachability that drops the most recent checkpoint but
    /// maintains the same notion of reachability relative to the previous
    /// two checkpoints.
    pub fn unsplit(&self) -> Reachability {
        if self.locally_reachable {
            self.parent.clone().unwrap()
        } else {
            self.parent.as_ref().unwrap().set_unreachable()
        }
    }

    /// Finds the common ancestor node of `r1` and `r2`, if any such node
    /// exists; otherwise `None`. If `r1` and `r2` are the same node, that
    /// node is returned.
    pub fn common_ancestor(
        r1: Option<&Reachability>,
        r2: Option<&Reachability>,
    ) -> Option<Reachability> {
        let (Some(r1), Some(r2)) = (r1, r2) else {
            return None;
        };
        let mut r1 = Some(r1.clone());
        let mut r2 = Some(r2.clone());
        while r1.as_ref().unwrap().depth > r2.as_ref().unwrap().depth {
            r1 = r1.unwrap().parent.clone();
        }
        while r2.as_ref().unwrap().depth > r1.as_ref().unwrap().depth {
            r2 = r2.unwrap().parent.clone();
        }
        while !Reachability::opt_ptr_eq(r1.as_ref(), r2.as_ref()) {
            r1 = r1.unwrap().parent.clone();
            r2 = r2.unwrap().parent.clone();
        }
        r1
    }
}

impl fmt::Debug for Reachability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut values = Vec::new();
        let mut node = Some(self);
        while let Some(n) = node {
            values.push(n.locally_reachable.to_string());
            node = n.parent.as_ref();
        }
        write!(f, "[{}]", values.join(", "))
    }
}

impl fmt::Display for Reachability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

// -------------------------------------------------------------------- SsaNode

thread_local! {
    static NEXT_SSA_DEBUG_ID: Cell<u32> = const { Cell::new(0) };
}

rc_newtype!(
    /// Data structure representing a unique value that a variable might take
    /// on during execution of the code being analyzed. SSA nodes are
    /// immutable (so they can be safety shared among data structures) and
    /// have identity (so that it is possible to tell whether one SSA node is
    /// the same as another).
    ///
    /// This is similar to the nodes used in traditional single assignment
    /// analysis except that it does not store a complete IR of the code
    /// being analyzed.
    ///
    /// Dart `_PropertySsaNode extends SsaNode` is an [`SsaNode`] whose
    /// [`property`](SsaNodeData::property) is set.
    SsaNode,
    SsaNodeData
);

/// The fields of an [`SsaNode`].
pub struct SsaNodeData<F: FlowTypes> {
    /// Flow analysis information was associated with the expression that
    /// produced the value represented by this SSA node, if it was
    /// non-trivial.
    ///
    /// This can be used at a later time to perform promotions if the value is
    /// used in a control flow construct. See
    /// [`ExpressionInfo::restore_condition_variable_state`].
    ///
    /// We don't bother storing flow analysis information if it's trivial
    /// (see [`ExpressionInfo`]) because such information does not lead to
    /// promotions.
    pub condition_variable_state: Option<ExpressionInfo<F>>,

    /// Map containing the set of promotable properties of the value tracked
    /// by this SSA node. Keys are the names of the properties.
    promotable_properties: RefCell<IndexMap<NameF<F>, SsaNode<F>>>,

    /// Map containing the set of non-promotable properties of the value
    /// tracked by this SSA node. These are tracked even though they're not
    /// promotable, so that if an error occurs due to the absence of type
    /// promotion, it will be possible to generate a message explaining to
    /// the user why type promotion failed.
    non_promotable_properties: RefCell<IndexMap<NameF<F>, SsaNode<F>>>,

    /// For a `_PropertySsaNode`: its promotion key and previous SSA node.
    pub property: Option<PropertySsaNodeData<F>>,

    /// Debug id, allocated by `Debug` (Dart `Expando` `_debugIds`).
    debug_id: Cell<Option<u32>>,
}

/// The fields added by Dart `_PropertySsaNode`: data structure representing
/// a unique value returned by the invocation of a property getter during
/// execution of the code being analyzed.
pub struct PropertySsaNodeData<F: FlowTypes> {
    /// The promotion key associated with this value. This allows for field
    /// promotion.
    pub promotion_key: PromotionKey,

    /// If this property is not promotable, then a fresh SSA node is assigned
    /// at the time of each access; when that occurs, this field points to
    /// the previous SSA node associated with the same property; otherwise it
    /// is `None`. This is used by the "why not promoted" logic to figure out
    /// what promotions *would* have occurred if the property had been
    /// promotable.
    pub previous_ssa_node: Option<SsaNode<F>>,
}

impl<F: FlowTypes> Default for SsaNode<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: FlowTypes> SsaNode<F> {
    /// `new SsaNode()`.
    pub fn new() -> Self {
        Self::with_condition_variable_state(None)
    }

    /// `new SsaNode(conditionVariableState: ...)`.
    pub fn with_condition_variable_state(
        condition_variable_state: Option<ExpressionInfo<F>>,
    ) -> Self {
        SsaNode(Rc::new(SsaNodeData {
            condition_variable_state,
            promotable_properties: RefCell::new(IndexMap::new()),
            non_promotable_properties: RefCell::new(IndexMap::new()),
            property: None,
            debug_id: Cell::new(None),
        }))
    }

    /// `new _PropertySsaNode(promotionKey, previousSsaNode: ...)`.
    pub fn new_property(
        promotion_key: PromotionKey,
        previous_ssa_node: Option<SsaNode<F>>,
    ) -> Self {
        SsaNode(Rc::new(SsaNodeData {
            condition_variable_state: None,
            promotable_properties: RefCell::new(IndexMap::new()),
            non_promotable_properties: RefCell::new(IndexMap::new()),
            property: Some(PropertySsaNodeData {
                promotion_key,
                previous_ssa_node,
            }),
            debug_id: Cell::new(None),
        }))
    }

    /// The promotion key of a `_PropertySsaNode`. Panics if this is not a
    /// property SSA node (Dart cast failure).
    pub fn property_promotion_key(&self) -> PromotionKey {
        self.property
            .as_ref()
            .expect("not a _PropertySsaNode")
            .promotion_key
    }

    /// A snapshot of the entries of `_promotableProperties`, in insertion
    /// order.
    pub fn promotable_properties(&self) -> Vec<(NameF<F>, SsaNode<F>)> {
        self.promotable_properties
            .borrow()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }

    /// Gets an SSA node representing the property named `property_name` of
    /// the value represented by `this`, creating it if necessary.
    ///
    /// If a new SSA node is created, it is allocated a fresh promotion key
    /// using `promotion_key_store`, so that type promotions for it can be
    /// tracked separately from other type promotions.
    pub fn get_or_create_property_node(
        &self,
        property_name: NameF<F>,
        promotion_key_store: &PromotionKeyStore<VariableF<F>>,
        is_promotable: bool,
    ) -> SsaNode<F> {
        if is_promotable {
            // The property is promotable, meaning it is known to produce the
            // same (or equivalent) value every time it is queried. So we only
            // create an SSA node if the property hasn't been accessed before;
            // otherwise we return the old SSA node unchanged.
            self.promotable_properties
                .borrow_mut()
                .entry(property_name)
                .or_insert_with(|| {
                    SsaNode::new_property(promotion_key_store.make_temporary_key(), None)
                })
                .clone()
        } else {
            // The property isn't promotable, meaning it is not known to
            // produce the same (or equivalent) value every time it is
            // queried. So we create a fresh SSA node for every access; but we
            // record the previous SSA node in `previous_ssa_node` so that the
            // "why not promoted" logic can figure out what promotions *would*
            // have occurred if the field had been promotable.
            let mut map = self.non_promotable_properties.borrow_mut();
            let previous_ssa_node = map.get(&property_name).cloned();
            let node =
                SsaNode::new_property(promotion_key_store.make_temporary_key(), previous_ssa_node);
            map.insert(property_name, node.clone());
            node
        }
    }

    /// Applies the property promotions from one SSA node to another. This is
    /// done as part of computing the effect of executing a try/finally's
    /// `try` and `finally` blocks in sequence, to apply the promotions that
    /// occurred in the `finally` block atop the promotions that occurred in
    /// the `try` block.
    ///
    /// `after_try_ssa_node` is the SSA node from the end of the `try` block,
    /// and `finally_ssa_node` is the SSA node from the end of the `finally`
    /// block (this method is only invoked when the variable in question was
    /// not written to in the `finally` block, so it is also the SSA node from
    /// the beginning of the `finally` block).
    ///
    /// `before_finally_info` is the promotion info map from the flow state at
    /// the beginning of the `finally` block, and `after_finally_info` is the
    /// promotion info map from the flow state at the end of the `finally`
    /// block. `new_flow_model` is the promotion info map for the flow state
    /// being built (the flow state after the try/finally block).
    pub(crate) fn apply_property_promotions(
        &self,
        helper: &dyn FlowModelHelper<F>,
        after_try_ssa_node: &SsaNode<F>,
        finally_ssa_node: &SsaNode<F>,
        before_finally_info: &PromotionInfoRef<F>,
        after_finally_info: &PromotionInfoRef<F>,
        mut new_flow_model: FlowModel<F>,
    ) -> FlowModel<F> {
        // TODO(paulberry): fix nomenclature to align with caller.
        for (property_name, finally_property_ssa_node) in finally_ssa_node.promotable_properties() {
            let finally_key = finally_property_ssa_node.property_promotion_key();
            // Since this method is only called when a variable is assigned in
            // a `try` block, a fresh SSA node should have been assigned for
            // the `finally` block by the conservative join in
            // `tryFinallyStatement_finallyBegin`. So the property should have
            // been unpromoted (and unknown) at the beginning of the `finally`
            // block.
            debug_assert!(promotion_info_get(before_finally_info, helper, finally_key).is_none());
            // Therefore all we need to do is apply any promotions that are in
            // force at the end of the `finally` block.
            let after_finally_model = promotion_info_get(after_finally_info, helper, finally_key);
            let after_try_property_ssa_node = after_try_ssa_node
                .promotable_properties
                .borrow_mut()
                .entry(property_name)
                .or_insert_with(|| {
                    SsaNode::new_property(helper.promotion_key_store().make_temporary_key(), None)
                })
                .clone();
            // Handle nested properties
            new_flow_model = self.apply_property_promotions(
                helper,
                &after_try_property_ssa_node,
                &finally_property_ssa_node,
                before_finally_info,
                after_finally_info,
                new_flow_model,
            );
            let Some(after_finally_model) = after_finally_model else {
                continue;
            };
            let after_finally_promoted_types = after_finally_model.promoted_types.clone();
            // The property was accessed in a promotion-relevant way in the
            // `try` block, so we need to apply the promotions from the
            // `finally` block to the flow model from the `try` block, and see
            // what sticks.
            let after_try_key = after_try_property_ssa_node.property_promotion_key();
            let new_model =
                match promotion_info_get(&new_flow_model.promotion_info, helper, after_try_key) {
                    Some(m) => m,
                    None => {
                        let m =
                            PromotionModel::fresh(false, Some(after_try_property_ssa_node.clone()));
                        new_flow_model =
                            new_flow_model.update_promotion_info(helper, after_try_key, m.clone());
                        m
                    }
                };
            let new_promoted_types = new_model.promoted_types.clone();
            let rebased_promoted_types = PromotionModel::rebase_promoted_types(
                &new_promoted_types,
                &after_finally_promoted_types,
                helper,
            );
            if !list_identical(&new_promoted_types, &rebased_promoted_types) {
                new_flow_model = new_flow_model.update_promotion_info(
                    helper,
                    after_try_key,
                    PromotionModel::new(
                        rebased_promoted_types,
                        new_model.tested.clone(),
                        true,
                        false,
                        new_model.ssa_node.clone(),
                        None,
                    ),
                );
            }
        }
        new_flow_model
    }

    /// Joins the promotion information for the promotable properties of two
    /// SSA nodes, `first` and `second`, and stores the results in
    /// `_promotableProperties`.
    ///
    /// Since properties may themselves be promoted, the caller must supply
    /// the promotion info maps for the two flow control paths being joined
    /// (`first_promotion_info` and `second_promotion_info`), as well as the
    /// promotion info map being built for the join point (`new_flow_model`).
    fn join_properties(
        &self,
        helper: &dyn FlowModelHelper<F>,
        first: &SsaNode<F>,
        first_promotion_info: &PromotionInfoRef<F>,
        second: &SsaNode<F>,
        second_promotion_info: &PromotionInfoRef<F>,
        mut new_flow_model: FlowModel<F>,
    ) -> FlowModel<F> {
        // If a property has been accessed along one of the two control flow
        // paths being joined, but not the other, then it shouldn't be
        // promoted after the join point, nor should any of its nested
        // properties. So it is only necessary to examine properties common to
        // the `first` and `second` maps.
        for (property_name, first_property) in first.promotable_properties() {
            let Some(second_property) = second
                .promotable_properties
                .borrow()
                .get(&property_name)
                .cloned()
            else {
                continue;
            };
            // Make a new promotion key to represent the joined property.
            let new_promotion_key = helper.promotion_key_store().make_temporary_key();
            // If the property has a promotion model along both control flow
            // paths, it might be promoted, so join the two promotion models to
            // preserve the promotion.
            let first_promotion_model = promotion_info_get(
                first_promotion_info,
                helper,
                first_property.property_promotion_key(),
            );
            let property_ssa_node = SsaNode::new_property(new_promotion_key, None);
            self.promotable_properties
                .borrow_mut()
                .insert(property_name, property_ssa_node.clone());
            if let Some(first_promotion_model) = first_promotion_model {
                let second_promotion_model = promotion_info_get(
                    second_promotion_info,
                    helper,
                    second_property.property_promotion_key(),
                );
                if let Some(second_promotion_model) = second_promotion_model {
                    let new_promotion_model;
                    (new_promotion_model, new_flow_model) = PromotionModel::join(
                        helper,
                        &first_promotion_model,
                        first_promotion_info,
                        &second_promotion_model,
                        second_promotion_info,
                        new_flow_model,
                        Some(&property_ssa_node),
                    );
                    new_flow_model = new_flow_model.update_promotion_info(
                        helper,
                        new_promotion_key,
                        new_promotion_model,
                    );
                }
            }
            // Join any nested properties.
            new_flow_model = property_ssa_node.join_properties(
                helper,
                &first_property,
                first_promotion_info,
                &second_property,
                second_promotion_info,
                new_flow_model,
            );
        }
        new_flow_model
    }

    /// Joins the promotion information for two SSA nodes, `first` and
    /// `second`.
    ///
    /// Since SSA nodes store information about properties, and properties
    /// may themselves be promoted, the caller must supply the promotion info
    /// maps for the two flow control paths being joined, as well as the
    /// promotion info map being built for the join point.
    fn join(
        helper: &dyn FlowModelHelper<F>,
        first: &SsaNode<F>,
        first_promotion_info: &PromotionInfoRef<F>,
        second: &SsaNode<F>,
        second_promotion_info: &PromotionInfoRef<F>,
        mut new_flow_model: FlowModel<F>,
    ) -> (SsaNode<F>, FlowModel<F>) {
        let ssa_node;
        if first.ptr_eq(second) {
            ssa_node = first.clone();
        } else {
            ssa_node = SsaNode::new();
            new_flow_model = ssa_node.join_properties(
                helper,
                first,
                first_promotion_info,
                second,
                second_promotion_info,
                new_flow_model,
            );
        }
        (ssa_node, new_flow_model)
    }
}

impl<F: FlowTypes> fmt::Debug for SsaNode<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id = match self.debug_id.get() {
            Some(id) => id,
            None => {
                let id = NEXT_SSA_DEBUG_ID.with(|n| {
                    let id = n.get();
                    n.set(id + 1);
                    id
                });
                self.debug_id.set(Some(id));
                id
            }
        };
        write!(f, "ssa{id}")
    }
}

// -------------------------------------------------------- NonPromotionHistory

/// Linked list node representing a set of reasons why a given expression
/// was not promoted.
///
/// We use a linked list representation because it is very efficient to
/// build; this means that in the "happy path" where no error occurs (so
/// non-promotion history is not needed) we do a minimal amount of work.
pub struct NonPromotionHistory<F: FlowTypes> {
    /// The type that was not promoted to.
    pub type_: TypeViewF<F>,

    /// The reason why the promotion didn't occur.
    pub non_promotion_reason: NonPromotionReasonF<F>,

    /// The previous link in the list.
    pub previous: Option<Rc<NonPromotionHistory<F>>>,
}

impl<F: FlowTypes> NonPromotionHistory<F> {
    /// `new NonPromotionHistory(type, nonPromotionReason, previous)`.
    pub fn new(
        type_: TypeViewF<F>,
        non_promotion_reason: NonPromotionReasonF<F>,
        previous: Option<Rc<NonPromotionHistory<F>>>,
    ) -> Rc<Self> {
        Rc::new(NonPromotionHistory {
            type_,
            non_promotion_reason,
            previous,
        })
    }
}

impl<F: FlowTypes> fmt::Debug for NonPromotionHistory<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut items = Vec::new();
        let mut link = Some(self);
        while let Some(l) = link {
            items.push(format!("{:?}: {:?}", l.type_, l.non_promotion_reason));
            link = l.previous.as_deref();
        }
        write!(f, "[{}]", items.join(", "))
    }
}

// ------------------------------------------------------ PromotionInfo / Model

/// Map-like data structure recording the [`PromotionModel`]s for each
/// promotable thing (variable, property, `this`, or `super`) being tracked
/// by flow analysis.
///
/// Each instance of [`PromotionInfo`] is an immutable key/value pair binding
/// a single promotion key (a unique integer assigned by
/// [`PromotionKeyStore`] to track a particular promotable thing) with an
/// instance of [`PromotionModel`] describing the promotion state of that
/// thing. Dart `PromotionInfo extends FlowLink<PromotionInfo>`; its `model`
/// is [`FlowLink::value`].
///
/// Flow analysis has no awareness of scope, so variables that are out of
/// scope are retained in the map until such time as their declaration no
/// longer dominates the control flow.
pub type PromotionInfo<F> = FlowLink<PromotionModel<F>>;

/// A possibly absent [`PromotionInfo`] (Dart `PromotionInfo?`).
pub type PromotionInfoRef<F> = LinkRef<PromotionModel<F>>;

/// Looks up the [`PromotionModel`] associated with `promotion_key` by
/// walking the linked list formed by `previous` to find the nearest link
/// whose key matches `promotion_key` (Dart `promotionInfo?.get(helper,
/// promotionKey)`).
pub fn promotion_info_get<F: FlowTypes>(
    info: &PromotionInfoRef<F>,
    helper: &dyn FlowModelHelper<F>,
    promotion_key: PromotionKey,
) -> Option<PromotionModel<F>> {
    helper
        .reader()
        .borrow_mut()
        .get(info, promotion_key as usize)
        .map(|link| link.value.clone())
}

rc_newtype!(
    /// An instance of the [`PromotionModel`] type represents the information
    /// gathered by flow analysis for a single variable or property at a
    /// single point in the control flow of the function or method being
    /// analyzed.
    ///
    /// Instances are immutable, so the methods below that "update" the state
    /// actually leave `this` unchanged and return a new state object.
    PromotionModel,
    PromotionModelData
);

/// The fields of a [`PromotionModel`].
pub struct PromotionModelData<F: FlowTypes> {
    /// Sequence of types that the variable or property has been promoted to,
    /// where each element of the sequence is a subtype of the previous.
    /// Empty if the variable or property hasn't been promoted.
    pub promoted_types: TypeList<F>,

    /// List of types that the variable has been tested against in all code
    /// paths leading to the given point in the source code. Not relevant for
    /// properties.
    pub tested: TypeList<F>,

    /// Indicates whether the variable has definitely been assigned. Not
    /// relevant for properties.
    pub assigned: bool,

    /// Indicates whether the variable is unassigned. Not relevant for
    /// properties.
    pub unassigned: bool,

    /// SSA node associated with this variable. Every time the variable's
    /// value potentially changes (either through an explicit write or a join
    /// with a control flow path that contains a write), this field is
    /// updated to point to a fresh node. Thus, it can be used to detect
    /// whether a variable's value has changed since a time in the past.
    ///
    /// `None` if the variable has been write captured.
    ///
    /// For promotable properties, this is is the property SSA node found in
    /// the target's `_promotableProperties` map.
    pub ssa_node: Option<SsaNode<F>>,

    /// Non-promotion history of this variable. Not relevant for properties.
    pub non_promotion_history: Option<Rc<NonPromotionHistory<F>>>,
}

/// Data structure representing the result of demoting a variable from one
/// type to another (Dart `_DemotionResult`).
struct DemotionResult<F: FlowTypes> {
    /// The new set of promoted types.
    promoted_types: TypeList<F>,

    /// The new non-promotion history (including the types that the variable
    /// is no longer promoted to).
    non_promotion_history: Option<Rc<NonPromotionHistory<F>>>,
}

impl<F: FlowTypes> PromotionModel<F> {
    /// `new PromotionModel(...)`.
    pub fn new(
        promoted_types: TypeList<F>,
        tested: TypeList<F>,
        assigned: bool,
        unassigned: bool,
        ssa_node: Option<SsaNode<F>>,
        non_promotion_history: Option<Rc<NonPromotionHistory<F>>>,
    ) -> Self {
        let write_captured = ssa_node.is_none();
        debug_assert!(
            !(assigned && unassigned),
            "Can't be both definitely assigned and unassigned"
        );
        debug_assert!(
            !write_captured || promoted_types.is_empty(),
            "Write-captured variables can't be promoted"
        );
        debug_assert!(
            !(write_captured && unassigned),
            "Write-captured variables can't be definitely unassigned"
        );
        PromotionModel(Rc::new(PromotionModelData {
            promoted_types,
            tested,
            assigned,
            unassigned,
            ssa_node,
            non_promotion_history,
        }))
    }

    /// Creates a [`PromotionModel`] representing a variable or property
    /// that's never been seen before (Dart `PromotionModel.fresh`, default
    /// `assigned = false`).
    pub fn fresh(assigned: bool, ssa_node: Option<SsaNode<F>>) -> Self {
        PromotionModel(Rc::new(PromotionModelData {
            promoted_types: empty_type_list::<F>(),
            tested: empty_type_list::<F>(),
            assigned,
            unassigned: !assigned,
            ssa_node,
            non_promotion_history: None,
        }))
    }

    /// Indicates whether the variable has been write captured. Not relevant
    /// for properties.
    pub fn write_captured(&self) -> bool {
        self.ssa_node.is_none()
    }

    /// Returns a new [`PromotionModel`] in which any promotions present have
    /// been dropped, and the variable has been marked as "not unassigned".
    ///
    /// Used by [`FlowModel::conservative_join`] to update the state of
    /// variables at the top of loops whose bodies write to them.
    pub fn discard_promotions_and_mark_not_unassigned(
        &self,
        non_promotion_reason: Option<NonPromotionReasonF<F>>,
    ) -> PromotionModel<F> {
        let mut new_non_promotion_history = None;
        if let Some(non_promotion_reason) = non_promotion_reason {
            new_non_promotion_history = self.non_promotion_history.clone();
            for i in (0..self.promoted_types.len()).rev() {
                new_non_promotion_history = Some(NonPromotionHistory::new(
                    self.promoted_types[i],
                    non_promotion_reason.clone(),
                    new_non_promotion_history,
                ));
            }
        }
        PromotionModel::new(
            empty_type_list::<F>(),
            self.tested.clone(),
            self.assigned,
            false,
            if self.write_captured() {
                None
            } else {
                Some(SsaNode::new())
            },
            new_non_promotion_history,
        )
    }

    /// Returns a new [`PromotionModel`] reflecting the fact that the variable
    /// was just written to.
    ///
    /// If there is any chance that the write will cause a demotion, the
    /// caller must pass in a non-null value for `non_promotion_reason`
    /// describing the reason for any potential demotion.
    pub fn write(
        &self,
        helper: &dyn FlowModelHelper<F>,
        non_promotion_reason: Option<&NonPromotionReasonF<F>>,
        _variable_key: PromotionKey,
        written_type: TypeViewF<F>,
        new_ssa_node: SsaNode<F>,
        promote_to_type_of_interest: bool,
        unpromoted_type: TypeViewF<F>,
    ) -> PromotionModel<F> {
        if self.write_captured() {
            return PromotionModel::new(
                self.promoted_types.clone(),
                self.tested.clone(),
                true,
                false,
                None,
                None,
            );
        }

        let demotion_result = self.demote_via_assignment(
            written_type,
            helper.type_operations(),
            non_promotion_reason,
        );
        let mut new_promoted_types = demotion_result.promoted_types;

        if promote_to_type_of_interest {
            new_promoted_types = self.try_promote_to_type_of_interest(
                helper,
                unpromoted_type,
                new_promoted_types,
                written_type,
            );
        }
        // TODO(paulberry): remove demotions from
        // demotionResult.nonPromotionHistory that are no longer in effect due
        // to re-promotion.
        if list_identical(&self.promoted_types, &new_promoted_types) && self.assigned {
            return PromotionModel::new(
                self.promoted_types.clone(),
                self.tested.clone(),
                self.assigned,
                self.unassigned,
                Some(new_ssa_node),
                None,
            );
        }

        let new_tested = if new_promoted_types.is_empty()
            && !self.promoted_types.is_empty()
            && !helper.type_analyzer_options().sound_flow_analysis_enabled
        {
            // A full demotion used to clear types of interest. This behavior
            // was removed as part of the sound-flow-analysis update (see
            // https://github.com/dart-lang/language/issues/4380).
            empty_type_list::<F>()
        } else {
            self.tested.clone()
        };

        PromotionModel::new(
            new_promoted_types,
            new_tested,
            true,
            false,
            Some(new_ssa_node),
            demotion_result.non_promotion_history,
        )
    }

    /// Returns a new [`PromotionModel`] reflecting the fact that the variable
    /// has been write-captured.
    pub fn write_capture(&self) -> PromotionModel<F> {
        PromotionModel::new(
            empty_type_list::<F>(),
            empty_type_list::<F>(),
            self.assigned,
            false,
            None,
            None,
        )
    }

    /// Computes the result of demoting this variable due to writing a value
    /// of type `written_type`.
    ///
    /// If there is any chance that the write will cause an actual demotion
    /// to occur, the caller must pass in a non-null value for
    /// `non_promotion_reason` describing the reason for the potential
    /// demotion.
    fn demote_via_assignment(
        &self,
        written_type: TypeViewF<F>,
        type_operations: &F::Ops,
        non_promotion_reason: Option<&NonPromotionReasonF<F>>,
    ) -> DemotionResult<F> {
        let promoted_types = &self.promoted_types;
        if promoted_types.is_empty() {
            return DemotionResult {
                promoted_types: empty_type_list::<F>(),
                non_promotion_history: self.non_promotion_history.clone(),
            };
        }

        let mut num_elements_to_keep = promoted_types.len();
        let mut new_non_promotion_history = self.non_promotion_history.clone();
        let mut new_promoted_types = empty_type_list::<F>();
        loop {
            if num_elements_to_keep == 0 {
                break;
            }
            let promoted = promoted_types[num_elements_to_keep - 1];
            if type_operations.is_subtype_of(written_type, promoted) {
                if num_elements_to_keep == promoted_types.len() {
                    new_promoted_types = promoted_types.clone();
                    break;
                }
                new_promoted_types = Rc::from(&promoted_types[..num_elements_to_keep]);
                break;
            }
            match non_promotion_reason {
                None => debug_assert!(false, "Demotion occurred but nonPromotionReason is null"),
                Some(reason) => {
                    new_non_promotion_history = Some(NonPromotionHistory::new(
                        promoted,
                        reason.clone(),
                        new_non_promotion_history,
                    ));
                }
            }
            num_elements_to_keep -= 1;
        }
        DemotionResult {
            promoted_types: new_promoted_types,
            non_promotion_history: new_non_promotion_history,
        }
    }

    /// Returns a promotion model that is the same as this one, but with the
    /// variable definitely assigned.
    pub(crate) fn set_assigned(&self) -> PromotionModel<F> {
        if self.assigned {
            self.clone()
        } else {
            PromotionModel::new(
                self.promoted_types.clone(),
                self.tested.clone(),
                true,
                false,
                self.ssa_node.clone(),
                self.non_promotion_history.clone(),
            )
        }
    }

    /// Determines whether a variable with the given `promoted_types` should
    /// be promoted to `written_type` based on types of interest. If it
    /// should, returns an updated promotion chain; otherwise returns
    /// `promoted_types` unchanged.
    ///
    /// Note that since promotion chains are considered immutable, if
    /// promotion is required, a new promotion chain will be created and
    /// returned.
    fn try_promote_to_type_of_interest(
        &self,
        helper: &dyn FlowModelHelper<F>,
        declared_type: TypeViewF<F>,
        promoted_types: TypeList<F>,
        written_type: TypeViewF<F>,
    ) -> TypeList<F> {
        debug_assert!(!self.write_captured());
        let ops = helper.type_operations();

        // Figure out if we have any promotion candidates (types that are a
        // supertype of writtenType and a proper subtype of the
        // currently-promoted type).  If at any point we find an exact match,
        // we take it immediately.
        let currently_promoted_type = promoted_types.last().copied().unwrap_or(declared_type);

        let mut result: Option<TypeList<F>> = None;
        let mut candidates: Option<Vec<TypeViewF<F>>> = None;

        let handle_type_of_interest =
            |ty: TypeViewF<F>,
             result: &mut Option<TypeList<F>>,
             candidates: &mut Option<Vec<TypeViewF<F>>>| {
                // If the written type is invalid, we assume no promotion.
                if ops.is_invalid_type(written_type) {
                    return;
                }

                // The written type must be a subtype of the type.
                if !ops.is_subtype_of(written_type, ty) {
                    return;
                }

                // Must be more specific that the currently promoted type.
                if !ops.is_subtype_of(ty, currently_promoted_type) {
                    return;
                }
                if !helper.is_valid_promotion_step(currently_promoted_type, ty) {
                    return;
                }

                // This is precisely the type we want to promote to; take it.
                if ty == written_type {
                    *result = Some(Self::add_to_promoted_types(&promoted_types, written_type));
                }

                match candidates {
                    None => {
                        *candidates = Some(vec![ty]);
                    }
                    Some(c) => {
                        // Add only unique candidates.
                        if !c.contains(&ty) {
                            c.push(ty);
                        }
                    }
                }
            };

        // The declared type is always a type of interest, but we never
        // promote to the declared type. So, try NonNull of it.
        let declared_type_non_null = ops.promote_to_non_null(declared_type);
        if declared_type_non_null != declared_type {
            handle_type_of_interest(declared_type_non_null, &mut result, &mut candidates);
            if let Some(result) = result {
                return result;
            }
        }

        for i in 0..self.tested.len() {
            let ty = self.tested[i];

            handle_type_of_interest(ty, &mut result, &mut candidates);
            if let Some(result) = result {
                return result;
            }

            let type_non_null = ops.promote_to_non_null(ty);
            if type_non_null != ty {
                handle_type_of_interest(type_non_null, &mut result, &mut candidates);
                if let Some(result) = result {
                    return result;
                }
            }
        }

        if let Some(candidates2) = candidates {
            // Figure out if we have a unique promotion candidate that's a
            // subtype of all the others.
            let mut promoted: Option<TypeViewF<F>> = None;
            'outer: for i in 0..candidates2.len() {
                for j in 0..candidates2.len() {
                    if j == i {
                        continue;
                    }
                    if !ops.is_subtype_of(candidates2[i], candidates2[j]) {
                        // Not a subtype of all the others.
                        continue 'outer;
                    }
                }
                if promoted.is_some() {
                    // Not unique.  Do not promote.
                    return promoted_types;
                } else {
                    promoted = Some(candidates2[i]);
                }
            }
            if let Some(promoted) = promoted {
                return Self::add_to_promoted_types(&promoted_types, promoted);
            }
        }
        // No suitable promotion found.
        promoted_types
    }

    /// Builds a [`PromotionModel`] based on `model`, but extending the
    /// `tested` set to include types from `tested`. This is used at the
    /// bottom of certain kinds of loops, to ensure that types tested within
    /// the body of the loop are consistently treated as "of interest" in
    /// code that follows the loop, regardless of the type of loop.
    pub fn inherit_tested(model: &PromotionModel<F>, tested: &TypeList<F>) -> PromotionModel<F> {
        let new_tested = Self::join_tested(tested, &model.tested);
        if list_identical(&new_tested, &model.tested) {
            return model.clone();
        }
        PromotionModel::new(
            model.promoted_types.clone(),
            new_tested,
            model.assigned,
            model.unassigned,
            model.ssa_node.clone(),
            None,
        )
    }

    /// Joins two promotion models. See [`FlowModel::join`] for details.
    ///
    /// Since properties of variables may be promoted, the caller must supply
    /// the promotion info maps for the two flow control paths being joined
    /// (`first_promotion_info` and `second_promotion_info`), as well as the
    /// promotion info map being built for the join point (`new_flow_model`).
    ///
    /// If a non-null `property_ssa_node` is supplied, it is used as the SSA
    /// node for the joined model, rather than joining the SSA nodes from
    /// `first` and `second`. This avoids redundant join operations for
    /// properties, since properties are joined recursively when this method
    /// is used on local variables.
    pub fn join(
        helper: &dyn FlowModelHelper<F>,
        first: &PromotionModel<F>,
        first_promotion_info: &PromotionInfoRef<F>,
        second: &PromotionModel<F>,
        second_promotion_info: &PromotionInfoRef<F>,
        mut new_flow_model: FlowModel<F>,
        property_ssa_node: Option<&SsaNode<F>>,
    ) -> (PromotionModel<F>, FlowModel<F>) {
        let type_operations = helper.type_operations();
        let new_promoted_types = Self::join_promoted_types(
            &first.promoted_types,
            &second.promoted_types,
            type_operations,
        );
        let new_assigned = first.assigned && second.assigned;
        let new_unassigned = first.unassigned && second.unassigned;
        let new_write_captured = first.write_captured() || second.write_captured();
        let new_tested = if new_write_captured {
            empty_type_list::<F>()
        } else {
            Self::join_tested(&first.tested, &second.tested)
        };
        let mut new_ssa_node = property_ssa_node.cloned();
        if new_ssa_node.is_none() && !new_write_captured {
            let node;
            (node, new_flow_model) = SsaNode::join(
                helper,
                first.ssa_node.as_ref().unwrap(),
                first_promotion_info,
                second.ssa_node.as_ref().unwrap(),
                second_promotion_info,
                new_flow_model,
            );
            new_ssa_node = Some(node);
        }
        let new_promotion_model = Self::identical_or_new(
            first,
            second,
            new_promoted_types,
            new_tested,
            new_assigned,
            new_unassigned,
            if new_write_captured {
                None
            } else {
                new_ssa_node
            },
        );
        (new_promotion_model, new_flow_model)
    }

    /// Performs the portion of the "join" algorithm that applies to
    /// promotion chains. Briefly, we intersect given chains. The chains are
    /// totally ordered subsets of a global partial order. Their intersection
    /// is a subset of each, and as such is also totally ordered.
    pub fn join_promoted_types(
        chain1: &TypeList<F>,
        chain2: &TypeList<F>,
        type_operations: &F::Ops,
    ) -> TypeList<F> {
        if chain1.is_empty() {
            return chain1.clone();
        }
        if chain2.is_empty() {
            return chain2.clone();
        }

        let mut index1 = 0;
        let mut index2 = 0;
        let mut skipped1 = false;
        let mut skipped2 = false;
        let mut result: Option<Vec<TypeViewF<F>>> = None;
        while index1 < chain1.len() && index2 < chain2.len() {
            let type1 = chain1[index1];
            let type2 = chain2[index2];
            if type1 == type2 {
                result.get_or_insert_with(Vec::new).push(type1);
                index1 += 1;
                index2 += 1;
            } else if type_operations.is_subtype_of(type2, type1) {
                index1 += 1;
                skipped1 = true;
            } else if type_operations.is_subtype_of(type1, type2) {
                index2 += 1;
                skipped2 = true;
            } else {
                skipped1 = true;
                skipped2 = true;
                break;
            }
        }

        if index1 == chain1.len() && !skipped1 {
            return chain1.clone();
        }
        if index2 == chain2.len() && !skipped2 {
            return chain2.clone();
        }
        match result {
            Some(r) => Rc::from(r),
            None => empty_type_list::<F>(),
        }
    }

    /// Performs the portion of the "join" algorithm that applies to
    /// promotion chains. Essentially this performs a set union, with the
    /// following caveats:
    /// - The "sets" are represented as lists (since they are expected to be
    ///   very small in real-world cases)
    /// - The sense of equality for the union operation is determined by `==`.
    /// - The types of interests lists are considered immutable.
    pub fn join_tested<T: Copy + PartialEq>(types1: &Rc<[T]>, types2: &Rc<[T]>) -> Rc<[T]> {
        // Ensure that types1 is the shorter list.
        let (types1, types2) = if types1.len() > types2.len() {
            (types2, types1)
        } else {
            (types1, types2)
        };
        // Determine the length of the common prefix the two lists share.
        let mut shared = 0;
        while shared < types1.len() {
            if types1[shared] != types2[shared] {
                break;
            }
            shared += 1;
        }
        // Use types2 as a starting point and add any entries from types1 that
        // are not present in it.
        let mut i = shared;
        while i < types1.len() {
            let type_to_add = types1[i];
            if types2.contains(&type_to_add) {
                i += 1;
                continue;
            }
            let mut result: Vec<T> = types2.to_vec();
            result.push(type_to_add);
            i += 1;
            while i < types1.len() {
                let type_to_add = types1[i];
                if !types2.contains(&type_to_add) {
                    result.push(type_to_add);
                }
                i += 1;
            }
            return Rc::from(result);
        }
        // No types needed to be added.
        types2.clone()
    }

    /// Forms a promotion chain by starting with `base_promotions` and
    /// applying promotions from `new_promotions` to it, to the extent
    /// possible without violating the usual ordering invariant (each
    /// promoted type must be a subtype of the previous).
    ///
    /// In degenerate cases, the returned chain will be identical to
    /// `new_promotions` or `base_promotions` (to make it easier for the
    /// caller to detect when data structures may be re-used).
    pub fn rebase_promoted_types(
        base_promotions: &TypeList<F>,
        new_promotions: &TypeList<F>,
        helper: &dyn FlowModelHelper<F>,
    ) -> TypeList<F> {
        if base_promotions.is_empty() {
            // The base promotion chain contributes nothing so we just use
            // this promotion chain directly.
            new_promotions.clone()
        } else if new_promotions.is_empty() {
            // This promotion chain contributes nothing so we just use the
            // base promotion chain directly. Note: this is a performance
            // optimization of the `else` block below; it is not required by
            // the spec.
            base_promotions.clone()
        } else {
            // Start with basePromotedTypes and apply each of the promotions
            // in thisPromotedTypes (discarding any that don't follow the
            // ordering invariant)
            let base_promoted_type = *base_promotions.last().unwrap();
            for i in 0..new_promotions.len() {
                let next_type = new_promotions[i];
                // Determine if `nextType` is safe to attach to
                // `basePromotedTypes`.
                if helper
                    .type_operations()
                    .is_subtype_of(next_type, base_promoted_type)
                    && helper.is_valid_promotion_step(base_promoted_type, next_type)
                {
                    // Since `newPromotions` is a valid promotion chain, it
                    // follows that all the types that follow `nextType` are
                    // also safe to attach to the base promotion chain, so
                    // simply concatenate `basePromotions` with the remainder
                    // of `newPromotions`.
                    let mut result = base_promotions.to_vec();
                    result.extend_from_slice(&new_promotions[i..]);
                    return Rc::from(result);
                }
            }
            // No types from `newPromotions` were safe to attach to
            // `basePromotedTypes`, so return `basePromotions` unchanged.
            base_promotions.clone()
        }
    }

    fn add_to_promoted_types(promoted_types: &TypeList<F>, promoted: TypeViewF<F>) -> TypeList<F> {
        let mut result = promoted_types.to_vec();
        result.push(promoted);
        Rc::from(result)
    }

    fn add_type_to_unique_list(types: &TypeList<F>, new_type: TypeViewF<F>) -> TypeList<F> {
        if types.contains(&new_type) {
            return types.clone();
        }
        let mut result = types.to_vec();
        result.push(new_type);
        Rc::from(result)
    }

    /// Creates a new [`PromotionModel`] object, unless it is equivalent to
    /// either `first` or `second`, in which case one of those objects is
    /// re-used.
    pub(crate) fn identical_or_new(
        first: &PromotionModel<F>,
        second: &PromotionModel<F>,
        new_promoted_types: TypeList<F>,
        new_tested: TypeList<F>,
        new_assigned: bool,
        new_unassigned: bool,
        new_ssa_node: Option<SsaNode<F>>,
    ) -> PromotionModel<F> {
        if list_identical(&first.promoted_types, &new_promoted_types)
            && list_identical(&first.tested, &new_tested)
            && first.assigned == new_assigned
            && first.unassigned == new_unassigned
            && opt_ptr_eq(first.ssa_node.as_ref(), new_ssa_node.as_ref())
        {
            first.clone()
        } else if list_identical(&second.promoted_types, &new_promoted_types)
            && list_identical(&second.tested, &new_tested)
            && second.assigned == new_assigned
            && second.unassigned == new_unassigned
            && opt_ptr_eq(second.ssa_node.as_ref(), new_ssa_node.as_ref())
        {
            second.clone()
        } else {
            PromotionModel::new(
                new_promoted_types,
                new_tested,
                new_assigned,
                new_unassigned,
                new_ssa_node,
                None,
            )
        }
    }
}

impl<F: FlowTypes> fmt::Debug for PromotionModel<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = vec![match &self.ssa_node {
            Some(n) => format!("{n:?}"),
            None => "null".to_string(),
        }];
        if !self.promoted_types.is_empty() {
            parts.push(format!("promotedTypes: {:?}", &self.promoted_types[..]));
        }
        if !self.tested.is_empty() {
            parts.push(format!("tested: {:?}", &self.tested[..]));
        }
        if self.assigned {
            parts.push("assigned: true".to_string());
        }
        if !self.unassigned {
            parts.push("unassigned: false".to_string());
        }
        if self.write_captured() {
            parts.push("writeCaptured: true".to_string());
        }
        if let Some(h) = &self.non_promotion_history {
            parts.push(format!("nonPromotionHistory: {h:?}"));
        }
        write!(f, "PromotionModel({})", parts.join(", "))
    }
}

// ------------------------------------------------------------------ FlowModel

rc_newtype!(
    /// The state of flow analysis at one point of the control flow: the
    /// reachability and the [`PromotionInfo`] map.
    FlowModel,
    FlowModelData
);

/// The fields of a [`FlowModel`].
pub struct FlowModelData<F: FlowTypes> {
    /// The reachability of this point.
    pub reachable: Reachability,

    /// [`PromotionInfo`] object tracking the [`PromotionModel`]s for each
    /// promotable thing being tracked by flow analysis.
    pub promotion_info: PromotionInfoRef<F>,
}

impl<F: FlowTypes> FlowModel<F> {
    /// Creates a state object with the given `reachable` status. All
    /// variables are assumed to be unpromoted and already assigned, so
    /// joining another state with this one will have no effect on it.
    pub fn new(reachable: Reachability) -> Self {
        Self::with_info(reachable, None)
    }

    /// `FlowModel.withInfo` (`@visibleForTesting`).
    pub fn with_info(reachable: Reachability, promotion_info: PromotionInfoRef<F>) -> Self {
        FlowModel(Rc::new(FlowModelData {
            reachable,
            promotion_info,
        }))
    }

    /// Updates the state to indicate that the given `written_variables` are
    /// no longer promoted and are no longer definitely unassigned, and the
    /// given `captured_variables` have been captured by closures.
    ///
    /// This is used at the top of loops to conservatively cancel the
    /// promotion of variables that are modified within the loop.
    ///
    /// Note that a more accurate analysis would be to iterate to a fixed
    /// point, and only remove promotions if it can be shown that they aren't
    /// restored later in the loop body. If we switch to a fixed point
    /// analysis, we should be able to remove this method.
    pub fn conservative_join(
        &self,
        helper: &dyn FlowModelHelper<F>,
        written_variables: impl IntoIterator<Item = PromotionKey>,
        captured_variables: impl IntoIterator<Item = PromotionKey>,
        get_non_promotion_reason: Option<&dyn Fn(PromotionKey) -> Option<NonPromotionReasonF<F>>>,
    ) -> FlowModel<F> {
        let mut result = self.clone();

        for variable_key in written_variables {
            let Some(info) = promotion_info_get(&result.promotion_info, helper, variable_key)
            else {
                continue;
            };

            // We don't need to discard promotions for final variables. They
            // are guaranteed to be already assigned and won't be assigned
            // again.
            if helper.is_final(variable_key) {
                continue;
            }

            let new_info = info.discard_promotions_and_mark_not_unassigned(
                get_non_promotion_reason.and_then(|f| f(variable_key)),
            );
            if !info.ptr_eq(&new_info) {
                result = result.update_promotion_info(helper, variable_key, new_info);
            }
        }

        for variable_key in captured_variables {
            let Some(info) = promotion_info_get(&result.promotion_info, helper, variable_key)
            else {
                continue;
            };
            if !info.write_captured() {
                result = result.update_promotion_info(helper, variable_key, info.write_capture());
                // Note: there's no need to discard dependent property
                // promotions, because when deciding whether a property is
                // promoted, `_handleProperty` checks whether the variable is
                // captured.
            }
        }

        result
    }

    /// Register a declaration of the variable whose key is `variable_key`.
    /// Should also be called for function parameters.
    ///
    /// A local variable is `initialized` if its declaration has an
    /// initializer. A function parameter is always initialized, so
    /// `initialized` is `true`.
    pub fn declare(
        &self,
        helper: &dyn FlowModelHelper<F>,
        variable_key: PromotionKey,
        initialized: bool,
    ) -> FlowModel<F> {
        let new_info_for_var = PromotionModel::fresh(initialized, Some(SsaNode::new()));
        self.update_promotion_info(helper, variable_key, new_info_for_var)
    }

    /// Gets the info for the given `promotion_key`, creating it if it
    /// doesn't exist.
    ///
    /// If new info must be created, `ssa_node` is used as its SSA node. This
    /// allows the caller to ensure that when the promotion key represents a
    /// promotable property, the SSA node will match the property SSA node
    /// found in the target's `_promotableProperties` map.
    pub fn info_for(
        &self,
        helper: &dyn FlowModelHelper<F>,
        promotion_key: PromotionKey,
        ssa_node: &SsaNode<F>,
    ) -> PromotionModel<F> {
        promotion_info_get(&self.promotion_info, helper, promotion_key)
            .unwrap_or_else(|| PromotionModel::fresh(false, Some(ssa_node.clone())))
    }

    /// Builds a [`FlowModel`] based on `this`, but extending the `tested`
    /// set to include types from `other`. This is used at the bottom of
    /// certain kinds of loops, to ensure that types tested within the body
    /// of the loop are consistently treated as "of interest" in code that
    /// follows the loop, regardless of the type of loop.
    pub fn inherit_tested(
        &self,
        helper: &dyn FlowModelHelper<F>,
        other: &FlowModel<F>,
    ) -> FlowModel<F> {
        let mut result = self.clone();
        let entries = helper
            .reader()
            .borrow_mut()
            .diff(&self.promotion_info, &other.promotion_info)
            .entries;
        for entry in entries {
            let promotion_key = entry.key as PromotionKey;
            let Some(promotion_model) = entry.left().map(|l| l.value.clone()) else {
                continue;
            };
            let other_promotion_model = entry.right().map(|l| l.value.clone());
            let new_promotion_model = match other_promotion_model {
                None => promotion_model.clone(),
                Some(other) => PromotionModel::inherit_tested(&promotion_model, &other.tested),
            };
            if !new_promotion_model.ptr_eq(&promotion_model) {
                result = result.update_promotion_info(helper, promotion_key, new_promotion_model);
            }
        }
        result
    }

    /// Updates `this` flow model to account for any promotions and
    /// assignments present in `base`.
    ///
    /// This is called "rebasing" the flow model by analogy to "git rebase";
    /// in effect, it rewinds any flow analysis state present in `this` but
    /// not in the history of `base`, and then reapplies that state using
    /// `base` as a starting point, to the extent possible without creating
    /// unsoundness. For example, if a variable is promoted in `this` but not
    /// in `base`, then it will be promoted in the output model, provided that
    /// hasn't been reassigned since then (which would make the promotion
    /// unsound).
    pub fn rebase_forward(
        &self,
        helper: &dyn FlowModelHelper<F>,
        base: &FlowModel<F>,
    ) -> FlowModel<F> {
        // The rebased model is reachable iff both `this` and the new base are
        // reachable.
        let new_reachable = self.reachable.rebase_forward(&base.reachable);
        let mut result = base.set_reachability(new_reachable.clone());

        let diff = helper
            .reader()
            .borrow_mut()
            .diff(&self.promotion_info, &base.promotion_info);
        // If `this` matches the ancestor, then there are no state changes
        // that need to be rewound and applied to `base`.
        if link_identical(&diff.ancestor, &self.promotion_info) {
            return result;
        }
        // If `base` matches the ancestor, then the act of rewinding `this`
        // back to the ancestor, and then reapplying the rewound changes to
        // `base`, reproduces `this` exactly (assuming reachability matches up
        // properly).
        if link_identical(&base.promotion_info, &diff.ancestor)
            && self.reachable.ptr_eq(&new_reachable)
        {
            return self.clone();
        }
        // Consider each promotion key in the new base model.
        for entry in diff.entries {
            let promotion_key = entry.key as PromotionKey;
            let Some(this_model) = entry.left().map(|l| l.value.clone()) else {
                // Either this promotion key represents a variable that has
                // newly come into scope since `thisModel`, or it represents a
                // property that flow analysis became aware of since
                // `thisModel`. In either case, the information in `baseModel`
                // is up to date.
                continue;
            };
            let Some(base_model) = entry.right().map(|l| l.value.clone()) else {
                // The promotion key exists in `this` model but not in the new
                // `base` model. This happens when either:
                // - The promotion key is associated with a local variable
                //   that was in scope at the time `this` model was created,
                //   but is no longer in scope as of the `base` model, or:
                // - The promotion key is associated with a property that was
                //   promoted in `this` model.
                //
                // In the first case, it doesn't matter what we do, because
                // the variable is no longer in scope. But in the second case,
                // we need to preserve the promotion.
                result = result.update_promotion_info(helper, promotion_key, this_model);
                continue;
            };
            // If the variable was write captured in either `this` or the new
            // base, it's captured now.
            let new_write_captured = this_model.write_captured() || base_model.write_captured();
            let new_promoted_types = if new_write_captured {
                // Write captured variables can't be promoted.
                empty_type_list::<F>()
            } else if !opt_ptr_eq(base_model.ssa_node.as_ref(), this_model.ssa_node.as_ref()) {
                // The variable may have been written to since `thisModel`, so
                // we can't use any of the promotions from `thisModel`.
                base_model.promoted_types.clone()
            } else {
                // The variable hasn't been written to since `thisModel`, so we
                // can keep all of the promotions from `thisModel`, provided
                // that we retain the usual "promotion chain" invariant (each
                // promoted type is a subtype of the previous).
                PromotionModel::rebase_promoted_types(
                    &base_model.promoted_types,
                    &this_model.promoted_types,
                    helper,
                )
            };
            // Tests are kept regardless of whether they are in `this` model
            // or the new base model.
            let new_tested =
                PromotionModel::<F>::join_tested(&this_model.tested, &base_model.tested);
            // The variable is definitely assigned if it was definitely
            // assigned either in `this` model or the new base model.
            let new_assigned = this_model.assigned || base_model.assigned;
            // The variable is definitely unassigned if it was definitely
            // unassigned in both `this` model and the new base model.
            let new_unassigned = this_model.unassigned && base_model.unassigned;
            let new_model = PromotionModel::identical_or_new(
                &this_model,
                &base_model,
                new_promoted_types,
                new_tested,
                new_assigned,
                new_unassigned,
                if new_write_captured {
                    None
                } else {
                    base_model.ssa_node.clone()
                },
            );
            result = result.update_promotion_info(helper, promotion_key, new_model);
        }
        result
    }

    /// Returns a model with the given reachability (or `this` if it already
    /// has it).
    pub fn set_reachability(&self, reachable: Reachability) -> FlowModel<F> {
        if self.reachable.ptr_eq(&reachable) {
            return self.clone();
        }
        FlowModel::with_info(reachable, self.promotion_info.clone())
    }

    /// Updates the state to indicate that the control flow path is
    /// unreachable.
    pub fn set_unreachable(&self) -> FlowModel<F> {
        if !self.reachable.locally_reachable {
            return self.clone();
        }
        FlowModel::with_info(
            self.reachable.set_unreachable(),
            self.promotion_info.clone(),
        )
    }

    /// Returns a [`FlowModel`] indicating the result of creating a control
    /// flow split. See [`Reachability::split`] for more information.
    pub fn split(&self) -> FlowModel<F> {
        FlowModel::with_info(self.reachable.split(), self.promotion_info.clone())
    }

    /// Returns an [`ExpressionInfo`] indicating the result of checking
    /// whether the given `reference` is non-null.
    ///
    /// Note that the state is only changed if the previous type of
    /// `reference` was potentially nullable.
    pub fn try_mark_non_nullable(
        &self,
        helper: &dyn FlowModelHelper<F>,
        reference: &ExpressionInfo<F>,
    ) -> ExpressionInfo<F> {
        let r = reference.reference_data();
        let info = self.info_for(helper, r.promotion_key, &r.ssa_node);
        if info.write_captured() {
            return ExpressionInfo::trivial(helper.bool_type(), self.clone());
        }

        let previous_type = reference.type_;
        let new_type = helper.type_operations().promote_to_non_null(previous_type);
        if !helper.is_valid_promotion_step(previous_type, new_type) {
            return ExpressionInfo::trivial(helper.bool_type(), self.clone());
        }

        let if_true = self.finish_type_test(helper, reference, &info, None, Some(new_type));

        ExpressionInfo::new(helper.bool_type(), if_true, self.clone())
    }

    /// Returns a [`FlowModel`] indicating the result of casting the given
    /// `reference` to the given `ty`, as a consequence of an `as` expression.
    ///
    /// Note that the state is only changed if `ty` is a subtype of the
    /// reference's previous (possibly promoted) type.
    pub fn try_promote_for_type_cast(
        &self,
        helper: &dyn FlowModelHelper<F>,
        reference: &ExpressionInfo<F>,
        ty: TypeViewF<F>,
    ) -> FlowModel<F> {
        let r = reference.reference_data();
        let info = self.info_for(helper, r.promotion_key, &r.ssa_node);
        if info.write_captured() {
            return self.clone();
        }

        let previous_type = reference.type_;
        let new_type = helper
            .type_operations()
            .try_promote_to_type(ty, previous_type);
        let Some(new_type) = new_type else {
            return self.clone();
        };
        if !helper.is_valid_promotion_step(previous_type, new_type) {
            return self.clone();
        }

        self.finish_type_test(helper, reference, &info, Some(ty), Some(new_type))
    }

    /// Returns an [`ExpressionInfo`] indicating the result of checking
    /// whether the given `reference` satisfies the given `ty`, e.g. as a
    /// consequence of an `is` expression as the condition of an `if`
    /// statement.
    ///
    /// Note that the "ifTrue" state is only changed if `ty` is a subtype of
    /// the variable's previous (possibly promoted) type.
    pub fn try_promote_for_type_check(
        &self,
        helper: &dyn FlowModelHelper<F>,
        reference: &ExpressionInfo<F>,
        ty: TypeViewF<F>,
    ) -> ExpressionInfo<F> {
        let r = reference.reference_data();
        let info = self.info_for(helper, r.promotion_key, &r.ssa_node);
        if info.write_captured() {
            return ExpressionInfo::trivial(helper.bool_type(), self.clone());
        }

        let ops = helper.type_operations();
        let previous_type = reference.type_;
        let mut if_true = self.clone();
        let type_if_success = ops.try_promote_to_type(ty, previous_type);
        if let Some(type_if_success) = type_if_success
            && helper.is_valid_promotion_step(previous_type, type_if_success)
        {
            if_true =
                self.finish_type_test(helper, reference, &info, Some(ty), Some(type_if_success));
        }

        let factored_type = ops.factor(previous_type, ty);
        let type_if_false;
        let mut if_false_is_unreachable = false;
        if ops.is_bottom_type(factored_type) {
            // Do not promote to `Never` (even if it would be sound to do so);
            // it's not useful.
            type_if_false = None;
            // If not sound, it might still be reachable.
            if_false_is_unreachable = helper.type_analyzer_options().sound_flow_analysis_enabled;
        } else if !helper.is_valid_promotion_step(previous_type, factored_type) {
            // Don't promote.
            type_if_false = None;
        } else {
            type_if_false = Some(factored_type);
        }
        let mut if_false = self.finish_type_test(helper, reference, &info, Some(ty), type_if_false);

        if if_false_is_unreachable {
            if_false = if_false.set_unreachable();
        }

        ExpressionInfo::new(helper.bool_type(), if_true, if_false)
    }

    /// Returns a [`FlowModel`] indicating the result of removing a control
    /// flow split. See [`Reachability::unsplit`] for more information.
    pub fn unsplit(&self) -> FlowModel<F> {
        FlowModel::with_info(self.reachable.unsplit(), self.promotion_info.clone())
    }

    /// Removes control flow splits until a [`FlowModel`] is obtained whose
    /// reachability has the given `parent`.
    pub fn unsplit_to(&self, parent: &Reachability) -> FlowModel<F> {
        if Reachability::opt_ptr_eq(self.reachable.parent.as_ref(), Some(parent)) {
            return self.clone();
        }
        let mut reachable = self.reachable.unsplit();
        while !Reachability::opt_ptr_eq(reachable.parent.as_ref(), Some(parent)) {
            reachable = reachable.unsplit();
        }
        FlowModel::with_info(reachable, self.promotion_info.clone())
    }

    /// Returns a new [`FlowModel`] where the information for `promotion_key`
    /// is replaced with `model`.
    pub fn update_promotion_info(
        &self,
        helper: &dyn FlowModelHelper<F>,
        promotion_key: PromotionKey,
        model: PromotionModel<F>,
    ) -> FlowModel<F> {
        let previous_for_key = helper
            .reader()
            .borrow_mut()
            .get(&self.promotion_info, promotion_key as usize);
        let new_promotion_info = Rc::new(FlowLink::new(
            promotion_key as usize,
            self.promotion_info.clone(),
            previous_for_key,
            model,
        ));
        FlowModel::with_info(self.reachable.clone(), Some(new_promotion_info))
    }

    /// Updates the state to indicate that an assignment was made to the
    /// variable whose key is `variable_key`. The variable is marked as
    /// definitely assigned, and any previous type promotion is removed.
    ///
    /// If there is any chance that the write will cause a demotion, the
    /// caller must pass in a non-null value for `non_promotion_reason`
    /// describing the reason for any potential demotion.
    ///
    /// Dart default: `promoteToTypeOfInterest = true`.
    pub fn write(
        &self,
        helper: &dyn FlowModelHelper<F>,
        non_promotion_reason: Option<&NonPromotionReasonF<F>>,
        variable_key: PromotionKey,
        written_type: TypeViewF<F>,
        new_ssa_node: SsaNode<F>,
        promote_to_type_of_interest: bool,
        unpromoted_type: TypeViewF<F>,
    ) -> FlowModel<F> {
        let mut new_model = None;
        if let Some(info_for_var) = promotion_info_get(&self.promotion_info, helper, variable_key) {
            let new_info_for_var = info_for_var.write(
                helper,
                non_promotion_reason,
                variable_key,
                written_type,
                new_ssa_node,
                promote_to_type_of_interest,
                unpromoted_type,
            );
            if !new_info_for_var.ptr_eq(&info_for_var) {
                new_model =
                    Some(self.update_promotion_info(helper, variable_key, new_info_for_var));
            }
        }

        new_model.unwrap_or_else(|| self.clone())
    }

    /// Common algorithm for [`try_mark_non_nullable`](Self::try_mark_non_nullable),
    /// [`try_promote_for_type_cast`](Self::try_promote_for_type_cast), and
    /// [`try_promote_for_type_check`](Self::try_promote_for_type_check).
    /// Builds a [`FlowModel`] object describing the effect of updating the
    /// `reference` by adding the `tested_type` to the list of tested types
    /// (if not `None`, and not there already), adding the `promoted_type` to
    /// the chain of promoted types.
    ///
    /// Preconditions:
    /// - `info` should be the result of calling [`info_for`](Self::info_for)
    ///   on the reference.
    /// - `promoted_type` should be a subtype of the currently-promoted type
    ///   (i.e. no redundant or side-promotions)
    /// - If the reference is a variable, it should not be write-captured.
    fn finish_type_test(
        &self,
        helper: &dyn FlowModelHelper<F>,
        reference: &ExpressionInfo<F>,
        info: &PromotionModel<F>,
        tested_type: Option<TypeViewF<F>>,
        promoted_type: Option<TypeViewF<F>>,
    ) -> FlowModel<F> {
        let mut new_tested = info.tested.clone();
        if let Some(tested_type) = tested_type {
            new_tested = PromotionModel::<F>::add_type_to_unique_list(&info.tested, tested_type);
        }

        let mut new_promoted_types = info.promoted_types.clone();
        if let Some(promoted_type) = promoted_type {
            new_promoted_types =
                PromotionModel::<F>::add_to_promoted_types(&info.promoted_types, promoted_type);
        }

        if list_identical(&new_tested, &info.tested)
            && list_identical(&new_promoted_types, &info.promoted_types)
        {
            self.clone()
        } else {
            self.update_promotion_info(
                helper,
                reference.reference_data().promotion_key,
                PromotionModel::new(
                    new_promoted_types,
                    new_tested,
                    info.assigned,
                    info.unassigned,
                    info.ssa_node.clone(),
                    info.non_promotion_history.clone(),
                ),
            )
        }
    }

    /// Forms a new state to reflect a control flow path that might have come
    /// from either the `first` or `second` state.
    ///
    /// The control flow path is considered reachable if either of the input
    /// states is reachable. Variables are considered definitely assigned if
    /// they were definitely assigned in both of the input states. Promotions
    /// are kept only if they are common to both input states; if a reference
    /// is promoted to one type in one state and a subtype in the other
    /// state, the less specific type promotion is kept.
    pub fn join(
        helper: &dyn FlowModelHelper<F>,
        first: Option<&FlowModel<F>>,
        second: Option<&FlowModel<F>>,
    ) -> FlowModel<F> {
        let Some(first) = first else {
            return second.unwrap().clone();
        };
        let Some(second) = second else {
            return first.clone();
        };

        debug_assert!(Reachability::opt_ptr_eq(
            first.reachable.parent.as_ref(),
            second.reachable.parent.as_ref()
        ));
        if first.reachable.locally_reachable && !second.reachable.locally_reachable {
            return first.clone();
        }
        if !first.reachable.locally_reachable && second.reachable.locally_reachable {
            return second.clone();
        }

        // first.reachable and second.reachable are equivalent, so we don't
        // need to join reachabilities.
        debug_assert_eq!(
            first.reachable.locally_reachable,
            second.reachable.locally_reachable
        );
        FlowModel::join_promotion_info(helper, first, second)
    }

    /// Joins two "promotion info" maps. See [`join`](Self::join) for details.
    pub fn join_promotion_info(
        helper: &dyn FlowModelHelper<F>,
        first: &FlowModel<F>,
        second: &FlowModel<F>,
    ) -> FlowModel<F> {
        if first.ptr_eq(second) {
            return first.clone();
        }
        if first.promotion_info.is_none() {
            return first.clone();
        }
        if second.promotion_info.is_none() {
            return second.clone();
        }

        let diff = helper
            .reader()
            .borrow_mut()
            .diff(&first.promotion_info, &second.promotion_info);
        let mut new_flow_model =
            FlowModel::with_info(first.reachable.clone(), diff.ancestor.clone());
        for entry in diff.entries {
            let promotion_key = entry.key as PromotionKey;
            let Some(first_model) = entry.left().map(|l| l.value.clone()) else {
                continue;
            };
            let Some(second_model) = entry.right().map(|l| l.value.clone()) else {
                continue;
            };
            let joined;
            (joined, new_flow_model) = PromotionModel::join(
                helper,
                &first_model,
                &first.promotion_info,
                &second_model,
                &second.promotion_info,
                new_flow_model,
                None,
            );
            new_flow_model = new_flow_model.update_promotion_info(helper, promotion_key, joined);
        }

        new_flow_model
    }
}

impl<F: FlowTypes> fmt::Debug for FlowModel<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:?}, ", self.reachable)?;
        match &self.promotion_info {
            None => write!(f, "null)"),
            Some(info) => {
                // Dart prints the `PromotionInfo` object; print the map it
                // represents (most recent entries first).
                let mut parts = Vec::new();
                let mut seen = indexmap::IndexSet::new();
                let mut link = Some(info.clone());
                while let Some(l) = link {
                    if seen.insert(l.key) {
                        parts.push(format!("{}: {:?}", l.key, l.value));
                    }
                    link = l.previous.clone();
                }
                write!(f, "{{{}}})", parts.join(", "))
            }
        }
    }
}

// ------------------------------------------------------------- ExpressionInfo

rc_newtype!(
    /// Information gathered by flow analysis about an expression. This
    /// includes its static type, whether it refers to `null` or to something
    /// promotable, and the flow models representing execution state after
    /// the expression is evaluated.
    ///
    /// The Dart subclasses `_NullInfo`, `_Reference`,
    /// `TrivialVariableReference` and `_PropertyReference` are the variants
    /// of [`ExpressionInfoKind`].
    ExpressionInfo,
    ExpressionInfoData
);

/// The fields of an [`ExpressionInfo`].
pub struct ExpressionInfoData<F: FlowTypes> {
    /// The static type of the expression (Dart `_type`).
    pub type_: TypeViewF<F>,

    /// The flow model representing execution state after the expression is
    /// evaluated, if the expression evaluates to `true`.
    pub if_true: FlowModel<F>,

    /// The flow model representing execution state after the expression is
    /// evaluated, if the expression evaluates to `false`.
    pub if_false: FlowModel<F>,

    /// Which Dart subclass this is.
    pub kind: ExpressionInfoKind<F>,
}

/// The Dart class of an [`ExpressionInfo`].
pub enum ExpressionInfoKind<F: FlowTypes> {
    /// A plain `ExpressionInfo`.
    Plain,
    /// `_NullInfo`: the expression is a `null` literal.
    Null,
    /// `_Reference` or one of its subclasses: the expression is a reference
    /// to a variable, property, `this`, or the pseudo-expression `super`.
    Reference(ReferenceData<F>),
}

/// The fields of Dart `_Reference`.
pub struct ReferenceData<F: FlowTypes> {
    /// The integer key representing the thing referred to by this expression
    /// in [`FlowModel::promotion_info`].
    pub promotion_key: PromotionKey,

    /// Whether the thing referred to by this expression is `this` (or the
    /// pseudo-expression `super`).
    pub is_this_or_super: bool,

    /// The SSA node representing the value of this expression.
    pub ssa_node: SsaNode<F>,

    /// Which Dart subclass of `_Reference` this is.
    pub reference_kind: ReferenceKind<F>,
}

/// The Dart subclass of `_Reference`.
pub enum ReferenceKind<F: FlowTypes> {
    /// A plain `_Reference`.
    Plain,
    /// `TrivialVariableReference`: a reference whose information is trivial
    /// by construction.
    TrivialVariable,
    /// `_PropertyReference`: a reference to a property.
    Property {
        /// The name of the property.
        property_name: NameF<F>,
        /// The field or property being accessed. This matches a
        /// `propertyMember` value that was passed to `propertyGet`.
        property_member: Option<PropertyMemberF<F>>,
    },
}

impl<F: FlowTypes> ExpressionInfo<F> {
    /// Creates an [`ExpressionInfo`] for an expression whose value influences
    /// the flow model (e.g. an `!= null` or `is Type` check applied to a
    /// promotable target, which causes a promotion if it evaluates to
    /// `true`).
    pub fn new(type_: TypeViewF<F>, if_true: FlowModel<F>, if_false: FlowModel<F>) -> Self {
        Self::with_kind(type_, if_true, if_false, ExpressionInfoKind::Plain)
    }

    fn with_kind(
        type_: TypeViewF<F>,
        if_true: FlowModel<F>,
        if_false: FlowModel<F>,
        kind: ExpressionInfoKind<F>,
    ) -> Self {
        ExpressionInfo(Rc::new(ExpressionInfoData {
            type_,
            if_true,
            if_false,
            kind,
        }))
    }

    /// Creates an [`ExpressionInfo`] for an expression whose value doesn't
    /// influence the flow model (Dart `ExpressionInfo.trivial`).
    pub fn trivial(type_: TypeViewF<F>, model: FlowModel<F>) -> Self {
        Self::with_kind(type_, model.clone(), model, ExpressionInfoKind::Plain)
    }

    /// `new _NullInfo(type: type, model: model)`.
    pub fn null_info(type_: TypeViewF<F>, model: FlowModel<F>) -> Self {
        Self::with_kind(type_, model.clone(), model, ExpressionInfoKind::Null)
    }

    /// `new _Reference(...)`.
    pub fn reference(
        type_: TypeViewF<F>,
        if_true: FlowModel<F>,
        if_false: FlowModel<F>,
        promotion_key: PromotionKey,
        is_this_or_super: bool,
        ssa_node: SsaNode<F>,
    ) -> Self {
        Self::with_kind(
            type_,
            if_true,
            if_false,
            ExpressionInfoKind::Reference(ReferenceData {
                promotion_key,
                is_this_or_super,
                ssa_node,
                reference_kind: ReferenceKind::Plain,
            }),
        )
    }

    /// `new TrivialVariableReference(...)` (`@visibleForTesting`).
    pub fn trivial_variable_reference(
        type_: TypeViewF<F>,
        model: FlowModel<F>,
        promotion_key: PromotionKey,
        is_this_or_super: bool,
        ssa_node: SsaNode<F>,
    ) -> Self {
        Self::with_kind(
            type_,
            model.clone(),
            model,
            ExpressionInfoKind::Reference(ReferenceData {
                promotion_key,
                is_this_or_super,
                ssa_node,
                reference_kind: ReferenceKind::TrivialVariable,
            }),
        )
    }

    /// `new _PropertyReference(...)`.
    pub(crate) fn property_reference(
        type_: TypeViewF<F>,
        model: FlowModel<F>,
        property_name: NameF<F>,
        property_member: Option<PropertyMemberF<F>>,
        promotion_key: PromotionKey,
        ssa_node: SsaNode<F>,
    ) -> Self {
        Self::with_kind(
            type_,
            model.clone(),
            model,
            ExpressionInfoKind::Reference(ReferenceData {
                promotion_key,
                is_this_or_super: false,
                ssa_node,
                reference_kind: ReferenceKind::Property {
                    property_name,
                    property_member,
                },
            }),
        )
    }

    /// Determines if the value of the expression represented by `this`
    /// influences the flow model.
    pub fn is_non_trivial(&self) -> bool {
        !self.if_true.ptr_eq(&self.if_false)
    }

    /// Indicates whether the expression represented by `this` is a `null`
    /// literal.
    pub fn is_null(&self) -> bool {
        matches!(self.kind, ExpressionInfoKind::Null)
    }

    /// Dart `this is _Reference`: the reference data, if any.
    pub fn as_reference(&self) -> Option<&ReferenceData<F>> {
        match &self.kind {
            ExpressionInfoKind::Reference(r) => Some(r),
            _ => None,
        }
    }

    /// Dart `this is _PropertyReference`.
    pub fn is_property_reference(&self) -> bool {
        matches!(
            self.kind,
            ExpressionInfoKind::Reference(ReferenceData {
                reference_kind: ReferenceKind::Property { .. },
                ..
            })
        )
    }

    /// The reference data; panics if this is not a `_Reference` (Dart cast
    /// failure).
    pub fn reference_data(&self) -> &ReferenceData<F> {
        self.as_reference().expect("not a _Reference")
    }

    /// Creates an [`ExpressionInfo`] containing information about the
    /// logical inversion of the expression represented by `this`. For
    /// example, if `this` contains information about the expression
    /// `x == null`, calling this method produces an [`ExpressionInfo`]
    /// containing information about the expression `x != null`.
    pub(crate) fn invert(&self) -> ExpressionInfo<F> {
        if self.is_non_trivial() {
            ExpressionInfo::new(self.type_, self.if_false.clone(), self.if_true.clone())
        } else {
            self.clone()
        }
    }

    /// Produces an updated version of `this` (a `TrivialVariableReference`)
    /// reflecting flow analysis state from `condition_variable_info`.
    ///
    /// `current` should be the current flow model, and `helper` should be
    /// the flow analysis implementation.
    ///
    /// UNSPECIFIED: This implements the "restore" part of the condition
    /// variable feature, in which writes to local variables cause flow
    /// analysis state to be saved, and reads of local variables cause flow
    /// analysis state to be partially restored. This is what allows type
    /// promotion in examples like the following:
    ///
    /// ```dart
    /// int? x = ...;
    /// var xIsNonNull = x != null; // The following state is now saved: if
    ///                             // `xIsNonNull` is `true`, `x` is known
    ///                             // to be non-null
    /// ...Other statements...
    /// if (xIsNonNull) {           // The state is now restored
    ///   print(x.isEven);          // Therefore this is ok, because `x` is
    ///                             // known to be non-null.
    /// }
    /// ```
    ///
    /// Note that in an example like this, the saved flow analysis state is
    /// only restored to the extent that it's sound to do so. There are two
    /// conditions for soundness, and they are addressed in different ways:
    ///
    /// 1. For the restore to be sound, the value of the condition variable
    ///    at the time of the read must be provably the same as the value
    ///    that was written. That is, there must not be any write captures or
    ///    intervening writes of the condition variable on the control path
    ///    leading up to the read. This is addressed by saving the flow
    ///    analysis state in the [`SsaNodeData::condition_variable_state`]
    ///    field. Since a write to a variable causes it to be associated with
    ///    a new [`SsaNode`], and a write capture of a variable causes its
    ///    [`SsaNode`] association to be permanently set to `None`, this
    ///    assures that an attempt to restore the saved state will only be
    ///    made if there are no write captures or intervening writes.
    ///
    /// 2. Considering each variable referred to in the stored state, it is
    ///    only sound to restore the state of that variable if its value is
    ///    provably the same as it was at the time the condition variable was
    ///    written. This is addressed by [`FlowModel::rebase_forward`] (which
    ///    is called by this method to do the restore); it only updates the
    ///    [`PromotionModel`]s of variables whose [`SsaNode`] is (a) non-null
    ///    (i.e., not write captured) and (b) the same as it was at the time
    ///    the state was saved (i.e., no intervening writes).
    ///
    /// Note that this method is also invoked by `_pushScrutinee`. This
    /// ensures that stored flow analysis state propagates through pattern
    /// assignments.
    ///
    /// See <https://github.com/dart-lang/language/issues/1274>, the original
    /// feature request for this feature.
    pub fn restore_condition_variable_state(
        &self,
        condition_variable_info: Option<&ExpressionInfo<F>>,
        helper: &dyn FlowModelHelper<F>,
        current: &FlowModel<F>,
    ) -> ExpressionInfo<F> {
        if let Some(condition_variable_info) = condition_variable_info
            && condition_variable_info.is_non_trivial()
        {
            // `conditionVariableInfo` contained non-trivial flow analysis
            // information, so we need to rebase its [ifTrue] and [ifFalse]
            // flow models.
            let r = self.reference_data();
            ExpressionInfo::reference(
                self.type_,
                condition_variable_info
                    .if_true
                    .rebase_forward(helper, current),
                condition_variable_info
                    .if_false
                    .rebase_forward(helper, current),
                r.promotion_key,
                r.is_this_or_super,
                r.ssa_node.clone(),
            )
        } else {
            // `conditionVariableInfo` didn't contain any non-trivial flow
            // analysis information, so nothing needs to be updated.
            self.clone()
        }
    }
}

impl<F: FlowTypes> fmt::Debug for ExpressionInfo<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ExpressionInfoKind::Plain => write!(
                f,
                "ExpressionInfo(type: {:?}, _ifTrue: {:?}, ifFalse: {:?})",
                self.type_, self.if_true, self.if_false
            ),
            ExpressionInfoKind::Null => write!(f, "_NullInfo(type: {:?})", self.type_),
            ExpressionInfoKind::Reference(r) => match &r.reference_kind {
                ReferenceKind::Plain => write!(
                    f,
                    "_Reference(type: {:?}, ifTrue: {:?}, ifFalse: {:?}, promotionKey: {}, \
                     isThisOrSuper: {}, ssaNode: {:?})",
                    self.type_,
                    self.if_true,
                    self.if_false,
                    r.promotion_key,
                    r.is_this_or_super,
                    r.ssa_node
                ),
                ReferenceKind::TrivialVariable => write!(
                    f,
                    "TrivialVariableReference(type: {:?}, promotionKey: {}, isThisOrSuper: {}, \
                     ssaNode: {:?})",
                    self.type_, r.promotion_key, r.is_this_or_super, r.ssa_node
                ),
                ReferenceKind::Property {
                    property_name,
                    property_member,
                } => write!(
                    f,
                    "_PropertyReference(type: {:?}, propertyName: {:?}, propertyMember: {:?}, \
                     promotionKey: {})",
                    self.type_, property_name, property_member, r.promotion_key
                ),
            },
        }
    }
}
