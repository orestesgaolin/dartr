// Dart source: pkg/analysis_server_plugin/lib/edit/dart/correction_producer.dart (inferUndefinedExpressionType, getTargetInterfaceElement, getDeclarationNodeFromElement)
// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (writeParameterMatchingArgument, writeParametersMatchingArguments, _getVariableNameSuggestionsForExpression, writeFunctionDeclaration, writeLocalVariableDeclaration)
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_function.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_local_variable.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_method.dart

//! The producers that create declarations for unresolved names: local
//! variables, functions and methods.

use dartr_ast::*;
use dartr_element::{Ctx, ElemRef, ElementId, Nullability, Tag, TypeId, TypeKind};
use dartr_typesystem::type_ext::TypeExt;

use super::super::change::LinkedEditSuggestionKind;
use super::super::change_builder::{ChangeBuilder, EditBuilder};
use super::super::dart_edit::{WriteType, get_camel_words};
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::simple::producer;

/// The static type of an expression.
pub fn static_type(c: &ProducerContext<'_>, node: NodeId) -> Option<TypeId> {
    c.tables.static_type.get(node).copied()
}

/// The type of the parameter that [argument] corresponds to.
fn corresponding_parameter_type(c: &ProducerContext<'_>, argument: NodeId) -> Option<TypeId> {
    let p = c.tables.param_element.get(argument).copied()?;
    Some(dartr_typesystem::member::type_(c.ctx, p))
}

fn bool_type(ctx: &Ctx<'_>) -> TypeId {
    ctx.tp.bool_type()
}

/// Dart `isDartCore<name>` of an interface type.
fn is_core(ctx: &Ctx<'_>, ty: TypeId, name: &str) -> bool {
    match *ctx.ty(ty) {
        TypeKind::Interface { element, .. } => ctx.is_element(element.raw(), "dart.core", name),
        _ => false,
    }
}

/// `inferUndefinedExpressionType` result: a type, invalid, or unknown.
pub enum Inferred {
    Type(TypeId),
    Invalid,
    Unknown,
}

impl Inferred {
    pub fn ty(&self) -> Option<TypeId> {
        match self {
            Inferred::Type(t) => Some(*t),
            _ => None,
        }
    }
}

/// Dart `inferUndefinedExpressionType` (the common cases).
pub fn infer_undefined_expression_type(c: &ProducerContext<'_>, expression: NodeId) -> Inferred {
    let ast = c.ast;
    let ctx = c.ctx;
    let Some(parent) = ast.parent(expression) else {
        return Inferred::Unknown;
    };
    let opt = |t: Option<TypeId>| match t {
        Some(t) if matches!(ctx.ty(t), TypeKind::Invalid) => Inferred::Invalid,
        Some(t) => Inferred::Type(t),
        None => Inferred::Unknown,
    };
    if let Some(n) = ast.cast::<NamedArgument>(parent) {
        if ast[n].argument_expression.raw() == expression {
            return opt(corresponding_parameter_type(c, parent)
                .or_else(|| corresponding_parameter_type(c, expression)));
        }
    }
    if ast.is::<ParenthesizedExpression>(parent) {
        return infer_undefined_expression_type(c, parent);
    }
    if ast.is::<MethodInvocation>(expression) {
        if let Some(cascade) = ast.cast::<CascadeExpression>(parent) {
            if ast
                .parent(cascade)
                .is_some_and(|p| ast.is::<ExpressionStatement>(p))
            {
                return Inferred::Type(ctx.tp.void_type());
            }
        }
        if ast.is::<ExpressionStatement>(parent) {
            return Inferred::Type(ctx.tp.void_type());
        }
    }
    if let Some(conditional) = ast.cast::<ConditionalExpression>(parent) {
        if ast[conditional].condition.raw() == expression {
            return Inferred::Type(bool_type(ctx));
        }
        return opt(corresponding_parameter_type(c, parent));
    }
    if ast.is::<ExpressionFunctionBody>(parent) || ast.is::<ReturnStatement>(parent) {
        if let Some(t) = executable_return_type(c, expression) {
            return opt(Some(t));
        }
    }
    if let Some(v) = ast.cast::<VariableDeclaration>(parent) {
        if ast[v].initializer.map(|i| i.raw()) == Some(expression) {
            if let Some(element) = c.locator().declared_element(v) {
                let ty = dartr_resolver::element_ext::variable_type(ctx, element);
                if matches!(ctx.ty(ty), TypeKind::Invalid) {
                    return Inferred::Type(ctx.tp.object_question_type());
                }
                return Inferred::Type(ty);
            }
        }
    }
    if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
        if ast[a].left_hand_side.raw() == expression {
            return opt(static_type(c, ast[a].right_hand_side.raw()));
        }
        if ast[a].right_hand_side.raw() == expression {
            if c.lexeme(ast[a].operator) == "=" {
                return opt(c.tables.write_type.get(parent).copied());
            }
            return Inferred::Invalid;
        }
    }
    if let Some(b) = ast.cast::<BinaryExpression>(parent) {
        let method = c.element_of(parent);
        if let Some(method) = method {
            if ast[b].right_operand.raw() == expression {
                let params =
                    dartr_typesystem::member::formal_parameters(ctx, ElemRef::Base(method));
                return if params.len() == 1 {
                    Inferred::Type(dartr_typesystem::member::type_(ctx, params[0]))
                } else {
                    Inferred::Unknown
                };
            }
        } else if matches!(c.lexeme(ast[b].operator), "&&" | "||") {
            return Inferred::Type(bool_type(ctx));
        }
    }
    if ast.is::<ArgumentList>(parent) {
        return opt(corresponding_parameter_type(c, expression));
    }
    let condition_of = |n: NodeId| -> Option<NodeId> {
        if let Some(s) = ast.cast::<AssertStatement>(n) {
            return Some(ast[s].condition.raw());
        }
        if let Some(s) = ast.cast::<IfStatement>(n) {
            if ast[s].case_clause.is_none() {
                return Some(ast[s].expression.raw());
            }
        }
        if let Some(s) = ast.cast::<WhileStatement>(n) {
            return Some(ast[s].condition.raw());
        }
        if let Some(s) = ast.cast::<DoStatement>(n) {
            return Some(ast[s].condition.raw());
        }
        None
    };
    if condition_of(parent) == Some(expression) {
        return Inferred::Type(bool_type(ctx));
    }
    if let Some(p) = ast.cast::<PrefixExpression>(parent) {
        if c.lexeme(ast[p].operator) == "!" {
            return Inferred::Type(bool_type(ctx));
        }
    }
    Inferred::Unknown
}

