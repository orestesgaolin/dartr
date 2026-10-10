// Dart source: pkg/analyzer_plugin/lib/src/utilities/completion/completion_target.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/completion_manager.dart (TokenData, DartCompletionRequest.targetPrefix)

//! The completion target (Dart `CompletionTarget`): the node that contains
//! the completion offset, the child entity at the offset, the dropped
//! token, the argument index, and the replacement range.

use dartr_ast::*;
use dartr_element::{ElemRef, ResolutionTables, TypeId, TypeKind};
use dartr_syntax::{TokenId, TokenType};

/// Token helpers on an [`Ast`].
pub(crate) trait TokenExt {
    fn t_offset(&self, t: TokenId) -> u32;
    fn t_end(&self, t: TokenId) -> u32;
    fn t_len(&self, t: TokenId) -> u32;
    fn t_ty(&self, t: TokenId) -> TokenType;
    fn t_lexeme(&self, t: TokenId) -> &str;
    fn t_synthetic(&self, t: TokenId) -> bool;
    fn t_kw_or_ident(&self, t: TokenId) -> bool;
    fn t_is_keyword(&self, t: TokenId) -> bool;
    fn t_is_eof(&self, t: TokenId) -> bool;
    fn t_next(&self, t: TokenId) -> TokenId;
    fn t_prev(&self, t: TokenId) -> Option<TokenId>;
    /// Dart `Token.coversOffset` (`offset <= o && o <= end`).
    fn t_covers(&self, t: Option<TokenId>, offset: u32) -> bool;
    /// Dart `SyntacticEntity.coversOffset` of a node.
    fn n_covers(&self, n: Option<NodeId>, offset: u32) -> bool;
    /// Dart `AstNode.isSynthetic` (`false` except for the nodes that
    /// override it).
    fn n_synthetic(&self, n: NodeId) -> bool;
    fn begin(&self, n: NodeId) -> TokenId;
    fn end_tok(&self, n: NodeId) -> TokenId;
    /// Dart `AstNode.findPrevious(target)`.
    fn find_previous(&self, target: TokenId) -> Option<TokenId>;
    /// Dart `isFullySynthetic`.
    fn fully_synthetic(&self, n: NodeId) -> bool;
}

impl TokenExt for Ast {
    fn t_offset(&self, t: TokenId) -> u32 {
        self.tokens.get(t).offset
    }
    fn t_end(&self, t: TokenId) -> u32 {
        self.tokens.get(t).end()
    }
    fn t_len(&self, t: TokenId) -> u32 {
        self.tokens.get(t).length
    }
    fn t_ty(&self, t: TokenId) -> TokenType {
        self.tokens.ty(t)
    }
    fn t_lexeme(&self, t: TokenId) -> &str {
        self.tokens.lexeme(t)
    }
    fn t_synthetic(&self, t: TokenId) -> bool {
        self.tokens.get(t).is_synthetic()
    }
    fn t_kw_or_ident(&self, t: TokenId) -> bool {
        self.tokens.get(t).is_keyword_or_identifier()
    }
    fn t_is_keyword(&self, t: TokenId) -> bool {
        self.tokens.ty(t).is_keyword()
    }
    fn t_is_eof(&self, t: TokenId) -> bool {
        self.tokens.get(t).is_eof()
    }
    fn t_next(&self, t: TokenId) -> TokenId {
        self.tokens.next(t)
    }
    fn t_prev(&self, t: TokenId) -> Option<TokenId> {
        self.tokens.previous(t).get()
    }
    fn t_covers(&self, t: Option<TokenId>, offset: u32) -> bool {
        t.is_some_and(|t| self.t_offset(t) <= offset && offset <= self.t_end(t))
    }
    fn n_covers(&self, n: Option<NodeId>, offset: u32) -> bool {
        n.is_some_and(|n| self.offset(n) <= offset && offset <= self.end(n))
    }
    fn n_synthetic(&self, n: NodeId) -> bool {
        if let Some(i) = self.cast::<SimpleIdentifier>(n) {
            return self.t_synthetic(self[i].token);
        }
        if let Some(s) = self.cast::<SimpleStringLiteral>(n) {
            return self.t_synthetic(self[s].literal);
        }
        if let Some(b) = self.cast::<BooleanLiteral>(n) {
            return self.t_synthetic(self[b].literal);
        }
        if let Some(t) = self.cast::<NamedType>(n) {
            return self.t_synthetic(self[t].name) && self[t].type_arguments.is_none();
        }
        if let Some(e) = self.cast::<EmptyStatement>(n) {
            return self.t_synthetic(self[e].semicolon);
        }
        if let Some(e) = self.cast::<ExpressionStatement>(n) {
            return self.n_synthetic(self[e].expression.raw())
                && self[e].semicolon.is_none_or(|s| self.t_synthetic(s));
        }
        false
    }
    fn begin(&self, n: NodeId) -> TokenId {
        self.begin_token(n)
    }
    fn end_tok(&self, n: NodeId) -> TokenId {
        self.end_token(n)
    }
    fn find_previous(&self, target: TokenId) -> Option<TokenId> {
        if self.tokens.get(target).is_comment() {
            return None;
        }
        let p = self.tokens.previous(target).get()?;
        if self.tokens.get(p).signed_offset() < 0 && self.t_is_eof(p) {
            return None;
        }
        Some(p)
    }
    fn fully_synthetic(&self, n: NodeId) -> bool {
        let mut current = self.begin_token(n);
        let stop = self.t_next(self.end_token(n));
        while current != stop {
            if !self.t_synthetic(current) {
                return false;
            }
            if self.t_is_eof(current) {
                break;
            }
            current = self.t_next(current);
        }
        true
    }
}

