// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_required_keyword.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_late.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_wildcard_variable.dart

//! Producers that change variables and parameters: `required`, `late`,
//! wildcard variables.

use dartr_ast::*;
use dartr_element::{ElementId, Tag};
use dartr_resolver::element_metadata::{AnnotationRef, UnitAst, flags};
use dartr_typesystem::type_ext::TypeExt;

use super::super::change_builder::ChangeBuilder;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::simple::producer;

/// The metadata of a formal parameter.
pub fn parameter_metadata(ast: &Ast, parameter: NodeId) -> Vec<Id<Annotation>> {
    if let Some(p) = ast.cast::<RegularFormalParameter>(parameter) {
        ast.list(ast[p].metadata).to_vec()
    } else if let Some(p) = ast.cast::<FieldFormalParameter>(parameter) {
        ast.list(ast[p].metadata).to_vec()
    } else if let Some(p) = ast.cast::<SuperFormalParameter>(parameter) {
        ast.list(ast[p].metadata).to_vec()
    } else {
        Vec::new()
    }
}

producer!(
    AddRequiredKeyword,
    k::ADD_REQUIRED,
    None,
    SingleLocation,
    |c, builder| {
        let ast = c.ast;
        let Some(parameter) = ast.this_or_ancestor_of_type::<FormalParameter>(c.node) else {
            builder.add_dart_file_edit(c.path, |_| {});
            return;
        };
        let mut insert_offset = ast.offset(parameter);
        let metadata = parameter_metadata(ast, parameter.raw());
        let mut deletion = None;
        if !metadata.is_empty() {
            let unit = UnitAst {
                ast,
                tables: c.tables,
            };
            let fragment = c.resolved.unit().fragment;
            for &annotation in &metadata {
                let r = AnnotationRef::of_node(c.ctx, unit, annotation, fragment);
                if r.is(c.ctx, flags::REQUIRED) {
                    let next = ast.tokens.next(ast.end_token(annotation));
                    let length = c.token_offset(next) - ast.offset(annotation);
                    deletion = Some((ast.offset(annotation), length));
                    break;
                }
            }
            let last = *metadata.last().unwrap();
            insert_offset = c.token_offset(ast.tokens.next(ast.end_token(last)));
        }
        builder.add_dart_file_edit(c.path, |b| {
            if let Some((offset, length)) = deletion {
                b.add_deletion(offset, length);
            }
            b.add_simple_insertion(insert_offset, "required ");
        });
    }
);

/// Dart `AddLate` (`AddLate.new` and `AddLate.this_`).
pub struct AddLate {
    pub this_: bool,
}

impl AddLate {
    fn insert_at(path: &str, builder: &mut ChangeBuilder<'_>, offset: u32) {
        builder.add_dart_file_edit(path, |b| b.add_simple_insertion(offset, "late "));
    }
}

