// Dart source: pkg/analysis_server/lib/src/computer/computer_inlay_hint.dart

//! LSP inlay hints (`textDocument/inlayHint`): the inferred types and the
//! parameter names that are not written in the source.

use dartr_ast::*;
use dartr_element::display_string::{DisplayOptions, type_display_string_with};
use dartr_element::{Ctx, ElemRef, ElementId, Nullability, Tag, TypeId, TypeKind};
use dartr_syntax::{LineInfo, TokenId};
use dartr_typesystem::member;
use serde_json::{Value, json};

use crate::client_configuration::{InlayHintsConfiguration, InlayHintsParameterNamesMode};
use crate::element_locator::Unit;
use crate::mapping;

/// Dart `_InlayHintKind`.
#[derive(Clone, Copy)]
enum Kind {
    DotShorthandType,
    ParameterNameLiteral,
    ParameterNameNonLiteral,
    ParameterType,
    ReturnType,
    TypeArgument,
    VariableType,
}

const TYPE: i64 = 1;
const PARAMETER: i64 = 2;

/// Dart `DartInlayHintComputer`.
pub struct Computer<'u, 'c, 'a> {
    unit: &'u Unit<'c, 'a>,
    lines: &'u LineInfo,
    config: InlayHintsConfiguration,
    pub hints: Vec<Value>,
}

impl<'u, 'c, 'a> Computer<'u, 'c, 'a> {
    pub fn new(unit: &'u Unit<'c, 'a>, lines: &'u LineInfo, config: InlayHintsConfiguration) -> Self {
        Computer {
            unit,
            lines,
            config,
            hints: Vec::new(),
        }
    }

    pub fn compute(mut self, root: Id<CompilationUnit>) -> Vec<Value> {
        let ast = self.unit.ast;
        ast.accept(root.raw(), &mut self);
        self.hints
    }

    fn ctx(&self) -> &'c Ctx<'a> {
        self.unit.ctx
    }

    fn position(&self, offset: u32) -> Value {
        mapping::to_position(self.lines, offset)
    }

    /// Dart `_addHint`.
    fn add(&mut self, hint: Value, kind: Kind) {
        let c = &self.config;
        let enabled = match kind {
            Kind::DotShorthandType => c.dot_shorthand_types,
            Kind::ParameterNameLiteral => c.parameter_names != InlayHintsParameterNamesMode::None,
            Kind::ParameterNameNonLiteral => c.parameter_names == InlayHintsParameterNamesMode::All,
            Kind::ReturnType => c.return_types,
            Kind::ParameterType => c.parameter_types,
            Kind::VariableType => c.variable_types,
            Kind::TypeArgument => c.type_arguments,
        };
        if enabled {
            self.hints.push(hint);
        }
    }

    /// Dart `_locationForElement`.
    fn location(&self, element: Option<ElementId>) -> Option<Value> {
        let ctx = self.ctx();
        let element = element?;
        let first = ctx.element_data(element)?.first_fragment;
        let data = ctx.fragment_data(first)?;
        let name_offset = data.name_offset?;
        let library = crate::navigation::library_fragment_of(ctx, first)?
            .cast::<dartr_element::LibraryFragment>()?;
        let library = ctx.fragment(library);
        let lines = LineInfo::new(library.line_starts.to_vec());
        let length = data
            .name
            .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
            .unwrap_or(0);
        Some(json!({
            "uri": crate::uri::path_to_uri(&library.source.path),
            "range": mapping::to_range(&lines, name_offset, length),
        }))
    }

    fn part(value: impl Into<String>, location: Option<Value>) -> Value {
        let mut p = json!({"value": value.into()});
        if let Some(l) = location {
            p["location"] = l;
        }
        p
    }

    /// Dart `DartType.element`.
    fn type_element(&self, ty: TypeId) -> Option<ElementId> {
        match *self.ctx().ty(ty) {
            TypeKind::Interface { element, .. } => Some(element.raw()),
            TypeKind::TypeParameter { param, .. } => Some(param.raw()),
            _ => None,
        }
    }

