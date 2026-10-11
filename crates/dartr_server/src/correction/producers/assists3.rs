// Dart source: pkg/analysis_server/lib/src/services/correction/dart/bind_to_field.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/bind_all_to_fields.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_initializing_formal.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/encapsulate_field.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_dot_shorthand.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/add_type_name.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_field_formal_to_normal.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_declaring_parameter.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_in_body_constructor.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_primary_constructor.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/dart/move_initialization_to_field_declaration.dart

//! Field, constructor, formal parameter, and dot-shorthand assists.

use dartr_ast::*;
use dartr_element::{ElementId, FormalParameterElement, InterfaceElement, Tag, TypeId, TypeKind};
use dartr_syntax::TokenId;
use dartr_typesystem::type_ext::TypeExt;

use super::super::change_builder::FileEditBuilder;
use super::super::dart_edit::WriteType;
use super::super::generated::assist_kinds as a;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::simple::{assist, producer};

fn annotation_type(c: &ProducerContext<'_>, node: NodeId) -> Option<TypeId> {
    c.tables.annotation_type.get(node).copied()
}

// ---------------------------------------------------------------------------
// BindToField & BindAllToFields
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum BindFixType {
    ReplaceWithThis,
    ReplaceWithThisAndNewField,
    AddVar,
    Noop,
}

fn constructor_parts<'a>(
    ast: &'a Ast,
    parameter: Id<FormalParameter>,
) -> Option<(&'a [NodeId], Option<TokenId>, Option<TokenId>, NodeId)> {
    if let Some(ctor) = ast.this_or_ancestor_of_type::<ConstructorDeclaration>(parameter) {
        let container = ast.this_or_ancestor_of_type::<CompilationUnitMember>(ctor)?;
        return Some((
            ast.list_raw(ast[ctor].initializers),
            ast[ctor].factory_keyword,
            ast[ctor].const_keyword,
            container.raw(),
        ));
    }
    if let Some(primary) = ast.this_or_ancestor_of_type::<PrimaryConstructorDeclaration>(parameter)
    {
        let container = ast.this_or_ancestor_of_type::<CompilationUnitMember>(primary)?;
        let body = super::super::insert::members_of(ast, container.raw()).and_then(|ms| {
            ms.into_iter()
                .find_map(|m| ast.cast::<PrimaryConstructorBody>(m))
        });
        let inits = body
            .map(|b| ast.list_raw(ast[b].initializers))
            .unwrap_or(&[]);
        return Some((inits, None, ast[primary].const_keyword, container.raw()));
    }
    None
}

fn whether_to_create_new_field(
    c: &ProducerContext<'_>,
    container: NodeId,
    parameter: Id<RegularFormalParameter>,
    param_name: &str,
    param_type: Option<TypeId>,
) -> BindFixType {
    let ast = c.ast;
    if let Some(cd) = ast.cast::<ClassDeclaration>(container) {
        let field_with_same_name = ast
            .cast::<BlockClassBody>(ast[cd].body)
            .and_then(|bcb| {
                ast.list_raw(ast[bcb].members)
                    .iter()
                    .find_map(|&m| ast.cast::<FieldDeclaration>(m))
            })
            .and_then(|fd| {
                ast.list(ast[ast[fd].fields].variables)
                    .iter()
                    .copied()
                    .find(|&v| c.lexeme(ast[v].name) == param_name)
            });
        if let Some(var_decl) = field_with_same_name {
            let field_type = c
                .locator()
                .declared_element(var_decl)
                .map(|e| dartr_resolver::element_ext::variable_type(c.ctx, e));
            if let (Some(t), Some(ft)) = (param_type, field_type) {
                let ts = dartr_typesystem::TypeSystem::new(*c.ctx);
                if ts.is_assignable_to(t, ft, false) {
                    return BindFixType::ReplaceWithThis;
                } else {
                    return BindFixType::Noop;
                }
            }
            return BindFixType::ReplaceWithThis;
        } else if ast
            .this_or_ancestor_of_type::<PrimaryConstructorDeclaration>(parameter)
            .is_some()
        {
            return BindFixType::AddVar;
        } else {
            return BindFixType::ReplaceWithThisAndNewField;
        }
    }
    BindFixType::ReplaceWithThisAndNewField
}

fn try_replacing_parameter(
    c: &ProducerContext<'_>,
    builder: &mut super::super::change_builder::ChangeBuilder<'_>,
    parameter: Id<FormalParameter>,
) {
    let ast = c.ast;
    let Some(rfp) = ast.cast::<RegularFormalParameter>(parameter) else {
        return;
    };
    let Some((initializers, factory_kw, const_kw, container)) = constructor_parts(ast, parameter)
    else {
        return;
    };
    if initializers
        .iter()
        .any(|&i| ast.is::<RedirectingConstructorInvocation>(i))
        || factory_kw.is_some()
    {
        return;
    }
    let param_name_opt = ast[rfp].name.map(|t| c.lexeme(t));
    if let Some(cd) = ast.cast::<ClassDeclaration>(container) {
        if let Some(bcb) = ast.cast::<BlockClassBody>(ast[cd].body) {
            if ast.list_raw(ast[bcb].members).iter().any(|&m| {
                ast.cast::<MethodDeclaration>(m)
                    .is_some_and(|md| Some(c.lexeme(ast[md].name)) == param_name_opt)
            }) {
                return;
            }
        }
    } else if let Some(ed) = ast.cast::<EnumDeclaration>(container) {
        if let Some(beb) = ast.cast::<BlockEnumBody>(ast[ed].body) {
            if ast
                .list(ast[beb].constants)
                .iter()
                .any(|&ec| Some(c.lexeme(ast[ec].name)) == param_name_opt)
            {
                return;
            }
        }
    } else {
        return;
    }

    builder.add_dart_file_edit(c.path, |b| {
        let Some(name_tok) = ast[rfp].name else {
            return;
        };
        let name = c.lexeme(name_tok);
        let ty = c
            .locator()
            .declared_element(rfp)
            .map(|e| dartr_resolver::element_ext::variable_type(c.ctx, e));
        let fix_type = whether_to_create_new_field(c, container, rfp, name, ty);
        if fix_type == BindFixType::AddVar {
            b.add_simple_insertion(ast.offset(rfp), "var ");
            return;
        }
        if fix_type != BindFixType::Noop {
            let start_tok = ast[rfp]
                .type_
                .map(|t| ast.begin_token(t))
                .or(ast[rfp].const_final_or_var_keyword)
                .unwrap_or(name_tok);
            let end_tok = ast[rfp]
                .function_typed_suffix
                .map(|s| ast.end_token(s))
                .unwrap_or(name_tok);
            let r = c.range().token_start_end(start_tok, end_tok);
            b.add_simple_replacement(r.offset, r.length, &format!("this.{name}"));
        }
        if fix_type == BindFixType::ReplaceWithThisAndNewField {
            let is_final = const_kw.is_some()
                || ast[rfp]
                    .const_final_or_var_keyword
                    .is_some_and(|k| c.lexeme(k) == "final");
            let field_ty =
                if is_final && ty.is_some_and(|t| matches!(c.ctx.ty(t), TypeKind::Dynamic)) {
                    None
                } else {
                    ty
                };
            b.insert_field(container, |e| {
                super::members::write_field(c.ctx, e, name, is_final, false, field_ty, false);
            });
        }
    });
}

assist!(BindToField, a::BIND_TO_FIELD, |c, builder| {
    let ast = c.ast;
    let Some(parameter) = ast.this_or_ancestor_of_type::<FormalParameter>(c.node) else {
        return;
    };
    let default_clause = if let Some(rfp) = ast.cast::<RegularFormalParameter>(parameter) {
        ast[rfp].default_clause
    } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(parameter) {
        ast[ffp].default_clause
    } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(parameter) {
        ast[sfp].default_clause
    } else {
        None
    };
    if let Some(dc) = default_clause {
        if ast.offset(c.node) >= c.token_offset(ast[dc].separator) {
            return;
        }
    }
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(parameter) {
        if ast[rfp].const_final_or_var_keyword.is_some_and(|k| {
            let lex = c.lexeme(k);
            lex == "var" || lex == "final"
        }) && ast
            .this_or_ancestor_of_type::<PrimaryConstructorDeclaration>(parameter)
            .is_some()
        {
            return;
        }
    }
    try_replacing_parameter(c, builder, parameter);
});

assist!(BindAllToFields, a::BIND_ALL_TO_FIELDS, |c, builder| {
    let ast = c.ast;
    let Some(param_list) = ast.this_or_ancestor_of_type::<FormalParameterList>(c.node) else {
        return;
    };
    for &p in ast.list(ast[param_list].parameters) {
        try_replacing_parameter(c, builder, p);
    }
});

// ---------------------------------------------------------------------------
// ConvertToInitializingFormal
// ---------------------------------------------------------------------------

struct InitFormalAnchor {
    parameter_list: Id<FormalParameterList>,
    initializers: Option<NodeList<ConstructorInitializer>>,
    documentation_comment: Option<Id<Comment>>,
    body: Option<NodeId>,
    field_initializers: Vec<NodeId>,
}

