#!/usr/bin/env python3
"""Generates the parser listener code from the pinned Dart SDK.

Source: third_party/dart-sdk/pkg/_fe_analyzer_shared/lib/src/parser/listener.dart
(`class Listener`). Run from the repository root:

    python3 tools/codegen/gen_listener.py

Outputs:

- crates/dartr_parser/src/listener.rs: the `Listener` trait (same methods and
  arguments as Dart, default no-op).
- crates/dartr_parser/src/listener_stack.rs: `ListenerStack`, the listener
  the parser calls. It owns the token arena and the primary listener, and
  ports the listener swaps of the Dart parser (`NullListener`,
  `ForwardingListener`, the recovery listeners of `recovery_listeners.dart`).
- crates/dartr_parser/src/event_recorder.rs: a listener that writes every
  call as JSON (`dartr dump events`).
- tools/oracle/bin/event_recorder.g.dart: the same recorder in Dart (oracle
  mode `events`).
"""
import os
import re

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SRC = os.path.join(ROOT, 'third_party/dart-sdk/pkg/_fe_analyzer_shared/lib/src/parser/listener.dart')
OUT_TRAIT = os.path.join(ROOT, 'crates/dartr_parser/src/listener.rs')
OUT_STACK = os.path.join(ROOT, 'crates/dartr_parser/src/listener_stack.rs')
OUT_REC = os.path.join(ROOT, 'crates/dartr_parser/src/event_recorder.rs')
OUT_DART = os.path.join(ROOT, 'tools/oracle/bin/event_recorder.g.dart')

RUST_KEYWORDS = {'type', 'in', 'as', 'if', 'else', 'for', 'while', 'loop', 'match', 'ref',
                 'mut', 'fn', 'let', 'use', 'mod', 'impl', 'trait', 'struct', 'enum', 'where',
                 'self', 'super', 'crate', 'move', 'return', 'static', 'const', 'async', 'await',
                 'dyn', 'box', 'yield', 'try', 'final', 'override', 'abstract', 'macro', 'priv',
                 'typeof', 'unsized', 'virtual', 'do', 'become', 'extern', 'pub', 'true', 'false',
                 'unsafe', 'break', 'continue'}


def snake(name):
    s = re.sub(r'(?<=[a-z0-9])([A-Z])', r'_\1', name)
    s = re.sub(r'(?<=[A-Z])([A-Z][a-z])', r'_\1', s)
    return s.lower()


def rust_ident(name):
    s = snake(name)
    return s + '_' if s in RUST_KEYWORDS else s


# Dart type -> (Rust type, kind)
TYPES = {
    'Token': ('TokenId', 'token'),
    'Token?': ('Option<TokenId>', 'otoken'),
    'ErrorToken': ('TokenId', 'error_token'),
    'location': ('TokenId', 'token'),
    'int': ('i32', 'int'),
    'bool': ('bool', 'bool'),
    'String': ('&str', 'string'),
    'String?': ('Option<&str>', 'ostring'),
    'Message': ('CfeMessage', 'message'),
    'MessageCode': ("&'static CfeCode", 'code'),
    'DeclarationKind': ('DeclarationKind', 'enum'),
    'DeclarationHeaderKind': ('DeclarationHeaderKind', 'enum'),
    'MemberKind': ('MemberKind', 'enum'),
    'IdentifierContext': ('IdentifierContext', 'enum'),
    'BlockKind': ('BlockKind', 'enum'),
    'FormalParameterKind': ('FormalParameterKind', 'enum'),
    'Assert': ('Assert', 'enum'),
    'ConstructorReferenceContext': ('ConstructorReferenceContext', 'enum'),
    'ExperimentalFlag': ('ExperimentalFlag', 'enum'),
}

# Default bodies of the Dart listener that do more than `logEvent`.
DEFAULT_BODIES = {
    'reportVarianceModifierNotEnabled':
        'if let Some(variance) = variance {\n'
        '            self.handle_experiment_not_enabled(tokens, ExperimentalFlag::Variance, variance, variance);\n'
        '        }',
    'handleExperimentNotEnabled':
        'self.handle_recoverable_error(\n'
        '            tokens,\n'
        '            get_experiment_not_enabled_message(experimental_flag),\n'
        '            begin_token,\n'
        '            end_token,\n'
        '        );',
    'handleErrorToken':
        'let message = error_token_assertion_message(tokens, token);\n'
        '        self.handle_recoverable_error(tokens, message, token, token);',
    'handleUnescapeError':
        'let _ = (string_offset, length);\n'
        '        self.handle_recoverable_error(tokens, message, location, location);',
    'handleInvalidStatement':
        'self.handle_recoverable_error(tokens, message, token, token);',
}