/// Dart `_executableReturnType`: the return type of the closure or of the
/// executable that contains [expression].
fn executable_return_type(c: &ProducerContext<'_>, expression: NodeId) -> Option<TypeId> {
    let ast = c.ast;
    let ctx = c.ctx;
    let mut node = ast.parent(expression);
    while let Some(n) = node {
        if let Some(f) = ast.cast::<FunctionExpression>(n) {
            let is_closure = ast
                .parent(f)
                .is_none_or(|p| !ast.is::<FunctionDeclaration>(p));
            if is_closure {
                let ty = static_type(c, n)?;
                return match *ctx.ty(ty) {
                    TypeKind::Function(data) => Some(data.ret),
                    _ => None,
                };
            }
        }
        if ast.is::<MethodDeclaration>(n)
            || ast.is::<FunctionDeclaration>(n)
            || ast.is::<ConstructorDeclaration>(n)
        {
            let element = c.locator().declared_element(n)?;
            return Some(dartr_typesystem::member::return_type(
                ctx,
                ElemRef::Base(element),
            ));
        }
        node = ast.parent(n);
    }
    None
}

/// An argument for the parameters of a new method (Dart
/// `writeParameterMatchingArgument`): the name of a named argument, the
/// type, the name suggestions of a positional one.
pub struct ArgumentInfo {
    pub named: Option<String>,
    pub ty: TypeId,
    pub suggestions: Vec<String>,
    pub index: usize,
}

