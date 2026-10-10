// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/rename_parameter.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/modify_parameters.dart (AddParameter, RemoveParameter, ModifyParameters)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/add_type_parameter.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/replaced_by.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/code_template.dart (CodeTemplate, TemplateContext)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/value_generator.dart (CodeFragment, ImportedName)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/accessor.dart (ArgumentAccessor, TypeArgumentAccessor)
// Dart source: pkg/analysis_server/lib/src/services/refactoring/framework/formal_parameter.dart (FormalParameterReference.argumentFrom)
// Dart source: pkg/analyzer_plugin/lib/utilities/range_factory.dart (argumentRange)

//! The changes of the data-driven fixes other than `rename`.

use dartr_ast::*;
use dartr_syntax::TokenId;

use super::super::change_builder::{EditBuilder, FileEditBuilder};
use super::super::producer::ProducerContext;
use super::super::utils::Range;

/// Dart `FormalParameterReference`.
#[derive(Clone, Debug)]
pub enum ParameterReference {
    Positional(usize),
    Named(String),
}

/// Dart `Accessor`.
#[derive(Clone, Debug)]
pub enum Accessor {
    Argument(ParameterReference),
    TypeArgument(usize),
}

/// Dart `TemplateComponent`.
#[derive(Clone, Debug)]
pub enum TemplatePart {
    Text(String),
    /// Dart `CodeFragment`.
    Fragment(Vec<Accessor>),
    /// Dart `ImportedName`.
    Import {
        uris: Vec<String>,
        name: String,
    },
}

/// Dart `Operator` of `expression.dart`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionOp {
    And,
    Equal,
    NotEqual,
}

/// Dart `Expression` of `expression.dart`.
#[derive(Clone, Debug)]
pub enum ConditionExpr {
    Binary(ConditionOp, Box<ConditionExpr>, Box<ConditionExpr>),
    Variable(TemplatePart),
    LiteralString(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ConditionValue {
    Bool(bool),
    Str(String),
}

impl ConditionExpr {
    /// Dart `Expression.evaluateIn` returning a boolean.
    pub fn evaluate_bool(&self, c: &ProducerContext<'_>, context: Option<NodeId>) -> Option<bool> {
        match self.evaluate(c, context)? {
            ConditionValue::Bool(b) => Some(b),
            ConditionValue::Str(_) => None,
        }
    }

    fn evaluate(&self, c: &ProducerContext<'_>, context: Option<NodeId>) -> Option<ConditionValue> {
        match self {
            ConditionExpr::LiteralString(s) => Some(ConditionValue::Str(s.clone())),
            ConditionExpr::Variable(part) => match part {
                TemplatePart::Text(s) => Some(ConditionValue::Str(s.clone())),
                TemplatePart::Import { name, .. } => Some(ConditionValue::Str(name.clone())),
                TemplatePart::Fragment(accessors) => {
                    let s = match fragment_target(c.ast, context, accessors) {
                        Some(target) => c.utils.get_range_text(c.range().node(target)),
                        None => String::new(),
                    };
                    Some(ConditionValue::Str(s))
                }
            },
            ConditionExpr::Binary(op, left, right) => match op {
                ConditionOp::And => {
                    let ConditionValue::Bool(l) = left.evaluate(c, context)? else {
                        return None;
                    };
                    let ConditionValue::Bool(r) = right.evaluate(c, context)? else {
                        return None;
                    };
                    Some(ConditionValue::Bool(l && r))
                }
                ConditionOp::Equal => {
                    let l = left.evaluate(c, context)?;
                    let r = right.evaluate(c, context)?;
                    Some(ConditionValue::Bool(l == r))
                }
                ConditionOp::NotEqual => {
                    let l = left.evaluate(c, context)?;
                    let r = right.evaluate(c, context)?;
                    Some(ConditionValue::Bool(l != r))
                }
            },
        }
    }
}

/// Dart `CodeTemplate`.
#[derive(Clone, Debug)]
pub struct CodeTemplate {
    pub parts: Vec<TemplatePart>,
    pub required_if: Option<ConditionExpr>,
}

/// Dart `ParameterModification`.
#[derive(Clone, Debug)]
pub enum ParameterModification {
    Add {
        index: usize,
        name: String,
        is_required: bool,
        is_positional: bool,
        argument_value: Option<CodeTemplate>,
    },
    Remove(ParameterReference),
    ChangeType {
        reference: ParameterReference,
        argument_value: Option<CodeTemplate>,
    },
}

/// Dart `ElementDescriptor` of a `replacedBy` change.
#[derive(Clone, Debug)]
pub struct NewElement {
    pub library_uris: Vec<String>,
    pub kind: super::data_driven::Kind,
    pub components: Vec<String>,
}

/// Dart `Change`.
#[derive(Clone, Debug)]
pub enum Change {
    Rename(String),
    RenameParameter {
        old_name: String,
        new_name: String,
    },
    ModifyParameters(Vec<ParameterModification>),
    AddTypeParameter {
        index: usize,
        name: String,
        extended_type: Option<CodeTemplate>,
        argument_value: CodeTemplate,
    },
    ReplacedBy {
        new_element: NewElement,
        replace_target: bool,
        arguments: Vec<CodeTemplate>,
    },
}

/// The argument list of an invocation-like node (Dart
/// `ArgumentAccessor._getArgumentList`).
fn argument_list_of(ast: &Ast, node: NodeId) -> Option<Id<ArgumentList>> {
    if let Some(a) = ast.cast::<Annotation>(node) {
        return ast[a].arguments;
    }
    if let Some(e) = ast.cast::<ExtensionOverride>(node) {
        return Some(ast[e].argument_list);
    }
    if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
        return Some(ast[i].argument_list);
    }
    if let Some(m) = ast.cast::<MethodInvocation>(node) {
        return Some(ast[m].argument_list);
    }
    if let Some(f) = ast.cast::<FunctionExpressionInvocation>(node) {
        return Some(ast[f].argument_list);
    }
    if let Some(r) = ast.cast::<RedirectingConstructorInvocation>(node) {
        return Some(ast[r].argument_list);
    }
    if let Some(s) = ast.cast::<SuperConstructorInvocation>(node) {
        return Some(ast[s].argument_list);
    }
    if let Some(d) = ast.cast::<DotShorthandInvocation>(node) {
        return Some(ast[d].argument_list);
    }
    if let Some(d) = ast.cast::<DotShorthandConstructorInvocation>(node) {
        return Some(ast[d].argument_list);
    }
    None
}

/// The type argument list of a node (Dart
/// `TypeArgumentAccessor._getTypeArgumentList`).
fn type_argument_list_of(ast: &Ast, node: NodeId) -> Option<Id<TypeArgumentList>> {
    if let Some(e) = ast.cast::<ExtensionOverride>(node) {
        return ast[e].type_arguments;
    }
    if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
        return ast[ast[ast[i].constructor_name].type_].type_arguments;
    }
    if let Some(m) = ast.cast::<MethodInvocation>(node) {
        return ast[m].type_arguments;
    }
    if let Some(f) = ast.cast::<FunctionExpressionInvocation>(node) {
        return ast[f].type_arguments;
    }
    if let Some(t) = ast.cast::<NamedType>(node) {
        return ast[t].type_arguments;
    }
    if let Some(l) = ast.cast::<ListLiteral>(node) {
        return ast[l].type_arguments;
    }
    if let Some(l) = ast.cast::<SetOrMapLiteral>(node) {
        return ast[l].type_arguments;
    }
    None
}