/// Dart `CompletionTarget`.
#[derive(Clone, Debug)]
pub struct CompletionTarget {
    pub offset: u32,
    pub containing_node: NodeId,
    pub dropped_token: Option<TokenId>,
    pub entity: Option<Entity>,
    pub is_comment_text: bool,
    pub arg_index: Option<usize>,
}

fn is_identifier_type(ty: TokenType) -> bool {
    ty == TokenType::IDENTIFIER
}

/// Dart `_isCandidateToken`.
fn is_candidate_token(ast: &Ast, token: Option<TokenId>, offset: u32) -> bool {
    let Some(token) = token else {
        return false;
    };
    let ty = ast.t_ty(token);
    if offset < ast.t_end(token) {
        return true;
    } else if offset == ast.t_end(token) {
        return ty.is_keyword() || is_identifier_type(ty) || ast.t_len(token) == 0;
    } else if !ast.t_synthetic(token) {
        return false;
    }
    let Some(previous) = ast.find_previous(token) else {
        return false;
    };
    if offset < ast.t_end(previous) {
        true
    } else if offset == ast.t_end(previous) {
        ty.is_keyword() || is_identifier_type(ast.t_ty(previous))
    } else {
        false
    }
}

/// Dart `_isCandidateNode`.
fn is_candidate_node(ast: &Ast, node: NodeId, offset: u32) -> bool {
    let begin = ast.begin(node);
    let ty = ast.t_ty(begin);
    if ty.is_keyword() || ty == TokenType::IDENTIFIER {
        return is_candidate_token(ast, Some(begin), offset);
    }
    offset <= ast.offset(node)
}

/// Dart `_getContainingCommentToken`.
fn containing_comment_token(ast: &Ast, token: Option<TokenId>, offset: u32) -> Option<TokenId> {
    let token = token?;
    if !ast.t_is_eof(token) && offset >= ast.t_offset(token) {
        return None;
    }
    for c in ast.tokens.comments(token) {
        if offset <= ast.t_offset(c) {
            return None;
        }
        if offset <= ast.t_end(c)
            && (ast.t_ty(c) == TokenType::SINGLE_LINE_COMMENT || offset < ast.t_end(c))
        {
            return Some(c);
        }
    }
    None
}

/// Dart `_getContainingDocComment`.
fn containing_doc_comment(ast: &Ast, node: NodeId, token: TokenId) -> Option<NodeId> {
    let comment = ast
        .children(node)
        .into_iter()
        .find(|&c| ast.is::<Comment>(c))?;
    let c = ast.cast::<Comment>(comment)?;
    ast.token_list(ast[c].tokens)
        .contains(&token)
        .then_some(comment)
}