impl CorrectionProducer for AddLate {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::ADD_LATE)
    }

    fn assist_kind(&self) -> Option<&'static FixKind> {
        Some(&crate::correction::generated::assist_kinds::ADD_LATE)
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let mut node = Some(c.node);
        if self.this_ {
            node = ast
                .this_or_ancestor_of_type::<VariableDeclaration>(c.node)
                .map(|v| v.raw());
        }
        let Some(node) = node else { return };
        if let Some(variable) = ast.cast::<VariableDeclaration>(node) {
            let Some(list) = ast
                .parent(variable)
                .and_then(|p| ast.cast::<VariableDeclarationList>(p))
            else {
                return;
            };
            if ast[list].late_keyword.is_some() {
                return;
            }
            let keyword = ast[list].keyword;
            match ast[list].type_ {
                None => match keyword {
                    None => {
                        let first = ast.list(ast[list].variables)[0];
                        Self::insert_at(c.path, builder, ast.offset(first));
                    }
                    Some(k) if c.lexeme(k) != "const" => {
                        Self::insert_at(c.path, builder, ast.offset(list))
                    }
                    _ => {}
                },
                Some(ty) => match keyword {
                    Some(k) => Self::insert_at(c.path, builder, c.token_offset(k)),
                    None => Self::insert_at(c.path, builder, ast.offset(ty)),
                },
            }
        } else if let Some(identifier) = ast.cast::<SimpleIdentifier>(node) {
            // A final field without initializer, assigned in this file.
            let Some(getter) = c.locator().write_or_read_element(identifier) else {
                return;
            };
            if getter.tag() != Tag::Getter {
                return;
            }
            let ctx = c.ctx;
            let Some(enclosing) = ctx.element_data(getter).and_then(|d| d.enclosing) else {
                return;
            };
            if enclosing
                .cast::<dartr_element::InterfaceElement>()
                .is_none()
            {
                return;
            }
            // The field declaration in this unit with the getter's name.
            let name = ctx.element_name(getter);
            let Some(name) = name else { return };
            let Some(field) = find_field_declaration(c, enclosing, name) else {
                return;
            };
            let list = ast[field].fields;
            let keyword = ast[list].keyword;
            if ast.list(ast[list].variables).len() == 1
                && ast[list].late_keyword.is_none()
                && keyword.is_some_and(|k| c.lexeme(k) == "final")
            {
                Self::insert_at(c.path, builder, c.token_offset(keyword.unwrap()));
            }
        }
    }
}

/// The field declaration of the field [name] of the class [enclosing] in
/// the unit of [c].
fn find_field_declaration(
    c: &ProducerContext<'_>,
    enclosing: ElementId,
    name: &str,
) -> Option<Id<FieldDeclaration>> {
    let ast = c.ast;
    for &declaration in ast.list_raw(ast[c.unit].declarations) {
        if c.locator().declared_element(declaration) != Some(enclosing) {
            continue;
        }
        let mut found = None;
        visit(ast, declaration, &mut |n| {
            if let Some(f) = ast.cast::<FieldDeclaration>(n) {
                let list = ast[f].fields;
                for &v in ast.list(ast[list].variables) {
                    if ast.tokens.lexeme(ast[v].name) == name && ast[f].static_keyword.is_none() {
                        found = Some(f);
                    }
                }
            }
        });
        return found;
    }
    None
}

/// Visits [node] and its descendants in pre-order.
pub fn visit(ast: &Ast, node: NodeId, f: &mut dyn FnMut(NodeId)) {
    f(node);
    for child in ast.children(node) {
        visit(ast, child, f);
    }
}

/// Dart `ConvertToWildcardVariable` (`.new` and `.automatically`).
pub struct ConvertToWildcardVariable {
    pub automatically: bool,
}