/// Dart `FormalParameterReference.argumentFrom`: the argument expression.
pub fn argument_from(
    ast: &Ast,
    list: Id<ArgumentList>,
    reference: &ParameterReference,
) -> Option<NodeId> {
    let arguments = ast.list_raw(ast[list].arguments);
    match reference {
        ParameterReference::Named(name) => arguments.iter().find_map(|&a| {
            let n = ast.cast::<NamedArgument>(a)?;
            (ast.tokens.lexeme(ast[n].name) == name).then(|| ast[n].argument_expression.raw())
        }),
        ParameterReference::Positional(index) => {
            let &a = arguments.get(*index)?;
            if ast.is::<NamedArgument>(a) {
                return None;
            }
            Some(a)
        }
    }
}

/// Dart `TemplateContext._getInvocation`.
pub fn invocation_of(ast: &Ast, node: NodeId) -> Option<NodeId> {
    let parent = ast.parent(node);
    if ast.is::<ArgumentList>(node) {
        return parent;
    }
    if parent.is_some_and(|p| ast.is::<ArgumentList>(p)) {
        return parent.and_then(|p| ast.parent(p));
    }
    if ast.is::<ImportDirective>(node) || ast.is::<ExportDirective>(node) {
        return Some(node);
    }
    if ast.is::<InstanceCreationExpression>(node)
        || ast.is::<MethodInvocation>(node)
        || ast.is::<FunctionExpressionInvocation>(node)
    {
        return Some(node);
    }
    if ast.is::<NamedType>(node) {
        if let Some(cn) = parent.filter(|p| ast.is::<ConstructorName>(*p)) {
            if let Some(g) = ast
                .parent(cn)
                .filter(|g| ast.is::<InstanceCreationExpression>(*g))
            {
                return Some(g);
            }
        }
    } else if ast.is::<SimpleStringLiteral>(node) {
        if let Some(p) =
            parent.filter(|p| ast.is::<ImportDirective>(*p) || ast.is::<ExportDirective>(*p))
        {
            return Some(p);
        }
    } else if let Some(id) = ast.cast::<SimpleIdentifier>(node) {
        if let Some(p) = parent {
            if ast.is::<ConstructorName>(p) {
                if let Some(g) = ast
                    .parent(p)
                    .filter(|g| ast.is::<InstanceCreationExpression>(*g))
                {
                    return Some(g);
                }
            } else if let Some(m) = ast.cast::<MethodInvocation>(p) {
                if ast[m].method_name == id {
                    return Some(p);
                }
            } else if ast.is::<NamedType>(p) {
                if let Some(cn) = ast.parent(p).filter(|c| ast.is::<ConstructorName>(*c)) {
                    if let Some(g) = ast
                        .parent(cn)
                        .filter(|g| ast.is::<InstanceCreationExpression>(*g))
                    {
                        return Some(g);
                    }
                }
            } else if ast.is::<PrefixedIdentifier>(p) {
                return Some(p);
            }
        }
    } else if ast.is::<TypeArgumentList>(node) {
        if let Some(p) = parent {
            if ast.is::<MethodInvocation>(p)
                || ast.is::<FunctionExpressionInvocation>(p)
                || ast.is::<ExtensionOverride>(p)
            {
                return Some(p);
            }
        }
    }
    if let Some(p) = parent.filter(|p| ast.is::<NamedArgument>(*p)) {
        let g = ast.parent(p).and_then(|l| ast.parent(l));
        if g.is_some_and(|g| ast.is::<InstanceCreationExpression>(g)) {
            return g;
        }
    }
    None
}