impl CompletionTarget {
    /// Dart `CompletionTarget.forOffset`.
    pub fn for_offset(ast: &Ast, entry_point: NodeId, offset: u32) -> CompletionTarget {
        let mut containing = entry_point;
        'outer: loop {
            if let Some(comment) = ast.cast::<Comment>(containing) {
                for &r in ast.list(ast[comment].references) {
                    if ast.offset(r) <= offset && offset <= ast.end(r) {
                        containing = r.raw();
                        continue 'outer;
                    }
                }
            }
            for entity in ast.child_entities(containing) {
                match entity {
                    Entity::Token(token) => {
                        if is_candidate_token(ast, Some(token), offset) {
                            if let Some(c) = containing_comment_token(ast, Some(token), offset) {
                                return match containing_doc_comment(ast, containing, c) {
                                    Some(doc) => {
                                        Self::new(ast, offset, doc, Some(Entity::Token(c)), false)
                                    }
                                    None => Self::new(
                                        ast,
                                        offset,
                                        entry_point,
                                        Some(Entity::Token(c)),
                                        true,
                                    ),
                                };
                            }
                            return Self::new(ast, offset, containing, Some(entity), false);
                        }
                    }
                    Entity::Node(node) => {
                        if !is_candidate_token(ast, Some(ast.end_tok(node)), offset) {
                            continue;
                        }
                        if is_candidate_node(ast, node, offset) {
                            if let Some(c) =
                                containing_comment_token(ast, Some(ast.begin(node)), offset)
                            {
                                return match containing_doc_comment(ast, containing, c) {
                                    Some(doc) => {
                                        Self::new(ast, offset, doc, Some(Entity::Token(c)), false)
                                    }
                                    None => Self::new(
                                        ast,
                                        offset,
                                        entry_point,
                                        Some(Entity::Token(c)),
                                        true,
                                    ),
                                };
                            }
                            return Self::new(ast, offset, containing, Some(entity), false);
                        }
                        containing = node;
                        continue 'outer;
                    }
                }
            }
            if let Some(unit) = ast.cast::<CompilationUnit>(entry_point) {
                if let Some(c) = containing_comment_token(ast, Some(ast[unit].end_token), offset) {
                    return Self::new(ast, offset, entry_point, Some(Entity::Token(c)), true);
                }
            }
            return Self::new(ast, offset, entry_point, None, false);
        }
    }

    fn new(
        ast: &Ast,
        offset: u32,
        containing_node: NodeId,
        entity: Option<Entity>,
        is_comment_text: bool,
    ) -> CompletionTarget {
        CompletionTarget {
            offset,
            containing_node,
            arg_index: compute_arg_index(ast, containing_node, entity),
            dropped_token: compute_dropped_token(ast, containing_node, entity, offset),
            entity,
            is_comment_text,
        }
    }

    pub fn entity_node(&self) -> Option<NodeId> {
        match self.entity {
            Some(Entity::Node(n)) => Some(n),
            _ => None,
        }
    }

    pub fn entity_token(&self) -> Option<TokenId> {
        match self.entity {
            Some(Entity::Token(t)) => Some(t),
            _ => None,
        }
    }

    /// Dart `lastTokenOfEntity`.
    fn last_token_of_entity(&self, ast: &Ast) -> Option<TokenId> {
        match self.entity? {
            Entity::Node(n) => Some(ast.end_tok(n)),
            Entity::Token(t) => Some(t),
        }
    }

    /// Dart `isFollowedByComma`.
    pub fn is_followed_by_comma(&self, ast: &Ast) -> bool {
        let existing = |t: TokenId| !ast.t_synthetic(t) && ast.t_ty(t) == TokenType::COMMA;
        let Some(token) = self.last_token_of_entity(ast) else {
            return false;
        };
        if ast.t_offset(token) <= self.offset && self.offset <= ast.t_end(token) {
            return existing(ast.t_next(token));
        }
        existing(token)
    }

    /// Dart `dotTarget` (the node of the target, or for a prefixed named
    /// type the import prefix name token).
    pub fn dot_target(&self, ast: &Ast) -> Option<NodeId> {
        let node = self.containing_node;
        let entity = self.entity_node();
        if let Some(m) = ast.cast::<MethodInvocation>(node) {
            if entity == Some(ast[m].method_name.raw()) {
                return real_target_of_method_invocation(ast, m);
            } else if dartr_resolver::ast_ext::method_invocation_is_cascaded(ast, m)
                && ast[m]
                    .operator
                    .is_some_and(|op| ast.t_offset(op) + 1 == self.offset)
            {
                return real_target_of_method_invocation(ast, m);
            }
        }
        if let Some(p) = ast.cast::<PropertyAccess>(node) {
            if entity == Some(ast[p].property_name.raw())
                || (dartr_resolver::ast_ext::property_access_is_cascaded(ast, p)
                    && ast.t_offset(ast[p].operator) + 1 == self.offset)
            {
                return real_target_of_property_access(ast, p);
            }
        }
        if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
            if entity == Some(ast[p].identifier.raw()) {
                return Some(ast[p].prefix.raw());
            }
        }
        None
    }

    /// Dart `executableElement`.
    pub fn executable_element(
        &self,
        ast: &Ast,
        ctx: &dartr_element::Ctx<'_>,
        tables: &ResolutionTables,
    ) -> Option<ElemRef> {
        let mut argument_list = self.containing_node;
        if ast.is::<NamedArgument>(argument_list) {
            argument_list = ast.parent(argument_list)?;
        }
        if !ast.is::<ArgumentList>(argument_list) {
            return None;
        }
        let invocation = ast.parent(argument_list)?;
        let element = if ast.is::<Annotation>(invocation) {
            tables.element.get(invocation).copied()
        } else if ast.is::<EnumConstantArguments>(invocation) {
            let constant = ast.parent(invocation)?;
            if !ast.is::<EnumConstantDeclaration>(constant) {
                return None;
            }
            tables.element.get(constant).copied()
        } else if let Some(i) = ast.cast::<InstanceCreationExpression>(invocation) {
            tables.element.get(ast[i].constructor_name.raw()).copied()
        } else if let Some(m) = ast.cast::<MethodInvocation>(invocation) {
            tables.element.get(ast[m].method_name.raw()).copied()
        } else if ast.is::<RedirectingConstructorInvocation>(invocation)
            || ast.is::<SuperConstructorInvocation>(invocation)
        {
            tables.element.get(invocation).copied()
        } else {
            None
        }?;
        crate::element_locator::is_executable(dartr_typesystem::member::base_element(ctx, element))
            .then_some(element)
    }

    /// Dart `functionType`.
    pub fn function_type(
        &self,
        ast: &Ast,
        ctx: &dartr_element::Ctx<'_>,
        tables: &ResolutionTables,
    ) -> Option<TypeId> {
        let mut argument_list = self.containing_node;
        if ast.is::<NamedArgument>(argument_list) {
            argument_list = ast.parent(argument_list)?;
        }
        if !ast.is::<ArgumentList>(argument_list) {
            return None;
        }
        let invocation = ast.parent(argument_list)?;
        if ast.is::<FunctionExpressionInvocation>(invocation) {
            let t = *tables.invoke_type.get(invocation)?;
            if matches!(ctx.ty(t), TypeKind::Function(_)) {
                return Some(t);
            }
        }
        None
    }

    /// Dart `parameterElement`: the type of the parameter.
    pub fn parameter_type(
        &self,
        ast: &Ast,
        ctx: &dartr_element::Ctx<'_>,
        tables: &ResolutionTables,
    ) -> Option<TypeId> {
        let executable = self.executable_element(ast, ctx, tables)?;
        let parameters = dartr_typesystem::member::formal_parameters(ctx, executable);
        let node = self.containing_node;
        if let Some(named) = ast.cast::<NamedArgument>(node) {
            let name = ast.t_lexeme(ast[named].name);
            for p in parameters {
                if dartr_typesystem::member::name(ctx, p) == Some(name) {
                    return Some(dartr_typesystem::member::type_(ctx, p));
                }
            }
            return None;
        }
        let index = self.arg_index?;
        parameters
            .get(index)
            .map(|p| dartr_typesystem::member::type_(ctx, *p))
    }

    /// Dart `isFunctionalArgument()`.
    pub fn is_functional_argument(
        &self,
        ast: &Ast,
        ctx: &dartr_element::Ctx<'_>,
        tables: &ResolutionTables,
    ) -> bool {
        self.parameter_type(ast, ctx, tables)
            .is_some_and(|t| matches!(ctx.ty(t), TypeKind::Function(_)))
    }

    /// Dart `computeReplacementRange`: (offset, length).
    pub fn compute_replacement_range(
        &self,
        ast: &Ast,
        request_offset: u32,
        is_dot_shorthand_enabled: bool,
    ) -> (u32, u32) {
        let mut token = self.dropped_token.or(match self.entity {
            Some(Entity::Node(n)) => Some(ast.begin(n)),
            Some(Entity::Token(t)) => Some(t),
            None => None,
        });
        if let Some(t) = token {
            if request_offset < ast.t_offset(t) {
                token = ast.find_previous(t);
            }
        }
        if let Some(t) = token {
            let mut token = Some(t);
            if request_offset == ast.t_offset(t) && !ast.t_kw_or_ident(t) {
                token = ast.find_previous(t);
            }
            let node = self.containing_node;
            if (ast.is::<DotShorthandConstructorInvocation>(node)
                || ast.is::<DotShorthandInvocation>(node)
                || ast.is::<DotShorthandPropertyAccess>(node))
                && !is_dot_shorthand_enabled
            {
                if let Some(t) = token {
                    let offset = if ast.t_kw_or_ident(t) {
                        ast.t_prev(t).map(|p| ast.t_offset(p)).unwrap_or(0)
                    } else {
                        ast.t_offset(t)
                    };
                    return (offset, ast.t_end(t) - offset);
                }
            }
            if let Some(t) = token {
                if ast.t_kw_or_ident(t)
                    && ast.t_offset(t) <= request_offset
                    && request_offset <= ast.t_end(t)
                {
                    return (ast.t_offset(t), ast.t_len(t));
                }
            }
            if let Some(t) = token {
                if ast.t_ty(t) == TokenType::STRING {
                    let node = self.containing_node;
                    let mut uri: Option<NodeId> = None;
                    let mut directive: Option<NodeId> = None;
                    if let Some(d) = ast.cast::<ImportDirective>(node) {
                        directive = Some(node);
                        uri = Some(ast[d].uri.raw());
                        for &c in ast.list(ast[d].configurations) {
                            let u = ast[c].uri.raw();
                            if ast.offset(u) <= request_offset && ast.end(u) >= request_offset {
                                uri = Some(u);
                                break;
                            }
                        }
                    } else if let Some(d) = ast.cast::<ExportDirective>(node) {
                        directive = Some(node);
                        uri = Some(ast[d].uri.raw());
                        for &c in ast.list(ast[d].configurations) {
                            let u = ast[c].uri.raw();
                            if ast.offset(u) <= request_offset && ast.end(u) >= request_offset {
                                uri = Some(u);
                                break;
                            }
                        }
                    } else if ast.is::<SimpleStringLiteral>(node) {
                        uri = Some(node);
                        let parent = ast.parent(node);
                        directive = parent.filter(|p| ast.is::<Directive>(*p)).or_else(|| {
                            parent
                                .and_then(|p| ast.parent(p))
                                .filter(|p| ast.is::<Directive>(*p))
                        });
                    } else if let Some(c) = ast.cast::<Comment>(node) {
                        for &r in ast.list(ast[c].references) {
                            if ast.offset(r) <= request_offset && ast.end(r) >= request_offset {
                                return (ast.offset(r), ast.length(r));
                            }
                        }
                    }
                    if directive.is_some() {
                        if let Some(s) = uri.and_then(|u| ast.cast::<SimpleStringLiteral>(u)) {
                            let (start, end) = string_contents_range(ast, s);
                            if start <= request_offset && request_offset <= end {
                                return (start, end - start);
                            }
                        }
                    }
                }
            }
        }
        (request_offset, 0)
    }

    /// Dart `isDoubleOrIntLiteral()`.
    pub fn is_double_or_int_literal(&self, ast: &Ast) -> bool {
        if let Some(Entity::Token(t)) = self.entity {
            if let Some(p) = ast.find_previous(t) {
                let ty = ast.t_ty(p);
                return ty == TokenType::DOUBLE
                    || ty == TokenType::DOUBLE_WITH_SEPARATORS
                    || ty == TokenType::INT
                    || ty == TokenType::INT_WITH_SEPARATORS;
            }
        }
        false
    }
}