fn find_init_formal_anchor(c: &ProducerContext<'_>) -> Option<InitFormalAnchor> {
    let ast = c.ast;
    let find_field_initializers = |container_body: Option<NodeId>| -> Vec<NodeId> {
        let Some(cb) = container_body else {
            return Vec::new();
        };
        let members = if let Some(b) = ast.cast::<BlockClassBody>(cb) {
            ast.list_raw(ast[b].members)
        } else if let Some(b) = ast.cast::<BlockEnumBody>(cb) {
            ast.list_raw(ast[b].members)
        } else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &m in members {
            if let Some(fd) = ast.cast::<FieldDeclaration>(m) {
                for &v in ast.list(ast[ast[fd].fields].variables) {
                    if let Some(init) = ast[v].initializer {
                        out.push(init.raw());
                    }
                }
            }
        }
        out
    };

    let mut cur = Some(c.node);
    let mut anchor = None;
    while let Some(n) = cur {
        if ast.is::<ConstructorDeclaration>(n)
            || ast.is::<PrimaryConstructorDeclaration>(n)
            || ast.is::<PrimaryConstructorBody>(n)
        {
            anchor = Some(n);
            break;
        }
        cur = ast.parent(n);
    }
    let anchor = anchor?;
    if let Some(cd) = ast.cast::<ConstructorDeclaration>(anchor) {
        Some(InitFormalAnchor {
            parameter_list: ast[cd].parameters,
            initializers: Some(ast[cd].initializers),
            documentation_comment: ast[cd].documentation_comment,
            body: Some(ast[cd].body.raw()),
            field_initializers: find_field_initializers(ast.parent(cd)),
        })
    } else if let Some(pcd) = ast.cast::<PrimaryConstructorDeclaration>(anchor) {
        let container = ast.parent(pcd);
        let container_body = container.and_then(|cnt| {
            if let Some(cd) = ast.cast::<ClassDeclaration>(cnt) {
                Some(ast[cd].body.raw())
            } else if let Some(ed) = ast.cast::<EnumDeclaration>(cnt) {
                Some(ast[ed].body.raw())
            } else if let Some(ed) = ast.cast::<ExtensionDeclaration>(cnt) {
                Some(ast[ed].body.raw())
            } else if let Some(etd) = ast.cast::<ExtensionTypeDeclaration>(cnt) {
                Some(ast[etd].body.raw())
            } else if let Some(md) = ast.cast::<MixinDeclaration>(cnt) {
                Some(ast[md].body.raw())
            } else {
                None
            }
        });
        let primary_body = container
            .and_then(|cnt| super::super::insert::members_of(ast, cnt))
            .and_then(|members| {
                members
                    .into_iter()
                    .find_map(|m| ast.cast::<PrimaryConstructorBody>(m))
            });
        Some(InitFormalAnchor {
            parameter_list: ast[pcd].formal_parameters,
            initializers: primary_body.map(|pb| ast[pb].initializers),
            documentation_comment: primary_body.and_then(|pb| ast[pb].documentation_comment),
            body: primary_body.map(|pb| ast[pb].body.raw()),
            field_initializers: find_field_initializers(container_body),
        })
    } else if let Some(pb) = ast.cast::<PrimaryConstructorBody>(anchor) {
        let container_body = ast.parent(pb)?;
        let container = ast.parent(container_body)?;
        let name_part = if let Some(cd) = ast.cast::<ClassDeclaration>(container) {
            ast[cd].name_part
        } else if let Some(ed) = ast.cast::<EnumDeclaration>(container) {
            ast[ed].name_part
        } else if let Some(etd) = ast.cast::<ExtensionTypeDeclaration>(container) {
            ast[etd].name_part
        } else {
            return None;
        };
        let pcd = ast.cast::<PrimaryConstructorDeclaration>(name_part)?;
        Some(InitFormalAnchor {
            parameter_list: ast[pcd].formal_parameters,
            initializers: Some(ast[pb].initializers),
            documentation_comment: ast[pb].documentation_comment,
            body: Some(ast[pb].body.raw()),
            field_initializers: find_field_initializers(Some(container_body)),
        })
    } else {
        None
    }
}

fn find_parameter_for_element(
    c: &ProducerContext<'_>,
    parameter_list: Id<FormalParameterList>,
    element: ElementId,
) -> Option<NodeId> {
    for &p in c.ast.list_raw(c.ast[parameter_list].parameters) {
        if c.locator().declared_element(p) == Some(element) {
            return Some(p);
        }
    }
    None
}

fn find_parameter_in_list(
    c: &ProducerContext<'_>,
    parameter_list: Id<FormalParameterList>,
    expr: NodeId,
) -> Option<(NodeId, ElementId)> {
    let id = c.ast.cast::<SimpleIdentifier>(expr)?;
    let elem = c.locator().element(id)?;
    if !matches!(
        elem.tag(),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
    ) {
        return None;
    }
    let param = find_parameter_for_element(c, parameter_list, elem)?;
    Some((param, elem))
}

fn find_parameter_references_for_init_formal(
    c: &ProducerContext<'_>,
    parameter_element: ElementId,
    anchor: &InitFormalAnchor,
    removed_init: Option<NodeId>,
    removed_assign: Option<NodeId>,
) -> Vec<Id<SimpleIdentifier>> {
    let ast = c.ast;
    let mut refs = Vec::new();
    let mut collect = |root: NodeId| {
        super::variables::visit(ast, root, &mut |n| {
            if Some(n) == removed_init || Some(n) == removed_assign {
                return;
            }
            if let Some(id) = ast.cast::<SimpleIdentifier>(n) {
                let in_removed = (removed_init.is_some_and(|ri| {
                    ast.offset(id) >= ast.offset(ri) && ast.end(id) <= ast.end(ri)
                })) || (removed_assign.is_some_and(|ra| {
                    ast.offset(id) >= ast.offset(ra) && ast.end(id) <= ast.end(ra)
                }));
                if !in_removed && c.locator().element(id) == Some(parameter_element) {
                    refs.push(id);
                }
            }
        });
    };
    for &fi in &anchor.field_initializers {
        collect(fi);
    }
    if let Some(inits) = anchor.initializers {
        for &init in ast.list_raw(inits) {
            if Some(init) != removed_init {
                collect(init);
            }
        }
    }
    if let Some(body) = anchor.body {
        collect(body);
    }
    refs
}

fn compute_initializing_formal_change(
    c: &ProducerContext<'_>,
    builder: &mut super::super::change_builder::ChangeBuilder<'_>,
    anchor: &InitFormalAnchor,
    parameter_node: NodeId,
    parameter_element: ElementId,
    field: ElementId,
    initializer: Option<Id<ConstructorFieldInitializer>>,
    assignment: Option<Id<ExpressionStatement>>,
) {
    let ast = c.ast;
    if ast.is::<SuperFormalParameter>(parameter_node) {
        return;
    }
    let (param_name_tok, is_named, param_type, param_fts) =
        if let Some(rfp) = ast.cast::<RegularFormalParameter>(parameter_node) {
            let Some(name_tok) = ast[rfp].name else {
                return;
            };
            (
                name_tok,
                matches!(
                    ast[rfp].kind,
                    ParameterKind::Named | ParameterKind::NamedRequired
                ),
                ast[rfp].type_,
                ast[rfp].function_typed_suffix,
            )
        } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(parameter_node) {
            (
                ast[ffp].name,
                matches!(
                    ast[ffp].kind,
                    ParameterKind::Named | ParameterKind::NamedRequired
                ),
                ast[ffp].type_,
                ast[ffp].function_typed_suffix,
            )
        } else {
            return;
        };
    let parameter_name = c.lexeme(param_name_tok).to_string();
    let Some(field_name) = c.ctx.element_name(field).map(|s| s.to_string()) else {
        return;
    };
    let mut references = Vec::new();
    let mut update_comment_references = false;
    if is_named {
        if field_name == format!("_{parameter_name}") {
            if !c.ctx.features.is_enabled("private-named-parameters") && field_name.starts_with('_')
            {
                return;
            }
            references = find_parameter_references_for_init_formal(
                c,
                parameter_element,
                anchor,
                initializer.map(|i| i.raw()),
                assignment.map(|a| a.raw()),
            );
            update_comment_references = true;
        } else if field_name != parameter_name {
            return;
        }
    }

    let param_ty = dartr_resolver::element_ext::variable_type(c.ctx, parameter_element);
    let field_ty = dartr_resolver::element_ext::variable_type(c.ctx, field);
    let same_type = dartr_typesystem::TypeSystem::new(*c.ctx).dart_eq(param_ty, field_ty);

    builder.add_dart_file_edit(c.path, |b| {
        b.add_simple_insertion(c.token_offset(param_name_tok), "this.");
        if field_name != parameter_name {
            let r = c.range().token(param_name_tok);
            b.add_simple_replacement(r.offset, r.length, &field_name);
        }
        if same_type {
            if let Some(fts) = param_fts {
                if let Some(t) = param_type {
                    let r = c.range().deletion_range(t.raw());
                    b.add_deletion(r.offset, r.length);
                }
                let start_node = ast[fts]
                    .type_parameters
                    .map(|tp| tp.raw())
                    .unwrap_or_else(|| ast[fts].formal_parameters.raw());
                let r = c
                    .range()
                    .deletion_range_with_end(start_node, Some(ast.end_token(fts)));
                b.add_deletion(r.offset, r.length);
            } else if let Some(t) = param_type {
                let r = c.range().deletion_range(t.raw());
                b.add_deletion(r.offset, r.length);
            }
        }
        if let Some(init) = initializer {
            if let Some(inits_list) = anchor.initializers {
                let inits = ast.list_raw(inits_list);
                if inits.len() == 1 {
                    if let Some(pb) = ast
                        .parent(init)
                        .and_then(|p| ast.cast::<PrimaryConstructorBody>(p))
                    {
                        let r = c
                            .range()
                            .token_end_node_end(ast[pb].this_keyword, init.raw());
                        b.add_deletion(r.offset, r.length);
                    } else {
                        let r = c.range().end_end(anchor.parameter_list, init);
                        b.add_deletion(r.offset, r.length);
                    }
                } else {
                    let r = c.range().node_in_list(inits, init.raw());
                    b.add_deletion(r.offset, r.length);
                }
            }
        }
        if let Some(assign_stmt) = assignment {
            if let Some(block) = ast.parent(assign_stmt).and_then(|p| ast.cast::<Block>(p)) {
                let stmts = ast.list_raw(ast[block].statements);
                let function_body = ast.parent(block);
                if stmts.len() == 1
                    && function_body.is_some_and(|fb| ast.is::<BlockFunctionBody>(fb))
                {
                    let fb = function_body.unwrap();
                    let inits = anchor
                        .initializers
                        .map(|l| ast.list_raw(l))
                        .unwrap_or_default();
                    if let Some(pb) = ast
                        .parent(fb)
                        .and_then(|p| ast.cast::<PrimaryConstructorBody>(p))
                    {
                        let r = if let Some(&last_init) = inits.last() {
                            c.range().end_end(last_init, fb)
                        } else {
                            c.range().token_end_node_end(ast[pb].this_keyword, fb)
                        };
                        b.add_simple_replacement(r.offset, r.length, ";");
                    } else {
                        let prev = inits
                            .last()
                            .copied()
                            .unwrap_or_else(|| anchor.parameter_list.raw());
                        let r = c.range().end_end(prev, fb);
                        b.add_simple_replacement(r.offset, r.length, ";");
                    }
                } else {
                    let r = c.range().node_in_list(stmts, assign_stmt.raw());
                    b.add_deletion(r.offset, r.length);
                }
            }
        }
        for ref_id in references {
            let r = c.range().node(ref_id);
            b.add_simple_replacement(r.offset, r.length, &field_name);
        }
        if update_comment_references {
            if let Some(doc) = anchor.documentation_comment {
                for &cr in ast.list(ast[doc].references) {
                    if let Some(id) = ast.cast::<SimpleIdentifier>(ast[cr].expression) {
                        if c.locator().element(id) == Some(parameter_element) {
                            b.add_simple_insertion(ast.offset(id), "_");
                        }
                    }
                }
            }
        }
    });
}

