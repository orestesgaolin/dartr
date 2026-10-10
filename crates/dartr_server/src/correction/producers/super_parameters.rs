// Dart source: pkg/analysis_server/lib/src/services/correction/dart/convert_to_super_parameters.dart
// Dart source: pkg/analysis_server/lib/src/utilities/extensions/range_factory.dart (nodesInList)

//! Dart `ConvertToSuperParameters` (for constructor declarations; primary
//! constructors are not ported).

use dartr_ast::*;
use dartr_element::{EId, ElementId, FormalParameterElement, TypeId};
use dartr_syntax::TokenId;

use super::super::change_builder::ChangeBuilder;
use super::super::generated::fix_kinds as k;
use super::super::producer::*;
use super::super::utils::Range;
use super::simple::producer;
use super::variables::visit;

struct ParameterData {
    final_keyword: Option<TokenId>,
    /// Dart `_TypeData`: the primary range and the parameter range.
    type_to_delete: Option<(Option<Range>, Option<Range>)>,
    name: TokenId,
    null_initializer: bool,
    default_value_range: Option<Range>,
    parameter_index: usize,
    argument_index: usize,
}

/// Dart `nodesInList`.
fn nodes_in_list(c: &ProducerContext<'_>, list: &[NodeId], indexes: &[usize]) -> Vec<Range> {
    let r = c.range();
    // Dart `IndexRange.contiguousSubRanges`.
    let mut sub: Vec<(usize, usize)> = Vec::new();
    for &i in indexes {
        match sub.last_mut() {
            Some(last) if last.1 + 1 == i => last.1 = i,
            _ => sub.push((i, i)),
        }
    }
    if sub.len() == 1 && sub[0].0 == 0 && sub[0].1 == list.len() - 1 {
        return vec![r.start_end(list[0], list[list.len() - 1])];
    }
    sub.iter()
        .map(|&(lower, upper)| {
            if lower == 0 {
                r.start_start(list[lower], list[upper + 1])
            } else {
                r.end_end(list[lower - 1], list[upper])
            }
        })
        .collect()
}

