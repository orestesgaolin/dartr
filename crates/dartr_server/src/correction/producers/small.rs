// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_annotation.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_name_from_declaration_clause.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/replace_with_null_aware.dart

//! Small producers with a computed message.

use dartr_ast::precedence::Precedence;
use dartr_ast::*;
use dartr_syntax::TokenId;

use super::super::change_builder::ChangeBuilder;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::variables::parameter_metadata;

/// Dart `RemoveAnnotation`.
pub struct RemoveAnnotation {
    name: String,
}

impl RemoveAnnotation {
    pub fn new() -> Self {
        RemoveAnnotation {
            name: String::new(),
        }
    }

    fn add_fix(
        &mut self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        node: Option<Id<Annotation>>,
    ) {
        let Some(node) = node else { return };
        let ast = c.ast;
        let following = ast.tokens.next(ast.end_token(node));
        let following = c.range().comment_or_token(following);
        let range = c.range().node_start_token_start(node, following);
        builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
        self.name = c.utils.get_node_text(ast[node].name);
    }
}

fn find_annotation(
    c: &ProducerContext<'_>,
    metadata: &[Id<Annotation>],
    name: &str,
) -> Option<Id<Annotation>> {
    metadata
        .iter()
        .copied()
        .find(|a| c.utils.get_node_text(c.ast[*a].name) == name)
}

impl CorrectionProducer for RemoveAnnotation {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_ANNOTATION)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let Some(node) = c.covering_node() else {
            return;
        };
        if ast.is::<Identifier>(node) {
            if let Some(a) = ast.parent(node).and_then(|p| ast.cast::<Annotation>(p)) {
                self.add_fix(c, builder, Some(a));
            }
        } else if ast.is::<FormalParameter>(node) {
            let metadata = parameter_metadata(ast, node);
            let a = find_annotation(c, &metadata, "required");
            self.add_fix(c, builder, a);
        } else if let Some(m) = ast.cast::<MethodDeclaration>(node) {
            let metadata = ast.list(ast[m].metadata).to_vec();
            let a = find_annotation(c, &metadata, "override");
            self.add_fix(c, builder, a);
            let a = find_annotation(c, &metadata, "redeclare");
            self.add_fix(c, builder, a);
        } else if ast.is::<VariableDeclaration>(node) {
            if let Some(f) = ast.this_or_ancestor_of_type::<FieldDeclaration>(node) {
                let metadata = ast.list(ast[f].metadata).to_vec();
                let a = find_annotation(c, &metadata, "override");
                self.add_fix(c, builder, a);
            }
        }
    }
}

/// Dart `RemoveNameFromDeclarationClause`.
pub struct RemoveNameFromDeclarationClause {
    message: String,
}

impl RemoveNameFromDeclarationClause {
    pub fn new() -> Self {
        RemoveNameFromDeclarationClause {
            message: String::new(),
        }
    }
}

impl CorrectionProducer for RemoveNameFromDeclarationClause {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REMOVE_NAME_FROM_DECLARATION_CLAUSE)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.message.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let ty = c.node;
        let Some(clause) = ast.parent(ty) else { return };
        let (name_list, clause_name): (Vec<NodeId>, &str) = if ast.is::<ExtendsClause>(clause) {
            (Vec::new(), "extends")
        } else if let Some(x) = ast.cast::<ImplementsClause>(clause) {
            (ast.list_raw(ast[x].interfaces).to_vec(), "implements")
        } else if let Some(x) = ast.cast::<MixinOnClause>(clause) {
            (ast.list_raw(ast[x].superclass_constraints).to_vec(), "on")
        } else if let Some(x) = ast.cast::<WithClause>(clause) {
            (ast.list_raw(ast[x].mixin_types).to_vec(), "with")
        } else {
            return;
        };
        if name_list.len() <= 1 {
            self.message = format!("Remove '{clause_name}' clause");
            let range = c.range().deletion_range(clause);
            builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
        } else {
            self.message = format!("Remove name from '{clause_name}' clause");
            let range = c.range().node_in_list(&name_list, ty);
            builder.add_dart_file_edit(c.path, |b| b.add_deletion(range.offset, range.length));
        }
    }
}

/// Dart `ReplaceWithNullAware` (`.inChain` and `.single`).
pub struct ReplaceWithNullAware {
    in_chain: bool,
    operator: String,
    operator_prefix: String,
}

impl ReplaceWithNullAware {
    pub fn new(in_chain: bool) -> Self {
        ReplaceWithNullAware {
            in_chain,
            operator: ".".into(),
            operator_prefix: "?".into(),
        }
    }