fn compute_initializing_formal_from_parameter(
    c: &ProducerContext<'_>,
    builder: &mut super::super::change_builder::ChangeBuilder<'_>,
    anchor: &InitFormalAnchor,
    parameter: NodeId,
    parameter_element: ElementId,
) {
    let ast = c.ast;
    if let Some(inits_list) = anchor.initializers {
        for &init in ast.list_raw(inits_list) {
            if let Some(cfi) = ast.cast::<ConstructorFieldInitializer>(init) {
                if let Some(id) = ast.cast::<SimpleIdentifier>(ast[cfi].expression) {
                    if c.locator().element(id) == Some(parameter_element) {
                        if let Some(field) = c.locator().element(ast[cfi].field_name) {
                            if field.tag() == Tag::Field {
                                compute_initializing_formal_change(
                                    c,
                                    builder,
                                    anchor,
                                    parameter,
                                    parameter_element,
                                    field,
                                    Some(cfi),
                                    None,
                                );
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(body) = anchor.body {
        if let Some(bfb) = ast.cast::<BlockFunctionBody>(body) {
            for &stmt in ast.list_raw(ast[ast[bfb].block].statements) {
                let Some(es) = ast.cast::<ExpressionStatement>(stmt) else {
                    continue;
                };
                let Some(ae) = ast.cast::<AssignmentExpression>(ast[es].expression) else {
                    continue;
                };
                let Some(pa) = ast.cast::<PropertyAccess>(ast[ae].left_hand_side) else {
                    continue;
                };
                if !ast[pa].target.is_some_and(|t| ast.is::<ThisExpression>(t)) {
                    continue;
                }
                let Some(rhs_id) = ast.cast::<SimpleIdentifier>(ast[ae].right_hand_side) else {
                    continue;
                };
                if c.locator().element(rhs_id) != Some(parameter_element) {
                    continue;
                }
                let Some(we) =
                    dartr_resolver::error::support::write_element(c.ctx, c.tables, ae.raw())
                else {
                    continue;
                };
                let Some(setter) = we.cast::<dartr_element::SetterElement>() else {
                    continue;
                };
                let Some(field) = c.ctx.get(setter).variable.get() else {
                    continue;
                };
                compute_initializing_formal_change(
                    c,
                    builder,
                    anchor,
                    parameter,
                    parameter_element,
                    field.raw(),
                    None,
                    Some(es),
                );
                return;
            }
        }
    }
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(parameter)
        && parameter_element.tag() == Tag::FieldFormalParameter
    {
        let Some(keyword) = ast[rfp].const_final_or_var_keyword else {
            return;
        };
        let Some(param_name) = ast[rfp].name else {
            return;
        };
        let Some(container) = ast.this_or_ancestor_of_type::<CompilationUnitMember>(c.node) else {
            return;
        };
        let is_final = c.lexeme(keyword) == "final";
        let ty = dartr_resolver::element_ext::variable_type(c.ctx, parameter_element);
        let name_str = c.lexeme(param_name).to_string();
        let kw_to_name = c.range().token_start_start(keyword, param_name);
        let suffix_range = ast[rfp].function_typed_suffix.map(|s| c.range().node(s));
        builder.add_dart_file_edit(c.path, |b| {
            b.add_simple_replacement(kw_to_name.offset, kw_to_name.length, "this.");
            if let Some(sr) = suffix_range {
                b.add_deletion(sr.offset, sr.length);
            }
            b.insert_field(container.raw(), |e| {
                super::members::write_field(c.ctx, e, &name_str, is_final, false, Some(ty), false);
            });
        });
    }
}

producer!(
    ConvertToInitializingFormal,
    k::CONVERT_TO_INITIALIZING_FORMAL,
    Some(&k::CONVERT_TO_INITIALIZING_FORMAL_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_TO_INITIALIZING_FORMAL),
    |c, builder| {
        let ast = c.ast;
        let Some(anchor) = find_init_formal_anchor(c) else {
            return;
        };
        if let Some(ae) = ast.cast::<AssignmentExpression>(c.node) {
            let Some(stmt) = ast.parent(ae).and_then(|p| ast.cast::<ExpressionStatement>(p))
            else {
                return;
            };
            if !ast.parent(stmt).is_some_and(|p| ast.is::<Block>(p)) {
                return;
            }
            let Some((param, param_elem)) =
                find_parameter_in_list(c, anchor.parameter_list, ast[ae].right_hand_side.raw())
            else {
                return;
            };
            let Some(we) =
                dartr_resolver::error::support::write_element(c.ctx, c.tables, ae.raw())
            else {
                return;
            };
            let field = if let Some(setter) = we.cast::<dartr_element::SetterElement>() {
                c.ctx.get(setter).variable.get().map(|v| v.raw())
            } else if we.tag() == Tag::Field {
                Some(we)
            } else {
                None
            };
            let Some(field) = field else {
                return;
            };
            compute_initializing_formal_change(
                c,
                builder,
                &anchor,
                param,
                param_elem,
                field,
                None,
                Some(stmt),
            );
        } else if let Some(cfi) = ast.cast::<ConstructorFieldInitializer>(c.node) {
            let Some((param, param_elem)) =
                find_parameter_in_list(c, anchor.parameter_list, ast[cfi].expression.raw())
            else {
                return;
            };
            let Some(field) = c.locator().element(ast[cfi].field_name) else {
                return;
            };
            if field.tag() != Tag::Field {
                return;
            }
            compute_initializing_formal_change(
                c,
                builder,
                &anchor,
                param,
                param_elem,
                field,
                Some(cfi),
                None,
            );
        } else if let Some(rfp) = ast.cast::<RegularFormalParameter>(c.node) {
            let Some(param_elem) = c.locator().declared_element(rfp) else {
                return;
            };
            compute_initializing_formal_from_parameter(
                c,
                builder,
                &anchor,
                rfp.raw(),
                param_elem,
            );
        } else if let Some(id) = ast.cast::<SimpleIdentifier>(c.node) {
            if let Some((param, param_elem)) =
                find_parameter_in_list(c, anchor.parameter_list, id.raw())
            {
                compute_initializing_formal_from_parameter(
                    c,
                    builder,
                    &anchor,
                    param,
                    param_elem,
                );
            }
        }
    }
);

// ---------------------------------------------------------------------------
// EncapsulateField
// ---------------------------------------------------------------------------

fn update_references_in_constructor_for_encapsulate(
    c: &ProducerContext<'_>,
    b: &mut FileEditBuilder<'_, '_>,
    field_element: ElementId,
    name: &str,
    type_node: Option<Id<TypeAnnotation>>,
    parameters: Id<FormalParameterList>,
    separator: Option<TokenId>,
    initializers: Option<NodeList<ConstructorInitializer>>,
) {
    let ast = c.ast;
    let mut renamed_param: Option<ElementId> = None;
    for &p in ast.list_raw(ast[parameters].parameters) {
        let Some(param_elem) = c.locator().declared_element(p) else {
            continue;
        };
        let Some(fp) = param_elem.cast::<FormalParameterElement>() else {
            continue;
        };
        if c.ctx.get(fp).field.get().map(|f| f.raw()) != Some(field_element) {
            continue;
        }
        let (name_tok, is_named, final_kw) = if let Some(ffp) = ast.cast::<FieldFormalParameter>(p)
        {
            (
                Some(ast[ffp].name),
                matches!(
                    ast[ffp].kind,
                    ParameterKind::Named | ParameterKind::NamedRequired
                ),
                ast[ffp]
                    .const_final_or_var_keyword
                    .filter(|&k| c.lexeme(k) == "final"),
            )
        } else if let Some(rfp) = ast.cast::<RegularFormalParameter>(p) {
            (
                ast[rfp].name,
                matches!(
                    ast[rfp].kind,
                    ParameterKind::Named | ParameterKind::NamedRequired
                ),
                ast[rfp]
                    .const_final_or_var_keyword
                    .filter(|&k| c.lexeme(k) == "final"),
            )
        } else {
            continue;
        };
        let Some(name_tok) = name_tok else {
            continue;
        };
        if is_named && !c.ctx.features.is_enabled("private-named-parameters") {
            if let Some(ffp) = ast.cast::<FieldFormalParameter>(p) {
                let r = c
                    .range()
                    .token_start_end(ast[ffp].this_keyword, ast[ffp].period);
                let ty = type_node.and_then(|t| annotation_type(c, t.raw()));
                b.add_replacement(r.offset, r.length, |e| {
                    if let Some(ty) = ty {
                        if e.write_type(Some(ty), &WriteType::default()) {
                            e.write(" ");
                        }
                    }
                });
                let prev_end = separator
                    .map(|s| c.token_end(s))
                    .unwrap_or_else(|| ast.end(parameters));
                let init_empty = initializers.is_some_and(|i| ast.list_raw(i).is_empty());
                let replacement = if init_empty {
                    format!(" : _{name} = {name}")
                } else {
                    format!(" _{name} = {name},")
                };
                b.add_simple_insertion(prev_end, &replacement);
            }
        } else {
            let r = c.range().token(name_tok);
            b.add_simple_replacement(r.offset, r.length, &format!("_{name}"));
            renamed_param = Some(param_elem);
        }
        if let Some(fk) = final_kw {
            let r = c.range().token(fk);
            b.add_simple_replacement(r.offset, r.length, "var");
        }
    }
    if let Some(inits) = initializers {
        for &init in ast.list_raw(inits) {
            if let Some(fi) = ast.cast::<ConstructorFieldInitializer>(init) {
                let fn_id = ast[fi].field_name;
                if c.locator().element(fn_id) == Some(field_element) {
                    let r = c.range().node(fn_id);
                    b.add_simple_replacement(r.offset, r.length, &format!("_{name}"));
                }
            }
            if let Some(rp) = renamed_param {
                super::variables::visit(ast, init, &mut |n| {
                    if let Some(id) = ast.cast::<SimpleIdentifier>(n) {
                        if c.locator().element(id) == Some(rp) {
                            let r = c.range().token(ast[id].token);
                            b.add_simple_replacement(r.offset, r.length, &format!("_{name}"));
                        }
                    }
                });
            }
        }
    }
}

assist!(EncapsulateField, a::ENCAPSULATE_FIELD, |c, builder| {
    let ast = c.ast;
    let ctx = c.ctx;
    // Try _findFieldDeclaration or _findDeclaringParameter.
    let (
        name_token,
        field_decl_opt,
        field_element,
        metadata_list,
        interface_decl,
        is_final,
        type_node,
        doc_comment,
    ) = if let Some(field_decl) = ast.this_or_ancestor_of_type::<FieldDeclaration>(c.node) {
        if ast[field_decl].static_keyword.is_some() {
            return;
        }
        let var_list = ast[field_decl].fields;
        if ast[var_list].keyword.is_none() && ast[var_list].type_.is_none() {
            return;
        }
        let vars = ast.list(ast[var_list].variables);
        if vars.len() != 1 {
            return;
        }
        let field = vars[0];
        let Some(parent) = ast.parent(field_decl).and_then(|p| ast.parent(p)) else {
            return;
        };
        if !ast.is::<CompilationUnitMember>(parent) {
            return;
        }
        let Some(elem) = c.locator().declared_element(field) else {
            return;
        };
        if elem.tag() != Tag::Field {
            return;
        }
        let is_final = ast[var_list]
            .keyword
            .is_some_and(|k| c.lexeme(k) == "final");
        (
            ast[field].name,
            Some(field_decl),
            elem,
            Some(ast[field_decl].metadata),
            parent,
            is_final,
            ast[var_list].type_,
            ast[field_decl].documentation_comment,
        )
    } else if let Some(param) = ast.this_or_ancestor_of_type::<RegularFormalParameter>(c.node) {
        let Some(name_tok) = ast[param].name else {
            return;
        };
        let Some(primary) = ast.this_or_ancestor_of_type::<PrimaryConstructorDeclaration>(param)
        else {
            return;
        };
        let Some(parent) = ast.parent(primary) else {
            return;
        };
        if !ast.is::<CompilationUnitMember>(parent) {
            return;
        }
        let Some(param_elem) = c
            .locator()
            .declared_element(param)
            .and_then(|e| e.cast::<FormalParameterElement>())
        else {
            return;
        };
        let Some(field_elem) = ctx.get(param_elem).field.get() else {
            return;
        };
        let is_final = ast[param]
            .const_final_or_var_keyword
            .is_some_and(|k| c.lexeme(k) == "final");
        (
            name_tok,
            None,
            field_elem.raw(),
            None,
            parent,
            is_final,
            ast[param].type_,
            None,
        )
    } else {
        return;
    };

    let name = c.lexeme(name_token).to_string();
    if name.starts_with('_') || name_token != c.token {
        return;
    }

    let (class_members, parent_element, name_part) =
        if let Some(cd) = ast.cast::<ClassDeclaration>(interface_decl) {
            let Some(body) = ast.cast::<BlockClassBody>(ast[cd].body) else {
                return;
            };
            let Some(pe) = c
                .locator()
                .declared_element(cd)
                .and_then(|e| e.cast::<InterfaceElement>())
            else {
                return;
            };
            (
                ast.list_raw(ast[body].members).to_vec(),
                pe,
                Some(ast[cd].name_part.raw()),
            )
        } else if let Some(md) = ast.cast::<MixinDeclaration>(interface_decl) {
            let Some(body) = ast.cast::<BlockClassBody>(ast[md].body) else {
                return;
            };
            let Some(pe) = c
                .locator()
                .declared_element(md)
                .and_then(|e| e.cast::<InterfaceElement>())
            else {
                return;
            };
            (ast.list_raw(ast[body].members).to_vec(), pe, None)
        } else {
            return;
        };

    let doc_code = doc_comment.map(|d| c.utils.get_node_text(d));
    let annotations: Vec<(bool, String)> = metadata_list
        .map(|list| {
            ast.list(list)
                .iter()
                .map(|&a| {
                    let text = c.utils.get_range_text(c.range().node(a));
                    let is_override = text == "@override";
                    (is_override, text)
                })
                .collect()
        })
        .unwrap_or_default();

    let has_overridden_getter = ctx
        .element_all_supertypes(parent_element)
        .iter()
        .any(|&st| {
            let TypeKind::Interface { element: se, .. } = *ctx.ty(st) else {
                return false;
            };
            ctx.interface(se)
                .getters
                .iter()
                .any(|&g| ctx.element_name(g.raw()) == Some(name.as_str()))
                || ctx
                    .interface(se)
                    .methods
                    .iter()
                    .any(|&m| ctx.element_name(m.raw()) == Some(name.as_str()))
        });
    let setter_lookup = format!("{name}=");
    let has_overridden_setter = ctx
        .element_all_supertypes(parent_element)
        .iter()
        .any(|&st| {
            let TypeKind::Interface { element: se, .. } = *ctx.ty(st) else {
                return false;
            };
            ctx.interface(se).setters.iter().any(|&s| {
                let sn = ctx.element_name(s.raw()).unwrap_or("");
                sn == name || sn == setter_lookup
            })
        });
    let field_ty = type_node.and_then(|t| annotation_type(c, t.raw()));

    builder.add_dart_file_edit(c.path, |b| {
        if let Some(field_decl) = field_decl_opt {
            let meta = ast.list(ast[field_decl].metadata);
            if let (Some(&first), Some(&last)) = (meta.first(), meta.last()) {
                let r = c
                    .utils
                    .get_lines_range(c.range().start_end(first, last), false);
                b.add_deletion(r.offset, r.length);
            }
            let nr = c.range().token(name_token);
            b.add_simple_replacement(nr.offset, nr.length, &format!("_{name}"));
            let var_list = ast[field_decl].fields;
            if let Some(kw) = ast[var_list].keyword {
                if c.lexeme(kw) == "final" {
                    let r = c.range().token_start_start(kw, ast.tokens.next(kw));
                    b.add_simple_replacement(r.offset, r.length, "");
                }
            }
        }

        if let Some(np) = name_part.and_then(|n| ast.cast::<PrimaryConstructorDeclaration>(n)) {
            let body = class_members
                .iter()
                .find_map(|&m| ast.cast::<PrimaryConstructorBody>(m));
            update_references_in_constructor_for_encapsulate(
                c,
                b,
                field_element,
                &name,
                type_node,
                ast[np].formal_parameters,
                body.and_then(|bd| ast[bd].colon),
                body.map(|bd| ast[bd].initializers),
            );
        }
        for &member in &class_members {
            if let Some(ctor) = ast.cast::<ConstructorDeclaration>(member) {
                update_references_in_constructor_for_encapsulate(
                    c,
                    b,
                    field_element,
                    &name,
                    type_node,
                    ast[ctor].parameters,
                    ast[ctor].separator,
                    Some(ast[ctor].initializers),
                );
            }
        }

        let filter_node = field_decl_opt.map(|f| f.raw());
        let filter_fn = move |_: &Ast, m: NodeId| Some(m) == filter_node;
        let filter: Option<&dyn Fn(&Ast, NodeId) -> bool> = if filter_node.is_some() {
            Some(&filter_fn)
        } else {
            None
        };
        b.insert_into_unit_member(interface_decl, filter, false, |e| {
            let write_header = |e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
                                preserve_override: bool| {
                if let Some(doc) = &doc_code {
                    e.write_indent(1);
                    e.writeln(doc);
                }
                for (is_override, text) in &annotations {
                    if !*is_override || preserve_override {
                        e.write_indent(1);
                        e.writeln(text);
                    }
                }
            };
            write_header(e, has_overridden_getter);
            e.write_indent(1);
            if let Some(ty) = field_ty {
                if !matches!(ctx.ty(ty), TypeKind::Dynamic) {
                    if e.write_type(Some(ty), &WriteType::default()) {
                        e.write(" ");
                    }
                }
            }
            e.write(&format!("get {name} => _{name};"));
            if !is_final {
                e.newline();
                e.newline();
                write_header(e, has_overridden_setter);
                e.write_indent(1);
                e.write(&format!("set {name}("));
                if let Some(ty) = field_ty {
                    if !matches!(ctx.ty(ty), TypeKind::Dynamic) {
                        if e.write_type(Some(ty), &WriteType::default()) {
                            e.write(" ");
                        }
                    }
                }
                e.writeln("value) {");
                e.write_indent(2);
                e.writeln(&format!("_{name} = value;"));
                e.write_indent(1);
                e.write("}");
            }
        });
    });
});

// ---------------------------------------------------------------------------
// ConvertToDotShorthand & AddTypeName
// ---------------------------------------------------------------------------

fn context_type_matches_type_element(
    c: &ProducerContext<'_>,
    node: NodeId,
    type_element: Option<ElementId>,
) -> bool {
    let Some(type_element) = type_element else {
        return false;
    };
    let req = crate::completion::relevance::ContextInput {
        ctx: c.ctx,
        ast: c.ast,
        tables: c.tables,
    };
    let Some(ctx_ty) =
        crate::completion::relevance::compute_context_type(&req, node, c.ast.offset(node))
    else {
        return false;
    };
    match *c.ctx.ty(ctx_ty) {
        TypeKind::Interface { element, .. } => element.raw() == type_element,
        _ => false,
    }
}

assist!(
    ConvertToDotShorthand,
    a::CONVERT_TO_DOT_SHORTHAND,
    |c, builder| {
        if !c.ctx.features.is_enabled("dot-shorthands") {
            return;
        }
        let ast = c.ast;
        let Some(parent) = ast.parent(c.node) else {
            return;
        };
        let ctor_name = if let Some(cn) = ast.cast::<ConstructorName>(parent) {
            Some(cn)
        } else if let Some(nt) = ast.cast::<NamedType>(parent) {
            ast.parent(nt).and_then(|p| ast.cast::<ConstructorName>(p))
        } else {
            None
        };
        if let Some(cn) = ctor_name {
            let named_type = ast[cn].type_;
            let type_elem = c.locator().element(named_type);
            if !context_type_matches_type_element(c, cn.raw(), type_elem) {
                return;
            }
            if ast[named_type].type_arguments.is_some() {
                return;
            }
            let r = c.range().node(named_type);
            if ast[cn].name.is_some() {
                builder.add_dart_file_edit(c.path, |b| b.add_deletion(r.offset, r.length));
            } else {
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(r.offset, r.length, ".new");
                });
            }
        } else if let Some(mi) = ast.cast::<MethodInvocation>(parent) {
            let Some(target) = ast[mi].target.and_then(|t| ast.cast::<SimpleIdentifier>(t)) else {
                return;
            };
            let target_elem = c.locator().element(target);
            if !context_type_matches_type_element(c, mi.raw(), target_elem) {
                return;
            }
            let r = c.range().node(target);
            builder.add_dart_file_edit(c.path, |b| b.add_deletion(r.offset, r.length));
        } else if let Some(pi) = ast.cast::<PrefixedIdentifier>(parent) {
            let prefix_id = ast[pi].prefix;
            let (prefix_node, prefix_elem) = if c
                .locator()
                .element(prefix_id)
                .is_some_and(|e| e.tag() == Tag::Prefix)
            {
                (pi.raw(), c.locator().element(ast[pi].identifier))
            } else {
                (prefix_id.raw(), c.locator().element(prefix_id))
            };
            if !context_type_matches_type_element(c, pi.raw(), prefix_elem) {
                return;
            }
            let r = c.range().node(prefix_node);
            builder.add_dart_file_edit(c.path, |b| b.add_deletion(r.offset, r.length));
        }
    }
);

assist!(AddTypeName, a::ADD_TYPE_NAME, |c, builder| {
    let ast = c.ast;
    let Some(parent) = ast.parent(c.node) else {
        return;
    };
    let add_name_for_element =
        |elem: Option<ElementId>, period: TokenId, b: &mut FileEditBuilder<'_, '_>| {
            let Some(elem) = elem else { return };
            let Some(enclosing) = c.ctx.element_data(elem).and_then(|d| d.enclosing) else {
                return;
            };
            if !matches!(
                enclosing.tag(),
                Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
            ) {
                return;
            }
            let Some(name) = c.ctx.element_name(enclosing) else {
                return;
            };
            b.add_simple_insertion(c.token_offset(period), name);
        };
    if let Some(dpa) = ast.cast::<DotShorthandPropertyAccess>(parent) {
        let elem = c.locator().element(ast[dpa].property_name);
        let period = ast[dpa].period;
        builder.add_dart_file_edit(c.path, |b| add_name_for_element(elem, period, b));
    } else if let Some(di) = ast.cast::<DotShorthandInvocation>(parent) {
        let elem = c.locator().element(ast[di].member_name);
        let period = ast[di].period;
        builder.add_dart_file_edit(c.path, |b| add_name_for_element(elem, period, b));
    } else if let Some(dci) = ast.cast::<DotShorthandConstructorInvocation>(parent) {
        let elem = c.locator().element(dci);
        let Some(elem) = elem else { return };
        let Some(enclosing) = c.ctx.element_data(elem).and_then(|d| d.enclosing) else {
            return;
        };
        if !matches!(
            enclosing.tag(),
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType
        ) {
            return;
        }
        let Some(class_name) = c.ctx.element_name(enclosing) else {
            return;
        };
        let ctor_name = ast[dci].constructor_name;
        if c.lexeme(ast[ctor_name].token) == "new" {
            let r = c
                .range()
                .token_start_end(ast[dci].period, ast[ctor_name].token);
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_replacement(r.offset, r.length, class_name);
            });
        } else {
            let period = ast[dci].period;
            builder.add_dart_file_edit(c.path, |b| {
                b.add_simple_insertion(c.token_offset(period), class_name);
            });
        }
    }
});

// ---------------------------------------------------------------------------
// ConvertFieldFormalToNormal
// ---------------------------------------------------------------------------

assist!(
    ConvertFieldFormalToNormal,
    a::CONVERT_FIELD_FORMAL_TO_NORMAL,
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        let Some(parameter) = ast.this_or_ancestor_of_type::<FieldFormalParameter>(c.node) else {
            return;
        };
        let Some(constructor) = ast.this_or_ancestor_of_type::<ConstructorDeclaration>(parameter)
        else {
            return;
        };
        let Some(param_elem) = c
            .locator()
            .declared_element(parameter)
            .and_then(|e| e.cast::<FormalParameterElement>())
        else {
            return;
        };
        let ty = ctx.get(param_elem).type_.get();
        let this_kw = ast[parameter].this_keyword;
        let period = ast[parameter].period;
        let name = c.lexeme(ast[parameter].name).to_string();
        let inits = ast.list_raw(ast[constructor].initializers);
        let (init_offset, prefix) = if inits.is_empty() {
            (ast.end(ast[constructor].parameters), " : ")
        } else {
            (ast.end(*inits.last().unwrap()), ", ")
        };
        let r = c.range().token_start_end(this_kw, period);
        builder.add_dart_file_edit(c.path, |b| {
            b.add_replacement(r.offset, r.length, |e| {
                if ast[parameter].type_.is_none()
                    && ast[parameter].function_typed_suffix.is_none()
                    && !matches!(ty.map(|t| ctx.ty(t)), Some(TypeKind::Dynamic) | None)
                {
                    if e.write_type(ty, &WriteType::default()) {
                        e.write(" ");
                    }
                }
            });
            b.add_simple_insertion(init_offset, &format!("{prefix}{name} = {name}"));
        });
    }
);

// ---------------------------------------------------------------------------
// ConvertToDeclaringParameter
// ---------------------------------------------------------------------------

assist!(
    ConvertToDeclaringParameter,
    a::CONVERT_TO_DECLARING_PARAMETER,
    |c, builder| {
        if !c.ctx.features.is_enabled("primary-constructors") {
            return;
        }
        let ast = c.ast;
        let Some(parameter) = ast.this_or_ancestor_of_type::<FieldFormalParameter>(c.node) else {
            return;
        };
        let Some(primary) =
            ast.this_or_ancestor_of_type::<PrimaryConstructorDeclaration>(parameter)
        else {
            return;
        };
        let Some(container) = ast.parent(primary) else {
            return;
        };
        let is_enum = ast.is::<EnumDeclaration>(container);
        if !ast.is::<ClassDeclaration>(container) && !is_enum {
            return;
        }
        let Some(param_elem) = c
            .locator()
            .declared_element(parameter)
            .and_then(|e| e.cast::<FormalParameterElement>())
        else {
            return;
        };
        let Some(field_elem) = c.ctx.get(param_elem).field.get() else {
            return;
        };
        let members = super::super::insert::members_of(ast, container).unwrap_or_default();
        let mut target_field_decl = None;
        let mut target_var_list = None;
        for &m in &members {
            let Some(fd) = ast.cast::<FieldDeclaration>(m) else {
                continue;
            };
            let vl = ast[fd].fields;
            for &v in ast.list(ast[vl].variables) {
                if c.locator().declared_element(v) == Some(field_elem.raw()) {
                    if ast.list(ast[vl].variables).len() != 1
                        || ast[vl].late_keyword.is_some()
                        || ast[fd].external_keyword.is_some()
                        || ast[fd].abstract_keyword.is_some()
                        || ast[fd].covariant_keyword.is_some()
                        || ast[v].initializer.is_some()
                    {
                        return;
                    }
                    target_field_decl = Some(fd);
                    target_var_list = Some(vl);
                    break;
                }
            }
        }
        let (Some(field_decl), Some(var_list)) = (target_field_decl, target_var_list) else {
            return;
        };
        let keyword = if is_enum {
            "final"
        } else {
            ast[var_list].keyword.map(|k| c.lexeme(k)).unwrap_or("var")
        };
        let field_type = ast[var_list].type_.map(|t| c.utils.get_node_text(t));
        let param_has_type = ast[parameter].type_.is_some();
        let r_this = c
            .range()
            .token_start_end(ast[parameter].this_keyword, ast[parameter].period);
        let r_field = c
            .utils
            .get_lines_range(c.range().node(field_decl.raw()), false);
        builder.add_dart_file_edit(c.path, |b| {
            if let Some(kw) = ast[parameter].const_final_or_var_keyword {
                let r = c.range().token(kw);
                b.add_simple_replacement(r.offset, r.length, keyword);
                if !param_has_type {
                    if let Some(ft) = &field_type {
                        b.add_simple_replacement(r_this.offset, r_this.length, &format!("{ft} "));
                    } else {
                        b.add_deletion(r_this.offset, r_this.length);
                    }
                } else {
                    b.add_deletion(r_this.offset, r_this.length);
                }
            } else {
                let replacement = if !param_has_type {
                    if let Some(ft) = &field_type {
                        format!("{keyword} {ft} ")
                    } else {
                        format!("{keyword} ")
                    }
                } else {
                    format!("{keyword} ")
                };
                b.add_simple_replacement(r_this.offset, r_this.length, &replacement);
            }
            b.add_deletion(r_field.offset, r_field.length);
        });
    }
);

// ---------------------------------------------------------------------------
// ConvertToInBodyConstructor
// ---------------------------------------------------------------------------

fn is_declaring_param(c: &ProducerContext<'_>, param: NodeId) -> bool {
    let ast = c.ast;
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(param) {
        ast[rfp].const_final_or_var_keyword.is_some_and(|k| {
            let l = c.lexeme(k);
            l == "final" || l == "var"
        })
    } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(param) {
        ast[ffp]
            .const_final_or_var_keyword
            .is_some_and(|k| c.lexeme(k) == "final")
    } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(param) {
        ast[sfp]
            .const_final_or_var_keyword
            .is_some_and(|k| c.lexeme(k) == "final")
    } else {
        false
    }
}

fn is_final_param(c: &ProducerContext<'_>, param: NodeId) -> bool {
    let ast = c.ast;
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(param) {
        ast[rfp]
            .const_final_or_var_keyword
            .is_some_and(|k| c.lexeme(k) == "final")
    } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(param) {
        ast[ffp]
            .const_final_or_var_keyword
            .is_some_and(|k| c.lexeme(k) == "final")
    } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(param) {
        ast[sfp]
            .const_final_or_var_keyword
            .is_some_and(|k| c.lexeme(k) == "final")
    } else {
        false
    }
}

fn param_name_token(ast: &Ast, param: NodeId) -> Option<TokenId> {
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(param) {
        ast[rfp].name
    } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(param) {
        Some(ast[ffp].name)
    } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(param) {
        Some(ast[sfp].name)
    } else {
        None
    }
}

fn param_kind(ast: &Ast, param: NodeId) -> ParameterKind {
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(param) {
        ast[rfp].kind
    } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(param) {
        ast[ffp].kind
    } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(param) {
        ast[sfp].kind
    } else {
        ParameterKind::Required
    }
}

fn has_function_typed_suffix(ast: &Ast, param: NodeId) -> bool {
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(param) {
        ast[rfp].function_typed_suffix.is_some()
    } else if let Some(ffp) = ast.cast::<FieldFormalParameter>(param) {
        ast[ffp].function_typed_suffix.is_some()
    } else if let Some(sfp) = ast.cast::<SuperFormalParameter>(param) {
        ast[sfp].function_typed_suffix.is_some()
    } else {
        false
    }
}

fn fields_referencing_parameters(
    c: &ProducerContext<'_>,
    members: &[NodeId],
    param_elements: &[ElementId],
) -> Vec<Id<VariableDeclaration>> {
    let ast = c.ast;
    let mut result = Vec::new();
    for &m in members {
        let Some(fd) = ast.cast::<FieldDeclaration>(m) else {
            continue;
        };
        for &v in ast.list(ast[ast[fd].fields].variables) {
            let Some(init) = ast[v].initializer else {
                continue;
            };
            let mut has_ref = false;
            super::variables::visit(ast, init.raw(), &mut |n| {
                if let Some(sid) = ast.cast::<SimpleIdentifier>(n) {
                    if let Some(e) = c.locator().element(sid) {
                        if param_elements.contains(&e) {
                            has_ref = true;
                        }
                    }
                }
            });
            if has_ref {
                result.push(v);
            }
        }
    }
    result
}

fn offset_for_constructor(
    c: &ProducerContext<'_>,
    members: &[NodeId],
    default_offset: u32,
    is_named: bool,
) -> u32 {
    let ast = c.ast;
    let style = super::super::code_style::CodeStyleOptions {
        options: &c.options,
    };
    if style.sort_constructors_first() {
        return members
            .iter()
            .rev()
            .find(|&&m| ast.is::<ConstructorDeclaration>(m))
            .map(|&m| ast.end(m))
            .unwrap_or(default_offset);
    }
    if !is_named && style.sort_unnamed_constructors_first() {
        return members
            .iter()
            .rev()
            .find(|&&m| {
                ast.cast::<ConstructorDeclaration>(m)
                    .is_some_and(|cd| ast[cd].name.is_none())
                    || ast.is::<FieldDeclaration>(m)
            })
            .map(|&m| ast.end(m))
            .unwrap_or(default_offset);
    }
    if is_named && style.sort_unnamed_constructors_first() {
        return members
            .iter()
            .rev()
            .find(|&&m| {
                ast.cast::<ConstructorDeclaration>(m)
                    .is_some_and(|cd| ast[cd].name.is_none())
            })
            .map(|&m| ast.end(m))
            .unwrap_or(default_offset);
    }
    let mut prev: Option<NodeId> = None;
    for &m in members {
        if ast.is::<ConstructorDeclaration>(m) {
            return prev.map(|p| ast.end(p)).unwrap_or(default_offset);
        }
        prev = Some(m);
    }
    default_offset
}

fn write_implicitly_declared_fields(
    c: &ProducerContext<'_>,
    e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
    param_list: Id<FormalParameterList>,
    needs_blank_line: bool,
) {
    let ast = c.ast;
    let params = ast.list_raw(ast[param_list].parameters);
    if needs_blank_line && !params.is_empty() {
        e.newline();
    }
    for &p in params {
        if is_declaring_param(c, p) {
            let Some(name_tok) = param_name_token(ast, p) else {
                continue;
            };
            e.newline();
            e.write("  ");
            if is_final_param(c, p) {
                e.write("final ");
            }
            let ty = c
                .locator()
                .declared_element(p)
                .map(|pe| dartr_resolver::element_ext::variable_type(c.ctx, pe));
            e.write_type(ty, &WriteType::default());
            e.write(" ");
            e.write(c.lexeme(name_tok));
            e.writeln(";");
        }
    }
}

fn write_name_and_parameter_list(
    c: &ProducerContext<'_>,
    e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
    declaration: Id<PrimaryConstructorDeclaration>,
) {
    let ast = c.ast;
    let param_list = ast[declaration].formal_parameters;
    let parent_is_enum = ast
        .parent(declaration)
        .is_some_and(|p| ast.is::<EnumDeclaration>(p));
    if ast[declaration].const_keyword.is_some() && !parent_is_enum {
        e.write("const ");
    }
    e.write("new");
    if let Some(cn) = ast[declaration].constructor_name {
        e.write(" ");
        e.write(c.lexeme(ast[cn].name));
    }
    e.write("(");
    let mut needs_comma = false;
    let mut group_end: Option<&str> = None;
    for &p in ast.list_raw(ast[param_list].parameters) {
        if needs_comma {
            e.write(", ");
        } else {
            needs_comma = true;
        }
        let kind = param_kind(ast, p);
        if group_end.is_none() {
            if kind == ParameterKind::Positional {
                e.write("[");
                group_end = Some("]");
            } else if matches!(kind, ParameterKind::Named | ParameterKind::NamedRequired) {
                e.write("{");
                group_end = Some("}");
            }
        }
        let (prefix, offset, end) = if is_declaring_param(c, p) {
            let name_tok = param_name_token(ast, p).unwrap();
            let pref = if kind == ParameterKind::NamedRequired {
                "required this."
            } else {
                "this."
            };
            let off = c.token_offset(name_tok);
            let end_off = if has_function_typed_suffix(ast, p) {
                c.token_end(name_tok)
            } else {
                ast.end(p)
            };
            (pref, off, end_off)
        } else {
            ("", ast.offset(p), ast.end(p))
        };
        let param_text = c.utils.get_text(offset, end - offset);
        e.write(prefix);
        e.write(&param_text);
    }
    if let Some(ge) = group_end {
        e.write(ge);
    }
    e.write(")");
}

fn write_referencing_fields(
    c: &ProducerContext<'_>,
    e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
    referencing_fields: &[Id<VariableDeclaration>],
) {
    let ast = c.ast;
    for (i, &field) in referencing_fields.iter().enumerate() {
        if i > 0 {
            e.write(", ");
        }
        let r = c
            .range()
            .token_start_node_end(ast[field].name, ast[field].initializer.unwrap());
        e.write(&c.utils.get_range_text(r));
    }
}

fn write_full_in_body_constructor(
    c: &ProducerContext<'_>,
    e: &mut super::super::change_builder::EditBuilder<'_, '_, '_>,
    declaration: Id<PrimaryConstructorDeclaration>,
    referencing_fields: &[Id<VariableDeclaration>],
    body: Option<Id<PrimaryConstructorBody>>,
) {
    let ast = c.ast;
    e.newline();
    e.write("  ");
    write_name_and_parameter_list(c, e, declaration);
    if let Some(body) = body {
        if let Some(colon) = ast[body].colon {
            if let Some(&last_init) = ast.list_raw(ast[body].initializers).last() {
                let r = c.range().token_start_node_end(colon, last_init);
                e.write(&c.utils.get_range_text(r));
            }
            if !referencing_fields.is_empty() {
                e.write(", ");
                write_referencing_fields(c, e, referencing_fields);
            }
        } else if !referencing_fields.is_empty() {
            e.write(" : ");
            write_referencing_fields(c, e, referencing_fields);
        }
        e.write(&c.utils.get_node_text(ast[body].body));
    } else {
        if !referencing_fields.is_empty() {
            e.write(" : ");
            write_referencing_fields(c, e, referencing_fields);
        }
        e.write(";");
    }
}

fn remove_primary_constructor(
    c: &ProducerContext<'_>,
    b: &mut FileEditBuilder<'_, '_>,
    declaration: Id<PrimaryConstructorDeclaration>,
) {
    let ast = c.ast;
    if let Some(ck) = ast[declaration].const_keyword {
        let r = c.range().token_start_start(ck, ast.tokens.next(ck));
        b.add_deletion(r.offset, r.length);
    }
    let param_list = ast[declaration].formal_parameters;
    if let Some(cn) = ast[declaration].constructor_name {
        let r = c.range().token_start_node_end(ast[cn].period, param_list);
        b.add_deletion(r.offset, r.length);
    } else {
        let r = c.range().node(param_list);
        b.add_deletion(r.offset, r.length);
    }
}

fn remove_field_initializers(
    c: &ProducerContext<'_>,
    b: &mut FileEditBuilder<'_, '_>,
    referencing_fields: &[Id<VariableDeclaration>],
) {
    let ast = c.ast;
    for &field in referencing_fields {
        let r = c
            .range()
            .token_end_node_end(ast[field].name, ast[field].initializer.unwrap());
        b.add_deletion(r.offset, r.length);
    }
}

assist!(
    ConvertToInBodyConstructor,
    a::CONVERT_TO_IN_BODY_CONSTRUCTOR,
    |c, builder| {
        let ast = c.ast;
        let mut node = c.node;
        if ast.is::<PrimaryConstructorName>(node) {
            let Some(p) = ast.parent(node) else {
                return;
            };
            node = p;
        }
        let Some(declaration) = ast.cast::<PrimaryConstructorDeclaration>(node) else {
            return;
        };
        let Some(container) = ast.parent(declaration) else {
            return;
        };
        let param_list = ast[declaration].formal_parameters;
        let mut param_elements = Vec::new();
        for &p in ast.list_raw(ast[param_list].parameters) {
            if param_name_token(ast, p).is_none() {
                return;
            }
            let Some(pe) = c.locator().declared_element(p) else {
                return;
            };
            param_elements.push(pe);
        }

        if let Some(cd) = ast.cast::<ClassDeclaration>(container) {
            let body_node = ast[cd].body.raw();
            if let Some(bcb) = ast.cast::<BlockClassBody>(body_node) {
                let members = ast.list_raw(ast[bcb].members);
                let primary_body = members
                    .iter()
                    .find_map(|&m| ast.cast::<PrimaryConstructorBody>(m));
                let left_bracket = ast[bcb].left_bracket;
                let right_bracket = ast[bcb].right_bracket;
                let referencing_fields = fields_referencing_parameters(c, members, &param_elements);
                builder.add_dart_file_edit(c.path, |b| {
                    let field_offset = c.token_end(left_bracket);
                    if let Some(pb) = primary_body {
                        b.add_insertion(field_offset, |e| {
                            write_implicitly_declared_fields(c, e, param_list, false);
                        });
                        let this_r = c.range().token(ast[pb].this_keyword);
                        b.add_replacement(this_r.offset, this_r.length, |e| {
                            write_name_and_parameter_list(c, e, declaration);
                        });
                    } else {
                        let is_named = ast[declaration].constructor_name.is_some();
                        let ctor_offset =
                            offset_for_constructor(c, members, field_offset, is_named);
                        if ctor_offset == field_offset {
                            let sort_ctors_first = super::super::code_style::CodeStyleOptions {
                                options: &c.options,
                            }
                            .sort_constructors_first();
                            b.add_insertion(ctor_offset, |e| {
                                if sort_ctors_first {
                                    write_full_in_body_constructor(
                                        c,
                                        e,
                                        declaration,
                                        &referencing_fields,
                                        None,
                                    );
                                    write_implicitly_declared_fields(c, e, param_list, false);
                                } else {
                                    write_implicitly_declared_fields(c, e, param_list, false);
                                    write_full_in_body_constructor(
                                        c,
                                        e,
                                        declaration,
                                        &referencing_fields,
                                        None,
                                    );
                                }
                                if !members.is_empty()
                                    || c.token_end(left_bracket) == c.token_offset(right_bracket)
                                {
                                    e.newline();
                                }
                            });
                        } else {
                            b.add_insertion(field_offset, |e| {
                                write_implicitly_declared_fields(c, e, param_list, false);
                            });
                            b.add_insertion(ctor_offset, |e| {
                                if !members.is_empty() {
                                    e.newline();
                                }
                                write_full_in_body_constructor(
                                    c,
                                    e,
                                    declaration,
                                    &referencing_fields,
                                    None,
                                );
                                if ctor_offset == c.token_offset(right_bracket) {
                                    e.newline();
                                }
                            });
                        }
                    }
                    remove_primary_constructor(c, b, declaration);
                    remove_field_initializers(c, b, &referencing_fields);
                });
            } else if let Some(ecb) = ast.cast::<EmptyClassBody>(body_node) {
                let semi_r = c.range().token(ast[ecb].semicolon);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_replacement(semi_r.offset, semi_r.length, |e| {
                        e.write(" {");
                        write_implicitly_declared_fields(c, e, param_list, false);
                        write_full_in_body_constructor(c, e, declaration, &[], None);
                        e.newline();
                        e.write("}");
                    });
                    remove_primary_constructor(c, b, declaration);
                });
            }
        } else if let Some(ed) = ast.cast::<EnumDeclaration>(container) {
            let Some(beb) = ast.cast::<BlockEnumBody>(ast[ed].body) else {
                return;
            };
            let constants = ast.list(ast[beb].constants);
            if constants.is_empty() {
                return;
            }
            let members = ast.list_raw(ast[beb].members);
            let primary_body = members
                .iter()
                .find_map(|&m| ast.cast::<PrimaryConstructorBody>(m));
            let left_bracket = ast[beb].left_bracket;
            let right_bracket = ast[beb].right_bracket;
            let referencing_fields = fields_referencing_parameters(c, members, &param_elements);
            builder.add_dart_file_edit(c.path, |b| {
                let mut field_offset = c.token_end(left_bracket);
                let mut needs_semicolon = false;
                if let Some(semi) = ast[beb].semicolon {
                    field_offset = c.token_end(semi);
                } else {
                    needs_semicolon = true;
                    if let Some(&last_c) = constants.last() {
                        field_offset = ast.end(last_c);
                    }
                }
                let needs_blank_line = field_offset != c.token_end(left_bracket);
                if let Some(pb) = primary_body {
                    b.add_insertion(field_offset, |e| {
                        if needs_semicolon {
                            e.write(";");
                        }
                        write_implicitly_declared_fields(c, e, param_list, needs_blank_line);
                    });
                    let this_r = c.range().token(ast[pb].this_keyword);
                    b.add_replacement(this_r.offset, this_r.length, |e| {
                        write_name_and_parameter_list(c, e, declaration);
                    });
                } else {
                    let is_named = ast[declaration].constructor_name.is_some();
                    let ctor_offset = offset_for_constructor(c, members, field_offset, is_named);
                    if ctor_offset == field_offset {
                        let sort_ctors_first = super::super::code_style::CodeStyleOptions {
                            options: &c.options,
                        }
                        .sort_constructors_first();
                        b.add_insertion(ctor_offset, |e| {
                            if needs_semicolon {
                                e.write(";");
                            }
                            if sort_ctors_first {
                                write_full_in_body_constructor(
                                    c,
                                    e,
                                    declaration,
                                    &referencing_fields,
                                    None,
                                );
                                write_implicitly_declared_fields(
                                    c,
                                    e,
                                    param_list,
                                    needs_blank_line,
                                );
                            } else {
                                write_implicitly_declared_fields(
                                    c,
                                    e,
                                    param_list,
                                    needs_blank_line,
                                );
                                write_full_in_body_constructor(
                                    c,
                                    e,
                                    declaration,
                                    &referencing_fields,
                                    None,
                                );
                            }
                            if !members.is_empty()
                                || c.token_end(left_bracket) == c.token_offset(right_bracket)
                            {
                                e.newline();
                            }
                        });
                    } else {
                        b.add_insertion(field_offset, |e| {
                            if needs_semicolon {
                                e.write(";");
                            }
                            write_implicitly_declared_fields(c, e, param_list, needs_blank_line);
                        });
                        b.add_insertion(ctor_offset, |e| {
                            if !members.is_empty() {
                                e.newline();
                            }
                            write_full_in_body_constructor(
                                c,
                                e,
                                declaration,
                                &referencing_fields,
                                None,
                            );
                            if ctor_offset == c.token_offset(right_bracket) {
                                e.newline();
                            }
                        });
                    }
                }
                remove_primary_constructor(c, b, declaration);
                remove_field_initializers(c, b, &referencing_fields);
            });
        }
    }
);

