// Dart source: pkg/analysis_server/lib/src/computer/computer_selection_ranges.dart

//! Selection ranges (Dart `DartSelectionRangeComputer`).

use dartr_ast::*;

/// Dart `SelectionRange` of the computer: an offset and a length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionRange {
    pub offset: u32,
    pub length: u32,
}

/// Dart `DartSelectionRangeComputer(unit, offset).compute()`: the ranges of
/// the nodes that contain [offset], innermost first.
pub fn compute_selection_ranges(
    ast: &Ast,
    unit: Id<CompilationUnit>,
    offset: u32,
) -> Vec<SelectionRange> {
    let mut ranges = Vec::new();
    let Some(mut node) = ast.node_covering(unit, offset, 0) else {
        return ranges;
    };
    while node != unit.raw() {
        record_range(ast, node, offset, &mut ranges);
        match ast.parent(node) {
            Some(p) => node = p,
            None => break,
        }
    }
    ranges
}

fn record_offset_length(ranges: &mut Vec<SelectionRange>, offset: u32, length: u32) {
    if let Some(last) = ranges.last() {
        if last.offset == offset && last.length == length {
            return;
        }
    }
    ranges.push(SelectionRange { offset, length });
}

struct ParameterParts {
    default_clause: Option<Id<FormalParameterDefaultClause>>,
    function_typed_suffix: Option<Id<FunctionTypedFormalParameterSuffix>>,
    name: Option<dartr_syntax::TokenId>,
    type_: Option<Id<TypeAnnotation>>,
    const_final_or_var_keyword: Option<dartr_syntax::TokenId>,
    required_keyword: Option<dartr_syntax::TokenId>,
    covariant_keyword: Option<dartr_syntax::TokenId>,
}

fn parameter_parts(ast: &Ast, node: NodeId) -> Option<ParameterParts> {
    macro_rules! parts {
        ($n:expr, $name:expr) => {{
            let n = $n;
            ParameterParts {
                default_clause: n.default_clause,
                function_typed_suffix: n.function_typed_suffix,
                name: $name,
                type_: n.type_,
                const_final_or_var_keyword: n.const_final_or_var_keyword,
                required_keyword: n.required_keyword,
                covariant_keyword: n.covariant_keyword,
            }
        }};
    }
    if let Some(p) = ast.cast::<RegularFormalParameter>(node) {
        let n = &ast[p];
        return Some(parts!(n, n.name));
    }
    if let Some(p) = ast.cast::<FieldFormalParameter>(node) {
        let n = &ast[p];
        return Some(parts!(n, Some(n.name)));
    }
    if let Some(p) = ast.cast::<SuperFormalParameter>(node) {
        let n = &ast[p];
        return Some(parts!(n, Some(n.name)));
    }
    None
}

/// Dart `_formalParameterEndWithoutDefault`.
fn formal_parameter_end_without_default(ast: &Ast, p: &ParameterParts) -> Option<u32> {
    if let Some(s) = p.function_typed_suffix {
        return Some(ast.end(s));
    }
    if let Some(n) = p.name {
        return Some(ast.tokens.get(n).end());
    }
    if let Some(t) = p.type_ {
        return Some(ast.end(t));
    }
    [
        p.const_final_or_var_keyword,
        p.required_keyword,
        p.covariant_keyword,
    ]
    .into_iter()
    .flatten()
    .next()
    .map(|t| ast.tokens.get(t).end())
}

/// Dart `_recordRange`.
fn record_range(ast: &Ast, node: NodeId, offset: u32, ranges: &mut Vec<SelectionRange>) {
    if ast.kind(node) == NodeKind::NameWithTypeParameters {
        return;
    }
    if let Some(p) = parameter_parts(ast, node) {
        if let Some(default_clause) = p.default_clause {
            if offset < ast.offset(default_clause) {
                if let Some(end) = formal_parameter_end_without_default(ast, &p) {
                    let start = ast.offset(node);
                    record_offset_length(ranges, start, end - start);
                }
            }
        }
    }
    let name_and_colon = match ast.cast::<NamedArgument>(node) {
        Some(a) => Some((ast[a].name, ast[a].colon)),
        None => ast
            .cast::<RecordLiteralNamedField>(node)
            .map(|f| (ast[f].name, ast[f].colon)),
    };
    if let Some((name, colon)) = name_and_colon {
        let colon_end = ast.tokens.get(colon).end();
        if offset <= colon_end {
            let name_offset = ast.tokens.offset(name);
            record_offset_length(ranges, name_offset, ast.tokens.get(name).length);
            record_offset_length(ranges, name_offset, colon_end - name_offset);
        }
    }
    record_offset_length(ranges, ast.offset(node), ast.length(node));
}