/// Dart `MethodInvocation.realTarget`.
pub(crate) fn real_target_of_method_invocation(
    ast: &Ast,
    m: Id<MethodInvocation>,
) -> Option<NodeId> {
    dartr_resolver::ast_ext::method_invocation_real_target(ast, m).map(|t| t.raw())
}

/// Dart `PropertyAccess.realTarget`.
pub(crate) fn real_target_of_property_access(ast: &Ast, p: Id<PropertyAccess>) -> Option<NodeId> {
    if dartr_resolver::ast_ext::property_access_is_cascaded(ast, p) {
        return dartr_resolver::ast_ext::ancestor_cascade_target(ast, p.raw()).map(|t| t.raw());
    }
    ast[p].target.map(|t| t.raw())
}

/// Dart `SimpleStringLiteral.contentsOffset` / `contentsEnd`.
pub(crate) fn string_contents_range(ast: &Ast, s: Id<SimpleStringLiteral>) -> (u32, u32) {
    let token = ast[s].literal;
    let lexeme = ast.t_lexeme(token);
    let offset = ast.t_offset(token);
    let raw = lexeme.starts_with('r');
    let body = if raw { &lexeme[1..] } else { lexeme };
    let multiline = body.starts_with("'''") || body.starts_with("\"\"\"");
    let quote_len = if multiline { 3 } else { 1 };
    let start = offset + raw as u32 + quote_len;
    let quote_char = body.chars().next();
    let closed = body.len() >= quote_len as usize * 2
        && quote_char.is_some_and(|q| {
            let close: String = std::iter::repeat_n(q, quote_len as usize).collect();
            body.ends_with(&close)
        });
    let end_offset = ast.t_end(token);
    let end = if closed {
        end_offset - quote_len
    } else {
        end_offset
    };
    (start, end.max(start))
}