impl CorrectionProducer for ConvertToWildcardVariable {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::CONVERT_TO_WILDCARD_VARIABLE)
    }

    fn multi_fix_kind(&self) -> Option<&'static FixKind> {
        if self.automatically {
            Some(&k::CONVERT_TO_WILDCARD_VARIABLE_MULTI)
        } else {
            None
        }
    }

    fn applicability(&self) -> Applicability {
        if self.automatically {
            Applicability::Automatically
        } else {
            Applicability::SingleLocation
        }
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        if !c.ctx.features.is_enabled("wildcard-variables") {
            return;
        }
        let node = c.node;
        let replace = |builder: &mut ChangeBuilder<'_>, ranges: Vec<(u32, u32)>| {
            builder.add_dart_file_edit(c.path, |b| {
                for (offset, length) in ranges {
                    b.add_simple_replacement(offset, length, "_");
                }
            });
        };
        if ast.is::<FormalParameter>(node) {
            let name = if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
                ast[p].name
            } else if let Some(p) = ast.cast::<FieldFormalParameter>(node) {
                Some(ast[p].name)
            } else if let Some(p) = ast.cast::<SuperFormalParameter>(node) {
                Some(ast[p].name)
            } else {
                None
            };
            if let Some(name) = name {
                let r = c.range().token(name);
                replace(builder, vec![(r.offset, r.length)]);
            }
            return;
        }
        if let Some(p) = ast.cast::<DeclaredVariablePattern>(node) {
            // Dart `fieldNameWithImplicitName == null`: not `:var name` in an
            // object or record pattern.
            let implicit = ast
                .parent(p)
                .and_then(|n| ast.cast::<PatternField>(n))
                .is_some_and(|f| ast[f].name.is_some_and(|n| ast[n].name.is_none()));
            if !implicit {
                let r = c.range().token(ast[p].name);
                replace(builder, vec![(r.offset, r.length)]);
            }
            return;
        }
        let Some(variable) = ast.cast::<VariableDeclaration>(node) else {
            return;
        };
        let Some(element) = c.locator().declared_element(variable) else {
            return;
        };
        if element.tag() != Tag::LocalVariable {
            return;
        }
        let Some(root) = ast.this_or_ancestor_of_type::<Block>(variable) else {
            return;
        };
        // Dart `findLocalElementReferences`.
        let mut references = Vec::new();
        visit(ast, root.raw(), &mut |n| {
            if let Some(id) = ast.cast::<SimpleIdentifier>(n) {
                if c.locator().write_or_read_element(id) == Some(element) {
                    references.push(n);
                }
            } else if let Some(p) = ast.cast::<AssignedVariablePattern>(n) {
                if c.locator().element(p) == Some(element) {
                    references.push(n);
                }
            }
        });
        if references
            .iter()
            .any(|r| !ast.is::<AssignedVariablePattern>(*r))
        {
            return;
        }
        let name = c.range().token(ast[variable].name);
        let mut ranges = vec![(name.offset, name.length)];
        for r in references {
            let range = (ast.offset(r), ast.length(r));
            if !ranges.contains(&range) {
                ranges.push(range);
            }
        }
        replace(builder, ranges);
    }
}

// Dart source: pkg/analysis_server/lib/src/services/correction/dart/remove_unused_local_variable.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/util.dart (findLocalElementReferences)

/// A command of `RemoveUnusedLocalVariable`: delete or replace a range.
#[allow(dead_code)]
enum Command {
    Delete(u32, u32),
    Replace(u32, u32, String),
}

/// Dart `_SideEffectVisitor`.
fn has_side_effect(c: &ProducerContext<'_>, node: NodeId, element: ElementId) -> bool {
    let ast = c.ast;
    let is_element = |n: NodeId| -> bool {
        let n = super::simple::un_parenthesized(ast, n);
        ast.cast::<SimpleIdentifier>(n)
            .is_some_and(|id| c.locator().write_or_read_element(id) == Some(element))
    };
    if let Some(a) = ast.cast::<AssignmentExpression>(node) {
        if is_element(ast[a].left_hand_side.raw()) {
            return has_side_effect(c, ast[a].right_hand_side.raw(), element);
        }
        return true;
    }
    if ast.is::<AwaitExpression>(node)
        || ast.is::<FunctionExpressionInvocation>(node)
        || ast.is::<MethodInvocation>(node)
    {
        return true;
    }
    if let Some(p) = ast.cast::<PostfixExpression>(node) {
        if matches!(c.lexeme(ast[p].operator), "++" | "--") {
            return !is_element(ast[p].operand.raw());
        }
    }
    if let Some(p) = ast.cast::<PrefixExpression>(node) {
        if matches!(c.lexeme(ast[p].operator), "++" | "--") {
            return !is_element(ast[p].operand.raw());
        }
    }
    ast.children(node)
        .into_iter()
        .any(|child| has_side_effect(c, child, element))
}