/// Dart `_getBaseNameFromExpression`.
fn base_name_from_expression(c: &ProducerContext<'_>, expression: NodeId) -> Option<String> {
    let ast = c.ast;
    if let Some(a) = ast.cast::<AsExpression>(expression) {
        return base_name_from_expression(c, ast[a].expression.raw());
    }
    if let Some(p) = ast.cast::<ParenthesizedExpression>(expression) {
        return base_name_from_expression(c, ast[p].expression.raw());
    }
    let id = |i: Id<SimpleIdentifier>| c.lexeme(ast[i].token).to_string();
    let mut name: Option<String> = None;
    if let Some(s) = ast.cast::<SimpleIdentifier>(expression) {
        return Some(id(s));
    } else if let Some(p) = ast.cast::<PrefixedIdentifier>(expression) {
        return Some(id(ast[p].identifier));
    } else if let Some(p) = ast.cast::<PropertyAccess>(expression) {
        return Some(id(ast[p].property_name));
    } else if let Some(m) = ast.cast::<MethodInvocation>(expression) {
        name = Some(id(ast[m].method_name));
    } else if let Some(i) = ast.cast::<InstanceCreationExpression>(expression) {
        let named_type = ast[ast[i].constructor_name].type_;
        let type_name = c.lexeme(ast[named_type].name).to_string();
        return match ast[named_type].import_prefix {
            None => Some(type_name),
            Some(prefix) => {
                if c.element_of(prefix.raw())
                    .is_some_and(|e| e.tag() == Tag::Prefix)
                {
                    Some(type_name)
                } else {
                    Some(c.lexeme(ast[prefix].name).to_string())
                }
            }
        };
    } else if let Some(i) = ast.cast::<IndexExpression>(expression) {
        if let Some(target) = ast[i].target {
            name = base_name_from_expression(c, target.raw());
            if let Some(n) = &mut name {
                if n.ends_with("es") {
                    n.truncate(n.len() - 2);
                } else if n.ends_with('s') {
                    n.truncate(n.len() - 1);
                }
            }
        }
    }
    if let Some(n) = &name {
        for prefix in ["get", "is", "to"] {
            if n.starts_with(prefix) {
                if n == prefix {
                    return None;
                } else if n[prefix.len()..]
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_uppercase())
                {
                    return Some(n[prefix.len()..].to_string());
                }
            }
        }
    }
    name
}

/// Dart `_getCamelWordCombinations`.
fn camel_word_combinations(name: &str) -> Vec<String> {
    let parts = get_camel_words(Some(name));
    (0..parts.len())
        .map(|i| format!("{}{}", parts[i].to_lowercase(), parts[i + 1..].join("")))
        .collect()
}

/// Dart `_addAll`.
fn add_all(excluded: &[String], result: &mut Vec<String>, items: Vec<String>) {
    for item in items {
        let mut suffix = 1;
        loop {
            let name = if suffix > 1 {
                format!("{item}{suffix}")
            } else {
                item.clone()
            };
            if !excluded.contains(&name) {
                if !result.contains(&name) {
                    result.push(name);
                }
                break;
            }
            suffix += 1;
        }
    }
}

/// Dart `_getVariableNameSuggestionsForExpression`.
fn variable_name_suggestions(
    c: &ProducerContext<'_>,
    ty: TypeId,
    expression: NodeId,
    excluded: &[String],
) -> Vec<String> {
    let ctx = c.ctx;
    let ast = c.ast;
    let mut res = Vec::new();
    if let Some(name) = base_name_from_expression(c, expression) {
        let name = name.strip_prefix('_').unwrap_or(&name).to_string();
        add_all(excluded, &mut res, camel_word_combinations(&name));
    }
    // Dart `_getBaseNameFromLocationInParent`.
    let from_parent = if let Some(n) = ast
        .parent(expression)
        .and_then(|p| ast.cast::<NamedArgument>(p))
    {
        Some(c.lexeme(ast[n].name).to_string())
    } else {
        c.tables
            .param_element
            .get(expression)
            .and_then(|p| dartr_typesystem::member::name(ctx, *p))
            .map(str::to_string)
    };
    if let Some(name) = from_parent {
        add_all(excluded, &mut res, camel_word_combinations(&name));
    }
    if !matches!(ctx.ty(ty), TypeKind::Dynamic) {
        let single = |res: &mut Vec<String>, first: u8| {
            let mut ch = first;
            while ch < b'z' {
                let name = (ch as char).to_string();
                if !excluded.contains(&name) {
                    if !res.contains(&name) {
                        res.push(name);
                    }
                    break;
                }
                ch += 1;
            }
        };
        if is_core(ctx, ty, "int") {
            single(&mut res, b'i');
        } else if is_core(ctx, ty, "double") {
            single(&mut res, b'd');
        } else if is_core(ctx, ty, "String") {
            single(&mut res, b's');
        } else if let TypeKind::Interface { element, .. } = *ctx.ty(ty) {
            let name = ctx.element_name(element.raw()).unwrap_or("").to_string();
            add_all(excluded, &mut res, camel_word_combinations(&name));
        }
    }
    res
}