/// Dart `_computeArgIndex`.
fn compute_arg_index(ast: &Ast, containing: NodeId, entity: Option<Entity>) -> Option<usize> {
    let mut entity = entity;
    let mut arg_list = containing;
    if ast.is::<NamedArgument>(arg_list) {
        entity = Some(Entity::Node(arg_list));
        arg_list = ast.parent(arg_list)?;
    }
    let list = ast.cast::<ArgumentList>(arg_list)?;
    let args = ast.list_raw(ast[list].arguments);
    for (index, &a) in args.iter().enumerate() {
        if entity == Some(Entity::Node(a)) {
            return Some(index);
        }
    }
    if args.is_empty() {
        return Some(0);
    }
    let right = ast[list].right_parenthesis;
    if entity == Some(Entity::Token(right)) {
        let previous = ast.find_previous(right);
        if previous.is_some_and(|p| ast.t_lexeme(p) == ",") {
            return Some(args.len());
        }
        return Some(args.len() - 1);
    }
    None
}

/// Dart `_computeDroppedToken`.
fn compute_dropped_token(
    ast: &Ast,
    containing: NodeId,
    entity: Option<Entity>,
    offset: u32,
) -> Option<TokenId> {
    let mut previous_member: Option<Entity> = None;
    for member in ast.child_entities(containing) {
        if entity == Some(member) {
            break;
        }
        let is_comment = match member {
            Entity::Node(n) => ast.is::<Comment>(n),
            Entity::Token(t) => ast.tokens.get(t).is_comment(),
        };
        if !is_comment {
            previous_member = Some(member);
        }
    }
    let mut token = match previous_member? {
        Entity::Node(n) => ast.end_tok(n),
        Entity::Token(t) => t,
    };
    let end_search = match entity? {
        Entity::Node(n) => ast.begin(n),
        Entity::Token(t) => t,
    };
    token = ast.t_next(token);
    while token != end_search && !ast.t_is_eof(token) {
        if ast.t_kw_or_ident(token) && ast.t_offset(token) <= offset && offset <= ast.t_end(token) {
            return Some(token);
        }
        token = ast.t_next(token);
    }
    None
}