/// The value of [accessors] from [node] (Dart `CodeFragment`), `None` when
/// an accessor is not valid.
fn fragment_target(ast: &Ast, node: Option<NodeId>, accessors: &[Accessor]) -> Option<NodeId> {
    let mut target = node?;
    for accessor in accessors {
        target = match accessor {
            Accessor::Argument(reference) => {
                let list = argument_list_of(ast, target)?;
                argument_from(ast, list, reference)?
            }
            Accessor::TypeArgument(index) => {
                let list = type_argument_list_of(ast, target)?;
                *ast.list_raw(ast[list].arguments).get(*index)?
            }
        };
    }
    Some(target)
}

impl CodeTemplate {
    /// Dart `CodeTemplate.validate`.
    pub fn validate(&self, c: &ProducerContext<'_>, context: Option<NodeId>) -> bool {
        self.parts.iter().all(|p| match p {
            TemplatePart::Fragment(accessors) => {
                fragment_target(c.ast, context, accessors).is_some()
            }
            _ => true,
        })
    }

    /// Dart `CodeTemplate.writeOn`.
    pub fn write_on(
        &self,
        c: &ProducerContext<'_>,
        context: Option<NodeId>,
        e: &mut EditBuilder<'_, '_, '_>,
    ) {
        for part in &self.parts {
            match part {
                TemplatePart::Text(t) => e.write(t),
                TemplatePart::Fragment(accessors) => {
                    if let Some(target) = fragment_target(c.ast, context, accessors) {
                        let text = c.utils.get_range_text(c.range().node(target));
                        e.write(&text);
                    }
                }
                TemplatePart::Import { uris, name } => e.write_imported_name(uris, name),
            }
        }
    }
}

/// Dart `RenameParameter.validate` and `apply` (the invocation sites and
/// the simple override case).
pub fn rename_parameter(
    c: &ProducerContext<'_>,
    old_name: &str,
    new_name: &str,
) -> Option<(u32, u32, String)> {
    let ast = c.ast;
    let node = c.node;
    let name_token: Option<TokenId> = if let Some(n) = ast.cast::<NamedArgument>(node) {
        (c.lexeme(ast[n].name) == old_name).then_some(ast[n].name)
    } else if let Some(m) = ast
        .cast::<SimpleIdentifier>(node)
        .and_then(|_| ast.parent(node))
        .and_then(|p| ast.cast::<MethodInvocation>(p))
    {
        ast.list_raw(ast[ast[m].argument_list].arguments)
            .iter()
            .filter_map(|a| ast.cast::<NamedArgument>(*a))
            .map(|a| ast[a].name)
            .find(|t| c.lexeme(*t) == old_name)
    } else if let Some(m) = ast.cast::<MethodDeclaration>(node) {
        // Dart `_OverrideData`: only the case where the overridden method no
        // longer has the parameter (the parameter is renamed).
        let parameters = ast[m].parameters?;
        ast.list_raw(ast[parameters].parameters)
            .iter()
            .filter_map(|p| parameter_name(ast, *p))
            .find(|t| c.lexeme(*t) == old_name)
    } else {
        None
    };
    let t = name_token?;
    Some((
        c.token_offset(t),
        ast.tokens.get(t).length,
        new_name.to_string(),
    ))
}