/// The arguments of [list] for `writeParametersMatchingArguments`.
pub fn argument_infos(c: &ProducerContext<'_>, list: Id<ArgumentList>) -> Vec<ArgumentInfo> {
    let ast = c.ast;
    let ctx = c.ctx;
    let arguments: Vec<NodeId> = ast.list_raw(ast[list].arguments).to_vec();
    let mut used: Vec<String> = arguments
        .iter()
        .filter_map(|a| {
            ast.cast::<NamedArgument>(*a)
                .map(|n| c.lexeme(ast[n].name).to_string())
        })
        .collect();
    let mut infos = Vec::new();
    // Positional arguments first, then named ones (Dart order).
    let order: Vec<(usize, NodeId)> = arguments
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, a)| !ast.is::<NamedArgument>(*a))
        .chain(
            arguments
                .iter()
                .copied()
                .enumerate()
                .filter(|(_, a)| ast.is::<NamedArgument>(*a)),
        )
        .collect();
    for (index, argument) in order {
        let named = ast.cast::<NamedArgument>(argument);
        let expression = match named {
            Some(n) => ast[n].argument_expression.raw(),
            None => argument,
        };
        let mut ty = static_type(c, expression).unwrap_or_else(|| ctx.tp.object_question_type());
        if matches!(ctx.ty(ty), TypeKind::Never(_)) || is_core(ctx, ty, "Null") {
            ty = ctx.tp.object_question_type();
        }
        let (named_name, suggestions) = match named {
            Some(n) => (Some(c.lexeme(ast[n].name).to_string()), Vec::new()),
            None => {
                let mut s = variable_name_suggestions(c, ty, expression, &used);
                if s.is_empty() {
                    s = vec![format!("param{index}")];
                }
                used.push(s[0].clone());
                (None, s)
            }
        };
        infos.push(ArgumentInfo {
            named: named_name,
            ty,
            suggestions,
            index,
        });
    }
    infos
}

/// Dart `type.nullabilitySuffix == NullabilitySuffix.none` (also for
/// `dynamic`, `void` and invalid types).
fn nullability_none(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    match *ctx.ty(ty) {
        TypeKind::Interface { nullability, .. }
        | TypeKind::Record { nullability, .. }
        | TypeKind::TypeParameter { nullability, .. }
        | TypeKind::Never(nullability) => nullability == Nullability::None,
        TypeKind::Function(f) => f.nullability == Nullability::None,
        _ => true,
    }
}

impl EditBuilder<'_, '_, '_> {
    /// Dart `writeParametersMatchingArguments` (with the arguments computed
    /// by [argument_infos]).
    pub fn write_parameters_matching_arguments(&mut self, ctx: &Ctx<'_>, infos: &[ArgumentInfo]) {
        let mut wrote = false;
        let positional: Vec<&ArgumentInfo> = infos.iter().filter(|i| i.named.is_none()).collect();
        let named: Vec<&ArgumentInfo> = infos.iter().filter(|i| i.named.is_some()).collect();
        for info in positional {
            if wrote {
                self.write(", ");
            }
            wrote = true;
            self.write_parameter_matching_argument(ctx, info);
        }
        if !named.is_empty() {
            if wrote {
                self.write(", ");
            }
            self.write("{");
            for (i, info) in named.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write_parameter_matching_argument(ctx, info);
            }
            self.write("}");
        }
    }

    /// Dart `writeParameterMatchingArgument`.
    fn write_parameter_matching_argument(&mut self, ctx: &Ctx<'_>, info: &ArgumentInfo) {
        if info.named.is_some() && nullability_none(ctx, info.ty) {
            self.write("required ");
        }
        let options = WriteType {
            add_supertype_proposals: true,
            group_name: Some(format!("TYPE{}", info.index)),
            ..Default::default()
        };
        if self.write_type(Some(info.ty), &options) {
            self.write(" ");
        }
        match &info.named {
            Some(name) => self.write(name),
            None => {
                let favorite = info.suggestions[0].clone();
                self.add_simple_linked_edit(
                    &format!("PARAM{}", info.index),
                    &favorite,
                    Some((LinkedEditSuggestionKind::Parameter, &info.suggestions)),
                );
            }
        }
    }
}

/// Dart `isDartAsyncFuture`.
fn is_future(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    match *ctx.ty(ty) {
        TypeKind::Interface { element, .. } => {
            ctx.is_element(element.raw(), "dart.async", "Future")
        }
        _ => false,
    }
}

/// Dart `CreateFunction`.
pub struct CreateFunction {
    name: String,
}

impl CreateFunction {
    pub fn new() -> Self {
        CreateFunction {
            name: String::new(),
        }
    }
}