/// Dart `TokenData.fromSelection`: the token and the prefix before the
/// offset.
pub fn token_data_prefix(ast: &Ast, covering: NodeId, offset: u32) -> Option<(TokenId, String)> {
    let mut current = ast.end_tok(covering);
    while (ast.t_synthetic(current)
        || ast.t_offset(current) > offset
        || (ast.t_offset(current) == offset && !ast.t_kw_or_ident(current)))
        && !ast.t_is_eof(current)
    {
        match ast.t_prev(current) {
            Some(p) => current = p,
            None => return None,
        }
    }
    if ast.t_is_eof(current) {
        return None;
    }
    if offset > ast.t_end(current) {
        let next = ast.t_next(current);
        for c in ast.tokens.comments(next) {
            if offset >= ast.t_offset(c) && offset <= ast.t_end(c) {
                return Some((c, String::new()));
            }
        }
        return None;
    }
    let lexeme = ast.t_lexeme(current);
    if ast.t_kw_or_ident(current) {
        let n = (offset - ast.t_offset(current)) as usize;
        return Some((current, utf16_prefix(lexeme, n)));
    } else if ast.t_ty(current) == TokenType::STRING {
        let start = if lexeme.starts_with("r'''") || lexeme.starts_with("r\"\"\"") {
            4
        } else if lexeme.starts_with("r'") || lexeme.starts_with("r\"") {
            2
        } else if lexeme.starts_with("'''") || lexeme.starts_with("\"\"\"") {
            3
        } else {
            1
        };
        let in_token = (offset - ast.t_offset(current)) as usize;
        if in_token < start {
            return Some((current, String::new()));
        }
        let all = utf16_prefix(lexeme, in_token);
        let skip = utf16_prefix(lexeme, start);
        return Some((current, all[skip.len()..].to_string()));
    }
    Some((current, String::new()))
}