producer!(
    ConvertToSuperParameters,
    k::CONVERT_TO_SUPER_PARAMETERS,
    Some(&k::CONVERT_TO_SUPER_PARAMETERS_MULTI),
    Automatically,
    assist: Some(&crate::correction::generated::assist_kinds::CONVERT_TO_SUPER_PARAMETERS),
    |c, builder| {
        let ast = c.ast;
        let ctx = c.ctx;
        if !ctx.features.is_enabled("super-parameters") {
            return;
        }
        // Dart `_findConstructor`.
        let node = c.node;
        let constructor = if let Some(cd) = ast.cast::<ConstructorDeclaration>(node) {
            Some(cd)
        } else if ast.is::<SimpleIdentifier>(node) {
            let parent = ast.parent(node);
            parent
                .and_then(|p| ast.cast::<ConstructorDeclaration>(p))
                .or_else(|| {
                    parent
                        .filter(|p| ast.is::<ConstructorName>(*p))
                        .and_then(|p| ast.parent(p))
                        .and_then(|g| ast.cast::<ConstructorDeclaration>(g))
                })
        } else {
            None
        };
        let Some(constructor) = constructor else {
            return;
        };
        let initializers: Vec<NodeId> = ast.list_raw(ast[constructor].initializers).to_vec();
        let Some(super_invocation) = initializers
            .iter()
            .rev()
            .find_map(|i| ast.cast::<SuperConstructorInvocation>(*i))
        else {
            return;
        };
        if c.element_of(super_invocation.raw()).is_none() {
            return;
        }
        // Dart `referencedParameters`.
        let mut referenced: Vec<ElementId> = Vec::new();
        visit(ast, ast[constructor].body.raw(), &mut |n| {
            if let Some(id) = ast.cast::<SimpleIdentifier>(n) {
                if let Some(e) = c.locator().element(id) {
                    if e.cast::<FormalParameterElement>().is_some() {
                        referenced.push(e);
                    }
                }
            }
        });
        // Dart `_parameterMap`.
        let parameters: Vec<NodeId> = ast
            .list_raw(ast[ast[constructor].parameters].parameters)
            .to_vec();
        let mut parameter_map: Vec<(ElementId, NodeId, usize)> = Vec::new();
        for (i, &p) in parameters.iter().enumerate() {
            if ast.is::<RegularFormalParameter>(p) {
                if let Some(e) = c.locator().declared_element(p) {
                    parameter_map.push((e, p, i));
                }
            }
        }
        let parameter_for = |expression: NodeId| -> Option<(ElementId, NodeId, usize)> {
            let id = ast.cast::<SimpleIdentifier>(expression)?;
            let e = c.locator().element(id)?;
            parameter_map.iter().find(|(x, _, _)| *x == e).copied()
        };
        let ts = dartr_typesystem::type_system::TypeSystem::new(*ctx);
        let data_for = |parameter: (ElementId, NodeId, usize),
                        argument_index: usize,
                        argument_expression: NodeId|
         -> Option<ParameterData> {
            let super_parameter = c.tables.param_element.get(argument_expression).copied()?;
            let super_type: TypeId = dartr_typesystem::member::type_(ctx, super_parameter);
            let this_type: TypeId = dartr_resolver::element_ext::variable_type(ctx, parameter.0);
            if !ts.is_subtype_of(this_type, super_type) {
                return None;
            }
            let p = ast.cast::<RegularFormalParameter>(parameter.1)?;
            let name = ast[p].name?;
            let super_element = dartr_typesystem::member::base_element(ctx, super_parameter);
            let super_default = super_element.cast::<FormalParameterElement>().and_then(
                |e: EId<FormalParameterElement>| {
                    dartr_element::display_string::default_value_code(ctx, e)
                },
            );
            let default_value_range = ast[p].default_clause.and_then(|d| {
                let this_default = c.utils.get_node_text(ast[d].value);
                (super_default.as_deref() == Some(this_default.as_str()))
                    .then(|| c.range().token_end_node_end(name, ast[d].value))
            });
            let final_keyword = ast[p]
                .const_final_or_var_keyword
                .filter(|k| c.lexeme(*k) == "final");
            let is_required = matches!(
                ast[p].kind,
                ParameterKind::Required | ParameterKind::NamedRequired
            );
            let null_initializer =
                !is_required && ast[p].default_clause.is_none() && super_default.is_some();
            let type_to_delete = if super_type == this_type {
                if let Some(suffix) = ast[p].function_typed_suffix {
                    let primary = ast[p]
                        .type_
                        .map(|t| c.range().node_start_token_start(t, name));
                    let start: NodeId = match ast[suffix].type_parameters {
                        Some(tp) => tp.raw(),
                        None => ast[suffix].formal_parameters.raw(),
                    };
                    Some((primary, Some(c.range().start_end(start, suffix))))
                } else {
                    ast[p]
                        .type_
                        .map(|t| (Some(c.range().node_start_token_start(t, name)), None))
                }
            } else {
                None
            };
            Some(ParameterData {
                final_keyword,
                type_to_delete,
                name,
                null_initializer,
                default_value_range,
                parameter_index: parameter.2,
                argument_index,
            })
        };
        let argument_list = ast[super_invocation].argument_list;
        let arguments: Vec<NodeId> = ast.list_raw(ast[argument_list].arguments).to_vec();
        let mut positional: Option<Vec<ParameterData>> = Some(Vec::new());
        let mut named: Vec<ParameterData> = Vec::new();
        for (index, &argument) in arguments.iter().enumerate() {
            if let Some(n) = ast.cast::<NamedArgument>(argument) {
                let expression = ast[n].argument_expression.raw();
                if let Some(parameter) = parameter_for(expression) {
                    let element = parameter.0;
                    let is_named = ast
                        .cast::<RegularFormalParameter>(parameter.1)
                        .is_some_and(|p| !ast[p].kind.is_positional());
                    let name_matches = ast
                        .cast::<RegularFormalParameter>(parameter.1)
                        .and_then(|p| ast[p].name)
                        .is_some_and(|t| c.lexeme(t) == c.lexeme(ast[n].name));
                    if is_named && name_matches && !referenced.contains(&element) {
                        if let Some(data) = data_for(parameter, index, expression) {
                            named.push(data);
                        }
                    }
                }
            } else if positional.is_some() {
                let parameter = parameter_for(argument);
                let ok = parameter.is_some_and(|p| {
                    ast.cast::<RegularFormalParameter>(p.1)
                        .is_some_and(|x| ast[x].kind.is_positional())
                        && !referenced.contains(&p.0)
                });
                if !ok {
                    positional = None;
                } else {
                    match data_for(parameter.unwrap(), index, argument) {
                        Some(d) => positional.as_mut().unwrap().push(d),
                        None => positional = None,
                    }
                }
            }
        }
        if let Some(p) = &positional {
            let mut previous = 0;
            let mut in_order = true;
            for d in p {
                if d.parameter_index < previous {
                    in_order = false;
                }
                previous = d.parameter_index;
            }
            if !in_order {
                positional = None;
            }
        }
        if positional.as_ref().is_none_or(|p| p.is_empty()) && named.is_empty() {
            return;
        }
        let all: Vec<ParameterData> = positional
            .unwrap_or_default()
            .into_iter()
            .chain(named)
            .collect();
        let mut to_delete: Vec<usize> = all.iter().map(|d| d.argument_index).collect();
        to_delete.sort();
        let r = c.range();
        let delete_all = to_delete.len() == arguments.len();
        let invocation_range = if delete_all {
            if ast[super_invocation].constructor_name.is_none() {
                Some(r.node_in_list(&initializers, super_invocation.raw()))
            } else {
                Some(r.token_end_start(
                    ast[argument_list].left_parenthesis,
                    ast[argument_list].right_parenthesis,
                ))
            }
        } else {
            None
        };
        let argument_ranges = if delete_all {
            Vec::new()
        } else {
            nodes_in_list(c, &arguments, &to_delete)
        };
        builder.add_dart_file_edit(c.path, |b| {
            for d in &all {
                let name_offset = c.token_offset(d.name);
                let insert_super = |b: &mut super::super::change_builder::FileEditBuilder<
                    '_,
                    '_,
                >| match d.final_keyword {
                    None => b.add_simple_insertion(name_offset, "super."),
                    Some(keyword) => {
                        let after = ast.tokens.next(keyword);
                        let range = r.token_start_start(keyword, after);
                        if c.token_offset(after) == name_offset {
                            b.add_simple_replacement(range.offset, range.length, "super.");
                        } else {
                            b.add_deletion(range.offset, range.length);
                            b.add_simple_insertion(name_offset, "super.");
                        }
                    }
                };
                match &d.type_to_delete {
                    None => insert_super(b),
                    Some((primary, parameter_range)) => {
                        match primary {
                            None => b.add_simple_insertion(name_offset, "super."),
                            Some(primary) => match d.final_keyword {
                                None => b.add_simple_replacement(
                                    primary.offset,
                                    primary.length,
                                    "super.",
                                ),
                                Some(keyword) => {
                                    let after = ast.tokens.next(keyword);
                                    if c.token_offset(after) == primary.offset {
                                        let start = c.token_offset(keyword);
                                        b.add_simple_replacement(
                                            start,
                                            primary.end() - start,
                                            "super.",
                                        );
                                    } else {
                                        let range = r.token_start_start(keyword, after);
                                        b.add_deletion(range.offset, range.length);
                                        b.add_simple_replacement(
                                            primary.offset,
                                            primary.length,
                                            "super.",
                                        );
                                    }
                                }
                            },
                        }
                        if d.null_initializer {
                            b.add_simple_insertion(c.token_end(d.name), " = null");
                        }
                        if let Some(pr) = parameter_range {
                            b.add_deletion(pr.offset, pr.length);
                        }
                    }
                }
                if let Some(dr) = d.default_value_range {
                    b.add_deletion(dr.offset, dr.length);
                }
            }
            if let Some(range) = invocation_range {
                b.add_deletion(range.offset, range.length);
            }
            for range in &argument_ranges {
                b.add_deletion(range.offset, range.length);
            }
        });
    }
);