/// The name token of a formal parameter.
fn parameter_name(ast: &Ast, parameter: NodeId) -> Option<TokenId> {
    if let Some(p) = ast.cast::<RegularFormalParameter>(parameter) {
        return ast[p].name;
    }
    if let Some(p) = ast.cast::<FieldFormalParameter>(parameter) {
        return Some(ast[p].name);
    }
    if let Some(p) = ast.cast::<SuperFormalParameter>(parameter) {
        return Some(ast[p].name);
    }
    None
}

/// Dart `ModifyParameters.validate`: the argument list.
pub fn modify_parameters_argument_list(c: &ProducerContext<'_>) -> Option<Id<ArgumentList>> {
    let ast = c.ast;
    let node = c.node;
    let parent = ast.parent(node);
    let grand = parent.and_then(|p| ast.parent(p));
    let great = grand.and_then(|g| ast.parent(g));
    let invocation_list = |n: Option<NodeId>| -> Option<Id<ArgumentList>> {
        let n = n?;
        if let Some(m) = ast.cast::<MethodInvocation>(n) {
            return Some(ast[m].argument_list);
        }
        if let Some(f) = ast.cast::<FunctionExpressionInvocation>(n) {
            return Some(ast[f].argument_list);
        }
        None
    };
    if let Some(l) = invocation_list(parent) {
        return Some(l);
    }
    if let Some(l) = invocation_list(grand) {
        return Some(l);
    }
    let creation = |n: Option<NodeId>| n.and_then(|n| ast.cast::<InstanceCreationExpression>(n));
    if parent.is_some_and(|p| ast.is::<NamedType>(p))
        && grand.is_some_and(|g| ast.is::<ConstructorName>(g))
    {
        if let Some(i) = creation(great) {
            return Some(ast[i].argument_list);
        }
    }
    let parent_named = parent.is_some_and(|p| ast.is::<NamedArgument>(p));
    if parent_named {
        if let Some(i) = creation(great) {
            return Some(ast[i].argument_list);
        }
    }
    if let Some(i) = creation(grand) {
        return Some(ast[i].argument_list);
    }
    if parent_named {
        if let Some(d) = great.and_then(|g| ast.cast::<DotShorthandInvocation>(g)) {
            return Some(ast[d].argument_list);
        }
        if let Some(d) = great.and_then(|g| ast.cast::<DotShorthandConstructorInvocation>(g)) {
            return Some(ast[d].argument_list);
        }
    }
    None
}

/// Dart `range.argumentRange`.
fn argument_range(
    c: &ProducerContext<'_>,
    list: Id<ArgumentList>,
    lower: usize,
    upper: usize,
    for_deletion: bool,
) -> Range {
    let ast = c.ast;
    let arguments = ast.list_raw(ast[list].arguments).to_vec();
    let r = c.range();
    if lower == upper {
        if for_deletion {
            return r.node_in_list(&arguments, arguments[lower]);
        }
        return r.node(arguments[lower]);
    }
    if !for_deletion {
        return r.start_end(arguments[lower], arguments[upper]);
    }
    if lower == 0 {
        if upper == arguments.len() - 1 {
            return r.token_end_start(ast[list].left_parenthesis, ast[list].right_parenthesis);
        }
        return r.start_start(arguments[lower], arguments[upper + 1]);
    }
    r.end_end(arguments[lower - 1], arguments[upper])
}

/// Dart `IndexRange.contiguousSubRanges`.
fn contiguous_sub_ranges(indexes: &[usize]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let Some(&first) = indexes.first() else {
        return ranges;
    };
    let (mut lower, mut previous) = (first, first);
    for &current in &indexes[1..] {
        if current == previous + 1 {
            previous = current;
        } else {
            ranges.push((lower, previous));
            lower = current;
            previous = current;
        }
    }
    ranges.push((lower, previous));
    ranges
}