# Events that the recovery listeners of `recovery_listeners.dart` record:
# method -> Rust `Route` expression.
ROUTES = {
    'handleClassExtends': 'Route::ClassExtends(extends_keyword)',
    'handleImplements': 'Route::Implements(implements_keyword)',
    'handleClassWithClause': 'Route::ClassWithClause(with_keyword)',
    'endConditionalUri': 'Route::ConditionalUri(if_keyword)',
    'endHide': 'Route::Combinator',
    'endShow': 'Route::Combinator',
    'handleImportPrefix': 'Route::ImportPrefix(deferred_keyword, as_keyword)',
    'handleMixinOn': 'Route::MixinOn(on_keyword)',
    'handleRecoverableError': 'Route::Error',
}


class Param:
    def __init__(self, dart_type, name):
        self.dart_type = dart_type
        self.name = name
        self.rust_name = rust_ident(name)
        self.rust_type, self.kind = TYPES[dart_type]


class Method:
    def __init__(self, name, params, doc):
        self.name = name
        self.rust_name = rust_ident(name)
        self.params = params
        self.doc = doc


def parse_params(text):
    text = re.sub(r'//[^\n]*', '', text)
    text = re.sub(r'\s+', ' ', text).replace('{', ',').replace('}', ',')
    text = text.replace('[', ',').replace(']', ',')
    params = []
    for p in text.split(','):
        p = p.strip()
        if not p:
            continue
        p = p.split('=')[0].strip()
        p = p.replace('required ', '').replace('covariant ', '').strip()
        parts = p.rsplit(' ', 1)
        if len(parts) == 1:
            # `covariant location` (untyped).
            params.append(Param('location', parts[0]))
        else:
            params.append(Param(parts[0].strip(), parts[1].strip()))
    return params


def strip_line_comments(src):
    # Remove `//` comment lines (not `///` docs): they can contain
    # parentheses inside parameter lists.
    return re.sub(r'\n[ \t]*//(?!/)[^\n]*', '', src)


def parse(src):
    start = src.index('abstract class Listener')
    body = strip_line_comments(src[start:])
    methods = []
    pat = re.compile(r'((?:\n  ///[^\n]*)*)(?:\n  //[^\n]*)*\n  void (\w+)\(([^)]*)\)', re.S)
    for m in pat.finditer(body):
        doc, name, params = m.groups()
        if name == 'logEvent':
            continue
        doc_lines = [l.strip()[3:].rstrip() for l in doc.strip('\n').split('\n') if l.strip()]
        methods.append(Method(name, parse_params(params), doc_lines))
    return methods


def rust_doc(lines, indent):
    out = []
    in_code = False
    in_indented = False
    for l in lines:
        t = l[1:] if l.startswith(' ') else l
        if t.strip().startswith('```'):
            in_code = not in_code
            out.append(f'{indent}/// ```text' if in_code else f'{indent}/// ```')
            continue
        if not in_code:
            # Indented code blocks (4 spaces) become `text` blocks so that
            # rustdoc does not compile them.
            if t.startswith('    ') and not in_indented:
                out.append(f'{indent}/// ```text')
                in_indented = True
            elif in_indented and not t.startswith('    '):
                out.append(f'{indent}/// ```')
                in_indented = False
            if not in_indented:
                t = re.sub(r'\[([A-Za-z_][\w.]*)\]', r'`\1`', t)
        out.append(f'{indent}/// {t}'.rstrip())
    if in_indented:
        out.append(f'{indent}/// ```')
    return '\n'.join(out)


HEADER = '''// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/listener.dart
//
// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_listener.py`.
'''


