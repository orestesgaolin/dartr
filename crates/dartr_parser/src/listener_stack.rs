// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/listener.dart
//
// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_listener.py`.

// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/forwarding_listener.dart
// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/recovery_listeners.dart

//! The listener that the parser calls: [`ListenerStack`].
//!
//! The Dart parser replaces its `listener` field for a while in a few
//! places: a `NullListener` for look-ahead parsing, a `ForwardingListener`
//! without a target to drop events, a `ForwardingListener` with
//! `forwardErrors = false`, and the recovery listeners of
//! `recovery_listeners.dart` that record some events and forward the
//! events to the primary listener only after a first pass. Here the
//! replacement listeners are [`Layer`]s on a stack in front of the primary
//! listener: an event goes from the top layer down and reaches the primary
//! listener only if every layer passes it on. Without layers (the normal
//! case) an event costs one length check.
//!
//! The stack also owns the token arena, so that the parser can call
//! `self.listener.begin_x(token)` while it reads tokens through the same
//! struct.

use dartr_diagnostics::cfe::{CfeCode, CfeMessage};
use dartr_syntax::{TokenId, Tokens};

use crate::assert::Assert;
use crate::block_kind::BlockKind;
use crate::constructor_reference_context::ConstructorReferenceContext;
use crate::declaration_kind::{DeclarationHeaderKind, DeclarationKind};
use crate::experimental_features::ExperimentalFlag;
use crate::formal_parameter_kind::FormalParameterKind;
use crate::identifier_context::IdentifierContext;
use crate::listener::Listener;
use crate::member_kind::MemberKind;

/// Dart `ImportRecoveryListener` (the recorded fields).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportRecovery {
    pub as_keyword: Option<TokenId>,
    pub deferred_keyword: Option<TokenId>,
    pub if_keyword: Option<TokenId>,
    pub has_combinator: bool,
}

/// Dart `DeclarationHeaderRecoveryListener` (the recorded fields).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeclarationHeaderRecovery {
    pub extends_keyword: Option<TokenId>,
    pub implements_keyword: Option<TokenId>,
    pub with_keyword: Option<TokenId>,
}

/// Dart `MixinHeaderRecoveryListener` (the recorded fields).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinHeaderRecovery {
    pub on_keyword: Option<TokenId>,
    pub implements_keyword: Option<TokenId>,
}

/// A listener that replaces the current listener for a while.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Dart `NullListener`: drops all events, remembers whether
    /// `handleRecoverableError` was called.
    Null { has_errors: bool },
    /// Dart `ForwardingListener()` without a target: drops all events.
    Silent,
    /// Dart `ForwardingListener(listener)..forwardErrors = false`.
    NoErrors,
    /// Dart `ImportRecoveryListener`; `forwarding` is true when its
    /// `listener` is set.
    ImportRecovery { forwarding: bool, state: ImportRecovery },
    /// Dart `DeclarationHeaderRecoveryListener`.
    DeclarationHeaderRecovery { forwarding: bool, state: DeclarationHeaderRecovery },
    /// Dart `MixinHeaderRecoveryListener`.
    MixinHeaderRecovery { forwarding: bool, state: MixinHeaderRecovery },
}

/// The events that a layer looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Other,
    Error,
    ClassExtends(Option<TokenId>),
    Implements(Option<TokenId>),
    ClassWithClause(TokenId),
    ConditionalUri(TokenId),
    Combinator,
    ImportPrefix(Option<TokenId>, Option<TokenId>),
    MixinOn(Option<TokenId>),
}

/// The listener that the parser calls (Dart `Parser.listener`): the primary
/// listener, the layers in front of it, and the token arena.
pub struct ListenerStack<L> {
    pub tokens: Tokens,
    pub primary: L,
    pub layers: Vec<Layer>,
}

impl<L: Listener> ListenerStack<L> {
    pub fn new(tokens: Tokens, primary: L) -> Self {
        ListenerStack {
            tokens,
            primary,
            layers: Vec::with_capacity(8),
        }
    }