/// Dart `ModifyParameters.apply`.
pub fn apply_modify_parameters(
    c: &ProducerContext<'_>,
    b: &mut FileEditBuilder<'_, '_>,
    list: Id<ArgumentList>,
    modifications: &[ParameterModification],
) {
    let ast = c.ast;
    let Some(invocation) = ast.parent(list) else {
        return;
    };
    let arguments = ast.list_raw(ast[list].arguments).to_vec();
    let argument_count = arguments.len();
    let template_context = Some(invocation);
    let mut index_to_new: std::collections::HashMap<usize, &ParameterModification> =
        std::collections::HashMap::new();
    let mut to_insert: Vec<usize> = Vec::new();
    let mut to_delete: Vec<usize> = Vec::new();
    let mut remaining: Vec<usize> = (0..argument_count).collect();
    for m in modifications {
        match m {
            ParameterModification::Add {
                index,
                is_positional,
                is_required,
                argument_value,
                ..
            } => {
                index_to_new.insert(*index, m);
                if *is_positional || *is_required {
                    to_insert.push(*index);
                } else if let Some(cond) =
                    argument_value.as_ref().and_then(|v| v.required_if.as_ref())
                {
                    if cond.evaluate_bool(c, template_context) == Some(true) {
                        to_insert.push(*index);
                    }
                }
            }
            ParameterModification::Remove(reference) => {
                if let Some(argument) = argument_from(ast, list, reference) {
                    let real = ast
                        .parent(argument)
                        .filter(|p| ast.is::<NamedArgument>(*p))
                        .unwrap_or(argument);
                    if let Some(index) = arguments.iter().position(|a| *a == real) {
                        to_delete.push(index);
                        remaining.retain(|i| *i != index);
                    }
                }
            }
            ParameterModification::ChangeType {
                reference,
                argument_value,
            } => match argument_from(ast, list, reference) {
                None => {
                    let index = match reference {
                        ParameterReference::Positional(i) => *i,
                        ParameterReference::Named(_) => remaining.last().map_or(0, |last| last + 1),
                    };
                    remaining.push(index);
                    index_to_new.insert(index, m);
                    to_insert.push(index);
                }
                Some(argument) => {
                    if ast.is::<NullLiteral>(argument) {
                        if let Some(value) = argument_value {
                            b.add_replacement(ast.offset(argument), ast.length(argument), |e| {
                                value.write_on(c, template_context, e);
                            });
                        }
                    }
                }
            },
        }
    }
    to_insert.sort();
    to_delete.sort();
    let write_argument = |e: &mut EditBuilder<'_, '_, '_>, m: &ParameterModification| match m {
        ParameterModification::Add {
            name,
            is_positional,
            argument_value: Some(value),
            ..
        } => {
            if !*is_positional {
                e.write(name);
                e.write(": ");
            }
            value.write_on(c, template_context, e);
        }
        ParameterModification::ChangeType {
            reference,
            argument_value: Some(value),
        } => {
            if let ParameterReference::Named(name) = reference {
                e.write(name);
                e.write(": ");
            }
            value.write_on(c, template_context, e);
        }
        _ => {}
    };
    let write_insertion_range =
        |e: &mut EditBuilder<'_, '_, '_>, (lower, upper): (usize, usize), initial_comma: bool| {
            let mut needs_comma = initial_comma;
            for index in lower..=upper {
                if needs_comma {
                    e.write(", ");
                } else {
                    needs_comma = true;
                }
                if let Some(m) = index_to_new.get(&index) {
                    write_argument(e, m);
                }
            }
        };
    let insertion_ranges = contiguous_sub_ranges(&to_insert);
    let mut deletion_ranges = contiguous_sub_ranges(&to_delete);
    if !insertion_ranges.is_empty() {
        let mut next_remaining = 0;
        let mut next_insertion = 0;
        let mut insertion_count = 0;
        while next_remaining < remaining.len() && next_insertion < insertion_ranges.len() {
            let remaining_index = remaining[next_remaining];
            let range = insertion_ranges[next_insertion];
            let insertion_index = range.0;
            if insertion_index <= remaining_index + insertion_count {
                let deletion = deletion_ranges.iter().position(|(l, u)| {
                    insertion_index >= 1 && insertion_index - 1 >= *l && insertion_index - 1 <= *u
                });
                match deletion {
                    None => {
                        let (offset, initial_comma) = if insertion_index > 0 {
                            (ast.end(arguments[remaining_index - 1]), true)
                        } else if !arguments.is_empty() {
                            (ast.offset(arguments[remaining_index]), false)
                        } else {
                            (c.token_end(ast[list].left_parenthesis), false)
                        };
                        let trailing = insertion_index == 0 && !arguments.is_empty();
                        b.add_insertion(offset, |e| {
                            write_insertion_range(e, range, initial_comma);
                            if trailing {
                                e.write(", ");
                            }
                        });
                    }
                    Some(d) => {
                        let (lower, upper) = deletion_ranges[d];
                        let r = argument_range(c, list, lower, upper, false);
                        b.add_replacement(r.offset, r.length, |e| {
                            write_insertion_range(e, range, false);
                        });
                        deletion_ranges.remove(d);
                    }
                }
                insertion_count += range.1 - range.0 + 1;
                next_insertion += 1;
            } else {
                next_remaining += 1;
            }
        }
        let offset = if arguments.is_empty() {
            c.token_end(ast[list].left_parenthesis)
        } else {
            ast.end(arguments[arguments.len() - 1])
        };
        while next_insertion < insertion_ranges.len() {
            let (lower, mut upper) = insertion_ranges[next_insertion];
            match index_to_new.get(&upper).copied() {
                Some(ParameterModification::Add {
                    is_positional,
                    is_required,
                    ..
                }) => {
                    // Dart: the loop does not update the parameter, so it removes
                    // the whole range of optional positional parameters.
                    if *is_positional && !*is_required {
                        upper = lower.wrapping_sub(1);
                    }
                }
                Some(ParameterModification::ChangeType { .. }) => {
                    upper = lower.wrapping_sub(1);
                }
                _ => {}
            }
            if upper != lower.wrapping_sub(1) && upper >= lower {
                let initial_comma = next_remaining > 0 || insertion_count > 0;
                b.add_insertion(offset, |e| {
                    write_insertion_range(e, (lower, upper), initial_comma);
                });
            }
            next_insertion += 1;
            insertion_count += 1;
        }
    }
    for (lower, upper) in deletion_ranges {
        if lower == 0 && upper == arguments.len() - 1 && !insertion_ranges.is_empty() {
            let r = c.range().start_end(arguments[lower], arguments[upper]);
            b.add_deletion(r.offset, r.length);
        } else {
            let r = argument_range(c, list, lower, upper, true);
            b.add_deletion(r.offset, r.length);
        }
    }
}