impl CorrectionProducer for CreateFunction {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_FUNCTION)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        let Some(invocation) = ast.parent(id).and_then(|p| ast.cast::<MethodInvocation>(p)) else {
            return;
        };
        self.name = c.lexeme(ast[id].token).to_string();
        if real_target(ast, invocation).is_some() {
            return;
        }
        let Some(member) = ast.this_or_ancestor_of_type::<CompilationUnitMember>(c.node) else {
            return;
        };
        let return_type = infer_undefined_expression_type(c, invocation.raw());
        if matches!(return_type, Inferred::Invalid) {
            return;
        }
        let return_type = return_type.ty();
        let infos = argument_infos(c, ast[invocation].argument_list);
        let end = ast.end(member);
        let specify = c.code_style().specify_return_types();
        let name = self.name.clone();
        builder.add_dart_file_edit(c.path, |b| {
            let eol = b.eol();
            b.add_insertion(end, |e| {
                e.write(&format!("{eol}{eol}"));
                write_function_declaration(c.ctx, e, &name, return_type, specify, &infos);
            });
        });
    }
}

/// Dart `writeFunctionDeclaration` with a parameter writer of the
/// arguments.
fn write_function_declaration(
    ctx: &Ctx<'_>,
    e: &mut EditBuilder<'_, '_, '_>,
    name: &str,
    return_type: Option<TypeId>,
    should_write_dynamic: bool,
    infos: &[ArgumentInfo],
) {
    if let Some(rt) = return_type {
        let options = WriteType {
            group_name: Some("RETURN_TYPE".into()),
            should_write_dynamic,
            ..Default::default()
        };
        if e.write_type(Some(rt), &options) {
            e.write(" ");
        }
    }
    e.add_simple_linked_edit("NAME", name, None);
    e.write("(");
    e.write_parameters_matching_arguments(ctx, infos);
    e.write(")");
    if return_type.is_some_and(|t| is_future(ctx, t)) {
        e.write(" async");
    }
    e.write(" {}");
}

/// Dart `MethodInvocation.realTarget`.
pub fn real_target(ast: &Ast, invocation: Id<MethodInvocation>) -> Option<NodeId> {
    if let Some(t) = ast[invocation].target {
        return Some(t.raw());
    }
    // In a cascade section: the target of the cascade.
    if ast[invocation].operator.is_some() {
        let mut n = ast.parent(invocation);
        while let Some(p) = n {
            if let Some(cascade) = ast.cast::<CascadeExpression>(p) {
                return Some(ast[cascade].target.raw());
            }
            n = ast.parent(p);
        }
    }
    None
}

/// Dart `CreateLocalVariable`.
pub struct CreateLocalVariable {
    name: String,
}

impl CreateLocalVariable {
    pub fn new() -> Self {
        CreateLocalVariable {
            name: String::new(),
        }
    }
}

impl CorrectionProducer for CreateLocalVariable {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_LOCAL_VARIABLE)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        self.name = c.lexeme(ast[id].token).to_string();
        let parent = ast.parent(id);
        if let Some(a) = parent.and_then(|p| ast.cast::<AssignmentExpression>(p)) {
            if ast[a].left_hand_side.raw() == c.node
                && c.lexeme(ast[a].operator) == "="
                && ast
                    .parent(a)
                    .is_some_and(|p| ast.is::<ExpressionStatement>(p))
            {
                let offset = ast.offset(id);
                builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, "var "));
                return;
            }
        }
        if parent.is_some_and(|p| ast.is::<PrefixedIdentifier>(p) || ast.is::<PropertyAccess>(p)) {
            return;
        }
        let Some(target) = ast.this_or_ancestor_of_type::<Statement>(c.node) else {
            return;
        };
        let prefix = c.utils.get_node_prefix(target.raw());
        let inferred = infer_undefined_expression_type(c, c.node);
        let ty = match inferred {
            Inferred::Type(t) => {
                if !matches!(
                    c.ctx.ty(t),
                    TypeKind::Interface { .. }
                        | TypeKind::Function(_)
                        | TypeKind::Record { .. }
                        | TypeKind::Invalid
                ) {
                    return;
                }
                Some(t)
            }
            Inferred::Invalid | Inferred::Unknown => None,
        };
        let offset = ast.offset(target);
        let (node_offset, node_length) = (ast.offset(c.node), ast.length(c.node));
        let name = self.name.clone();
        builder.add_dart_file_edit(c.path, |b| {
            b.add_insertion(offset, |e| {
                // Dart `writeLocalVariableDeclaration`.
                match ty {
                    Some(t) => {
                        let options = WriteType {
                            group_name: Some("TYPE".into()),
                            ..Default::default()
                        };
                        e.write_type(Some(t), &options);
                    }
                    None => e.write("var"),
                }
                e.write(" ");
                e.add_simple_linked_edit("NAME", &name, None);
                e.write(";");
                e.newline();
                e.write(&prefix);
            });
            b.add_linked_position(node_offset, node_length, "NAME");
        });
    }
}