    fn insert(
        &mut self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        token: Option<TokenId>,
    ) {
        let Some(token) = token else { return };
        self.operator = c.lexeme(token).to_string();
        let offset = c.token_offset(token);
        let prefix = self.operator_prefix.clone();
        builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, &prefix));
    }
}

impl CorrectionProducer for ReplaceWithNullAware {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::REPLACE_WITH_NULL_AWARE)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![
            self.operator.clone(),
            format!("{}{}", self.operator_prefix, self.operator),
        ]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let covering = c.covering_node();
        if self.in_chain {
            let mut offsets = Vec::new();
            if let Some(mut node) = covering.filter(|n| ast.is::<Expression>(*n)) {
                let mut parent = ast.parent(node);
                while let Some(p) = parent {
                    if let Some(m) = ast.cast::<MethodInvocation>(p) {
                        if ast[m].target.map(|t| t.raw()) != Some(node) {
                            break;
                        }
                        if let Some(op) = ast[m].operator {
                            offsets.push(c.token_offset(op));
                        }
                    } else if let Some(pa) = ast.cast::<PropertyAccess>(p) {
                        if ast[pa].target.map(|t| t.raw()) != Some(node) {
                            break;
                        }
                        offsets.push(c.token_offset(ast[pa].operator));
                    } else {
                        break;
                    }
                    node = p;
                    parent = ast.parent(node);
                }
            }
            builder.add_dart_file_edit(c.path, |b| {
                for o in offsets {
                    b.add_simple_insertion(o, "?");
                }
            });
            return;
        }
        let mut node = covering.and_then(|n| ast.parent(n));
        if let Some(cascade) = node.and_then(|n| ast.cast::<CascadeExpression>(n)) {
            node = ast.list_raw(ast[cascade].cascade_sections).first().copied();
        } else {
            if let Some(i) = covering.and_then(|n| ast.cast::<IndexExpression>(n)) {
                self.insert(c, builder, Some(ast[i].left_bracket));
                return;
            }
            if let Some(cascade) = node
                .and_then(|n| ast.parent(n))
                .and_then(|p| ast.cast::<CascadeExpression>(p))
            {
                node = ast.list_raw(ast[cascade].cascade_sections).first().copied();
            }
        }
        let Some(node) = node else { return };
        if let Some(m) = ast.cast::<MethodInvocation>(node) {
            self.insert(c, builder, ast[m].operator);
        } else if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
            self.insert(c, builder, Some(ast[p].period));
        } else if let Some(p) = ast.cast::<PropertyAccess>(node) {
            self.insert(c, builder, Some(ast[p].operator));
        } else if let Some(i) = ast.cast::<IndexExpression>(node) {
            self.insert(c, builder, ast[i].period);
        } else if let Some(f) = ast.cast::<FunctionExpressionInvocation>(node) {
            self.operator = "(".into();
            self.operator_prefix = "?.call".into();
            let offset = ast.offset(ast[f].argument_list);
            builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(offset, "?.call"));
        }
    }
}

// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_null_check.dart

/// Dart `_hasNullAware`.
fn has_null_aware(c: &ProducerContext<'_>, node: Option<NodeId>) -> Option<TokenId> {
    let ast = c.ast;
    let node = node?;
    let null_aware = |t: TokenId| matches!(c.lexeme(t), "?." | "?..");
    if let Some(p) = ast.cast::<PropertyAccess>(node) {
        if null_aware(ast[p].operator) {
            return Some(ast[p].operator);
        }
        return has_null_aware(c, ast[p].target.map(|t| t.raw()));
    }
    if let Some(m) = ast.cast::<MethodInvocation>(node) {
        if let Some(op) = ast[m].operator {
            if null_aware(op) {
                return Some(op);
            }
            return has_null_aware(c, ast[m].target.map(|t| t.raw()));
        }
        return None;
    }
    if let Some(i) = ast.cast::<IndexExpression>(node) {
        if let Some(q) = ast[i].question {
            return Some(q);
        }
        return has_null_aware(c, ast[i].target.map(|t| t.raw()));
    }
    if ast.is::<SimpleIdentifier>(node) {
        if let Some(cascade) = ast
            .parent(node)
            .and_then(|p| ast.cast::<CascadeExpression>(p))
        {
            return has_null_aware(
                c,
                ast.list_raw(ast[cascade].cascade_sections).first().copied(),
            );
        }
    }
    None
}