// ---------------------------------------------------------------------------
// ConvertToPrimaryConstructor
// ---------------------------------------------------------------------------

producer!(
    ConvertToPrimaryConstructor,
    k::CONVERT_TO_PRIMARY_CONSTRUCTOR,
    Some(&k::CONVERT_TO_PRIMARY_CONSTRUCTOR_MULTI),
    Automatically,
    assist: Some(&a::CONVERT_TO_PRIMARY_CONSTRUCTOR),
    |c, builder| {
        if !c.ctx.features.is_enabled("primary-constructors") {
            return;
        }
        let ast = c.ast;
        let constructor = if let Some(cd) = ast.cast::<ConstructorDeclaration>(c.node) {
            cd
        } else if ast.is::<SimpleIdentifier>(c.node) {
            let Some(cd) = ast
                .parent(c.node)
                .and_then(|p| ast.cast::<ConstructorDeclaration>(p))
            else {
                return;
            };
            if ast[cd].type_name.map(|t| t.raw()) != Some(c.node) {
                return;
            }
            cd
        } else {
            return;
        };
        if ast[constructor].factory_keyword.is_some()
            || ast[constructor].external_keyword.is_some()
            || ast[constructor].redirected_constructor.is_some()
            || ast
                .list_raw(ast[constructor].initializers)
                .last()
                .is_some_and(|&i| ast.is::<RedirectingConstructorInvocation>(i))
        {
            return;
        }
        let Some(enclosing_body) = ast.parent(constructor) else {
            return;
        };
        let Some(container) = ast.parent(enclosing_body) else {
            return;
        };
        let (name_part, members, is_enum) = if let Some(cd) = ast.cast::<ClassDeclaration>(container)
        {
            let Some(b) = ast.cast::<BlockClassBody>(ast[cd].body) else {
                return;
            };
            (ast[cd].name_part, ast.list_raw(ast[b].members), false)
        } else if let Some(ed) = ast.cast::<EnumDeclaration>(container) {
            let Some(b) = ast.cast::<BlockEnumBody>(ast[ed].body) else {
                return;
            };
            (ast[ed].name_part, ast.list_raw(ast[b].members), true)
        } else {
            return;
        };
        if ast.is::<PrimaryConstructorDeclaration>(name_part) {
            return;
        }
        let non_redirecting_generative_count = members
            .iter()
            .filter(|&&m| {
                ast.cast::<ConstructorDeclaration>(m).is_some_and(|cd| {
                    ast[cd].factory_keyword.is_none()
                        && ast[cd].redirected_constructor.is_none()
                        && !ast
                            .list_raw(ast[cd].initializers)
                            .last()
                            .is_some_and(|&i| ast.is::<RedirectingConstructorInvocation>(i))
                })
            })
            .count();
        if non_redirecting_generative_count > 1 {
            return;
        }

        let type_name_tok = dartr_resolver::error::support::class_name_token(ast, name_part);
        let type_params = if let Some(nwtp) = ast.cast::<NameWithTypeParameters>(name_part) {
            ast[nwtp].type_parameters
        } else if let Some(pcd) = ast.cast::<PrimaryConstructorDeclaration>(name_part) {
            ast[pcd].type_parameters
        } else {
            None
        };
        let insert_offset = type_params
            .map(|tp| ast.end(tp))
            .unwrap_or_else(|| c.token_end(type_name_tok));

        let body_not_empty = match ast.cast::<BlockFunctionBody>(ast[constructor].body) {
            Some(bfb) => !ast.list_raw(ast[ast[bfb].block].statements).is_empty(),
            None => ast.is::<ExpressionFunctionBody>(ast[constructor].body.raw()),
        };
        let has_inits_or_body_or_meta = !ast.list_raw(ast[constructor].initializers).is_empty()
            || body_not_empty
            || ast[constructor].documentation_comment.is_some()
            || !ast.list(ast[constructor].metadata).is_empty();

        builder.add_dart_file_edit(c.path, |b| {
            if ast[constructor].const_keyword.is_some() && !is_enum {
                b.add_simple_insertion(c.token_offset(type_name_tok), "const ");
            }
            b.add_insertion(insert_offset, |e| {
                if let Some(name_tok) = ast[constructor].name {
                    e.write(".");
                    e.write(c.lexeme(name_tok));
                }
                e.write(&c.utils.get_node_text(ast[constructor].parameters));
            });
            if has_inits_or_body_or_meta {
                let r = c.range().token_start_end(
                    super::super::utils::first_token_after_comment_and_metadata(
                        ast,
                        constructor.raw(),
                    ),
                    ast.end_token(ast[constructor].parameters),
                );
                b.add_simple_replacement(r.offset, r.length, "this");
            } else {
                b.delete_class_member(constructor.raw());
            }
        });
    }
);