    /// Dart `_appendTypePart`.
    fn type_part(&self, parts: &mut Vec<Value>, ty: TypeId) {
        let ctx = *self.ctx();
        let nullability;
        match *ctx.ty(ty) {
            TypeKind::Record {
                positional,
                named,
                nullability: n,
                ..
            } => {
                nullability = n;
                parts.push(Self::part("(", None));
                let positional = ctx.list(positional).to_vec();
                let named = ctx.list(named).to_vec();
                let count = positional.len() + named.len();
                let mut index = 0;
                for t in &positional {
                    self.type_part(parts, *t);
                    index += 1;
                    if index != count {
                        parts.push(Self::part(", ", None));
                    }
                }
                if !named.is_empty() {
                    parts.push(Self::part("{", None));
                    for f in &named {
                        self.type_part(parts, f.ty);
                        parts.push(Self::part(format!(" {}", ctx.name_str(f.name)), None));
                        index += 1;
                        if index != count {
                            parts.push(Self::part(", ", None));
                        }
                    }
                    parts.push(Self::part("}", None));
                }
                if positional.len() == 1 && named.is_empty() {
                    parts.push(Self::part(",", None));
                }
                parts.push(Self::part(")", None));
            }
            kind => {
                let element = self.type_element(ty);
                let name = match kind {
                    TypeKind::Dynamic => Some("dynamic".to_string()),
                    TypeKind::Never(_) => Some("Never".to_string()),
                    _ => element.and_then(|e| element_name(&ctx, e).map(str::to_string)),
                };
                let value = name.unwrap_or_else(|| type_display_string_with(&ctx, ty, DisplayOptions::default()));
                parts.push(Self::part(value, self.location(element)));
                if let TypeKind::Interface { args, .. } = kind {
                    let args = ctx.list(args).to_vec();
                    if !args.is_empty() {
                        self.type_argument_parts(parts, &args);
                    }
                }
                nullability = type_nullability(kind);
            }
        }
        if nullability == Nullability::Question {
            parts.push(Self::part("?", None));
        }
    }

    /// Dart `_appendTypeArgumentParts`.
    fn type_argument_parts(&self, parts: &mut Vec<Value>, types: &[TypeId]) {
        parts.push(Self::part("<", None));
        for (i, t) in types.iter().enumerate() {
            self.type_part(parts, *t);
            if i != types.len() - 1 {
                parts.push(Self::part(", ", None));
            }
        }
        parts.push(Self::part(">", None));
    }

    /// Dart `_addTypePrefix`.
    fn type_prefix(&mut self, offset: u32, ty: TypeId, kind: Kind, padding_right: bool) {
        let mut parts = Vec::new();
        self.type_part(&mut parts, ty);
        let hint = json!({
            "label": parts,
            "position": self.position(offset),
            "kind": TYPE,
            "paddingRight": padding_right,
        });
        self.add(hint, kind);
    }

    /// Dart `addTypeArguments`.
    fn type_arguments(&mut self, offset: u32, types: &[TypeId]) {
        if types.is_empty() {
            return;
        }
        let mut parts = Vec::new();
        self.type_argument_parts(&mut parts, types);
        let hint = json!({
            "label": parts,
            "position": self.position(offset),
            "kind": TYPE,
        });
        self.add(hint, Kind::TypeArgument);
    }

    /// Dart `maybeAddTypeArguments`.
    fn maybe_type_arguments(&mut self, offset: u32, ty: Option<TypeId>) {
        let Some(ty) = ty else { return };
        let ctx = *self.ctx();
        let args = match *ctx.ty(ty) {
            TypeKind::Interface { args, .. } => ctx.list(args).to_vec(),
            _ => return,
        };
        self.type_arguments(offset, &args);
    }

    /// Dart `addParameterNamePrefix`.
    fn parameter_name(&mut self, offset: u32, parameter: ElemRef, literal: bool, substituted: bool) {
        let ctx = *self.ctx();
        let base = member::base_element(&ctx, parameter);
        let Some(name) = element_name(&ctx, base).filter(|n| !n.is_empty()) else {
            return;
        };
        // A substituted parameter (of an instantiated invoke type) is a new
        // element in Dart, without a fragment: no location.
        let location = match parameter {
            ElemRef::Base(_) if !substituted => self.location(Some(base)),
            _ => None,
        };
        let hint = json!({
            "label": [Self::part(format!("{name}:"), location)],
            "position": self.position(offset),
            "kind": PARAMETER,
            "paddingRight": true,
        });
        let kind = if literal {
            Kind::ParameterNameLiteral
        } else {
            Kind::ParameterNameNonLiteral
        };
        self.add(hint, kind);
    }

    fn token_offset(&self, t: TokenId) -> u32 {
        self.unit.ast.tokens.get(t).offset
    }

    fn element_type(&self, e: ElementId) -> TypeId {
        member::type_(self.ctx(), ElemRef::Base(e))
    }