/// Dart `IndexExpression.realTarget` / `MethodInvocation.realTarget` /
/// `PropertyAccess.realTarget` (the cascade target in a cascade section).
fn real_target_of(
    ast: &Ast,
    target: Option<NodeId>,
    in_cascade: bool,
    node: NodeId,
) -> Option<NodeId> {
    if target.is_some() || !in_cascade {
        return target;
    }
    let mut n = ast.parent(node);
    while let Some(p) = n {
        if let Some(cascade) = ast.cast::<CascadeExpression>(p) {
            return Some(ast[cascade].target.raw());
        }
        n = ast.parent(p);
    }
    None
}

/// Dart `AddNullCheck` (`.new` and `.withoutAssignabilityCheck`).
pub struct AddNullCheck {
    skip_assignability_check: bool,
    target: Option<NodeId>,
    null_aware_token: Option<TokenId>,
    arguments: Vec<String>,
}

impl AddNullCheck {
    pub fn new(c: &ProducerContext<'_>, skip_assignability_check: bool) -> Self {
        let (target, null_aware_token) = Self::target_and_null_aware_token(c, Some(c.node));
        AddNullCheck {
            skip_assignability_check,
            target,
            null_aware_token,
            arguments: Vec::new(),
        }
    }

    /// Dart `_computeTargetAndNullAwareToken`.
    fn target_and_null_aware_token(
        c: &ProducerContext<'_>,
        covering: Option<NodeId>,
    ) -> (Option<NodeId>, Option<TokenId>) {
        let ast = c.ast;
        let Some(covering) = covering else {
            return (None, None);
        };
        let token = has_null_aware(c, Some(covering));
        if ast.is::<Expression>(covering) && token.is_some() {
            return (Some(covering), token);
        }
        let parent = ast.parent(covering);
        let mut target = None;
        if ast.is::<SimpleIdentifier>(covering) {
            if let Some(m) = parent.and_then(|p| ast.cast::<MethodInvocation>(p)) {
                target = real_target_of(
                    ast,
                    ast[m].target.map(|t| t.raw()),
                    ast[m].operator.is_some(),
                    m.raw(),
                );
            } else if let Some(p) = parent.and_then(|p| ast.cast::<PrefixedIdentifier>(p)) {
                target = Some(ast[p].prefix.raw());
            } else if let Some(p) = parent.and_then(|p| ast.cast::<PropertyAccess>(p)) {
                target = real_target_of(ast, ast[p].target.map(|t| t.raw()), true, p.raw());
            } else {
                target = Some(covering);
            }
        } else if let Some(i) = ast.cast::<IndexExpression>(covering) {
            target = real_target_of(ast, ast[i].target.map(|t| t.raw()), true, covering);
            let nullable = target
                .and_then(|t| c.tables.static_type.get(t).copied())
                .is_some_and(|t| {
                    dartr_typesystem::type_system::TypeSystem::new(*c.ctx).is_nullable(t)
                        && !matches!(c.ctx.ty(t), dartr_element::TypeKind::Dynamic)
                });
            if !nullable {
                target = Some(covering);
            }
        } else if ast.is::<Expression>(covering)
            && parent.is_some_and(|p| ast.is::<FunctionExpressionInvocation>(p))
        {
            target = Some(covering);
        } else if let Some(a) = parent.and_then(|p| ast.cast::<AssignmentExpression>(p)) {
            target = Some(ast[a].right_hand_side.raw());
        } else if let Some(p) = ast.cast::<PostfixExpression>(covering) {
            target = Some(ast[p].operand.raw());
        } else if let Some(p) = ast.cast::<PrefixExpression>(covering) {
            target = Some(ast[p].operand.raw());
        } else if let Some(b) = ast.cast::<BinaryExpression>(covering) {
            if c.lexeme(ast[b].operator) != "??" {
                target = Some(ast[b].left_operand.raw());
            } else if c.tables.param_element.get(covering).is_some() {
                target = Some(ast[b].right_operand.raw());
            }
        } else if let Some(a) = ast.cast::<AsExpression>(covering) {
            target = Some(ast[a].expression.raw());
        }
        let Some(target) = target else {
            return (None, None);
        };
        (Some(target), has_null_aware(c, Some(target)))
    }
}