/// The data of `AddTypeParameter.validate`.
pub enum TypeParameterData {
    /// The type argument list (`None`: insert a new list at the offset).
    TypeArguments(Option<Id<TypeArgumentList>>, u32),
    TypeParameters(Option<Id<TypeParameterList>>, u32),
}

/// Dart `AddTypeParameter.validate`.
pub fn add_type_parameter_validate(
    c: &ProducerContext<'_>,
    index: usize,
    extended_type: Option<&CodeTemplate>,
    argument_value: &CodeTemplate,
) -> Option<TypeParameterData> {
    let ast = c.ast;
    let node = c.node;
    let context = invocation_of(ast, node);
    let invalid = |len: usize| index > len;
    if let Some(t) = ast.cast::<NamedType>(node) {
        if !argument_value.validate(c, context) {
            return None;
        }
        let args = ast[t].type_arguments;
        if invalid(args.map_or(0, |a| ast.list_raw(ast[a].arguments).len())) {
            return None;
        }
        return Some(TypeParameterData::TypeArguments(
            args,
            c.token_end(ast[t].name),
        ));
    }
    let parent = ast.parent(node);
    let invocation = parent.and_then(|p| {
        if let Some(m) = ast.cast::<MethodInvocation>(p) {
            Some((ast[m].type_arguments, ast[m].argument_list))
        } else {
            ast.cast::<FunctionExpressionInvocation>(p)
                .map(|f| (ast[f].type_arguments, ast[f].argument_list))
        }
    });
    if let Some((args, list)) = invocation {
        if !argument_value.validate(c, context) {
            return None;
        }
        if invalid(args.map_or(0, |a| ast.list_raw(ast[a].arguments).len())) {
            return None;
        }
        return Some(TypeParameterData::TypeArguments(args, ast.offset(list)));
    }
    if let Some(m) = ast.cast::<MethodDeclaration>(node) {
        if let Some(t) = extended_type {
            if !t.validate(c, context) {
                return None;
            }
        }
        let params = ast[m].type_parameters;
        if invalid(params.map_or(0, |p| ast.list(ast[p].type_parameters).len())) {
            return None;
        }
        return Some(TypeParameterData::TypeParameters(
            params,
            c.token_end(ast[m].name),
        ));
    }
    if let Some(l) = ast.cast::<TypeArgumentList>(node) {
        if let Some(e) = parent.and_then(|p| ast.cast::<ExtensionOverride>(p)) {
            if !argument_value.validate(c, context) {
                return None;
            }
            if invalid(ast.list_raw(ast[l].arguments).len()) {
                return None;
            }
            return Some(TypeParameterData::TypeArguments(
                Some(l),
                c.token_end(ast[e].name),
            ));
        }
    }
    None
}