// ---------------------------------------------------------------------------
// MoveInitializationToFieldDeclaration
// ---------------------------------------------------------------------------

producer!(
    MoveInitializationToFieldDeclaration,
    k::MOVE_INITIALIZATION_TO_FIELD_DECLARATION,
    Some(&k::MOVE_INITIALIZATION_TO_FIELD_DECLARATION_MULTI),
    Automatically,
    assist: Some(&a::MOVE_INITIALIZATION_TO_FIELD_DECLARATION),
    |c, builder| {
        let ast = c.ast;
        let Some(id) = ast.cast::<SimpleIdentifier>(c.node) else {
            return;
        };
        let Some(init) = ast
            .parent(id)
            .and_then(|p| ast.cast::<ConstructorFieldInitializer>(p))
        else {
            return;
        };
        if ast[init].field_name != id {
            return;
        }
        let Some(field) = c.locator().element(id) else {
            return;
        };
        if field.tag() != Tag::Field {
            return;
        }
        let Some(ctor_decl) = ast.parent(init) else {
            return;
        };
        let initializers = if let Some(cd) = ast.cast::<ConstructorDeclaration>(ctor_decl) {
            ast.list_raw(ast[cd].initializers)
        } else if let Some(pcb) = ast.cast::<PrimaryConstructorBody>(ctor_decl) {
            ast.list_raw(ast[pcb].initializers)
        } else {
            return;
        };
        let Some(container) = ast.parent(ctor_decl) else {
            return;
        };
        let members = if let Some(cb) = ast.cast::<BlockClassBody>(container) {
            ast.list_raw(ast[cb].members)
        } else if let Some(eb) = ast.cast::<BlockEnumBody>(container) {
            ast.list_raw(ast[eb].members)
        } else {
            return;
        };
        let mut target_var: Option<Id<VariableDeclaration>> = None;
        for &m in members {
            let Some(fd) = ast.cast::<FieldDeclaration>(m) else {
                continue;
            };
            for &v in ast.list(ast[ast[fd].fields].variables) {
                if c.locator().declared_element(v) == Some(field) {
                    target_var = Some(v);
                    break;
                }
            }
            if target_var.is_some() {
                break;
            }
        }
        let Some(field_decl) = target_var else {
            return;
        };
        if ast[field_decl].equals.is_some() || ast[field_decl].initializer.is_some() {
            return;
        }
        let expr_text = c.utils.get_node_text(ast[init].expression);
        let del_r = c.range().node_in_list(initializers, init.raw());
        builder.add_dart_file_edit(c.path, |b| {
            b.add_deletion(del_r.offset, del_r.length);
            b.add_simple_insertion(ast.end(field_decl), &format!(" = {expr_text}"));
        });
    }
);