impl CorrectionProducer for AddNullCheck {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(if self.null_aware_token.is_none() {
            &k::ADD_NULL_CHECK
        } else {
            &k::REPLACE_WITH_NULL_AWARE
        })
    }

    fn fix_arguments(&self) -> Vec<String> {
        self.arguments.clone()
    }

    fn applicability(&self) -> Applicability {
        if self.skip_assignability_check {
            Applicability::AutomaticallyButOncePerFile
        } else {
            Applicability::SingleLocation
        }
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let ctx = c.ctx;
        if let Some(token) = self.null_aware_token {
            let lexeme = c.lexeme(token).to_string();
            let replacement = format!("!{}", &lexeme[1..]);
            self.arguments = vec![lexeme, replacement.clone()];
            let r = c.range().token(token);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(r.offset, r.length, &replacement)
            });
            return;
        }
        let Some(target) = self.target else { return };
        let Some(from_type) = c.tables.static_type.get(target).copied() else {
            return;
        };
        if matches!(ctx.ty(from_type), dartr_element::TypeKind::Invalid) {
            return;
        }
        if from_type == ctx.tp.null_type() {
            return;
        }
        let ts = dartr_typesystem::type_system::TypeSystem::new(*ctx);
        let strict = c.options.strict_casts;
        let param_type = |n: NodeId| {
            c.tables
                .param_element
                .get(n)
                .map(|p| dartr_typesystem::member::type_(ctx, *p))
        };
        if let Some(b) = c
            .covering_node()
            .and_then(|n| ast.cast::<BinaryExpression>(n))
        {
            if c.lexeme(ast[b].operator) == "??" && ast[b].right_operand.raw() == target {
                // Dart `_couldBeAssignableInNullAwareExpression`.
                let could = match param_type(b.raw()) {
                    None => true,
                    Some(expected) => c
                        .tables
                        .static_type
                        .get(ast[b].left_operand.raw())
                        .is_some_and(|l| {
                            ts.is_assignable_to(ts.promote_to_non_null(*l), expected, strict)
                        }),
                };
                if !could {
                    return;
                }
            }
        }
        let Some(parent) = ast.parent(target) else {
            return;
        };
        let mut to_type = None;
        if let Some(a) = ast
            .cast::<AssignmentExpression>(parent)
            .filter(|a| ast[*a].right_hand_side.raw() == target)
        {
            to_type = c.tables.write_type.get(a.raw()).copied();
        } else if ast.is::<AsExpression>(parent) {
            to_type = c.tables.static_type.get(parent).copied();
        } else if let Some(v) = ast
            .cast::<VariableDeclaration>(parent)
            .filter(|v| ast[*v].initializer.map(|i| i.raw()) == Some(target))
        {
            to_type = c
                .locator()
                .declared_element(v)
                .map(|e| dartr_resolver::element_ext::variable_type(ctx, e));
        } else if ast.is::<ArgumentList>(parent) {
            to_type = param_type(target);
        } else if let Some(i) = ast.cast::<IndexExpression>(parent) {
            to_type = real_target_of(ast, ast[i].target.map(|t| t.raw()), true, parent)
                .and_then(|t| c.tables.static_type.get(t).copied());
        } else if ast.is::<BinaryExpression>(parent) {
            if ts.is_non_nullable(from_type) {
                return;
            }
            if let Some(expected) = param_type(parent) {
                if !ts.is_assignable_to(ts.promote_to_non_null(from_type), expected, strict) {
                    return;
                }
            }
        } else if ast
            .cast::<PrefixedIdentifier>(parent)
            .is_some_and(|p| ast[p].prefix.raw() == target)
            || ast.is::<PostfixExpression>(parent)
            || ast.is::<PrefixExpression>(parent)
            || ast
                .cast::<PropertyAccess>(parent)
                .is_some_and(|p| ast[p].target.map(|t| t.raw()) == Some(target))
            || ast
                .cast::<CascadeExpression>(parent)
                .is_some_and(|p| ast[p].target.raw() == target)
            || ast
                .cast::<MethodInvocation>(parent)
                .is_some_and(|p| ast[p].target.map(|t| t.raw()) == Some(target))
            || ast
                .cast::<FunctionExpressionInvocation>(parent)
                .is_some_and(|p| ast[p].function.raw() == target)
        {
        } else {
            return;
        }
        if let Some(to_type) = to_type {
            if !self.skip_assignability_check
                && !ts.is_assignable_to(ts.promote_to_non_null(from_type), to_type, strict)
            {
                return;
            }
        }
        let needs_parentheses = ast.precedence(target) < Precedence::POSTFIX;
        let (offset, end) = (ast.offset(target), ast.end(target));
        builder.add_dart_file_edit(c.path, |b| {
            if needs_parentheses {
                b.add_simple_insertion(offset, "(");
            }
            b.add_insertion(end, |e| {
                if needs_parentheses {
                    e.write(")");
                }
                e.write("!");
            });
        });
    }
}