/// Dart `getTargetInterfaceElement`.
fn target_interface_element(c: &ProducerContext<'_>, target: NodeId) -> Option<ElementId> {
    if let Some(ty) = static_type(c, target) {
        if let TypeKind::Interface { element, .. } = *c.ctx.ty(ty) {
            return Some(element.raw());
        }
    }
    if c.ast.is::<Identifier>(target) {
        let e = c.element_of(target)?;
        if matches!(
            e.tag(),
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
        ) {
            return Some(e);
        }
    }
    None
}

/// Dart `CreateMethod.method`.
pub struct CreateMethod {
    name: String,
}

impl CreateMethod {
    pub fn new() -> Self {
        CreateMethod {
            name: String::new(),
        }
    }
}

impl CorrectionProducer for CreateMethod {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_METHOD)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_METHOD_MULTI)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        self.name = c.lexeme(ast[id].token).to_string();
        let Some(invocation) = ast.parent(id).and_then(|p| ast.cast::<MethodInvocation>(p)) else {
            return;
        };
        let mut has_static = false;
        let target_node;
        let mut target_path = c.path.to_string();
        match real_target(ast, invocation) {
            Some(target) => {
                if ast.is::<ExtensionOverride>(target) {
                    return;
                }
                if ast.is::<Identifier>(target)
                    && c.element_of(target)
                        .is_some_and(|e| e.tag() == Tag::Extension)
                {
                    return;
                }
                let Some(element) = target_interface_element(c, target) else {
                    return;
                };
                let Some((path, node)) = super::members::declaration_of(c, builder, element) else {
                    return;
                };
                target_path = path;
                target_node = Some(node);
                if ast.is::<Identifier>(target) {
                    has_static = c.element_of(target).is_some_and(|e| {
                        matches!(
                            e.tag(),
                            Tag::Class | Tag::Enum | Tag::ExtensionType | Tag::Mixin
                        )
                    });
                }
            }
            None => {
                let Some(enclosing) = ast.this_or_ancestor_of_type::<ClassMember>(c.node) else {
                    return;
                };
                let member_parent = ast.parent(enclosing).and_then(|p| ast.parent(p));
                match member_parent {
                    Some(p)
                        if ast.is::<CompilationUnitMember>(p)
                            && !ast.is::<ExtensionDeclaration>(p) =>
                    {
                        target_node = Some(p);
                        has_static = if let Some(cd) = ast.cast::<ConstructorDeclaration>(enclosing)
                        {
                            ast[cd].factory_keyword.is_some()
                        } else if let Some(m) = ast.cast::<MethodDeclaration>(enclosing) {
                            ast[m]
                                .modifier_keyword
                                .is_some_and(|t| c.lexeme(t) == "static")
                        } else if let Some(f) = ast.cast::<FieldDeclaration>(enclosing) {
                            ast[f].static_keyword.is_some()
                                || ast[ast[f].fields].late_keyword.is_none()
                        } else {
                            false
                        };
                    }
                    _ => target_node = None,
                }
            }
        }
        let inferred = infer_undefined_expression_type(c, invocation.raw());
        if matches!(inferred, Inferred::Invalid) {
            return;
        }
        let ty = inferred.ty();
        let infos = argument_infos(c, ast[invocation].argument_list);
        let (node_offset, node_length) = (ast.offset(c.node), ast.length(c.node));
        let name = self.name.clone();
        let ctx = c.ctx;
        let same_file = target_path == c.path;
        builder.add_dart_file_edit(&target_path, |b| {
            let Some(target_node) = target_node else {
                return;
            };
            b.insert_method(target_node, |e| {
                if has_static {
                    e.write("static ");
                }
                let options = WriteType {
                    group_name: Some("RETURN_TYPE".into()),
                    ..Default::default()
                };
                if e.write_type(ty, &options) {
                    e.write(" ");
                }
                e.add_linked_edit("NAME", |e| e.write(&name));
                e.write("(");
                e.write_parameters_matching_arguments(ctx, &infos);
                e.write(")");
                if ty.is_some_and(|t| is_future(ctx, t)) {
                    e.write(" async");
                }
                e.write(" {}");
            });
            if same_file {
                b.add_linked_position(node_offset, node_length, "NAME");
            }
        });
    }
}

// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_return_type.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/util.dart (ReturnTypeComputer)