def gen_trait(methods):
    out = [HEADER, '''
//! The parser event listener (Dart `Listener`).
//!
//! Every method has the arguments of the Dart method, in the same order
//! (named and optional parameters are positional here). The first argument
//! is the token arena of the parser, so that a listener can read tokens and
//! change the token stream (the analyzer `AstBuilder` uses the parser's
//! `rewriter` from inside events, see `crate::token_stream_rewriter`).
//!
//! The defaults do nothing, except the methods that report errors through
//! `handle_recoverable_error` in Dart.

#![allow(unused_variables)]

use dartr_diagnostics::cfe::{CfeCode, CfeMessage};
use dartr_syntax::{TokenId, Tokens};

use crate::assert::Assert;
use crate::block_kind::BlockKind;
use crate::constructor_reference_context::ConstructorReferenceContext;
use crate::declaration_kind::{DeclarationHeaderKind, DeclarationKind};
use crate::error_token::error_token_assertion_message;
use crate::experimental_features::{ExperimentalFlag, get_experiment_not_enabled_message};
use crate::formal_parameter_kind::FormalParameterKind;
use crate::identifier_context::IdentifierContext;
use crate::member_kind::MemberKind;

/// A parser event listener (Dart `Listener`).
///
/// Events are methods that begin with one of: `begin`, `end`, or `handle`.
///
/// Events starting with `begin` and `end` come in pairs. Normally, a
/// `begin_foo` event is followed by an `end_foo` event. There's a few
/// exceptions documented below.
///
/// Events starting with `handle` are used when isn't possible to have a
/// begin event.
pub trait Listener {''']
    for m in methods:
        if m.doc:
            out.append(rust_doc(m.doc, '    '))
        params = ''.join(f', {p.rust_name}: {p.rust_type}' for p in m.params)
        body = DEFAULT_BODIES.get(m.name)
        if body:
            out.append(f'    fn {m.rust_name}(&mut self, tokens: &mut Tokens{params}) {{\n        {body}\n    }}\n')
        else:
            out.append(f'    fn {m.rust_name}(&mut self, tokens: &mut Tokens{params}) {{}}\n')
    out.append('}\n')
    return '\n'.join(out)


def gen_stack(methods):
    out = [HEADER, '''// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/forwarding_listener.dart
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
''']
    for m in methods:
        params = ''.join(f', {p.rust_name}: {p.rust_type}' for p in m.params)
        args = ''.join(f', {p.rust_name}' for p in m.params)
        route = ROUTES.get(m.name, 'Route::Other')
        out.append(f'''    #[inline]
    pub fn {m.rust_name}(&mut self{params}) {{
        if !self.layers.is_empty() && !self.route({route}) {{
            return;
        }}
        self.primary.{m.rust_name}(&mut self.tokens{args});
    }}
''')
    out.append('}\n')
    return '\n'.join(out)


def rec_arg(p):
    n = p.rust_name
    k = p.kind
    if k == 'token':
        return f'self.token(tokens, {n});'
    if k == 'otoken':
        return f'self.opt_token(tokens, {n});'
    if k == 'error_token':
        return f'self.error_token(tokens, {n});'
    if k == 'int':
        return f'self.int({n} as i64);'
    if k == 'bool':
        return f'self.bool({n});'
    if k == 'string':
        return f'self.string({n});'
    if k == 'ostring':
        return f'self.opt_string({n});'
    if k == 'message':
        return f'self.message(&{n});'
    if k == 'code':
        return f'self.string({n}.name);'
    if k == 'enum':
        return f'self.string({n}.name());'
    raise Exception(k)