    /// Puts [layer] in front of the current listener (Dart
    /// `listener = new XListener(...)`).
    #[inline]
    pub fn push_layer(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    /// Restores the listener that was current before the last
    /// [`Self::push_layer`] and returns the removed layer.
    #[inline]
    pub fn pop_layer(&mut self) -> Layer {
        self.layers.pop().expect("no listener layer")
    }

    /// The top layer.
    pub fn top_layer(&mut self) -> &mut Layer {
        self.layers.last_mut().expect("no listener layer")
    }

    /// The topmost `Layer::Null`'s `has_errors` (Dart `nullListener.hasErrors`).
    pub fn null_listener_has_errors(&self) -> bool {
        for layer in self.layers.iter().rev() {
            if let Layer::Null { has_errors } = layer {
                return *has_errors;
            }
        }
        panic!("no NullListener layer")
    }

    /// The topmost `Layer::ImportRecovery` (Dart `ImportRecoveryListener`):
    /// `(forwarding, state)`. Setting `forwarding` is Dart
    /// `recoveryListener.listener = primaryListener`; Dart `clear()` is
    /// `*state = Default::default()`.
    pub fn import_recovery(&mut self) -> (&mut bool, &mut ImportRecovery) {
        for layer in self.layers.iter_mut().rev() {
            if let Layer::ImportRecovery { forwarding, state } = layer {
                return (forwarding, state);
            }
        }
        panic!("no ImportRecovery layer")
    }

    /// The topmost `Layer::DeclarationHeaderRecovery` (Dart
    /// `DeclarationHeaderRecoveryListener`): `(forwarding, state)`.
    pub fn declaration_header_recovery(&mut self) -> (&mut bool, &mut DeclarationHeaderRecovery) {
        for layer in self.layers.iter_mut().rev() {
            if let Layer::DeclarationHeaderRecovery { forwarding, state } = layer {
                return (forwarding, state);
            }
        }
        panic!("no DeclarationHeaderRecovery layer")
    }

    /// The topmost `Layer::MixinHeaderRecovery` (Dart
    /// `MixinHeaderRecoveryListener`): `(forwarding, state)`.
    pub fn mixin_header_recovery(&mut self) -> (&mut bool, &mut MixinHeaderRecovery) {
        for layer in self.layers.iter_mut().rev() {
            if let Layer::MixinHeaderRecovery { forwarding, state } = layer {
                return (forwarding, state);
            }
        }
        panic!("no MixinHeaderRecovery layer")
    }

    /// Sends [route] through the layers. Returns true if the event reaches
    /// the primary listener.
    #[cold]
    fn route(&mut self, route: Route) -> bool {
        for layer in self.layers.iter_mut().rev() {
            match layer {
                Layer::Null { has_errors } => {
                    if route == Route::Error {
                        *has_errors = true;
                    }
                    return false;
                }
                Layer::Silent => return false,
                Layer::NoErrors => {
                    if route == Route::Error {
                        return false;
                    }
                }
                Layer::ImportRecovery { forwarding, state } => {
                    match route {
                        Route::ConditionalUri(if_keyword) => state.if_keyword = Some(if_keyword),
                        Route::Combinator => state.has_combinator = true,
                        Route::ImportPrefix(deferred_keyword, as_keyword) => {
                            state.deferred_keyword = deferred_keyword;
                            state.as_keyword = as_keyword;
                        }
                        _ => {}
                    }
                    if !*forwarding {
                        return false;
                    }
                }
                Layer::DeclarationHeaderRecovery { forwarding, state } => {
                    match route {
                        Route::ClassExtends(k) => state.extends_keyword = k,
                        Route::Implements(k) => state.implements_keyword = k,
                        Route::ClassWithClause(k) => state.with_keyword = Some(k),
                        _ => {}
                    }
                    if !*forwarding {
                        return false;
                    }
                }
                Layer::MixinHeaderRecovery { forwarding, state } => {
                    match route {
                        Route::MixinOn(k) => state.on_keyword = k,
                        Route::Implements(k) => state.implements_keyword = k,
                        _ => {}
                    }
                    if !*forwarding {
                        return false;
                    }
                }
            }
        }
        true
    }

    #[inline]
    pub fn begin_arguments(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_arguments(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_arguments(&mut self, count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_arguments(&mut self.tokens, count, begin_token, end_token);
    }

    #[inline]
    pub fn handle_object_pattern_fields(&mut self, count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_object_pattern_fields(&mut self.tokens, count, begin_token, end_token);
    }

    #[inline]
    pub fn handle_async_modifier(&mut self, async_token: Option<TokenId>, star_token: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_async_modifier(&mut self.tokens, async_token, star_token);
    }

    #[inline]
    pub fn begin_await_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_await_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_await_expression(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_await_expression(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn end_invalid_await_expression(&mut self, begin_token: TokenId, end_token: TokenId, error_code: &'static CfeCode) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_invalid_await_expression(&mut self.tokens, begin_token, end_token, error_code);
    }

    #[inline]
    pub fn begin_block(&mut self, token: TokenId, block_kind: BlockKind) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_block(&mut self.tokens, token, block_kind);
    }

    #[inline]
    pub fn end_block(&mut self, count: i32, begin_token: TokenId, end_token: TokenId, block_kind: BlockKind) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_block(&mut self.tokens, count, begin_token, end_token, block_kind);
    }

    #[inline]
    pub fn handle_invalid_top_level_block(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_invalid_top_level_block(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_cascade(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_cascade(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_cascade(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_cascade(&mut self.tokens);
    }

    #[inline]
    pub fn begin_case_expression(&mut self, case_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_case_expression(&mut self.tokens, case_keyword);
    }

    #[inline]
    pub fn end_case_expression(&mut self, case_keyword: TokenId, when: Option<TokenId>, colon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_case_expression(&mut self.tokens, case_keyword, when, colon);
    }

    #[inline]
    pub fn begin_class_or_mixin_or_extension_body(&mut self, kind: DeclarationKind, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_class_or_mixin_or_extension_body(&mut self.tokens, kind, token);
    }

    #[inline]
    pub fn end_class_or_mixin_or_extension_body(&mut self, kind: DeclarationKind, member_count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_class_or_mixin_or_extension_body(&mut self.tokens, kind, member_count, begin_token, end_token);
    }

    #[inline]
    pub fn begin_class_or_mixin_or_named_mixin_application_prelude(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_class_or_mixin_or_named_mixin_application_prelude(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_class_declaration(&mut self, begin: TokenId, abstract_token: Option<TokenId>, sealed_token: Option<TokenId>, base_token: Option<TokenId>, interface_token: Option<TokenId>, final_token: Option<TokenId>, augment_token: Option<TokenId>, mixin_token: Option<TokenId>, name: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_class_declaration(&mut self.tokens, begin, abstract_token, sealed_token, base_token, interface_token, final_token, augment_token, mixin_token, name);
    }

    #[inline]
    pub fn handle_class_extends(&mut self, extends_keyword: Option<TokenId>, type_count: i32) {
        if !self.layers.is_empty() && !self.route(Route::ClassExtends(extends_keyword)) {
            return;
        }
        self.primary.handle_class_extends(&mut self.tokens, extends_keyword, type_count);
    }

    #[inline]
    pub fn handle_implements(&mut self, implements_keyword: Option<TokenId>, interfaces_count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Implements(implements_keyword)) {
            return;
        }
        self.primary.handle_implements(&mut self.tokens, implements_keyword, interfaces_count);
    }

    #[inline]
    pub fn handle_class_header(&mut self, begin: TokenId, class_keyword: TokenId, native_token: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_class_header(&mut self.tokens, begin, class_keyword, native_token);
    }

    #[inline]
    pub fn handle_recover_declaration_header(&mut self, kind: DeclarationHeaderKind) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_recover_declaration_header(&mut self.tokens, kind);
    }

    #[inline]
    pub fn end_class_declaration(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_class_declaration(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn handle_no_class_body(&mut self, semicolon_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_class_body(&mut self.tokens, semicolon_token);
    }

    #[inline]
    pub fn handle_no_extension_type_body(&mut self, semicolon_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_extension_type_body(&mut self.tokens, semicolon_token);
    }

    #[inline]
    pub fn begin_mixin_declaration(&mut self, begin_token: TokenId, augment_token: Option<TokenId>, base_token: Option<TokenId>, mixin_keyword: TokenId, name: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_mixin_declaration(&mut self.tokens, begin_token, augment_token, base_token, mixin_keyword, name);
    }

    #[inline]
    pub fn handle_mixin_on(&mut self, on_keyword: Option<TokenId>, type_count: i32) {
        if !self.layers.is_empty() && !self.route(Route::MixinOn(on_keyword)) {
            return;
        }
        self.primary.handle_mixin_on(&mut self.tokens, on_keyword, type_count);
    }

    #[inline]
    pub fn handle_mixin_header(&mut self, mixin_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_mixin_header(&mut self.tokens, mixin_keyword);
    }

    #[inline]
    pub fn handle_recover_mixin_header(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_recover_mixin_header(&mut self.tokens);
    }

    #[inline]
    pub fn handle_no_mixin_body(&mut self, semicolon_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_mixin_body(&mut self.tokens, semicolon_token);
    }

    #[inline]
    pub fn end_mixin_declaration(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_mixin_declaration(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn begin_uncategorized_top_level_declaration(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_uncategorized_top_level_declaration(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_extension_declaration_prelude(&mut self, extension_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_extension_declaration_prelude(&mut self.tokens, extension_keyword);
    }

    #[inline]
    pub fn begin_extension_declaration(&mut self, augment_token: Option<TokenId>, extension_keyword: TokenId, name: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_extension_declaration(&mut self.tokens, augment_token, extension_keyword, name);
    }

    #[inline]
    pub fn end_extension_declaration(&mut self, begin_token: TokenId, extension_keyword: TokenId, on_keyword: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_extension_declaration(&mut self.tokens, begin_token, extension_keyword, on_keyword, end_token);
    }

    #[inline]
    pub fn handle_no_extension_body(&mut self, semicolon_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_extension_body(&mut self.tokens, semicolon_token);
    }

    #[inline]
    pub fn begin_extension_type_declaration(&mut self, augment_keyword: Option<TokenId>, extension_keyword: TokenId, name: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_extension_type_declaration(&mut self.tokens, augment_keyword, extension_keyword, name);
    }

    #[inline]
    pub fn end_extension_type_declaration(&mut self, begin_token: TokenId, augment_token: Option<TokenId>, extension_keyword: TokenId, type_keyword: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_extension_type_declaration(&mut self.tokens, begin_token, augment_token, extension_keyword, type_keyword, end_token);
    }

    #[inline]
    pub fn begin_primary_constructor(&mut self, begin_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_primary_constructor(&mut self.tokens, begin_token);
    }

    #[inline]
    pub fn end_primary_constructor(&mut self, kind: DeclarationKind, begin_token: TokenId, end_token: TokenId, const_keyword: Option<TokenId>, has_constructor_name: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_primary_constructor(&mut self.tokens, kind, begin_token, end_token, const_keyword, has_constructor_name);
    }

    #[inline]
    pub fn handle_no_primary_constructor(&mut self, kind: DeclarationKind, token: TokenId, const_keyword: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_primary_constructor(&mut self.tokens, kind, token, const_keyword);
    }

    #[inline]
    pub fn begin_primary_constructor_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_primary_constructor_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_primary_constructor_body(&mut self, begin_token: TokenId, begin_initializers: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_primary_constructor_body(&mut self.tokens, begin_token, begin_initializers, end_token);
    }

    #[inline]
    pub fn begin_combinators(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_combinators(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_combinators(&mut self, count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_combinators(&mut self.tokens, count);
    }

    #[inline]
    pub fn begin_compilation_unit(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_compilation_unit(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_directives_only(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_directives_only(&mut self.tokens);
    }

    #[inline]
    pub fn end_compilation_unit(&mut self, count: i32, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_compilation_unit(&mut self.tokens, count, token);
    }

    #[inline]
    pub fn begin_const_literal(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_const_literal(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_const_literal(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_const_literal(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn begin_constructor_reference(&mut self, start: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_constructor_reference(&mut self.tokens, start);
    }

    #[inline]
    pub fn end_constructor_reference(&mut self, start: TokenId, period_before_name: Option<TokenId>, end_token: TokenId, constructor_reference_context: ConstructorReferenceContext) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_constructor_reference(&mut self.tokens, start, period_before_name, end_token, constructor_reference_context);
    }

    #[inline]
    pub fn begin_do_while_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_do_while_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_do_while_statement(&mut self, do_keyword: TokenId, while_keyword: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_do_while_statement(&mut self.tokens, do_keyword, while_keyword, end_token);
    }

    #[inline]
    pub fn begin_do_while_statement_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_do_while_statement_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_do_while_statement_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_do_while_statement_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_while_statement_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_while_statement_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_while_statement_body(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_while_statement_body(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn begin_enum_declaration_prelude(&mut self, enum_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_enum_declaration_prelude(&mut self.tokens, enum_keyword);
    }

    #[inline]
    pub fn begin_enum_declaration(&mut self, begin_token: TokenId, augment_token: Option<TokenId>, enum_keyword: TokenId, name: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_enum_declaration(&mut self.tokens, begin_token, augment_token, enum_keyword, name);
    }

    #[inline]
    pub fn end_enum_declaration(&mut self, begin_token: TokenId, enum_keyword: TokenId, left_brace: TokenId, member_count: i32, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_enum_declaration(&mut self.tokens, begin_token, enum_keyword, left_brace, member_count, end_token);
    }

    #[inline]
    pub fn handle_enum_elements(&mut self, elements_end_token: TokenId, elements_count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_enum_elements(&mut self.tokens, elements_end_token, elements_count);
    }

    #[inline]
    pub fn handle_enum_header(&mut self, augment_token: Option<TokenId>, enum_keyword: TokenId, left_brace: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_enum_header(&mut self.tokens, augment_token, enum_keyword, left_brace);
    }

    #[inline]
    pub fn begin_enum_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_enum_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_enum_body(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_enum_body(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn handle_no_enum_body(&mut self, semicolon_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_enum_body(&mut self.tokens, semicolon_token);
    }

    #[inline]
    pub fn handle_enum_element(&mut self, begin_token: TokenId, augment_token: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_enum_element(&mut self.tokens, begin_token, augment_token);
    }

    #[inline]
    pub fn begin_export(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_export(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_export(&mut self, export_keyword: TokenId, semicolon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_export(&mut self.tokens, export_keyword, semicolon);
    }

    #[inline]
    pub fn handle_extraneous_expression(&mut self, token: TokenId, message: CfeMessage) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_extraneous_expression(&mut self.tokens, token, message);
    }

    #[inline]
    pub fn handle_expression_statement(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_expression_statement(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn begin_factory(&mut self, declaration_kind: DeclarationKind, last_consumed: TokenId, augment_token: Option<TokenId>, external_token: Option<TokenId>, const_token: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_factory(&mut self.tokens, declaration_kind, last_consumed, augment_token, external_token, const_token);
    }

    #[inline]
    pub fn end_factory(&mut self, kind: DeclarationKind, begin_token: TokenId, factory_keyword: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_factory(&mut self.tokens, kind, begin_token, factory_keyword, end_token);
    }

    #[inline]
    pub fn begin_formal_parameter(&mut self, token: TokenId, kind: MemberKind, required_token: Option<TokenId>, covariant_token: Option<TokenId>, var_final_or_const: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_formal_parameter(&mut self.tokens, token, kind, required_token, covariant_token, var_final_or_const);
    }

    #[inline]
    pub fn end_formal_parameter(&mut self, var_or_final: Option<TokenId>, this_keyword: Option<TokenId>, super_keyword: Option<TokenId>, period_after_this_or_super: Option<TokenId>, name_token: TokenId, initializer_start: Option<TokenId>, initializer_end: Option<TokenId>, kind: FormalParameterKind, member_kind: MemberKind) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_formal_parameter(&mut self.tokens, var_or_final, this_keyword, super_keyword, period_after_this_or_super, name_token, initializer_start, initializer_end, kind, member_kind);
    }

    #[inline]
    pub fn handle_no_formal_parameters(&mut self, token: TokenId, kind: MemberKind) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_formal_parameters(&mut self.tokens, token, kind);
    }

    #[inline]
    pub fn begin_formal_parameters(&mut self, token: TokenId, kind: MemberKind) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_formal_parameters(&mut self.tokens, token, kind);
    }

    #[inline]
    pub fn end_formal_parameters(&mut self, count: i32, begin_token: TokenId, end_token: TokenId, kind: MemberKind) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_formal_parameters(&mut self.tokens, count, begin_token, end_token, kind);
    }

    #[inline]
    pub fn end_fields(&mut self, kind: DeclarationKind, abstract_token: Option<TokenId>, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_fields(&mut self.tokens, kind, abstract_token, augment_token, external_token, static_token, covariant_token, late_token, var_final_or_const, count, begin_token, end_token);
    }

    #[inline]
    pub fn handle_for_initializer_empty_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_for_initializer_empty_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_for_initializer_expression_statement(&mut self, token: TokenId, for_in: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_for_initializer_expression_statement(&mut self.tokens, token, for_in);
    }

    #[inline]
    pub fn handle_for_initializer_local_variable_declaration(&mut self, token: TokenId, for_in: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_for_initializer_local_variable_declaration(&mut self.tokens, token, for_in);
    }

    #[inline]
    pub fn handle_for_initializer_pattern_variable_assignment(&mut self, keyword: TokenId, equals: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_for_initializer_pattern_variable_assignment(&mut self.tokens, keyword, equals);
    }

    #[inline]
    pub fn begin_for_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_for_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_for_loop_parts(&mut self, for_keyword: TokenId, left_paren: TokenId, left_separator: TokenId, right_separator: TokenId, update_expression_count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_for_loop_parts(&mut self.tokens, for_keyword, left_paren, left_separator, right_separator, update_expression_count);
    }

    #[inline]
    pub fn end_for_statement(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_for_statement(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn begin_for_statement_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_for_statement_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_for_statement_body(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_for_statement_body(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn handle_for_in_loop_parts(&mut self, await_token: Option<TokenId>, for_token: TokenId, left_parenthesis: TokenId, pattern_keyword: Option<TokenId>, in_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_for_in_loop_parts(&mut self.tokens, await_token, for_token, left_parenthesis, pattern_keyword, in_keyword);
    }

    #[inline]
    pub fn end_for_in(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_for_in(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn begin_for_in_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_for_in_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_for_in_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_for_in_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_for_in_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_for_in_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_for_in_body(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_for_in_body(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn begin_named_function_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_named_function_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_named_function_expression(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_named_function_expression(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn begin_local_function_declaration(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_local_function_declaration(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_local_function_declaration(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_local_function_declaration(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn begin_block_function_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_block_function_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_block_function_body(&mut self, count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_block_function_body(&mut self.tokens, count, begin_token, end_token);
    }

    #[inline]
    pub fn handle_no_function_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_function_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_function_body_skipped(&mut self, begin_token: TokenId, end_token: TokenId, is_expression_body: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_function_body_skipped(&mut self.tokens, begin_token, end_token, is_expression_body);
    }

    #[inline]
    pub fn begin_function_name(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_function_name(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_function_name(&mut self, begin_token: TokenId, token: TokenId, is_function_expression: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_function_name(&mut self.tokens, begin_token, token, is_function_expression);
    }

    #[inline]
    pub fn begin_typedef(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_typedef(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_typedef(&mut self, augment_token: Option<TokenId>, typedef_keyword: TokenId, equals: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_typedef(&mut self.tokens, augment_token, typedef_keyword, equals, end_token);
    }

    #[inline]
    pub fn handle_class_with_clause(&mut self, with_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::ClassWithClause(with_keyword)) {
            return;
        }
        self.primary.handle_class_with_clause(&mut self.tokens, with_keyword);
    }

    #[inline]
    pub fn handle_class_no_with_clause(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_class_no_with_clause(&mut self.tokens);
    }

    #[inline]
    pub fn handle_enum_with_clause(&mut self, with_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_enum_with_clause(&mut self.tokens, with_keyword);
    }

    #[inline]
    pub fn handle_enum_no_with_clause(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_enum_no_with_clause(&mut self.tokens);
    }

    #[inline]
    pub fn handle_mixin_with_clause(&mut self, with_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_mixin_with_clause(&mut self.tokens, with_keyword);
    }

    #[inline]
    pub fn begin_named_mixin_application(&mut self, begin_token: TokenId, abstract_token: Option<TokenId>, sealed_token: Option<TokenId>, base_token: Option<TokenId>, interface_token: Option<TokenId>, final_token: Option<TokenId>, augment_token: Option<TokenId>, mixin_token: Option<TokenId>, name: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_named_mixin_application(&mut self.tokens, begin_token, abstract_token, sealed_token, base_token, interface_token, final_token, augment_token, mixin_token, name);
    }

    #[inline]
    pub fn handle_named_mixin_application_with_clause(&mut self, with_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_named_mixin_application_with_clause(&mut self.tokens, with_keyword);
    }

    #[inline]
    pub fn end_named_mixin_application(&mut self, begin: TokenId, class_keyword: TokenId, equals: TokenId, implements_keyword: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_named_mixin_application(&mut self.tokens, begin, class_keyword, equals, implements_keyword, end_token);
    }

    #[inline]
    pub fn begin_hide(&mut self, hide_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_hide(&mut self.tokens, hide_keyword);
    }

    #[inline]
    pub fn end_hide(&mut self, hide_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Combinator) {
            return;
        }
        self.primary.end_hide(&mut self.tokens, hide_keyword);
    }

    #[inline]
    pub fn handle_identifier_list(&mut self, count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_identifier_list(&mut self.tokens, count);
    }

    #[inline]
    pub fn begin_type_list(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_type_list(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_type_list(&mut self, count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_type_list(&mut self.tokens, count);
    }

    #[inline]
    pub fn begin_if_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_if_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_if_statement(&mut self, if_token: TokenId, else_token: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_if_statement(&mut self.tokens, if_token, else_token, end_token);
    }

    #[inline]
    pub fn begin_then_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_then_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_then_statement(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_then_statement(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn begin_else_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_else_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_else_statement(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_else_statement(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn begin_import(&mut self, import_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_import(&mut self.tokens, import_keyword);
    }

    #[inline]
    pub fn handle_import_prefix(&mut self, deferred_keyword: Option<TokenId>, as_keyword: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::ImportPrefix(deferred_keyword, as_keyword)) {
            return;
        }
        self.primary.handle_import_prefix(&mut self.tokens, deferred_keyword, as_keyword);
    }

    #[inline]
    pub fn end_import(&mut self, import_keyword: TokenId, semicolon: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_import(&mut self.tokens, import_keyword, semicolon);
    }

    #[inline]
    pub fn handle_recover_import(&mut self, semicolon: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_recover_import(&mut self.tokens, semicolon);
    }

    #[inline]
    pub fn begin_conditional_uris(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_conditional_uris(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_conditional_uris(&mut self, count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_conditional_uris(&mut self.tokens, count);
    }

    #[inline]
    pub fn begin_conditional_uri(&mut self, if_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_conditional_uri(&mut self.tokens, if_keyword);
    }

    #[inline]
    pub fn end_conditional_uri(&mut self, if_keyword: TokenId, left_paren: TokenId, equal_sign: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::ConditionalUri(if_keyword)) {
            return;
        }
        self.primary.end_conditional_uri(&mut self.tokens, if_keyword, left_paren, equal_sign);
    }

    #[inline]
    pub fn handle_dotted_name(&mut self, count: i32, first_identifier: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_dotted_name(&mut self.tokens, count, first_identifier);
    }

    #[inline]
    pub fn begin_implicit_creation_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_implicit_creation_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_implicit_creation_expression(&mut self, token: TokenId, open_angle_bracket: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_implicit_creation_expression(&mut self.tokens, token, open_angle_bracket);
    }

    #[inline]
    pub fn begin_initialized_identifier(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_initialized_identifier(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_initialized_identifier(&mut self, name_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_initialized_identifier(&mut self.tokens, name_token);
    }

    #[inline]
    pub fn begin_field_initializer(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_field_initializer(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_field_initializer(&mut self, assignment: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_field_initializer(&mut self.tokens, assignment, end_token);
    }

    #[inline]
    pub fn handle_no_field_initializer(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_field_initializer(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_variable_initializer(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_variable_initializer(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_variable_initializer(&mut self, assignment_operator: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_variable_initializer(&mut self.tokens, assignment_operator);
    }

    #[inline]
    pub fn handle_no_variable_initializer(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_variable_initializer(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_initializer(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_initializer(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_initializer(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_initializer(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn begin_initializers(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_initializers(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_initializers(&mut self, count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_initializers(&mut self.tokens, count, begin_token, end_token);
    }

    #[inline]
    pub fn handle_no_initializers(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_initializers(&mut self.tokens);
    }

    #[inline]
    pub fn handle_invalid_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_invalid_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_invalid_function_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_invalid_function_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_invalid_type_reference(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_invalid_type_reference(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_label(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_label(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_labeled_statement(&mut self, token: TokenId, label_count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_labeled_statement(&mut self.tokens, token, label_count);
    }

    #[inline]
    pub fn end_labeled_statement(&mut self, label_count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_labeled_statement(&mut self.tokens, label_count);
    }

    #[inline]
    pub fn begin_library_augmentation(&mut self, augment_keyword: TokenId, library_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_library_augmentation(&mut self.tokens, augment_keyword, library_keyword);
    }

    #[inline]
    pub fn end_library_augmentation(&mut self, augment_keyword: TokenId, library_keyword: TokenId, semicolon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_library_augmentation(&mut self.tokens, augment_keyword, library_keyword, semicolon);
    }

    #[inline]
    pub fn begin_library_name(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_library_name(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_library_name(&mut self, library_keyword: TokenId, semicolon: TokenId, has_name: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_library_name(&mut self.tokens, library_keyword, semicolon, has_name);
    }

    #[inline]
    pub fn handle_literal_map_entry(&mut self, colon: TokenId, end_token: TokenId, null_aware_key_token: Option<TokenId>, null_aware_value_token: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_literal_map_entry(&mut self.tokens, colon, end_token, null_aware_key_token, null_aware_value_token);
    }

    #[inline]
    pub fn handle_map_pattern_entry(&mut self, colon: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_map_pattern_entry(&mut self.tokens, colon, end_token);
    }

    #[inline]
    pub fn begin_literal_string(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_literal_string(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_interpolation_expression(&mut self, left_bracket: TokenId, right_bracket: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_interpolation_expression(&mut self.tokens, left_bracket, right_bracket);
    }

    #[inline]
    pub fn end_literal_string(&mut self, interpolation_count: i32, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_literal_string(&mut self.tokens, interpolation_count, end_token);
    }

    #[inline]
    pub fn handle_adjacent_string_literals(&mut self, start_token: TokenId, literal_count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_adjacent_string_literals(&mut self.tokens, start_token, literal_count);
    }

    #[inline]
    pub fn begin_member(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_member(&mut self.tokens);
    }

    #[inline]
    pub fn handle_invalid_member(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_invalid_member(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn end_member(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_member(&mut self.tokens);
    }

    #[inline]
    pub fn begin_method(&mut self, declaration_kind: DeclarationKind, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, var_final_or_const: Option<TokenId>, get_or_set: Option<TokenId>, name: TokenId, enclosing_declaration_name: Option<&str>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_method(&mut self.tokens, declaration_kind, augment_token, external_token, static_token, covariant_token, var_final_or_const, get_or_set, name, enclosing_declaration_name);
    }

    #[inline]
    pub fn end_method(&mut self, kind: DeclarationKind, get_or_set: Option<TokenId>, begin_token: TokenId, begin_param: TokenId, begin_initializers: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_method(&mut self.tokens, kind, get_or_set, begin_token, begin_param, begin_initializers, end_token);
    }

    #[inline]
    pub fn begin_constructor(&mut self, declaration_kind: DeclarationKind, augment_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, var_final_or_const: Option<TokenId>, get_or_set: Option<TokenId>, new_token: Option<TokenId>, name: TokenId, enclosing_declaration_name: Option<&str>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_constructor(&mut self.tokens, declaration_kind, augment_token, external_token, static_token, covariant_token, var_final_or_const, get_or_set, new_token, name, enclosing_declaration_name);
    }

    #[inline]
    pub fn end_constructor(&mut self, kind: DeclarationKind, begin_token: TokenId, new_token: Option<TokenId>, begin_param: TokenId, begin_initializers: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_constructor(&mut self.tokens, kind, begin_token, new_token, begin_param, begin_initializers, end_token);
    }

    #[inline]
    pub fn begin_metadata_star(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_metadata_star(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_metadata_star(&mut self, count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_metadata_star(&mut self.tokens, count);
    }

    #[inline]
    pub fn begin_metadata(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_metadata(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_metadata(&mut self, begin_token: TokenId, period_before_name: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_metadata(&mut self.tokens, begin_token, period_before_name, end_token);
    }

    #[inline]
    pub fn begin_optional_formal_parameters(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_optional_formal_parameters(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_optional_formal_parameters(&mut self, count: i32, begin_token: TokenId, end_token: TokenId, kind: MemberKind) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_optional_formal_parameters(&mut self.tokens, count, begin_token, end_token, kind);
    }

    #[inline]
    pub fn begin_part(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_part(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_part(&mut self, part_keyword: TokenId, semicolon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_part(&mut self.tokens, part_keyword, semicolon);
    }

    #[inline]
    pub fn begin_part_of(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_part_of(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_part_of(&mut self, part_keyword: TokenId, of_keyword: TokenId, semicolon: TokenId, has_name: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_part_of(&mut self.tokens, part_keyword, of_keyword, semicolon, has_name);
    }

    #[inline]
    pub fn begin_redirecting_factory_body(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_redirecting_factory_body(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_redirecting_factory_body(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_redirecting_factory_body(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn begin_return_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_return_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_native_function_body(&mut self, native_token: TokenId, semicolon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_native_function_body(&mut self.tokens, native_token, semicolon);
    }

    #[inline]
    pub fn handle_native_function_body_ignored(&mut self, native_token: TokenId, semicolon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_native_function_body_ignored(&mut self.tokens, native_token, semicolon);
    }

    #[inline]
    pub fn handle_native_function_body_skipped(&mut self, native_token: TokenId, semicolon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_native_function_body_skipped(&mut self.tokens, native_token, semicolon);
    }

    #[inline]
    pub fn handle_empty_function_body(&mut self, semicolon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_empty_function_body(&mut self.tokens, semicolon);
    }

    #[inline]
    pub fn handle_expression_function_body(&mut self, arrow_token: TokenId, end_token: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_expression_function_body(&mut self.tokens, arrow_token, end_token);
    }

    #[inline]
    pub fn end_return_statement(&mut self, has_expression: bool, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_return_statement(&mut self.tokens, has_expression, begin_token, end_token);
    }

    #[inline]
    pub fn handle_send(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_send(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn begin_show(&mut self, show_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_show(&mut self.tokens, show_keyword);
    }

    #[inline]
    pub fn end_show(&mut self, show_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Combinator) {
            return;
        }
        self.primary.end_show(&mut self.tokens, show_keyword);
    }

    #[inline]
    pub fn begin_switch_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_switch_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_switch_statement(&mut self, switch_keyword: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_switch_statement(&mut self.tokens, switch_keyword, end_token);
    }

    #[inline]
    pub fn begin_switch_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_switch_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_switch_expression(&mut self, switch_keyword: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_switch_expression(&mut self.tokens, switch_keyword, end_token);
    }

    #[inline]
    pub fn begin_switch_block(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_switch_block(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_switch_block(&mut self, case_count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_switch_block(&mut self.tokens, case_count, begin_token, end_token);
    }

    #[inline]
    pub fn begin_switch_expression_block(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_switch_expression_block(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_switch_expression_block(&mut self, case_count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_switch_expression_block(&mut self.tokens, case_count, begin_token, end_token);
    }

    #[inline]
    pub fn begin_literal_symbol(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_literal_symbol(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_literal_symbol(&mut self, hash_token: TokenId, identifier_count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_literal_symbol(&mut self.tokens, hash_token, identifier_count);
    }

    #[inline]
    pub fn handle_throw_expression(&mut self, throw_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_throw_expression(&mut self.tokens, throw_token, end_token);
    }

    #[inline]
    pub fn begin_rethrow_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_rethrow_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_rethrow_statement(&mut self, rethrow_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_rethrow_statement(&mut self.tokens, rethrow_token, end_token);
    }

    #[inline]
    pub fn end_top_level_declaration(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_top_level_declaration(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn handle_invalid_top_level_declaration(&mut self, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_invalid_top_level_declaration(&mut self.tokens, end_token);
    }

    #[inline]
    pub fn begin_top_level_member(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_top_level_member(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_fields(&mut self, declaration_kind: DeclarationKind, augment_token: Option<TokenId>, abstract_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, last_consumed: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_fields(&mut self.tokens, declaration_kind, augment_token, abstract_token, external_token, static_token, covariant_token, late_token, var_final_or_const, last_consumed);
    }

    #[inline]
    pub fn end_top_level_fields(&mut self, augment_token: Option<TokenId>, abstract_token: Option<TokenId>, external_token: Option<TokenId>, static_token: Option<TokenId>, covariant_token: Option<TokenId>, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>, count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_top_level_fields(&mut self.tokens, augment_token, abstract_token, external_token, static_token, covariant_token, late_token, var_final_or_const, count, begin_token, end_token);
    }

    #[inline]
    pub fn begin_top_level_method(&mut self, last_consumed: TokenId, augment_token: Option<TokenId>, external_token: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_top_level_method(&mut self.tokens, last_consumed, augment_token, external_token);
    }

    #[inline]
    pub fn end_top_level_method(&mut self, begin_token: TokenId, get_or_set: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_top_level_method(&mut self.tokens, begin_token, get_or_set, end_token);
    }

    #[inline]
    pub fn begin_try_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_try_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_catch_clause(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_catch_clause(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_catch_clause(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_catch_clause(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_catch_block(&mut self, on_keyword: Option<TokenId>, catch_keyword: Option<TokenId>, comma: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_catch_block(&mut self.tokens, on_keyword, catch_keyword, comma);
    }

    #[inline]
    pub fn handle_finally_block(&mut self, finally_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_finally_block(&mut self.tokens, finally_keyword);
    }

    #[inline]
    pub fn end_try_statement(&mut self, catch_count: i32, try_keyword: TokenId, finally_keyword: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_try_statement(&mut self.tokens, catch_count, try_keyword, finally_keyword, end_token);
    }

    #[inline]
    pub fn handle_type(&mut self, begin_token: TokenId, question_mark: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_type(&mut self.tokens, begin_token, question_mark);
    }

    #[inline]
    pub fn handle_non_null_assert_expression(&mut self, bang: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_non_null_assert_expression(&mut self.tokens, bang);
    }

    #[inline]
    pub fn handle_null_assert_pattern(&mut self, bang: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_null_assert_pattern(&mut self.tokens, bang);
    }

    #[inline]
    pub fn handle_null_check_pattern(&mut self, question: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_null_check_pattern(&mut self.tokens, question);
    }

    #[inline]
    pub fn handle_assigned_variable_pattern(&mut self, variable: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_assigned_variable_pattern(&mut self.tokens, variable);
    }

    #[inline]
    pub fn handle_declared_variable_pattern(&mut self, keyword: Option<TokenId>, variable: TokenId, in_assignment_pattern: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_declared_variable_pattern(&mut self.tokens, keyword, variable, in_assignment_pattern);
    }

    #[inline]
    pub fn handle_wildcard_pattern(&mut self, keyword: Option<TokenId>, wildcard: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_wildcard_pattern(&mut self.tokens, keyword, wildcard);
    }

    #[inline]
    pub fn handle_no_name(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_name(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_record_type(&mut self, left_bracket: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_record_type(&mut self.tokens, left_bracket);
    }

    #[inline]
    pub fn end_record_type(&mut self, left_bracket: TokenId, question_mark: Option<TokenId>, count: i32, has_named_fields: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_record_type(&mut self.tokens, left_bracket, question_mark, count, has_named_fields);
    }

    #[inline]
    pub fn begin_record_type_entry(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_record_type_entry(&mut self.tokens);
    }

    #[inline]
    pub fn end_record_type_entry(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_record_type_entry(&mut self.tokens);
    }

    #[inline]
    pub fn begin_record_type_named_fields(&mut self, left_bracket: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_record_type_named_fields(&mut self.tokens, left_bracket);
    }

    #[inline]
    pub fn end_record_type_named_fields(&mut self, count: i32, left_bracket: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_record_type_named_fields(&mut self.tokens, count, left_bracket);
    }

    #[inline]
    pub fn begin_function_type(&mut self, begin_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_function_type(&mut self.tokens, begin_token);
    }

    #[inline]
    pub fn end_function_type(&mut self, function_token: TokenId, question_mark: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_function_type(&mut self.tokens, function_token, question_mark);
    }

    #[inline]
    pub fn begin_type_arguments(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_type_arguments(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_type_arguments(&mut self, count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_type_arguments(&mut self.tokens, count, begin_token, end_token);
    }

    #[inline]
    pub fn handle_invalid_type_arguments(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_invalid_type_arguments(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_no_type_arguments(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_type_arguments(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_type_variable(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_type_variable(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_type_variables_defined(&mut self, token: TokenId, count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_type_variables_defined(&mut self.tokens, token, count);
    }

    #[inline]
    pub fn end_type_variable(&mut self, token: TokenId, index: i32, extends_or_super: Option<TokenId>, variance: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_type_variable(&mut self.tokens, token, index, extends_or_super, variance);
    }

    #[inline]
    pub fn begin_type_variables(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_type_variables(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_type_variables(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_type_variables(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn report_variance_modifier_not_enabled(&mut self, variance: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.report_variance_modifier_not_enabled(&mut self.tokens, variance);
    }

    #[inline]
    pub fn begin_function_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_function_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_function_expression(&mut self, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_function_expression(&mut self.tokens, begin_token, end_token);
    }

    #[inline]
    pub fn begin_variables_declaration(&mut self, token: TokenId, late_token: Option<TokenId>, var_final_or_const: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_variables_declaration(&mut self.tokens, token, late_token, var_final_or_const);
    }

    #[inline]
    pub fn end_variables_declaration(&mut self, count: i32, end_token: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_variables_declaration(&mut self.tokens, count, end_token);
    }

    #[inline]
    pub fn begin_while_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_while_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_while_statement(&mut self, while_keyword: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_while_statement(&mut self.tokens, while_keyword, end_token);
    }

    #[inline]
    pub fn begin_as_operator_type(&mut self, operator: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_as_operator_type(&mut self.tokens, operator);
    }

    #[inline]
    pub fn end_as_operator_type(&mut self, operator: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_as_operator_type(&mut self.tokens, operator);
    }

    #[inline]
    pub fn handle_as_operator(&mut self, operator: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_as_operator(&mut self.tokens, operator);
    }

    #[inline]
    pub fn handle_cast_pattern(&mut self, operator: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_cast_pattern(&mut self.tokens, operator);
    }

    #[inline]
    pub fn handle_assignment_expression(&mut self, token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_assignment_expression(&mut self.tokens, token, end_token);
    }

    #[inline]
    pub fn begin_anonymous_method_invocation(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_anonymous_method_invocation(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_anonymous_method_invocation(&mut self, begin_token: TokenId, function_definition: Option<TokenId>, end_token: TokenId, is_expression: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_anonymous_method_invocation(&mut self.tokens, begin_token, function_definition, end_token, is_expression);
    }

    #[inline]
    pub fn handle_implicit_formal_parameters(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_implicit_formal_parameters(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_binary_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_binary_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_binary_expression(&mut self, token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_binary_expression(&mut self.tokens, token, end_token);
    }

    #[inline]
    pub fn begin_binary_pattern(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_binary_pattern(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_binary_pattern(&mut self, operator_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_binary_pattern(&mut self.tokens, operator_token);
    }

    #[inline]
    pub fn handle_dot_access(&mut self, token: TokenId, end_token: TokenId, is_null_aware: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_dot_access(&mut self.tokens, token, end_token, is_null_aware);
    }

    #[inline]
    pub fn handle_cascade_access(&mut self, token: TokenId, end_token: TokenId, is_null_aware: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_cascade_access(&mut self.tokens, token, end_token, is_null_aware);
    }

    #[inline]
    pub fn begin_conditional_expression(&mut self, question: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_conditional_expression(&mut self.tokens, question);
    }

    #[inline]
    pub fn handle_conditional_expression_colon(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_conditional_expression_colon(&mut self.tokens);
    }

    #[inline]
    pub fn end_conditional_expression(&mut self, question: TokenId, colon: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_conditional_expression(&mut self.tokens, question, colon, end_token);
    }

    #[inline]
    pub fn begin_const_expression(&mut self, const_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_const_expression(&mut self.tokens, const_keyword);
    }

    #[inline]
    pub fn end_const_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_const_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_const_factory(&mut self, const_keyword: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_const_factory(&mut self.tokens, const_keyword);
    }

    #[inline]
    pub fn begin_for_control_flow(&mut self, await_token: Option<TokenId>, for_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_for_control_flow(&mut self.tokens, await_token, for_token);
    }

    #[inline]
    pub fn end_for_control_flow(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_for_control_flow(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_for_in_control_flow(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_for_in_control_flow(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_if_control_flow(&mut self, if_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_if_control_flow(&mut self.tokens, if_token);
    }

    #[inline]
    pub fn handle_then_control_flow(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_then_control_flow(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_else_control_flow(&mut self, else_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_else_control_flow(&mut self.tokens, else_token);
    }

    #[inline]
    pub fn end_if_control_flow(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_if_control_flow(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_if_else_control_flow(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_if_else_control_flow(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_spread_expression(&mut self, spread_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_spread_expression(&mut self.tokens, spread_token);
    }

    #[inline]
    pub fn handle_null_aware_element(&mut self, null_aware_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_null_aware_element(&mut self.tokens, null_aware_token);
    }

    #[inline]
    pub fn handle_rest_pattern(&mut self, dots: TokenId, has_sub_pattern: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_rest_pattern(&mut self.tokens, dots, has_sub_pattern);
    }

    #[inline]
    pub fn begin_function_typed_formal_parameter(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_function_typed_formal_parameter(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_function_typed_formal_parameter(&mut self, name_token: TokenId, question: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_function_typed_formal_parameter(&mut self.tokens, name_token, question);
    }

    #[inline]
    pub fn handle_identifier(&mut self, token: TokenId, context: IdentifierContext) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_identifier(&mut self.tokens, token, context);
    }

    #[inline]
    pub fn handle_indexed_expression(&mut self, question: Option<TokenId>, open_square_bracket: TokenId, close_square_bracket: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_indexed_expression(&mut self.tokens, question, open_square_bracket, close_square_bracket);
    }

    #[inline]
    pub fn begin_is_operator_type(&mut self, operator: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_is_operator_type(&mut self.tokens, operator);
    }

    #[inline]
    pub fn end_is_operator_type(&mut self, operator: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_is_operator_type(&mut self.tokens, operator);
    }

    #[inline]
    pub fn handle_is_operator(&mut self, is_operator: TokenId, not: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_is_operator(&mut self.tokens, is_operator, not);
    }

    #[inline]
    pub fn handle_literal_bool(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_literal_bool(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_break_statement(&mut self, has_target: bool, break_keyword: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_break_statement(&mut self.tokens, has_target, break_keyword, end_token);
    }

    #[inline]
    pub fn handle_continue_statement(&mut self, has_target: bool, continue_keyword: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_continue_statement(&mut self.tokens, has_target, continue_keyword, end_token);
    }

    #[inline]
    pub fn handle_empty_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_empty_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_assert(&mut self, assert_keyword: TokenId, kind: Assert) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_assert(&mut self.tokens, assert_keyword, kind);
    }

    #[inline]
    pub fn end_assert(&mut self, assert_keyword: TokenId, kind: Assert, left_parenthesis: TokenId, comma_token: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_assert(&mut self.tokens, assert_keyword, kind, left_parenthesis, comma_token, end_token);
    }

    #[inline]
    pub fn handle_literal_double(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_literal_double(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_literal_double_with_separators(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_literal_double_with_separators(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_literal_int(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_literal_int(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_literal_int_with_separators(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_literal_int_with_separators(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_literal_list(&mut self, count: i32, left_bracket: TokenId, const_keyword: Option<TokenId>, right_bracket: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_literal_list(&mut self.tokens, count, left_bracket, const_keyword, right_bracket);
    }

    #[inline]
    pub fn handle_list_pattern(&mut self, count: i32, left_bracket: TokenId, right_bracket: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_list_pattern(&mut self.tokens, count, left_bracket, right_bracket);
    }

    #[inline]
    pub fn handle_literal_set_or_map(&mut self, count: i32, left_brace: TokenId, const_keyword: Option<TokenId>, right_brace: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_literal_set_or_map(&mut self.tokens, count, left_brace, const_keyword, right_brace);
    }

    #[inline]
    pub fn handle_map_pattern(&mut self, count: i32, left_brace: TokenId, right_brace: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_map_pattern(&mut self.tokens, count, left_brace, right_brace);
    }

    #[inline]
    pub fn handle_literal_null(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_literal_null(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_native_clause(&mut self, native_token: TokenId, has_name: bool) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_native_clause(&mut self.tokens, native_token, has_name);
    }

    #[inline]
    pub fn handle_named_argument(&mut self, colon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_named_argument(&mut self.tokens, colon);
    }

    #[inline]
    pub fn handle_positional_argument(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_positional_argument(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_pattern_field(&mut self, colon: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_pattern_field(&mut self.tokens, colon);
    }

    #[inline]
    pub fn handle_named_record_field(&mut self, colon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_named_record_field(&mut self.tokens, colon);
    }

    #[inline]
    pub fn handle_positional_record_field(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_positional_record_field(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_new_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_new_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_new_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_new_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_no_arguments(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_arguments(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_no_constructor_reference_continuation_after_type_arguments(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_constructor_reference_continuation_after_type_arguments(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_no_identifier(&mut self, token: TokenId, identifier_context: IdentifierContext) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_identifier(&mut self.tokens, token, identifier_context);
    }

    #[inline]
    pub fn handle_no_type_name_in_constructor_reference(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_type_name_in_constructor_reference(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_no_type(&mut self, last_consumed: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_type(&mut self.tokens, last_consumed);
    }

    #[inline]
    pub fn handle_no_type_variables(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_no_type_variables(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_operator(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_operator(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_switch_case_no_when_clause(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_switch_case_no_when_clause(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_switch_expression_case_pattern(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_switch_expression_case_pattern(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_symbol_void(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_symbol_void(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_operator_name(&mut self, operator_keyword: TokenId, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_operator_name(&mut self.tokens, operator_keyword, token);
    }

    #[inline]
    pub fn handle_invalid_operator_name(&mut self, operator_keyword: TokenId, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_invalid_operator_name(&mut self.tokens, operator_keyword, token);
    }

    #[inline]
    pub fn handle_parenthesized_condition(&mut self, token: TokenId, case_: Option<TokenId>, when: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_parenthesized_condition(&mut self.tokens, token, case_, when);
    }

    #[inline]
    pub fn begin_pattern(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_pattern(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_pattern_guard(&mut self, when: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_pattern_guard(&mut self.tokens, when);
    }

    #[inline]
    pub fn begin_parenthesized_expression_or_record_literal(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_parenthesized_expression_or_record_literal(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_switch_case_when_clause(&mut self, when: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_switch_case_when_clause(&mut self.tokens, when);
    }

    #[inline]
    pub fn end_record_literal(&mut self, token: TokenId, count: i32, const_keyword: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_record_literal(&mut self.tokens, token, count, const_keyword);
    }

    #[inline]
    pub fn handle_record_pattern(&mut self, token: TokenId, count: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_record_pattern(&mut self.tokens, token, count);
    }

    #[inline]
    pub fn end_pattern(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_pattern(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_pattern_guard(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_pattern_guard(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_parenthesized_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_parenthesized_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_switch_case_when_clause(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_switch_case_when_clause(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_parenthesized_pattern(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_parenthesized_pattern(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_constant_pattern(&mut self, const_keyword: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_constant_pattern(&mut self.tokens, const_keyword);
    }

    #[inline]
    pub fn end_constant_pattern(&mut self, const_keyword: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_constant_pattern(&mut self.tokens, const_keyword);
    }

    #[inline]
    pub fn handle_object_pattern(&mut self, first_identifier: TokenId, dot: Option<TokenId>, second_identifier: Option<TokenId>) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_object_pattern(&mut self.tokens, first_identifier, dot, second_identifier);
    }

    #[inline]
    pub fn handle_qualified(&mut self, period: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_qualified(&mut self.tokens, period);
    }

    #[inline]
    pub fn handle_string_part(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_string_part(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_super_expression(&mut self, token: TokenId, context: IdentifierContext) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_super_expression(&mut self.tokens, token, context);
    }

    #[inline]
    pub fn begin_switch_case(&mut self, label_count: i32, expression_count: i32, begin_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_switch_case(&mut self.tokens, label_count, expression_count, begin_token);
    }

    #[inline]
    pub fn end_switch_case(&mut self, label_count: i32, expression_count: i32, default_keyword: Option<TokenId>, colon_after_default: Option<TokenId>, statement_count: i32, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_switch_case(&mut self.tokens, label_count, expression_count, default_keyword, colon_after_default, statement_count, begin_token, end_token);
    }

    #[inline]
    pub fn begin_switch_expression_case(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_switch_expression_case(&mut self.tokens);
    }

    #[inline]
    pub fn end_switch_expression_case(&mut self, begin_token: TokenId, when: Option<TokenId>, arrow: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_switch_expression_case(&mut self.tokens, begin_token, when, arrow, end_token);
    }

    #[inline]
    pub fn handle_this_expression(&mut self, token: TokenId, context: IdentifierContext) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_this_expression(&mut self.tokens, token, context);
    }

    #[inline]
    pub fn handle_unary_postfix_assignment_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_unary_postfix_assignment_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_unary_prefix_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_unary_prefix_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_relational_pattern(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_relational_pattern(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_unary_prefix_assignment_expression(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_unary_prefix_assignment_expression(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_formal_parameter_default_value_expression(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_formal_parameter_default_value_expression(&mut self.tokens);
    }

    #[inline]
    pub fn end_formal_parameter_default_value_expression(&mut self) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_formal_parameter_default_value_expression(&mut self.tokens);
    }

    #[inline]
    pub fn handle_valued_formal_parameter(&mut self, equals: TokenId, token: TokenId, kind: FormalParameterKind) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_valued_formal_parameter(&mut self.tokens, equals, token, kind);
    }

    #[inline]
    pub fn handle_formal_parameter_without_value(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_formal_parameter_without_value(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_void_keyword(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_void_keyword(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_void_keyword_with_type_arguments(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_void_keyword_with_type_arguments(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_yield_statement(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_yield_statement(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_yield_statement(&mut self, yield_token: TokenId, star_token: Option<TokenId>, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_yield_statement(&mut self.tokens, yield_token, star_token, end_token);
    }

    #[inline]
    pub fn end_invalid_yield_statement(&mut self, begin_token: TokenId, star_token: Option<TokenId>, end_token: TokenId, error_code: &'static CfeCode) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_invalid_yield_statement(&mut self.tokens, begin_token, star_token, end_token, error_code);
    }

    #[inline]
    pub fn handle_recoverable_error(&mut self, message: CfeMessage, start_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Error) {
            return;
        }
        self.primary.handle_recoverable_error(&mut self.tokens, message, start_token, end_token);
    }

    #[inline]
    pub fn handle_experiment_not_enabled(&mut self, experimental_flag: ExperimentalFlag, begin_token: TokenId, end_token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_experiment_not_enabled(&mut self.tokens, experimental_flag, begin_token, end_token);
    }

    #[inline]
    pub fn handle_error_token(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_error_token(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_unescape_error(&mut self, message: CfeMessage, location: TokenId, string_offset: i32, length: i32) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_unescape_error(&mut self.tokens, message, location, string_offset, length);
    }

    #[inline]
    pub fn handle_invalid_statement(&mut self, token: TokenId, message: CfeMessage) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_invalid_statement(&mut self.tokens, token, message);
    }

    #[inline]
    pub fn handle_script(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_script(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_type_argument_application(&mut self, open_angle_bracket: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_type_argument_application(&mut self.tokens, open_angle_bracket);
    }

    #[inline]
    pub fn handle_new_as_identifier(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_new_as_identifier(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_pattern_variable_declaration_statement(&mut self, keyword: TokenId, equals: TokenId, semicolon: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_pattern_variable_declaration_statement(&mut self.tokens, keyword, equals, semicolon);
    }

    #[inline]
    pub fn handle_pattern_assignment(&mut self, equals: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_pattern_assignment(&mut self.tokens, equals);
    }

    #[inline]
    pub fn handle_dot_shorthand_context(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_dot_shorthand_context(&mut self.tokens, token);
    }

    #[inline]
    pub fn handle_dot_shorthand_head(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.handle_dot_shorthand_head(&mut self.tokens, token);
    }

    #[inline]
    pub fn begin_const_dot_shorthand(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.begin_const_dot_shorthand(&mut self.tokens, token);
    }

    #[inline]
    pub fn end_const_dot_shorthand(&mut self, token: TokenId) {
        if !self.layers.is_empty() && !self.route(Route::Other) {
            return;
        }
        self.primary.end_const_dot_shorthand(&mut self.tokens, token);
    }

}