/// Dart `ReturnTypeComputer`: the least upper bound of the types of the
/// return (or yield) statements of [node], not in nested closures.
fn compute_return_type(
    c: &ProducerContext<'_>,
    node: NodeId,
    is_generator: bool,
    result: &mut Option<TypeId>,
) {
    let ast = c.ast;
    if ast.is::<BlockFunctionBody>(node) {
        return;
    }
    let expression = if let Some(r) = ast.cast::<ReturnStatement>(node) {
        if is_generator {
            None
        } else {
            ast[r].expression.map(|e| e.raw())
        }
    } else if let Some(y) = ast.cast::<YieldStatement>(node) {
        if is_generator {
            Some(ast[y].expression.raw())
        } else {
            None
        }
    } else {
        None
    };
    if ast.is::<ReturnStatement>(node) || ast.is::<YieldStatement>(node) {
        if let Some(t) = expression.and_then(|e| static_type(c, e)) {
            if !matches!(c.ctx.ty(t), TypeKind::Never(Nullability::None)) {
                *result = Some(match *result {
                    None => t,
                    Some(current) => dartr_typesystem::type_system::TypeSystem::new(*c.ctx)
                        .least_upper_bound(current, t),
                });
            }
        }
        return;
    }
    for child in ast.children(node) {
        compute_return_type(c, child, is_generator, result);
    }
}

producer!(
    AddReturnType,
    k::ADD_RETURN_TYPE,
    Some(&k::ADD_RETURN_TYPE_MULTI),
    Automatically,
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let node = c.node;
        let (insert_before, body) = if let Some(m) = ast.cast::<MethodDeclaration>(node) {
            if ast[m].name != c.token || ast[m].return_type.is_some() {
                return;
            }
            if ast[m]
                .property_keyword
                .is_some_and(|t| c.lexeme(t) == "set")
            {
                return;
            }
            (
                ast[m]
                    .operator_keyword
                    .or(ast[m].property_keyword)
                    .unwrap_or(ast[m].name),
                ast[m].body.raw(),
            )
        } else if let Some(f) = ast.cast::<FunctionDeclaration>(node) {
            if ast[f].name != c.token || ast[f].return_type.is_some() {
                return;
            }
            if ast[f]
                .property_keyword
                .is_some_and(|t| c.lexeme(t) == "set")
            {
                return;
            }
            let expression = ast[f].function_expression;
            (
                ast[f].property_keyword.unwrap_or(ast[f].name),
                ast[expression].body.raw(),
            )
        } else {
            return;
        };
        // Dart `_inferReturnType`.
        let (keyword, star) = if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
            (ast[b].keyword, ast[b].star)
        } else if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
            (ast[b].keyword, ast[b].star)
        } else {
            return;
        };
        let is_async = keyword.is_some_and(|k| c.lexeme(k) == "async");
        let is_generator = star.is_some();
        let base = if let Some(b) = ast.cast::<BlockFunctionBody>(body) {
            let mut result = None;
            compute_return_type(c, ast[b].block.raw(), is_generator, &mut result);
            result.unwrap_or_else(|| ctx.tp.void_type())
        } else if let Some(b) = ast.cast::<ExpressionFunctionBody>(body) {
            match static_type(c, ast[b].expression.raw()) {
                Some(t) => t,
                None => return,
            }
        } else {
            return;
        };
        let return_type = if is_async {
            if is_generator {
                ctx.tp.stream_type(ctx, base)
            } else {
                ctx.tp.future_type(ctx, base)
            }
        } else if is_generator {
            ctx.tp.iterable_type(ctx, base)
        } else {
            base
        };
        let offset = c.token_offset(insert_before);
        builder.add_dart_file_edit(c.path, |b| {
            let dynamic = matches!(ctx.ty(return_type), TypeKind::Dynamic);
            if dynamic || b.can_write_type_at(offset, return_type) {
                b.add_insertion(offset, |e| {
                    let options = WriteType {
                        should_write_dynamic: true,
                        ..Default::default()
                    };
                    e.write_type(Some(return_type), &options);
                    e.write(" ");
                });
            }
        });
    }
);

// Dart source: pkg/analysis_server/lib/src/services/correction/dart/create_parameter.dart
// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (writeParameter, DartLinkedEditBuilderImpl.addSuperTypesAsSuggestions)

/// The kind of a formal parameter node.
fn parameter_kind(ast: &Ast, p: NodeId) -> ParameterKind {
    if let Some(x) = ast.cast::<RegularFormalParameter>(p) {
        ast[x].kind
    } else if let Some(x) = ast.cast::<FieldFormalParameter>(p) {
        ast[x].kind
    } else if let Some(x) = ast.cast::<SuperFormalParameter>(p) {
        ast[x].kind
    } else {
        ParameterKind::Required
    }
}