producer!(
    RemoveUnusedLocalVariable,
    k::REMOVE_UNUSED_LOCAL_VARIABLE,
    None,
    SingleLocation,
    |c, builder| {
        let ast = c.ast;
        let r = c.range();
        // Dart `_localVariableElement`.
        let Some(variable) = ast.cast::<VariableDeclaration>(c.node) else {
            return;
        };
        if ast[variable].name != c.token {
            return;
        }
        let Some(element) = c.locator().declared_element(variable) else {
            return;
        };
        if element.tag() != Tag::LocalVariable {
            return;
        }
        let mut commands = Vec::new();
        // Dart `_deleteDeclaration`.
        let Some(list) = ast
            .parent(variable)
            .and_then(|p| ast.cast::<VariableDeclarationList>(p))
        else {
            return;
        };
        let Some(statement) = ast
            .parent(list)
            .filter(|p| ast.is::<VariableDeclarationStatement>(*p))
        else {
            return;
        };
        let variables = ast.list_raw(ast[list].variables).to_vec();
        if variables.len() != 1 {
            let range = r.node_in_list(&variables, variable.raw());
            commands.push(Command::Delete(range.offset, range.length));
        } else {
            let initializer = ast[variable].initializer.map(|i| i.raw());
            let un_parenthesized = initializer.map(|i| super::simple::un_parenthesized(ast, i));
            match un_parenthesized {
                Some(u) if has_side_effect(c, u, element) => {
                    if let Some(a) = ast.cast::<AsExpression>(u) {
                        let first = r.start_start(statement, ast[a].expression);
                        let second = r.end_end(ast[a].expression, a);
                        commands.push(Command::Delete(first.offset, first.length));
                        commands.push(Command::Delete(second.offset, second.length));
                    } else {
                        let start = ast.offset(statement);
                        commands.push(Command::Delete(
                            start,
                            ast.offset(initializer.unwrap()) - start,
                        ));
                    }
                }
                _ => {
                    let range = c.utils.get_lines_range(r.node(statement), false);
                    commands.push(Command::Delete(range.offset, range.length));
                }
            }
        }
        // Dart `_deleteReferences`.
        let Some(body) = ast.this_or_ancestor_of_type::<FunctionBody>(c.node) else {
            return;
        };
        let mut references = Vec::new();
        visit(ast, body.raw(), &mut |n| {
            if let Some(id) = ast.cast::<SimpleIdentifier>(n) {
                if c.locator().write_or_read_element(id) == Some(element) {
                    references.push(n);
                }
            }
        });
        let mut deleted: Vec<(u32, u32)> = Vec::new();
        for reference in references {
            let parent = ast.parent(reference);
            let Some(assignment) = parent.and_then(|p| ast.cast::<AssignmentExpression>(p)) else {
                return;
            };
            if ast[assignment].left_hand_side.raw() != reference {
                return;
            }
            // Dart `_forAssignmentExpression`.
            let Some(outer) = ast.parent(assignment) else {
                return;
            };
            let ranges: Vec<(u32, u32)> = if ast.is::<ArgumentList>(outer) {
                let next = ast.tokens.next(ast[assignment].operator);
                let range = r.node_start_token_start(assignment, next);
                vec![(range.offset, range.length)]
            } else {
                let u = super::simple::un_parenthesized(ast, ast[assignment].right_hand_side.raw());
                if has_side_effect(c, u, element) {
                    if ast.is::<AssignmentExpression>(u) {
                        let a = r.start_start(outer, u);
                        let b = r.end_end(u, assignment);
                        vec![(a.offset, a.length), (b.offset, b.length)]
                    } else if let Some(x) = ast.cast::<AsExpression>(u) {
                        let a = r.start_start(outer, ast[x].expression);
                        let b = r.end_end(ast[x].expression, x);
                        vec![(a.offset, a.length), (b.offset, b.length)]
                    } else {
                        let a = r.start_start(assignment, ast[assignment].right_hand_side);
                        vec![(a.offset, a.length)]
                    }
                } else {
                    let lines = c.utils.get_lines_range(r.node(outer), false);
                    vec![(lines.offset, lines.length)]
                }
            };
            // Dart `_addReferenceRanges`.
            let mut to_add = Vec::new();
            for (offset, length) in ranges {
                let end = offset + length;
                let mut covered = false;
                for &(o, l) in &deleted {
                    if o <= offset && end <= o + l {
                        covered = true;
                        break;
                    } else if offset < o + l && o < end {
                        return;
                    }
                }
                if !covered {
                    to_add.push((offset, length));
                }
            }
            for (o, l) in to_add {
                commands.push(Command::Delete(o, l));
                deleted.push((o, l));
            }
        }
        builder.add_dart_file_edit(c.path, |b| {
            for command in &commands {
                match command {
                    Command::Delete(o, l) => b.add_deletion(*o, *l),
                    Command::Replace(o, l, s) => b.add_simple_replacement(*o, *l, s),
                }
            }
        });
    }
);