/// Dart `AddTypeParameter.apply`.
pub fn add_type_parameter_apply(
    c: &ProducerContext<'_>,
    b: &mut FileEditBuilder<'_, '_>,
    data: &TypeParameterData,
    index: usize,
    name: &str,
    extended_type: Option<&CodeTemplate>,
    argument_value: &CodeTemplate,
) {
    let ast = c.ast;
    let context = invocation_of(ast, c.node);
    match *data {
        TypeParameterData::TypeArguments(list, new_offset) => match list {
            None => b.add_insertion(new_offset, |e| {
                e.write("<");
                argument_value.write_on(c, context, e);
                e.write(">");
            }),
            Some(list) if index == 0 => b.add_insertion(c.token_end(ast[list].left_bracket), |e| {
                argument_value.write_on(c, context, e);
                e.write(", ");
            }),
            Some(list) => {
                let previous = ast.list_raw(ast[list].arguments)[index - 1];
                b.add_insertion(ast.end(previous), |e| {
                    e.write(", ");
                    argument_value.write_on(c, context, e);
                });
            }
        },
        TypeParameterData::TypeParameters(list, new_offset) => {
            let write_parameter = |e: &mut EditBuilder<'_, '_, '_>| {
                e.write(name);
                if let Some(t) = extended_type {
                    e.write(" extends ");
                    t.write_on(c, context, e);
                }
            };
            match list {
                None => b.add_insertion(new_offset, |e| {
                    e.write("<");
                    write_parameter(e);
                    e.write(">");
                }),
                Some(list) if index == 0 => {
                    b.add_insertion(c.token_end(ast[list].left_bracket), |e| {
                        write_parameter(e);
                        e.write(", ");
                    })
                }
                Some(list) => {
                    let previous = ast.list(ast[list].type_parameters)[index - 1];
                    b.add_insertion(ast.end(previous), |e| {
                        e.write(", ");
                        write_parameter(e);
                    });
                }
            }
        }
    }
}

/// The data of `ReplacedBy.validate`.
pub struct ReplacedByData {
    pub range: Range,
    pub suffix: Option<String>,
    pub is_instance_member: bool,
}

/// Dart `ReplacedBy._simpleIdentifierRangeStartWithTarget`.
fn range_start_with_target(ast: &Ast, node: NodeId) -> NodeId {
    let Some(parent) = ast.parent(node) else {
        return node;
    };
    if let Some(m) = ast.cast::<MethodInvocation>(parent) {
        return ast[m].target.map(|t| t.raw()).unwrap_or(node);
    }
    if let Some(p) = ast.cast::<PropertyAccess>(parent) {
        return ast[p].target.map(|t| t.raw()).unwrap_or(node);
    }
    if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
        return ast[p].prefix.raw();
    }
    if let Some(cn) = ast.cast::<ConstructorName>(parent) {
        return ast[cn].type_.raw();
    }
    if let Some(a) = ast.cast::<Annotation>(parent) {
        return ast[a].name.raw();
    }
    if ast.is::<DotShorthandConstructorInvocation>(parent)
        || ast.is::<DotShorthandInvocation>(parent)
        || ast.is::<DotShorthandPropertyAccess>(parent)
    {
        return parent;
    }
    node
}