def gen_recorder(methods):
    out = [HEADER, '''
//! A listener that writes every call as JSON: `["eventName", arg, ...]`.
//! Used by `dartr dump events`; the oracle writes the same JSON with
//! `tools/oracle/bin/event_recorder.g.dart`.
//!
//! Arguments: a token is `[offset, lexeme]`, `null` for no token; an error
//! token is `[offset, errorCode]` (also where a `Token` argument is an
//! error token); a message is
//! `{"code": name, "msg": problem, "fix": correction}` (`fix` only when
//! there is one); enums are their Dart names.

#![allow(unused_variables)]

use dartr_diagnostics::cfe::CfeMessage;
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
use dartr_diagnostics::cfe::CfeCode;

/// Records listener calls as a JSON array (without the brackets) in
/// [`EventRecorder::out`], and the recoverable errors as
/// `[code, offset, length]` in [`EventRecorder::errors`].
#[derive(Default)]
pub struct EventRecorder {
    pub out: String,
    pub errors: String,
    first: bool,
    count: usize,
}

impl EventRecorder {
    pub fn new() -> Self {
        EventRecorder {
            out: String::new(),
            errors: String::new(),
            first: true,
            count: 0,
        }
    }

    /// The number of recorded events.
    pub fn count(&self) -> usize {
        self.count
    }

    fn begin(&mut self, name: &str) {
        if self.count > 0 {
            self.out.push(',');
        }
        self.count += 1;
        self.out.push_str("[\\"");
        self.out.push_str(name);
        self.out.push('"');
    }

    fn end(&mut self) {
        self.out.push(']');
    }

    fn token(&mut self, tokens: &Tokens, token: TokenId) {
        self.out.push(',');
        write_token(&mut self.out, tokens, token);
    }

    fn opt_token(&mut self, tokens: &Tokens, token: Option<TokenId>) {
        match token {
            Some(t) => self.token(tokens, t),
            None => self.out.push_str(",null"),
        }
    }

    fn error_token(&mut self, tokens: &Tokens, token: TokenId) {
        use std::fmt::Write;
        let error = tokens.error(token).expect("not an error token");
        let _ = write!(self.out, ",[{},", error.char_offset);
        write_json_string(&mut self.out, error.error_code().name());
        self.out.push(']');
    }

    fn int(&mut self, value: i64) {
        use std::fmt::Write;
        let _ = write!(self.out, ",{value}");
    }

    fn bool(&mut self, value: bool) {
        self.out.push_str(if value { ",true" } else { ",false" });
    }

    fn string(&mut self, value: &str) {
        self.out.push(',');
        write_json_string(&mut self.out, value);
    }

    fn opt_string(&mut self, value: Option<&str>) {
        match value {
            Some(s) => self.string(s),
            None => self.out.push_str(",null"),
        }
    }

    fn message(&mut self, message: &CfeMessage) {
        self.out.push_str(",{\\"code\\":");
        write_json_string(&mut self.out, message.code.name);
        self.out.push_str(",\\"msg\\":");
        write_json_string(&mut self.out, &message.problem_message);
        if let Some(fix) = &message.correction_message {
            self.out.push_str(",\\"fix\\":");
            write_json_string(&mut self.out, fix);
        }
        self.out.push('}');
    }

    /// Dart `ParserError.fromTokens`.
    fn record_error(&mut self, tokens: &Tokens, message: &CfeMessage, start: TokenId, end: TokenId) {
        use std::fmt::Write;
        if !self.first {
            self.errors.push(',');
        }
        self.first = false;
        let begin = tokens.get(start).offset as i64;
        let end = tokens.get(end).end() as i64;
        self.errors.push('[');
        write_json_string(&mut self.errors, message.code.name);
        let _ = write!(self.errors, ",{},{}]", begin, end - begin);
    }
}

/// Writes a token as `[offset, lexeme]`. The offset is signed (the
/// synthetic token before the first token has offset -1).
pub fn write_token(out: &mut String, tokens: &Tokens, token: TokenId) {
    use std::fmt::Write;
    if let Some(error) = tokens.error(token) {
        // Dart `ErrorToken.lexeme` throws: write the error code.
        let _ = write!(out, "[{},", error.char_offset);
        write_json_string(out, error.error_code().name());
        out.push(']');
        return;
    }
    let _ = write!(out, "[{},", tokens.get(token).offset as i32);
    write_json_string(out, tokens.lexeme(token));
    out.push(']');
}

/// Writes [s] as a JSON string like Dart `jsonEncode`.
pub fn write_json_string(out: &mut String, s: &str) {
    use std::fmt::Write;
    out.push('"');
    let bytes = s.as_bytes();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if b >= 0x20 && b != b'"' && b != b'\\\\' {
            continue;
        }
        out.push_str(&s[start..i]);
        start = i + 1;
        match b {
            b'"' => out.push_str("\\\\\\""),
            b'\\\\' => out.push_str("\\\\\\\\"),
            8 => out.push_str("\\\\b"),
            9 => out.push_str("\\\\t"),
            10 => out.push_str("\\\\n"),
            12 => out.push_str("\\\\f"),
            13 => out.push_str("\\\\r"),
            _ => {
                let _ = write!(out, "\\\\u{:04x}", b);
            }
        }
    }
    out.push_str(&s[start..]);
    out.push('"');
}

impl Listener for EventRecorder {''']
    for m in methods:
        params = ''.join(f', {p.rust_name}: {p.rust_type}' for p in m.params)
        lines = [f'        self.begin("{m.name}");']
        for p in m.params:
            lines.append('        ' + rec_arg(p))
        lines.append('        self.end();')
        if m.name == 'handleRecoverableError':
            lines.append('        self.record_error(tokens, &message, start_token, end_token);')
        uses = ', '.join(p.rust_name for p in m.params)
        body = '\n'.join(lines)
        out.append(f'''    fn {m.rust_name}(&mut self, tokens: &mut Tokens{params}) {{
{body}
    }}
''')
    out.append('}\n')
    # Silence unused imports when no method takes a type.
    out.append('#[allow(dead_code)]\nfn _types(_: Assert, _: BlockKind, _: ConstructorReferenceContext, _: DeclarationHeaderKind, _: DeclarationKind, _: ExperimentalFlag, _: FormalParameterKind, _: IdentifierContext, _: MemberKind, _: &CfeCode) {}\n')
    return '\n'.join(out)