/// The prefix of [s] of [n] UTF-16 code units.
pub(crate) fn utf16_prefix(s: &str, n: usize) -> String {
    let mut count = 0;
    let mut out = String::new();
    for c in s.chars() {
        if count >= n {
            break;
        }
        count += c.len_utf16();
        out.push(c);
    }
    out
}

/// Dart `DartCompletionRequest.targetPrefix`.
pub fn target_prefix(ast: &Ast, target: &CompletionTarget, offset: u32) -> String {
    let from_token = |t: TokenId| -> String {
        let lexeme = ast.t_lexeme(t);
        if offset >= ast.t_offset(t) && offset < ast.t_end(t) {
            utf16_prefix(lexeme, (offset - ast.t_offset(t)) as usize)
        } else if offset == ast.t_end(t) {
            lexeme.to_string()
        } else {
            String::new()
        }
    };
    if let Some(Entity::Token(t)) = target.entity {
        if let Some(prev) = ast.t_prev(t) {
            if ast.t_end(prev) == offset && ast.t_kw_or_ident(prev) {
                return ast.t_lexeme(prev).to_string();
            }
        }
        if ast.t_ty(t) == TokenType::STRING && ast.t_offset(t) < offset && offset < ast.t_end(t) {
            let uri_node = target.containing_node;
            if let Some(s) = ast.cast::<SimpleStringLiteral>(uri_node) {
                if ast[s].literal == t {
                    if let Some(d) = ast.parent(uri_node) {
                        if ast.is::<UriBasedDirective>(d) && directive_uri(ast, d) == Some(uri_node)
                        {
                            let (contents, _) = string_contents_range(ast, s);
                            if offset >= contents {
                                let value: &str = &ast[s].value;
                                return utf16_prefix(value, (offset - contents) as usize);
                            }
                        }
                    }
                }
            }
        }
        if ast.t_end(t) == offset && ast.t_kw_or_ident(t) {
            return from_token(t);
        }
    }
    if let Some(Entity::Node(n)) = target.entity {
        if let Some(p) = ast.cast::<DeclaredVariablePattern>(n) {
            if ast.t_offset(ast[p].name) <= offset {
                return from_token(ast[p].name);
            }
        }
    }
    let mut entity = target.entity;
    while let Some(Entity::Node(n)) = entity {
        if let Some(s) = ast.cast::<SimpleIdentifier>(n) {
            return from_token(ast[s].token);
        }
        let children = ast.child_entities(n);
        entity = children.first().copied();
        if let Some(Entity::Token(t)) = entity {
            return from_token(t);
        }
    }
    String::new()
}

/// The URI of a URI based directive.
pub(crate) fn directive_uri(ast: &Ast, d: NodeId) -> Option<NodeId> {
    if let Some(i) = ast.cast::<ImportDirective>(d) {
        return Some(ast[i].uri.raw());
    }
    if let Some(e) = ast.cast::<ExportDirective>(d) {
        return Some(ast[e].uri.raw());
    }
    if let Some(p) = ast.cast::<PartDirective>(d) {
        return Some(ast[p].uri.raw());
    }
    None
}