// Dart source: pkg/analysis_server/lib/src/services/correction/dart/make_variable_nullable.dart

/// Dart `MakeVariableNullable`.
pub struct MakeVariableNullable {
    name: String,
}

impl MakeVariableNullable {
    pub fn new() -> Self {
        MakeVariableNullable {
            name: String::new(),
        }
    }

    /// The parameter parts: name, type annotation and function-typed
    /// suffix.
    fn parameter_parts(
        ast: &Ast,
        node: NodeId,
    ) -> Option<(
        Option<dartr_syntax::TokenId>,
        Option<Id<TypeAnnotation>>,
        Option<Id<FunctionTypedFormalParameterSuffix>>,
    )> {
        if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
            Some((ast[p].name, ast[p].type_, ast[p].function_typed_suffix))
        } else if let Some(p) = ast.cast::<FieldFormalParameter>(node) {
            Some((
                Some(ast[p].name),
                ast[p].type_,
                ast[p].function_typed_suffix,
            ))
        } else if let Some(p) = ast.cast::<SuperFormalParameter>(node) {
            Some((
                Some(ast[p].name),
                ast[p].type_,
                ast[p].function_typed_suffix,
            ))
        } else {
            None
        }
    }

    /// Dart `_updateVariableType`.
    fn update_variable_type(
        &mut self,
        c: &ProducerContext<'_>,
        builder: &mut ChangeBuilder<'_>,
        list: Id<VariableDeclarationList>,
        new_type: dartr_element::TypeId,
    ) {
        let ast = c.ast;
        let variable = ast.list(ast[list].variables)[0];
        self.name = c.lexeme(ast[variable].name).to_string();
        let keyword = ast[list].keyword;
        let type_annotation = ast[list].type_;
        let variable_offset = ast.offset(variable);
        builder.add_dart_file_edit(c.path, |b| {
            let options = super::super::dart_edit::WriteType::default();
            match keyword {
                Some(k) if c.lexeme(k) == "var" => {
                    let r = c.range().token(k);
                    b.add_replacement(r.offset, r.length, |e| {
                        e.write_type(Some(new_type), &options);
                    });
                }
                None => match type_annotation {
                    None => b.add_insertion(variable_offset, |e| {
                        e.write_type(Some(new_type), &options);
                        e.write(" ");
                    }),
                    Some(t) => b.add_simple_insertion(ast.end(t), "?"),
                },
                _ => {}
            }
        });
    }

    /// The new type of a variable assigned [expression] (Dart, shared by
    /// `_forAssignment` and `_forVariableDeclaration`).
    fn new_type(
        c: &ProducerContext<'_>,
        old_type: dartr_element::TypeId,
        expression: NodeId,
    ) -> Option<dartr_element::TypeId> {
        let ctx = c.ctx;
        if !matches!(
            ctx.ty(old_type),
            dartr_element::TypeKind::Interface { .. } | dartr_element::TypeKind::Record { .. }
        ) {
            return None;
        }
        if c.ast.is::<NullLiteral>(expression) {
            return Some(ctx.with_nullability(old_type, dartr_element::Nullability::Question));
        }
        let new_type = super::create::static_type(c, expression)?;
        let ts = dartr_typesystem::type_system::TypeSystem::new(*ctx);
        if !ts.is_assignable_to(
            old_type,
            ts.promote_to_non_null(new_type),
            c.options.strict_casts,
        ) {
            return None;
        }
        Some(new_type)
    }
}

