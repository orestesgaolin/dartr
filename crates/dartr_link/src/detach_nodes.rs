// Dart source: pkg/analyzer/lib/src/summary2/detach_nodes.dart,
// pkg/analyzer/lib/src/summary2/not_serializable_nodes.dart

//! The last linking phase (`Linker._detachNodes`): the constant
//! initializers of variables (also default values of formal parameters)
//! that cannot be serialized are replaced by the marker identifier
//! `_notSerializableExpression`.
//!
//! The expressions are already detached (they are copies in the cycle's
//! `ConstExprs`). Not ported yet: the same replacement in annotation
//! arguments and constructor initializers (`_sanitizeArguments`).

use dartr_ast::*;
use dartr_element::*;
use dartr_syntax::TokenType;

use crate::link::LinkerCore;

/// Dart `_notSerializableName`.
pub const NOT_SERIALIZABLE_NAME: &str = "_notSerializableExpression";

/// Dart `_IsSerializableNodeVisitor`.
struct IsSerializable {
    result: bool,
}

impl AstVisitor for IsSerializable {
    fn visit_for_element(&mut self, _: &Ast, _: Id<ForElement>) {
        self.result = false;
    }
    fn visit_function_expression(&mut self, _: &Ast, _: Id<FunctionExpression>) {
        self.result = false;
    }
    fn visit_pattern_assignment(&mut self, _: &Ast, _: Id<PatternAssignment>) {
        self.result = false;
    }
    fn visit_switch_expression(&mut self, _: &Ast, _: Id<SwitchExpression>) {
        self.result = false;
    }
}

/// Dart `replaceNotSerializableExpression`: `None` when [expr] can stay.
fn replacement(core: &mut LinkerCore<'_>, expr: ConstExprId) -> Option<ConstExprId> {
    let mut v = IsSerializable { result: true };
    core.const_exprs.ast.accept(expr.0, &mut v);
    if v.result {
        return None;
    }
    let ast = &mut core.const_exprs.ast;
    let token =
        ast.tokens
            .push_synthetic_string(TokenType::STRING, NOT_SERIALIZABLE_NAME, 0, 0, Some(0));
    Some(ConstExprId(ast.add(SimpleIdentifier { token }).raw()))
}

/// Dart `_detachConstVariable` for every variable fragment of the cycle.
pub fn detach_nodes(core: &mut LinkerCore<'_>) {
    let mut fixes: Vec<(FragmentId, ConstExprId)> = Vec::new();
    {
        let store = &core.store;
        let mut collect = |id: FragmentId, init: Option<ConstExprId>| {
            if let Some(i) = init {
                fixes.push((id, i));
            }
        };
        for (i, f) in store.fragments.fields.iter() {
            collect(
                FragmentId::new(store.id, Tag::Field, i),
                f.constant_initializer,
            );
        }
        for (i, f) in store.fragments.variables.iter() {
            collect(
                FragmentId::new(store.id, Tag::TopLevelVariable, i),
                f.constant_initializer,
            );
        }
        for (i, f) in store.fragments.params.iter() {
            let _ = f;
            collect(
                FragmentId::new(store.id, Tag::FormalParameter, i),
                store.fragments.params.get(i).constant_initializer,
            );
        }
    }
    for (id, init) in fixes {
        let Some(new) = replacement(core, init) else {
            continue;
        };
        let store = &mut core.store;
        let i = id.index();
        match id.tag() {
            Tag::Field => store.fragments.fields.get_mut(i).constant_initializer = Some(new),
            Tag::TopLevelVariable => {
                store.fragments.variables.get_mut(i).constant_initializer = Some(new)
            }
            _ => store.fragments.params.get_mut(i).constant_initializer = Some(new),
        }
    }
}
