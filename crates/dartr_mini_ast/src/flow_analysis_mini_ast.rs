// Dart source: pkg/_fe_analyzer_shared/test/flow_analysis/flow_analysis_mini_ast.dart

//! The flow analysis additions to the mini AST: [`get_ssa_nodes`],
//! [`implicit_this_why_not_promoted`], the `getExpressionInfo` and
//! `whyNotPromoted` extension methods (here methods of [`Node`]),
//! [`FlowAnalysisTestHarness`] and [`SsaNodeHarness`].

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::panic::Location;
use std::rc::Rc;

use dartr_flow::flow_analysis::PromotionKey;
use dartr_flow::flow_analysis_impl::FlowAnalysisImpl;
use dartr_flow::flow_analysis_impl::model::{FlowModelHelper, PromotionModel, SsaNode};
use dartr_flow::flow_analysis_operations::{FlowAnalysisOperations, FlowAnalysisTypeOperations};
use dartr_flow::flow_link::FlowLinkReader;
use dartr_flow::promotion_key_store::PromotionKeyStore;
use dartr_flow::shared_type::SharedTypeView;
use dartr_flow::type_analyzer::TypeAnalyzerOptions;

use crate::harness::Harness;
use crate::mini_types::{Type, TypeRegistryScope};
use crate::node::{ExprInfo, Node, NodeKind, Var, WhyNotPromotedMap, as_expression};
use crate::operations::{MiniAstOperations, MiniAstTypes};

type View = SharedTypeView<Type>;

/// Creates an expression that, when analyzed, passes an
/// [`SsaNodeHarness`] to `callback`, allowing the test to examine the
/// values of variables' SSA nodes (Dart `getSsaNodes`).
#[track_caller]
pub fn get_ssa_nodes(callback: impl Fn(&SsaNodeHarness) + 'static) -> Node {
    Node::alloc(
        NodeKind::GetSsaNodes {
            callback: Rc::new(callback),
        },
        Location::caller(),
    )
}

/// Dart `implicitThis_whyNotPromoted(staticType, callback)`.
#[track_caller]
pub fn implicit_this_why_not_promoted(
    static_type: &str,
    callback: impl Fn(WhyNotPromotedMap) + 'static,
) -> Node {
    Node::alloc(
        NodeKind::WhyNotPromotedImplicitThis {
            static_type: Type::parse(static_type),
            callback: Rc::new(callback),
        },
        Location::caller(),
    )
}

/// Dart `extension ExpressionExtensionForFlowAnalysisTesting on
/// ProtoExpression`.
impl Node {
    /// An expression that behaves like `self`, but after visiting it passes
    /// the `ExpressionInfo` associated with it (or `None`) to `callback`
    /// (Dart `getExpressionInfo`).
    #[track_caller]
    pub fn get_expression_info(self, callback: impl Fn(Option<ExprInfo>) + 'static) -> Node {
        Node::alloc(
            NodeKind::GetExpressionInfo {
                target: as_expression(self),
                callback: Rc::new(callback),
            },
            Location::caller(),
        )
    }

    /// An expression that behaves like `self`, but after visiting it passes
    /// the non-promotion reasons associated with it (empty if none) to
    /// `callback` (Dart `whyNotPromoted`).
    #[track_caller]
    pub fn why_not_promoted(self, callback: impl Fn(WhyNotPromotedMap) + 'static) -> Node {
        Node::alloc(
            NodeKind::WhyNotPromoted {
                target: as_expression(self),
                callback: Rc::new(callback),
            },
            Location::caller(),
        )
    }
}

/// Test harness for creating flow analysis tests (Dart
/// `FlowAnalysisTestHarness extends Harness with FlowModelHelper`): a
/// [`Harness`] (reached through `Deref`) that also provides the
/// [`FlowModelHelper`] used by the tests that call `FlowModel` methods
/// directly.
pub struct FlowAnalysisTestHarness {
    harness: Harness,

    /// Dart `promotionKeyStore` (separate from the one that flow analysis
    /// uses in [`Harness::run`]).
    pub promotion_key_store: PromotionKeyStore<Var>,

    /// Dart `boolType`.
    bool_type: View,