def dart_arg(p):
    n = p.name
    k = p.kind
    if k in ('token', 'otoken'):
        return f't({n})'
    if k == 'error_token':
        return f'e({n})'
    if k in ('int', 'bool', 'string', 'ostring'):
        return n
    if k == 'message':
        return f'm({n})'
    if k == 'code':
        return f'{n}.name'
    if k == 'enum':
        if p.dart_type == 'IdentifierContext':
            return f'{n}.toString()'
        return f'{n}.name'
    raise Exception(k)


def dart_params(m, src_params):
    return src_params


def gen_dart(methods, src):
    out = ['''// GENERATED FILE. DO NOT EDIT. Run `python3 tools/codegen/gen_listener.py`.
//
// A parser listener that records every call as JSON, for oracle mode
// `events` (see events.dart). Generated from
// pkg/_fe_analyzer_shared/lib/src/parser/listener.dart.

// ignore_for_file: implementation_imports

import 'package:_fe_analyzer_shared/src/experiments/flags.dart';
import 'package:_fe_analyzer_shared/src/messages/codes.dart';
import 'package:_fe_analyzer_shared/src/parser/assert.dart';
import 'package:_fe_analyzer_shared/src/parser/block_kind.dart';
import 'package:_fe_analyzer_shared/src/parser/constructor_reference_context.dart';
import 'package:_fe_analyzer_shared/src/parser/declaration_kind.dart';
import 'package:_fe_analyzer_shared/src/parser/formal_parameter_kind.dart';
import 'package:_fe_analyzer_shared/src/parser/identifier_context.dart';
import 'package:_fe_analyzer_shared/src/parser/listener.dart';
import 'package:_fe_analyzer_shared/src/parser/member_kind.dart';
import 'package:_fe_analyzer_shared/src/scanner/error_token.dart';
import 'package:_fe_analyzer_shared/src/scanner/token.dart';

class EventRecorder extends Listener {
  final List<Object?> events = [];
  final List<Object?> errors = [];

  Object? t(Token? token) => token == null
      ? null
      : token is ErrorToken
      ? e(token)
      : [token.offset, token.lexeme];

  Object? e(ErrorToken token) => [token.charOffset, token.errorCode.name];

  Object? m(Message message) => {
    'code': message.code.name,
    'msg': message.problemMessage,
    if (message.correctionMessage != null) 'fix': message.correctionMessage,
  };
''']
    for m in methods:
        # Reconstruct the Dart parameter list from the source.
        sig = m.dart_sig
        args = ''.join(', ' + dart_arg(p) for p in m.params)
        extra = ''
        if m.name == 'handleRecoverableError':
            extra = ('\n    errors.add([message.code.name, startToken.charOffset, '
                     'endToken.charOffset + endToken.charCount - startToken.charOffset]);')
        out.append(f'''  @override
  void {m.name}({sig}) {{
    events.add(['{m.name}'{args}]);{extra}
  }}
''')
    out.append('}\n')
    return '\n'.join(out)


def main():
    src = open(SRC).read()
    methods = parse(src)
    # Keep the Dart parameter lists for the Dart recorder.
    body = strip_line_comments(src[src.index('abstract class Listener'):])
    for m in methods:
        mm = re.search(r'\n  void ' + m.name + r'\(([^)]*)\)', body, re.S)
        sig = re.sub(r'//[^\n]*', '', mm.group(1))
        sig = re.sub(r'\s+', ' ', sig).strip()
        sig = sig.replace('covariant location', 'covariant Token location')
        m.dart_sig = sig
    names = [m.name for m in methods]
    assert len(names) == len(set(names)), 'duplicate methods'
    with open(OUT_TRAIT, 'w') as f:
        f.write(gen_trait(methods))
    with open(OUT_STACK, 'w') as f:
        f.write(gen_stack(methods))
    with open(OUT_REC, 'w') as f:
        f.write(gen_recorder(methods))
    with open(OUT_DART, 'w') as f:
        f.write(gen_dart(methods, src))
    print(f'{len(methods)} listener methods')


if __name__ == '__main__':
    main()