    fn return_type(&self, e: ElementId) -> TypeId {
        member::return_type(self.ctx(), ElemRef::Base(e))
    }

    fn static_type(&self, node: NodeId) -> Option<TypeId> {
        self.unit.tables.static_type.get(node).copied()
    }

    /// `InvocationExpression`: the inferred type arguments.
    fn invocation(&mut self, node: NodeId, type_arguments: bool, argument_list: Id<ArgumentList>) {
        if type_arguments {
            return;
        }
        let ctx = *self.ctx();
        let types: Vec<TypeId> = self
            .unit
            .tables
            .type_arg_types
            .get(node)
            .map(|l| ctx.list(*l).to_vec())
            .unwrap_or_default();
        let offset = self.token_offset(self.unit.ast[argument_list].left_parenthesis);
        self.type_arguments(offset, &types);
    }
}

fn type_nullability(kind: TypeKind) -> Nullability {
    match kind {
        TypeKind::Interface { nullability, .. }
        | TypeKind::Never(nullability)
        | TypeKind::Record { nullability, .. } => nullability,
        TypeKind::TypeParameter { nullability, .. } => nullability,
        TypeKind::Function(f) => f.nullability,
        _ => Nullability::None,
    }
}

/// Dart `Literal`.
fn is_literal(ast: &Ast, node: NodeId) -> bool {
    ast.is::<Literal>(node)
}