/// Dart `CreateParameter`.
pub struct CreateParameter {
    name: String,
}

impl CreateParameter {
    pub fn new() -> Self {
        CreateParameter {
            name: String::new(),
        }
    }
}

impl CorrectionProducer for CreateParameter {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CREATE_PARAMETER)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        self.name = c.lexeme(ast[id].token).to_string();
        let parameters = ast
            .this_or_ancestor_of_type::<FunctionExpression>(id)
            .and_then(|f| ast[f].parameters)
            .or_else(|| {
                ast.this_or_ancestor_of_type::<MethodDeclaration>(id)
                    .and_then(|m| ast[m].parameters)
            })
            .or_else(|| {
                ast.this_or_ancestor_of_type::<ConstructorDeclaration>(id)
                    .map(|m| ast[m].parameters)
            });
        let Some(parameters) = parameters else { return };
        let list: Vec<NodeId> = ast.list_raw(ast[parameters].parameters).to_vec();
        let required: Vec<NodeId> = list
            .iter()
            .copied()
            .filter(|p| parameter_kind(ast, *p) == ParameterKind::Required)
            .collect();
        let named: Vec<NodeId> = list
            .iter()
            .copied()
            .filter(|p| {
                matches!(
                    parameter_kind(ast, *p),
                    ParameterKind::Named | ParameterKind::NamedRequired
                )
            })
            .collect();
        let something_after_positionals = !required.is_empty()
            && list
                .iter()
                .any(|p| parameter_kind(ast, *p) != ParameterKind::Required);
        let something_before_named = required.is_empty()
            && list.iter().any(|p| {
                !matches!(
                    parameter_kind(ast, *p),
                    ParameterKind::Named | ParameterKind::NamedRequired
                )
            });
        let has_following = something_after_positionals || something_before_named;
        let ty = match infer_undefined_expression_type(c, c.node) {
            Inferred::Invalid => return,
            Inferred::Type(t) => t,
            Inferred::Unknown => ctx.tp.dynamic_type(),
        };
        let last_required = required.last().copied();
        let last_named = named.last().copied();
        let has_previous = last_required.is_some() || last_named.is_some();
        let last = last_required.or(last_named);
        let trailing_comma = list
            .last()
            .is_some_and(|p| c.lexeme(ast.tokens.next(ast.end_token(*p))) == ",");
        let insertion = if let Some(last) = last {
            let next = ast.tokens.next(ast.end_token(last));
            if trailing_comma {
                c.token_end(next)
            } else if has_following {
                c.token_end(next) + 1
            } else {
                ast.end(last)
            }
        } else {
            c.token_end(ast[parameters].left_parenthesis)
        };
        let whitespace = last.map(|l| c.utils.get_node_prefix(l)).unwrap_or_default();
        let is_required_named = last.is_some() && last == last_named && nullability_none(ctx, ty);
        let name = self.name.clone();
        let (node_offset, node_length) = (ast.offset(c.node), ast.length(c.node));
        builder.add_dart_file_edit(c.path, |b| {
            b.add_insertion(insertion, |e| {
                if has_previous {
                    if trailing_comma {
                        e.newline();
                        e.write(&whitespace);
                    } else if !has_following {
                        e.write(", ");
                    }
                }
                // Dart `writeParameter`.
                if is_required_named {
                    e.write("required ");
                }
                let mut has_type = false;
                e.add_linked_edit("TYPE", |e| {
                    has_type = e.write_type(
                        Some(ty),
                        &WriteType {
                            should_write_dynamic: true,
                            ..Default::default()
                        },
                    );
                    // Dart `addSuperTypesAsSuggestions`.
                    if matches!(ctx.ty(ty), TypeKind::Interface { .. }) {
                        let mut types = vec![ty];
                        types.extend(ctx.all_supertypes(ty));
                        for t in types {
                            let display =
                                dartr_element::type_display_string_with(ctx, t, Default::default());
                            e.add_suggestion(LinkedEditSuggestionKind::Type, &display);
                        }
                    }
                });
                if !name.is_empty() {
                    if has_type {
                        e.write(" ");
                    }
                    e.add_linked_edit("NAME", |e| e.write(&name));
                }
                if trailing_comma {
                    e.write(",");
                } else if has_following {
                    e.write(", ");
                }
            });
            b.add_linked_position(node_offset, node_length, "NAME");
        });
    }
}