/// Dart `ReplacedBy.validate`.
pub fn replaced_by_validate(
    c: &ProducerContext<'_>,
    old_components: &[String],
    new_kind: super::data_driven::Kind,
    replace_target: bool,
) -> Option<ReplacedByData> {
    use super::data_driven::Kind;
    let ast = c.ast;
    let ctx = c.ctx;
    let node = c.node;
    let r = c.range();
    if let Some(id) = ast.cast::<SimpleIdentifier>(node) {
        if let Some(element) = c.locator().element(id) {
            let executable = matches!(
                element.tag(),
                dartr_element::Tag::Method
                    | dartr_element::Tag::Getter
                    | dartr_element::Tag::Setter
                    | dartr_element::Tag::TopLevelFunction
            );
            if executable
                && !dartr_typesystem::member::is_static(ctx, dartr_element::ElemRef::Base(element))
            {
                // Dart `_instanceInvocation`.
                let mut suffix = None;
                let mut range = None;
                if new_kind == Kind::Method && element.tag() == dartr_element::Tag::Getter {
                    suffix = Some("()".to_string());
                    let start = if replace_target {
                        range_start_with_target(ast, node)
                    } else {
                        node
                    };
                    range = Some(r.start_end(start, node));
                } else if new_kind == Kind::Getter {
                    if let Some(m) = ast
                        .parent(node)
                        .and_then(|p| ast.cast::<MethodInvocation>(p))
                    {
                        let list = ast[m].argument_list;
                        if !ast.list_raw(ast[list].arguments).is_empty() {
                            return None;
                        }
                        if ast[m].type_arguments.is_some() {
                            return None;
                        }
                        let start = if replace_target {
                            range_start_with_target(ast, node)
                        } else {
                            node
                        };
                        range = Some(r.start_end(start, list));
                    }
                }
                let range = match range {
                    Some(range) => range,
                    None if replace_target => r.start_end(range_start_with_target(ast, node), node),
                    None => return None,
                };
                return Some(ReplacedByData {
                    range,
                    suffix,
                    is_instance_member: true,
                });
            }
        }
    }
    // Dart `_rangeWithTarget`.
    let simple_name = old_components.first()?;
    let range = if let Some(s) = ast.cast::<SimpleStringLiteral>(node) {
        let lexeme = c.lexeme(ast[s].literal);
        let value = lexeme.trim_matches(|ch| ch == '\'' || ch == '"');
        if value != simple_name {
            return None;
        }
        let quote = 1;
        Range::new(ast.offset(node) + quote, ast.length(node) - 2 * quote)
    } else if let Some(id) = ast.cast::<SimpleIdentifier>(node) {
        let expected = if simple_name.is_empty() {
            old_components.get(1)?
        } else {
            simple_name
        };
        if c.lexeme(ast[id].token) != expected {
            return None;
        }
        r.start_end(range_start_with_target(ast, node), node)
    } else if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
        r.start_end(ast[p].prefix, node)
    } else if let Some(cn) = ast.cast::<ConstructorName>(node) {
        if ast[cn].name.is_some() {
            r.node(node)
        } else {
            r.node(ast[cn].type_)
        }
    } else if let Some(t) = ast.cast::<NamedType>(node) {
        let type_name = c.lexeme(ast[t].name);
        if (old_components.len() == 1 && simple_name == type_name)
            || (old_components.len() == 2
                && simple_name.is_empty()
                && old_components[1] == type_name)
        {
            r.node_start_token_end(node, ast[t].name)
        } else if old_components.len() == 2 && old_components[1] == type_name {
            let parent = ast.parent(node)?;
            if let Some(cn) = ast.cast::<ConstructorName>(parent) {
                if ast[cn]
                    .name
                    .is_some_and(|n| c.lexeme(ast[n].token) == simple_name)
                {
                    r.node(parent)
                } else {
                    return None;
                }
            } else if let Some(m) = ast.cast::<MethodInvocation>(parent) {
                if c.lexeme(ast[ast[m].method_name].token) == simple_name {
                    r.start_end(node, ast[m].method_name)
                } else {
                    return None;
                }
            } else if let Some(p) = ast.cast::<PropertyAccess>(parent) {
                if c.lexeme(ast[ast[p].property_name].token) == simple_name {
                    r.start_end(node, ast[p].property_name)
                } else {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            return None;
        }
    } else {
        return None;
    };
    Some(ReplacedByData {
        range,
        suffix: None,
        is_instance_member: false,
    })
}

/// Dart `ReplacedBy.apply`.
pub fn replaced_by_apply(
    c: &ProducerContext<'_>,
    b: &mut FileEditBuilder<'_, '_>,
    data: &ReplacedByData,
    new_element: &NewElement,
    replace_target: bool,
    arguments: &[CodeTemplate],
) {
    let mut prefix: Option<String> = None;
    if !new_element.library_uris.is_empty() {
        let uri = new_element
            .library_uris
            .iter()
            .find(|u| b.imports_library(u))
            .unwrap_or(&new_element.library_uris[0])
            .clone();
        prefix = b.import_library_element(&uri, None, None, false);
    }
    let context = Some(c.node);
    b.add_replacement(data.range.offset, data.range.length, |e| {
        let components = &new_element.components;
        if data.is_instance_member && !replace_target {
            e.write(&components[0]);
        } else {
            if let Some(p) = &prefix {
                e.write(p);
                e.write(".");
            }
            if components[0].is_empty() {
                e.write(&components[1]);
            } else {
                let reversed: Vec<&str> = components.iter().rev().map(String::as_str).collect();
                e.write(&reversed.join("."));
            }
        }
        if !arguments.is_empty() {
            e.write("(");
            arguments[0].write_on(c, context, e);
            for a in &arguments[1..] {
                e.write(",");
                a.write_on(c, context, e);
            }
            e.write(")");
        } else if let Some(s) = &data.suffix {
            e.write(s);
        }
    });
}