impl AstVisitor for Computer<'_, '_, '_> {
    fn visit_argument_list(&mut self, ast: &Ast, node: Id<ArgumentList>) {
        // Dart: the parameters of a member instantiated with type arguments
        // are new elements without a fragment (no location).
        // (A constructor member keeps the parameters of the declaration.)
        let (member, _) = crate::signature::invocation_substitution(self.unit, node.raw());
        let creation = ast.parent(node.raw()).is_some_and(|p| {
            ast.is::<InstanceCreationExpression>(p)
                || ast.is::<SuperConstructorInvocation>(p)
                || ast.is::<RedirectingConstructorInvocation>(p)
                || ast.is::<DotShorthandConstructorInvocation>(p)
        });
        let substituted = member && !creation;
        for &argument in ast.list_raw(ast[node].arguments) {
            if ast.is::<NamedArgument>(argument) {
                continue;
            }
            if let Some(p) = self.unit.tables.param_element.get(argument).copied() {
                let offset = ast.offset(argument);
                self.parameter_name(offset, p, is_literal(ast, argument), substituted);
            } else if let Some(p) = crate::signature::invoke_type_parameter(self.unit, argument) {
                // A parameter of a function type without a declaration (a
                // new element in Dart, without a location).
                let name = p.name.map(|n| self.ctx().name_str(n).to_string()).unwrap_or_default();
                if !name.is_empty() {
                    let hint = json!({
                        "label": [Self::part(format!("{name}:"), None)],
                        "position": self.position(ast.offset(argument)),
                        "kind": PARAMETER,
                        "paddingRight": true,
                    });
                    let kind = if is_literal(ast, argument) {
                        Kind::ParameterNameLiteral
                    } else {
                        Kind::ParameterNameNonLiteral
                    };
                    self.add(hint, kind);
                }
            }
        }
        ast.visit_children(node.raw(), self);
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        ast.visit_children(node.raw(), self);
        if ast[node].type_.is_some() {
            return;
        }
        if let Some(e) = self.unit.declared_element(node.raw()).filter(|e| e.tag() == Tag::LocalVariable) {
            let offset = self.token_offset(ast[node].name);
            self.type_prefix(offset, self.element_type(e), Kind::VariableType, true);
        }
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        ast.visit_children(node.raw(), self);
        if ast[node].type_.is_some() {
            return;
        }
        if let Some(e) = self.unit.declared_element(node.raw()) {
            let offset = self.token_offset(ast[node].name);
            self.type_prefix(offset, self.element_type(e), Kind::VariableType, true);
        }
    }

    fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        ast: &Ast,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        ast.visit_children(node.raw(), self);
        if let Some(t) = self.static_type(node.raw()) {
            let offset = self.token_offset(ast[node].period);
            self.type_prefix(offset, t, Kind::DotShorthandType, false);
        }
    }

    fn visit_dot_shorthand_invocation(&mut self, ast: &Ast, node: Id<DotShorthandInvocation>) {
        ast.visit_children(node.raw(), self);
        self.invocation(node.raw(), ast[node].type_arguments.is_some(), ast[node].argument_list);
        if let Some(t) = self.static_type(node.raw()) {
            let offset = self.token_offset(ast[node].period);
            self.type_prefix(offset, t, Kind::DotShorthandType, false);
        }
    }

    fn visit_dot_shorthand_property_access(&mut self, ast: &Ast, node: Id<DotShorthandPropertyAccess>) {
        ast.visit_children(node.raw(), self);
        if let Some(t) = self.static_type(node.raw()) {
            let offset = self.token_offset(ast[node].period);
            self.type_prefix(offset, t, Kind::DotShorthandType, false);
        }
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        ast.visit_children(node.raw(), self);
        if ast[node].return_type.is_some() {
            return;
        }
        let property = ast[node].property_keyword;
        if property.is_some_and(|p| ast.tokens.lexeme(p) == "set") {
            return;
        }
        if let Some(e) = self.unit.declared_element(node.raw()) {
            let token = property.unwrap_or(ast[node].name);
            let offset = self.token_offset(token);
            self.type_prefix(offset, self.return_type(e), Kind::ReturnType, true);
        }
    }

    fn visit_function_expression_invocation(&mut self, ast: &Ast, node: Id<FunctionExpressionInvocation>) {
        ast.visit_children(node.raw(), self);
        self.invocation(node.raw(), ast[node].type_arguments.is_some(), ast[node].argument_list);
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        ast.visit_children(node.raw(), self);
        self.invocation(node.raw(), ast[node].type_arguments.is_some(), ast[node].argument_list);
    }

    fn visit_list_literal(&mut self, ast: &Ast, node: Id<ListLiteral>) {
        ast.visit_children(node.raw(), self);
        if ast[node].type_arguments.is_some() {
            return;
        }
        let offset = self.token_offset(ast[node].left_bracket);
        self.maybe_type_arguments(offset, self.static_type(node.raw()));
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        ast.visit_children(node.raw(), self);
        if ast[node].return_type.is_some() {
            return;
        }
        if let Some(e) = self.unit.declared_element(node.raw()) {
            let offset = self.token_offset(ast[node].name);
            self.type_prefix(offset, self.return_type(e), Kind::ReturnType, true);
        }
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        ast.visit_children(node.raw(), self);
        if ast[node].type_arguments.is_some() {
            return;
        }
        let ty = self.unit.tables.annotation_type.get(node.raw()).copied();
        let end = ast.end(node.raw());
        // Dart `maybeAddTypeArguments(node.endToken, suffix: true)`: at the
        // end of the last token.
        let Some(ty) = ty else { return };
        let ctx = *self.ctx();
        let args = match *ctx.ty(ty) {
            TypeKind::Interface { args, .. } => ctx.list(args).to_vec(),
            _ => return,
        };
        // `endToken` of `int?` is `?`: the hint goes after it.
        self.type_arguments(end, &args);
    }

    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        ast.visit_children(node.raw(), self);
        // Dart `isExplicitlyTyped`.
        if ast[node].type_.is_some() || ast[node].function_typed_suffix.is_some() {
            return;
        }
        if let Some(e) = self.unit.declared_element(node.raw()) {
            let offset = match ast[node].name {
                Some(n) => self.token_offset(n),
                None => ast.offset(node.raw()),
            };
            self.type_prefix(offset, self.element_type(e), Kind::ParameterType, true);
        }
    }

    fn visit_set_or_map_literal(&mut self, ast: &Ast, node: Id<SetOrMapLiteral>) {
        ast.visit_children(node.raw(), self);
        if ast[node].type_arguments.is_some() {
            return;
        }
        let offset = self.token_offset(ast[node].left_bracket);
        self.maybe_type_arguments(offset, self.static_type(node.raw()));
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        ast.visit_children(node.raw(), self);
        let Some(parent) = ast.parent(node.raw()).and_then(|p| ast.cast::<VariableDeclarationList>(p)) else {
            return;
        };
        if ast[parent].type_.is_some() {
            return;
        }
        if let Some(e) = self.unit.declared_element(node.raw()) {
            let offset = self.token_offset(ast[node].name);
            self.type_prefix(offset, self.element_type(e), Kind::VariableType, true);
        }
    }
}

/// The name of [element] (Dart `element.name`).
fn element_name<'a>(ctx: &Ctx<'a>, element: ElementId) -> Option<&'a str> {
    let name = ctx.element_data(element)?.name?;
    Some(ctx.name_str(name))
}