impl CorrectionProducer for MakeVariableNullable {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::MAKE_VARIABLE_NULLABLE)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.name.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let node = c.node;
        if let Some((name, ty, suffix)) = Self::parameter_parts(ast, node) {
            if let Some(suffix) = suffix {
                if ast[suffix].question.is_some() {
                    return;
                }
                let Some(name) = name else { return };
                self.name = c.lexeme(name).to_string();
                let end = ast.end(suffix);
                builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(end, "?"));
                return;
            }
            let Some(ty) = ty else { return };
            let nullable = c.tables.annotation_type.get(ty.raw()).is_none_or(|t| {
                dartr_typesystem::type_system::TypeSystem::new(*c.ctx).is_nullable(*t)
            });
            if nullable {
                return;
            }
            let Some(name) = name else { return };
            self.name = c.lexeme(name).to_string();
            let end = ast.end(ty);
            builder.add_dart_file_edit(c.path, |b| b.add_simple_insertion(end, "?"));
            return;
        }
        if !ast.is::<Expression>(node) {
            return;
        }
        let Some(parent) = ast.parent(node) else {
            return;
        };
        if let Some(a) = ast.cast::<AssignmentExpression>(parent) {
            if ast[a].right_hand_side.raw() != node {
                return;
            }
            let Some(lhs) = ast.cast::<SimpleIdentifier>(ast[a].left_hand_side) else {
                return;
            };
            let Some(element) = c.locator().write_or_read_element(lhs) else {
                return;
            };
            if element.tag() != Tag::LocalVariable {
                return;
            }
            let old_type = dartr_resolver::element_ext::variable_type(c.ctx, element);
            let Some(new_type) = Self::new_type(c, old_type, node) else {
                return;
            };
            // Dart `_findDeclaration`.
            let mut block = ast.this_or_ancestor_of_type::<Block>(a);
            while let Some(bl) = block {
                for &statement in ast.list_raw(ast[bl].statements) {
                    let Some(s) = ast.cast::<VariableDeclarationStatement>(statement) else {
                        continue;
                    };
                    let list = ast[s].variables;
                    if ast
                        .list(ast[list].variables)
                        .iter()
                        .any(|v| c.locator().declared_element(*v) == Some(element))
                    {
                        if ast.list(ast[list].variables).len() > 1 {
                            return;
                        }
                        self.update_variable_type(c, builder, list, new_type);
                        return;
                    }
                }
                block = ast
                    .parent(bl)
                    .and_then(|p| ast.this_or_ancestor_of_type::<Block>(p));
            }
        } else if let Some(v) = ast.cast::<VariableDeclaration>(parent) {
            if ast[v].initializer.map(|i| i.raw()) != Some(node) {
                return;
            }
            let Some(list) = ast
                .parent(v)
                .and_then(|p| ast.cast::<VariableDeclarationList>(p))
            else {
                return;
            };
            if ast.list(ast[list].variables).len() > 1 {
                return;
            }
            let Some(element) = c.locator().declared_element(v) else {
                return;
            };
            let old_type = dartr_resolver::element_ext::variable_type(c.ctx, element);
            let Some(new_type) = Self::new_type(c, old_type, node) else {
                return;
            };
            self.update_variable_type(c, builder, list, new_type);
        }
    }
}