    /// Dart `FlowModelHelper.reader`.
    pub reader: RefCell<FlowLinkReader<PromotionModel<MiniAstTypes>>>,

    /// Keeps the type registry initialized as long as the harness lives
    /// (Dart `setUp`/`tearDown`); see
    /// [`FlowAnalysisTestHarness::with_registry_scope`].
    registry_scope: Option<TypeRegistryScope>,
}

impl Default for FlowAnalysisTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowAnalysisTestHarness {
    /// `FlowAnalysisTestHarness()`. The type registry must be initialized.
    pub fn new() -> Self {
        FlowAnalysisTestHarness {
            harness: Harness::new(),
            promotion_key_store: PromotionKeyStore::new(),
            bool_type: SharedTypeView::new(Type::parse("bool")),
            reader: RefCell::new(FlowLinkReader::new()),
            registry_scope: None,
        }
    }

    /// Keeps `scope` (from `type_registry_scope()`) alive as long as the
    /// harness, so the registry is un-initialized when the harness is
    /// dropped (Dart `tearDown`).
    pub fn with_registry_scope(mut self, scope: TypeRegistryScope) -> Self {
        self.registry_scope = Some(scope);
        self
    }

    /// Dart `h.promotionKeyStore.keyForVariable(variable)`.
    pub fn key_for_variable(&self, variable: Var) -> PromotionKey {
        self.promotion_key_store.key_for_variable(variable)
    }

    /// Dart `h.typeOperations` (the operations object).
    pub fn type_operations(&self) -> MiniAstOperations {
        self.harness.operations.clone()
    }
}

impl Deref for FlowAnalysisTestHarness {
    type Target = Harness;
    fn deref(&self) -> &Harness {
        &self.harness
    }
}

impl DerefMut for FlowAnalysisTestHarness {
    fn deref_mut(&mut self) -> &mut Harness {
        &mut self.harness
    }
}

impl FlowModelHelper<MiniAstTypes> for FlowAnalysisTestHarness {
    fn reader(&self) -> &RefCell<FlowLinkReader<PromotionModel<MiniAstTypes>>> {
        &self.reader
    }

    fn bool_type(&self) -> View {
        self.bool_type
    }

    fn promotion_key_store(&self) -> &PromotionKeyStore<Var> {
        &self.promotion_key_store
    }

    /// Dart `typeAnalyzerOptions => computeTypeAnalyzerOptions()`.
    fn type_analyzer_options(&self) -> &TypeAnalyzerOptions {
        self.harness.type_analyzer_options()
    }

    /// Dart `typeOperations => typeAnalyzer.operations`.
    fn type_operations(&self) -> &MiniAstOperations {
        &self.harness.operations
    }

    fn is_final(&self, variable_key: PromotionKey) -> bool {
        let variable = self.promotion_key_store.variable_for_key(variable_key);
        if let Some(variable) = variable
            && self.harness.operations.is_final(variable)
        {
            return true;
        }
        false
    }

    fn is_valid_promotion_step(&self, previous_type: View, new_type: View) -> bool {
        let ops = &self.harness.operations;
        // Caller must ensure that `newType <: previousType`.
        debug_assert!(
            ops.is_subtype_of(new_type, previous_type),
            "Expected {new_type:?} to be a subtype of {previous_type:?}."
        );
        // Promotion to a mutual subtype is not allowed. Since the caller has
        // already ensured that `newType <: previousType`, it's only necessary
        // to check whether `previousType <: newType`.
        !ops.is_subtype_of(previous_type, new_type)
    }
}

/// Helper allowing tests to examine the values of variables' SSA nodes
/// (Dart `SsaNodeHarness`).
pub struct SsaNodeHarness<'a> {
    flow: &'a FlowAnalysisImpl<MiniAstTypes>,
}

impl<'a> SsaNodeHarness<'a> {
    pub(crate) fn new(flow: &'a FlowAnalysisImpl<MiniAstTypes>) -> Self {
        SsaNodeHarness { flow }
    }

    /// Dart `nodes[variable]`: the SSA node associated with `variable` at
    /// the current point in control flow, or `None` if the variable has
    /// been write captured.
    pub fn get(&self, variable: Var) -> Option<SsaNode<MiniAstTypes>> {
        self.flow.ssa_node_for_testing(variable)
    }
}
