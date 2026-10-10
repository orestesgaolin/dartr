// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_get_available_refactorings.dart
// Dart source: pkg/analysis_server/lib/src/handler/legacy/edit_get_refactoring.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/refactoring_manager.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/extract_local.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/inline_local.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/convert_method_to_getter.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/convert_getter_to_method.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/extract_method.dart

use dartr_ast::{
    AssignmentExpression, Ast, Block, CompilationUnit, ExportDirective, Expression,
    ExpressionFunctionBody, FunctionBody, FunctionDeclaration, Id, ImportDirective,
    MethodDeclaration, MethodInvocation, NodeId, NodeKind, PartDirective, PrefixedIdentifier,
    PropertyAccess, SimpleIdentifier, Statement, VariableDeclaration, VariableDeclarationList,
    VariableDeclarationStatement,
};
use dartr_element::{AnyElement, Ctx, ElementId, NoopSink, Tag, TypeKind};
use dartr_server::completion::candidate::display_name;
use serde_json::Value;

use crate::protocol;
use crate::search::{ResolvedUnitRef, SElem, SearchEngine};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct RefactoringResponse {
    #[serde(rename = "initialProblems")]
    pub initial_problems: Vec<protocol::RefactoringProblem>,
    #[serde(rename = "optionsProblems")]
    pub options_problems: Vec<protocol::RefactoringProblem>,
    #[serde(rename = "finalProblems")]
    pub final_problems: Vec<protocol::RefactoringProblem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<protocol::SourceChange>,
    #[serde(rename = "potentialEdits", skip_serializing_if = "Option::is_none")]
    pub potential_edits: Option<Vec<String>>,
}

pub fn get_available_refactorings(
    resolved: Option<&ResolvedUnitRef>,
    offset: i64,
    length: i64,
) -> protocol::EditGetAvailableRefactoringsResult {
    let mut kinds = Vec::new();
    if let Some(resolved) = resolved
        && let (Ok(off), Ok(len)) = (u32::try_from(offset), u32::try_from(length))
    {
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        if is_extract_local_available(&u, unit.unit, off, len) {
            kinds.push(protocol::RefactoringKind::ExtractLocalVariable);
        }
        if is_extract_method_available(&u, unit.unit, off, len) {
            kinds.push(protocol::RefactoringKind::ExtractMethod);
        }
        if is_extract_widget_available(&u, unit.unit, off, len) {
            kinds.push(protocol::RefactoringKind::ExtractWidget);
        }
        if is_convert_method_to_getter_available(&u, unit.unit, off) {
            kinds.push(protocol::RefactoringKind::ConvertMethodToGetter);
        }
        if find_rename_target(&u, unit.unit, off).is_some() {
            kinds.push(protocol::RefactoringKind::RENAME);
        }
    }
    protocol::EditGetAvailableRefactoringsResult { kinds }
}

fn is_extract_local_available(
    u: &dartr_server::element_locator::Unit<'_, '_>,
    root: Id<CompilationUnit>,
    offset: u32,
    length: u32,
) -> bool {
    let ast = u.ast;
    let src_len = ast.tokens.source.len() as u32;
    if offset == 0 || offset + length >= src_len {
        return false;
    }
    let Some(covering) = ast.node_covering(root, offset, length) else {
        return false;
    };
    let mut has_fn_body = false;
    let mut p = Some(covering);
    while let Some(n) = p {
        if ast.is::<FunctionBody>(n) {
            has_fn_body = true;
            break;
        }
        p = ast.parent(n);
    }
    if !has_fn_body {
        return false;
    }
    let mut single_expr: Option<NodeId> = None;
    let mut has_covering_expr = false;
    let mut cur = Some(covering);
    while let Some(node) = cur {
        let parent = ast.parent(node);
        if matches!(
            ast.kind(node),
            NodeKind::ArgumentList
                | NodeKind::AssignmentExpression
                | NodeKind::NamedArgument
                | NodeKind::TypeArgumentList
        ) {
            cur = parent;
            continue;
        }
        if matches!(
            ast.kind(node),
            NodeKind::ConstructorName | NodeKind::Label | NodeKind::NamedType
        ) {
            single_expr = None;
            has_covering_expr = false;
            cur = parent;
            continue;
        }
        if let Some(p_id) = parent {
            if let Some(pi) = ast.cast::<PrefixedIdentifier>(p_id)
                && ast[pi].identifier.raw() == node
            {
                cur = parent;
                continue;
            }
            if let Some(pa) = ast.cast::<PropertyAccess>(p_id)
                && ast[pa].property_name.raw() == node
            {
                cur = parent;
                continue;
            }
        }
        if !ast.is::<Expression>(node) {
            break;
        }
        if ast.is::<MethodInvocation>(node)
            && let Some(&ty) = u.tables.static_type.get(node)
            && matches!(u.ctx.ty(ty), TypeKind::Void)
        {
            if single_expr.is_none() {
                return false;
            }
            break;
        }
        if !has_covering_expr {
            if let Some(sid) = ast.cast::<SimpleIdentifier>(node)
                && let Some(el) = u.element(sid)
                && matches!(
                    el.tag(),
                    Tag::LocalFunction | Tag::Method | Tag::TopLevelFunction
                )
            {
                cur = parent;
                continue;
            }
            if let Some(p_id) = parent
                && let Some(assign) = ast.cast::<AssignmentExpression>(p_id)
                && ast[assign].left_hand_side.raw() == node
            {
                return false;
            }
        }
        if single_expr.is_none() {
            single_expr = Some(node);
        }
        has_covering_expr = true;
        cur = parent;
    }
    single_expr.is_some()
}

fn is_extract_method_available(
    u: &dartr_server::element_locator::Unit<'_, '_>,
    root: Id<CompilationUnit>,
    offset: u32,
    length: u32,
) -> bool {
    is_extract_local_available(u, root, offset, length)
}

fn is_extract_widget_available(
    u: &dartr_server::element_locator::Unit<'_, '_>,
    root: Id<CompilationUnit>,
    offset: u32,
    length: u32,
) -> bool {
    find_extract_widget_creation(u, root, offset, length).is_some()
}

fn find_extract_widget_creation(
    u: &dartr_server::element_locator::Unit<'_, '_>,
    root: Id<CompilationUnit>,
    offset: u32,
    length: u32,
) -> Option<Id<dartr_ast::InstanceCreationExpression>> {
    let ast = u.ast;
    let mut node = ast.node_covering(root, offset, length)?;
    if let Some(ret) = ast.cast::<dartr_ast::ReturnStatement>(node) {
        node = ast[ret].expression?.raw();
    }
    let creation = crate::flutter_outline::find_instance_creation_expression(ast, node)?;
    let ctor = crate::flutter_outline::constructor_of_creation(u.ctx, ast, u.tables, creation)?;
    let enc = u
        .ctx
        .get(ctor)
        .enclosing?
        .cast::<dartr_element::InterfaceElement>()?;
    if crate::flutter_outline::is_widget_interface(u.ctx, enc) {
        Some(creation)
    } else {
        None
    }
}

fn is_convert_method_to_getter_available(
    u: &dartr_server::element_locator::Unit<'_, '_>,
    root: Id<CompilationUnit>,
    offset: u32,
) -> bool {
    let ast = u.ast;
    let Some(node) = ast.node_covering(root, offset, 0) else {
        return false;
    };
    let mut cur = Some(node);
    while let Some(n) = cur {
        if let Some(md) = ast.cast::<MethodDeclaration>(n) {
            let d = &ast[md];
            if d.property_keyword.is_some() || d.operator_keyword.is_some() {
                return false;
            }
            let body_start = ast.offset(d.body.raw());
            if offset >= body_start {
                return false;
            }
            let has_empty_params = match d.parameters {
                Some(pl) => ast[pl].parameters.is_empty(),
                None => false,
            };
            if !has_empty_params {
                return false;
            }
            if let Some(el) = u.declared_element(md)
                && let AnyElement::Method(m) = u.ctx.any(el)
                && let Some(ret) = m.return_type.get()
            {
                return !matches!(u.ctx.ty(ret), TypeKind::Void);
            }
            return false;
        }
        if let Some(fd) = ast.cast::<FunctionDeclaration>(n) {
            let d = &ast[fd];
            if d.property_keyword.is_some() {
                return false;
            }
            let fe = d.function_expression;
            let body_start = ast.offset(ast[fe].body.raw());
            if offset >= body_start {
                return false;
            }
            let has_empty_params = match ast[fe].parameters {
                Some(pl) => ast[pl].parameters.is_empty(),
                None => false,
            };
            if !has_empty_params {
                return false;
            }
            if let Some(el) = u.declared_element(fd)
                && let AnyElement::TopLevelFunction(f) = u.ctx.any(el)
                && let Some(ret) = f.return_type.get()
            {
                return !matches!(u.ctx.ty(ret), TypeKind::Void);
            }
            return false;
        }
        cur = ast.parent(n);
    }
    false
}

pub struct RenameTarget {
    pub element: ElementId,
    pub node_offset: u32,
    pub node_length: u32,
    pub old_name: String,
    pub element_kind_name: String,
}

pub fn find_rename_target(
    u: &dartr_server::element_locator::Unit<'_, '_>,
    root: Id<CompilationUnit>,
    offset: u32,
) -> Option<RenameTarget> {
    let ast = u.ast;
    let node = ast.node_covering(root, offset, 0).or_else(|| {
        if offset > 0 {
            ast.node_covering(root, offset - 1, 0)
        } else {
            None
        }
    })?;
    let mut element = dartr_server::element_locator::get_element(u, node)?;
    if element.tag() == Tag::FieldFormalParameter
        && let AnyElement::FormalParameter(p) = u.ctx.any(element)
        && let Some(field) = p.field.get()
    {
        element = field.raw();
    }
    if matches!(element.tag(), Tag::Getter | Tag::Setter)
        && let Some(var) = dartr_resolver::element_metadata::accessor_variable_any(u.ctx, element)
    {
        element = var;
    }
    if matches!(element.tag(), Tag::Library) {
        return None;
    }
    let old_name = display_name(u.ctx, element);
    let (node_offset, node_length) = find_name_range_at(ast, node, offset, &old_name)?;
    let element_kind_name = element.kind().display_name().to_string();
    Some(RenameTarget {
        element,
        node_offset,
        node_length,
        old_name,
        element_kind_name,
    })
}

fn find_name_range_at(
    ast: &Ast,
    node: NodeId,
    offset: u32,
    expected_name: &str,
) -> Option<(u32, u32)> {
    if let Some(sid) = ast.cast::<SimpleIdentifier>(node) {
        let tok = ast.tokens.get(ast[sid].token);
        return Some((tok.offset, tok.end() - tok.offset));
    }
    let mut tok_id = ast.begin_token(node);
    let end_tok = ast.end_token(node);
    loop {
        let tok = ast.tokens.get(tok_id);
        let tok_end = tok.end();
        if tok.offset <= offset && offset <= tok_end {
            let lex = ast.tokens.lexeme(tok_id);
            if !lex.is_empty() && (lex == expected_name || tok.is_identifier()) {
                return Some((tok.offset, tok_end - tok.offset));
            }
        }
        if tok_id == end_tok || tok.ty == dartr_syntax::TokenType::EOF {
            break;
        }
        tok_id = tok.next;
    }
    let start = ast.offset(node);
    let len = ast.length(node);
    if len > 0 { Some((start, len)) } else { None }
}

fn is_valid_identifier(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn get_refactoring(
    resolved: Option<ResolvedUnitRef>,
    search_engine: Option<SearchEngine<'_>>,
    kind: protocol::RefactoringKind,
    file: &str,
    offset: i64,
    length: i64,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(resolved) = resolved else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let Ok(off) = u32::try_from(offset.max(0)) else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let len = u32::try_from(length.max(0)).unwrap_or(0);

    match kind {
        protocol::RefactoringKind::RENAME => {
            compute_rename_refactoring(&resolved, search_engine, file, off, validate_only, options)
        }
        protocol::RefactoringKind::ExtractLocalVariable => {
            compute_extract_local_refactoring(&resolved, file, off, len, validate_only, options)
        }
        protocol::RefactoringKind::InlineLocalVariable => {
            compute_inline_local_refactoring(&resolved, file, off, validate_only)
        }
        protocol::RefactoringKind::ConvertMethodToGetter => {
            compute_convert_method_to_getter(&resolved, search_engine, file, off, validate_only)
        }
        protocol::RefactoringKind::ConvertGetterToMethod => {
            compute_convert_getter_to_method(&resolved, search_engine, file, off, validate_only)
        }
        protocol::RefactoringKind::ExtractMethod => {
            compute_extract_method_refactoring(&resolved, file, off, len, validate_only, options)
        }
        protocol::RefactoringKind::ExtractWidget => {
            compute_extract_widget_refactoring(&resolved, file, off, len, validate_only, options)
        }
        protocol::RefactoringKind::InlineMethod => compute_inline_method_refactoring(
            &resolved,
            search_engine,
            file,
            off,
            validate_only,
            options,
        ),
        protocol::RefactoringKind::MoveFile => {
            compute_move_file_refactoring(search_engine, file, validate_only, options)
        }
    }
}

fn fatal_initial_problem(message: impl Into<String>) -> RefactoringResponse {
    RefactoringResponse {
        initial_problems: vec![protocol::RefactoringProblem {
            severity: protocol::RefactoringProblemSeverity::FATAL,
            message: message.into(),
            location: None,
        }],
        options_problems: Vec::new(),
        final_problems: Vec::new(),
        feedback: None,
        change: None,
        potential_edits: None,
    }
}

fn compute_rename_refactoring(
    resolved: &ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    file: &str,
    offset: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let target = {
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast: &unit.ast,
            tables: &unit.tables,
        };
        find_rename_target(&u, unit.unit, offset)
    };
    let Some(target) = target else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };

    let feedback = serde_json::to_value(protocol::RENAMEFeedback {
        offset: target.node_offset as i64,
        length: target.node_length as i64,
        element_kind_name: target.element_kind_name.clone(),
        old_name: target.old_name.clone(),
    })
    .ok();

    let mut options_problems = Vec::new();
    let new_name = options
        .and_then(|o| o.get("newName"))
        .and_then(Value::as_str);

    if let Some(new_name) = new_name {
        let cap_kind = capitalize(&target.element_kind_name);
        if new_name.is_empty() {
            options_problems.push(protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: format!("{cap_kind} name must not be empty."),
                location: None,
            });
        } else if !is_valid_identifier(new_name) {
            options_problems.push(protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: format!("{cap_kind} name must be a valid identifier."),
                location: None,
            });
        } else if new_name == target.old_name {
            options_problems.push(protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: "The new name must be different than the current name.".to_string(),
                location: None,
            });
        }
    }

    let has_fatal = options_problems
        .iter()
        .any(|p| matches!(p.severity, protocol::RefactoringProblemSeverity::FATAL));

    let mut change = None;
    let mut potential_edits = None;

    if !validate_only
        && !has_fatal
        && let Some(new_name) = new_name
    {
        let mut file_edits_map: indexmap::IndexMap<String, Vec<protocol::SourceEdit>> =
            indexmap::IndexMap::new();

        // Add declaration edit
        {
            let sink = NoopSink;
            let ctx = resolved.ctx(&sink);
            let non_syn = dartr_element::diagnostics::non_synthetic(&ctx, target.element);
            if let Some(ed) = ctx.element_data(non_syn) {
                let mut cur_frag = Some(ed.first_fragment);
                while let Some(frag) = cur_frag {
                    if let Some(fd) = ctx.fragment_data(frag) {
                        if let Some(name_off) = fd.name_offset
                            && let Some(frag_path) =
                                dartr_server::navigation::fragment_path(&ctx, frag)
                        {
                            let name_len = fd
                                .name
                                .map(|n| ctx.name_str(n).len())
                                .unwrap_or(target.old_name.len());
                            file_edits_map.entry(frag_path).or_default().push(
                                protocol::SourceEdit {
                                    offset: name_off as i64,
                                    length: name_len as i64,
                                    replacement: new_name.to_string(),
                                    id: None,
                                    description: None,
                                },
                            );
                        }
                        cur_frag = fd.next_fragment;
                    } else {
                        break;
                    }
                }
            }
        }

        // Add reference edits
        if let Some(mut engine) = search_engine {
            let selem = SElem {
                lib: resolved.library.clone(),
                unit: resolved.index,
                id: target.element,
            };
            let refs = engine.find_element_references(&selem, false);
            for r in refs {
                let loc = r.location;
                let edits = file_edits_map.entry(loc.file).or_default();
                if !edits
                    .iter()
                    .any(|e| e.offset == loc.offset && e.length == loc.length)
                {
                    edits.push(protocol::SourceEdit {
                        offset: loc.offset,
                        length: loc.length,
                        replacement: new_name.to_string(),
                        id: None,
                        description: None,
                    });
                }
            }
        }

        // Fallback if target node wasn't covered
        let file_edits = file_edits_map.entry(file.to_string()).or_default();
        if !file_edits
            .iter()
            .any(|e| e.offset == target.node_offset as i64)
        {
            file_edits.push(protocol::SourceEdit {
                offset: target.node_offset as i64,
                length: target.node_length as i64,
                replacement: new_name.to_string(),
                id: None,
                description: None,
            });
        }

        let mut source_file_edits = Vec::new();
        for (f, mut edits) in file_edits_map {
            edits.sort_by(|a, b| b.offset.cmp(&a.offset));
            source_file_edits.push(protocol::SourceFileEdit {
                file: f,
                file_stamp: 0,
                edits,
            });
        }

        let title_kind = target
            .element_kind_name
            .split(' ')
            .map(capitalize)
            .collect::<Vec<_>>()
            .join(" ");
        change = Some(protocol::SourceChange {
            message: format!("Rename {title_kind} '{}' to '{new_name}'", target.old_name),
            edits: source_file_edits,
            linked_edit_groups: Vec::new(),
            selection: None,
            selection_length: None,
            id: None,
        });
        let _ = &mut potential_edits;
    }

    Ok(RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems,
        final_problems: Vec::new(),
        feedback,
        change,
        potential_edits,
    })
}

fn find_selected_expression(
    ast: &Ast,
    root: Id<CompilationUnit>,
    offset: u32,
    length: u32,
) -> Option<NodeId> {
    if length == 0 {
        return None;
    }
    let end = offset + length;
    let node = ast.node_covering(root, offset, length)?;
    let mut cur = Some(node);
    while let Some(n) = cur {
        if ast.is::<Expression>(n) && ast.offset(n) == offset && ast.end(n) == end {
            // Check not LHS of assignment or declaration name
            if let Some(p) = ast.parent(n)
                && let Some(assign) = ast.cast::<AssignmentExpression>(p)
                && ast[assign].left_hand_side.raw() == n
            {
                return None;
            }
            return Some(n);
        }
        cur = ast.parent(n);
    }
    if ast.is::<Expression>(node) {
        Some(node)
    } else {
        None
    }
}

fn compute_extract_local_refactoring(
    resolved: &ResolvedUnitRef,
    file: &str,
    offset: u32,
    length: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let ast = &unit.ast;
    let source = &ast.tokens.source;

    let Some(expr) = find_selected_expression(ast, unit.unit, offset, length) else {
        return Ok(fatal_initial_problem(
            "Expression must be selected to activate this refactoring.",
        ));
    };
    if let Some(&ty) = unit.tables.static_type.get(expr)
        && matches!(ctx.ty(ty), TypeKind::Void)
    {
        return Ok(fatal_initial_problem("Cannot extract the void expression."));
    }

    // Find enclosing statement
    let mut stmt = Some(expr);
    while let Some(n) = stmt {
        if ast.is::<Statement>(n) && !ast.is::<Block>(n) {
            break;
        }
        stmt = ast.parent(n);
    }
    let Some(enclosing_stmt) = stmt else {
        return Ok(fatal_initial_problem(
            "Expression must be inside a function body.",
        ));
    };

    let expr_start = ast.offset(expr) as usize;
    let expr_end = ast.end(expr) as usize;
    let expr_text = &source[expr_start..expr_end];

    // Suggest names
    let names = suggest_expression_names(&ctx, ast, &unit.tables, expr);

    let feedback = serde_json::to_value(protocol::ExtractLocalVariableFeedback {
        covering_expression_offsets: Some(vec![expr_start as i64]),
        covering_expression_lengths: Some(vec![(expr_end - expr_start) as i64]),
        names: names.clone(),
        offsets: vec![expr_start as i64],
        lengths: vec![(expr_end - expr_start) as i64],
    })
    .ok();

    let mut options_problems = Vec::new();
    let var_name = options.and_then(|o| o.get("name")).and_then(Value::as_str);
    let _extract_all = options
        .and_then(|o| o.get("extractAll"))
        .and_then(Value::as_bool)
        .unwrap_or(true);

    if let Some(name) = var_name {
        if name.is_empty() {
            options_problems.push(protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: "Variable name must not be empty.".to_string(),
                location: None,
            });
        } else if !is_valid_identifier(name) {
            options_problems.push(protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: "Variable name must be a valid identifier.".to_string(),
                location: None,
            });
        }
    }

    let has_fatal = options_problems
        .iter()
        .any(|p| matches!(p.severity, protocol::RefactoringProblemSeverity::FATAL));

    let mut change = None;
    if !validate_only
        && !has_fatal
        && let Some(name) = var_name
    {
        let stmt_offset = ast.offset(enclosing_stmt) as usize;
        let line_start = source[..stmt_offset].rfind('\n').map_or(0, |p| p + 1);
        let indent: String = source[line_start..stmt_offset]
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .collect();
        let eol = if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let decl_str = format!("var {name} = {expr_text};{eol}{indent}");
        let decl_len = decl_str.len();

        let edits = vec![
            protocol::SourceEdit {
                offset: expr_start as i64,
                length: (expr_end - expr_start) as i64,
                replacement: name.to_string(),
                id: None,
                description: None,
            },
            protocol::SourceEdit {
                offset: stmt_offset as i64,
                length: 0,
                replacement: decl_str,
                id: None,
                description: None,
            },
        ];

        let linked_edit_groups = vec![protocol::LinkedEditGroup {
            positions: vec![
                protocol::Position {
                    file: file.to_string(),
                    offset: (stmt_offset + 4) as i64,
                },
                protocol::Position {
                    file: file.to_string(),
                    offset: (expr_start + decl_len) as i64,
                },
            ],
            length: name.len() as i64,
            suggestions: names
                .iter()
                .map(|n| protocol::LinkedEditSuggestion {
                    value: n.clone(),
                    kind: protocol::LinkedEditSuggestionKind::VARIABLE,
                })
                .collect(),
        }];

        change = Some(protocol::SourceChange {
            message: "Extract Local Variable".to_string(),
            edits: vec![protocol::SourceFileEdit {
                file: file.to_string(),
                file_stamp: 0,
                edits,
            }],
            linked_edit_groups,
            selection: None,
            selection_length: None,
            id: None,
        });
    }

    Ok(RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems,
        final_problems: Vec::new(),
        feedback,
        change,
        potential_edits: None,
    })
}

fn suggest_expression_names(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &dartr_element::ResolutionTables,
    expr: NodeId,
) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(mi) = ast.cast::<MethodInvocation>(expr) {
        let m_name = ast.tokens.lexeme(ast[ast[mi].method_name].token);
        if !m_name.is_empty() {
            names.push(m_name.to_string());
        }
    } else if let Some(pa) = ast.cast::<PropertyAccess>(expr) {
        let p_name = ast.tokens.lexeme(ast[ast[pa].property_name].token);
        if !p_name.is_empty() {
            names.push(p_name.to_string());
        }
    } else if let Some(pi) = ast.cast::<PrefixedIdentifier>(expr) {
        let p_name = ast.tokens.lexeme(ast[ast[pi].identifier].token);
        if !p_name.is_empty() {
            names.push(p_name.to_string());
        }
    }
    if let Some(ty) = tables.static_type.get(expr).copied() {
        let ty_str = dartr_element::display_string::type_display_string_with(
            ctx,
            ty,
            dartr_element::display_string::DisplayOptions::default(),
        );
        if ty_str == "int" && !names.iter().any(|n| n == "i") {
            names.push("i".to_string());
        } else if ty_str == "double" && !names.iter().any(|n| n == "d") {
            names.push("d".to_string());
        } else if ty_str == "String" && !names.iter().any(|n| n == "s") {
            names.push("s".to_string());
        }
    }
    if names.is_empty() {
        names.push("res".to_string());
        names.push("object".to_string());
    }
    names
}

fn compute_inline_local_refactoring(
    resolved: &ResolvedUnitRef,
    file: &str,
    offset: u32,
    validate_only: bool,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let ast = &unit.ast;
    let source = &ast.tokens.source;
    let u = dartr_server::element_locator::Unit {
        ctx: &ctx,
        ast,
        tables: &unit.tables,
    };

    let Some(node) = ast.node_covering(unit.unit, offset, 0) else {
        return Ok(fatal_initial_problem(
            "Local variable declaration or reference must be selected to activate this refactoring.",
        ));
    };
    let Some(element) = dartr_server::element_locator::get_element(&u, node) else {
        return Ok(fatal_initial_problem(
            "Local variable declaration or reference must be selected to activate this refactoring.",
        ));
    };
    if element.tag() != Tag::LocalVariable {
        return Ok(fatal_initial_problem(
            "Local variable declaration or reference must be selected to activate this refactoring.",
        ));
    }
    let var_name = display_name(&ctx, element);

    // Find the VariableDeclarationStatement and all references in the enclosing function body
    let mut fn_body = Some(node);
    while let Some(n) = fn_body {
        if ast.is::<FunctionBody>(n) {
            break;
        }
        fn_body = ast.parent(n);
    }
    let Some(fn_body) = fn_body else {
        return Ok(fatal_initial_problem(
            "Local variable declaration or reference must be selected to activate this refactoring.",
        ));
    };

    // Walk AST inside fn_body to find the VariableDeclaration and SimpleIdentifier references
    let mut decl_node: Option<Id<VariableDeclaration>> = None;
    let mut ref_nodes: Vec<Id<SimpleIdentifier>> = Vec::new();
    let mut stack = vec![fn_body];
    while let Some(cur) = stack.pop() {
        if let Some(vd) = ast.cast::<VariableDeclaration>(cur)
            && u.declared_element(vd) == Some(element)
        {
            decl_node = Some(vd);
        } else if let Some(sid) = ast.cast::<SimpleIdentifier>(cur)
            && u.element(sid) == Some(element)
        {
            ref_nodes.push(sid);
        }
        for child in ast.children(cur) {
            stack.push(child);
        }
    }

    let Some(vd) = decl_node else {
        return Ok(fatal_initial_problem(
            "Local variable declaration or reference must be selected to activate this refactoring.",
        ));
    };
    let Some(init_expr) = ast[vd].initializer else {
        return Ok(fatal_initial_problem(format!(
            "Local variable '{var_name}' is not initialized at declaration."
        )));
    };
    let init_text =
        source[ast.offset(init_expr.raw()) as usize..ast.end(init_expr.raw()) as usize].to_string();

    let feedback = serde_json::to_value(protocol::InlineLocalVariableFeedback {
        name: var_name.clone(),
        occurrences: ref_nodes.len() as i64,
    })
    .ok();

    let mut change = None;
    if !validate_only {
        let mut edits = Vec::new();
        for sid in &ref_nodes {
            let tok = ast.tokens.get(ast[*sid].token);
            edits.push(protocol::SourceEdit {
                offset: tok.offset as i64,
                length: (tok.end() - tok.offset) as i64,
                replacement: init_text.clone(),
                id: None,
                description: None,
            });
        }
        // Remove the VariableDeclarationStatement
        if let Some(vdl) = ast
            .parent(vd)
            .and_then(|p| ast.cast::<VariableDeclarationList>(p))
            && let Some(vds) = ast
                .parent(vdl)
                .and_then(|p| ast.cast::<VariableDeclarationStatement>(p))
        {
            let stmt_start = ast.offset(vds.raw()) as usize;
            let stmt_end = ast.end(vds.raw()) as usize;
            // Remove leading whitespace on the same line and trailing newline if on its own line
            let line_start = source[..stmt_start].rfind('\n').map_or(0, |p| p + 1);
            let del_start = if source[line_start..stmt_start].trim().is_empty() {
                line_start
            } else {
                stmt_start
            };
            let mut del_end = stmt_end;
            if source[del_end..].starts_with("\r\n") {
                del_end += 2;
            } else if source[del_end..].starts_with('\n') {
                del_end += 1;
            }
            edits.push(protocol::SourceEdit {
                offset: del_start as i64,
                length: (del_end - del_start) as i64,
                replacement: String::new(),
                id: None,
                description: None,
            });
        }
        edits.sort_by(|a, b| b.offset.cmp(&a.offset));
        change = Some(protocol::SourceChange {
            message: "Inline Local Variable".to_string(),
            edits: vec![protocol::SourceFileEdit {
                file: file.to_string(),
                file_stamp: 0,
                edits,
            }],
            linked_edit_groups: Vec::new(),
            selection: None,
            selection_length: None,
            id: None,
        });
    }

    Ok(RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems: Vec::new(),
        final_problems: Vec::new(),
        feedback,
        change,
        potential_edits: None,
    })
}

fn compute_convert_method_to_getter(
    resolved: &ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    file: &str,
    offset: u32,
    validate_only: bool,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let target_info = (|| -> Option<_> {
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        if !is_convert_method_to_getter_available(&u, unit.unit, offset) {
            return None;
        }
        let node = ast.node_covering(unit.unit, offset, 0)?;
        let mut cur = Some(node);
        let mut found = None;
        while let Some(n) = cur {
            if let Some(md) = ast.cast::<MethodDeclaration>(n) {
                let d = &ast[md];
                let params = d.parameters?;
                let name_tok = ast.tokens.get(d.name);
                let params_start = ast.offset(params.raw());
                let params_end = ast.end(params.raw());
                let el = u.declared_element(md)?;
                found = Some((el, name_tok.offset, params_start, params_end - params_start));
                break;
            }
            if let Some(fd) = ast.cast::<FunctionDeclaration>(n) {
                let d = &ast[fd];
                let fe = d.function_expression;
                let params = ast[fe].parameters?;
                let name_tok = ast.tokens.get(d.name);
                let params_start = ast.offset(params.raw());
                let params_end = ast.end(params.raw());
                let el = u.declared_element(fd)?;
                found = Some((el, name_tok.offset, params_start, params_end - params_start));
                break;
            }
            cur = ast.parent(n);
        }
        found
    })();
    let Some((element, name_offset, params_offset, params_length)) = target_info else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };

    let mut change = None;
    if !validate_only {
        let mut file_edits: indexmap::IndexMap<String, Vec<protocol::SourceEdit>> =
            indexmap::IndexMap::new();
        let decl_edits = file_edits.entry(file.to_string()).or_default();
        decl_edits.push(protocol::SourceEdit {
            offset: params_offset as i64,
            length: params_length as i64,
            replacement: String::new(),
            id: None,
            description: None,
        });
        decl_edits.push(protocol::SourceEdit {
            offset: name_offset as i64,
            length: 0,
            replacement: "get ".to_string(),
            id: None,
            description: None,
        });

        if let Some(mut engine) = search_engine {
            let selem = SElem {
                lib: resolved.library.clone(),
                unit: resolved.index,
                id: element,
            };
            let refs = engine.find_element_references(&selem, false);
            for r in refs {
                let loc = r.location;
                // Remove trailing `()` after reference if present in source
                if let Some(src) = dartr_project::fs::read_string(&loc.file) {
                    let after = (loc.offset + loc.length) as usize;
                    if src.get(after..after + 2) == Some("()") {
                        file_edits
                            .entry(loc.file)
                            .or_default()
                            .push(protocol::SourceEdit {
                                offset: after as i64,
                                length: 2,
                                replacement: String::new(),
                                id: None,
                                description: None,
                            });
                    }
                }
            }
        }

        let mut edits_list = Vec::new();
        for (f, mut edits) in file_edits {
            edits.sort_by(|a, b| b.offset.cmp(&a.offset));
            edits_list.push(protocol::SourceFileEdit {
                file: f,
                file_stamp: 0,
                edits,
            });
        }
        change = Some(protocol::SourceChange {
            message: "Convert Method To Getter".to_string(),
            edits: edits_list,
            linked_edit_groups: Vec::new(),
            selection: None,
            selection_length: None,
            id: None,
        });
    }

    Ok(RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems: Vec::new(),
        final_problems: Vec::new(),
        feedback: None,
        change,
        potential_edits: None,
    })
}

fn compute_convert_getter_to_method(
    resolved: &ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    file: &str,
    offset: u32,
    validate_only: bool,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let target_info = (|| -> Option<_> {
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let node = ast.node_covering(unit.unit, offset, 0)?;
        let mut cur = Some(node);
        let mut found = None;
        while let Some(n) = cur {
            if let Some(md) = ast.cast::<MethodDeclaration>(n) {
                let d = &ast[md];
                let prop = d.property_keyword?;
                if ast.tokens.lexeme(prop) != "get" {
                    break;
                }
                let prop_tok = ast.tokens.get(prop);
                let name_tok = ast.tokens.get(d.name);
                let el = u.declared_element(md)?;
                found = Some((
                    el,
                    prop_tok.offset,
                    name_tok.offset - prop_tok.offset,
                    name_tok.end(),
                ));
                break;
            }
            if let Some(fd) = ast.cast::<FunctionDeclaration>(n) {
                let d = &ast[fd];
                let prop = d.property_keyword?;
                if ast.tokens.lexeme(prop) != "get" {
                    break;
                }
                let prop_tok = ast.tokens.get(prop);
                let name_tok = ast.tokens.get(d.name);
                let el = u.declared_element(fd)?;
                found = Some((
                    el,
                    prop_tok.offset,
                    name_tok.offset - prop_tok.offset,
                    name_tok.end(),
                ));
                break;
            }
            cur = ast.parent(n);
        }
        found
    })();
    let Some((element, get_offset, get_len, name_end)) = target_info else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };

    let mut change = None;
    if !validate_only {
        let mut file_edits: indexmap::IndexMap<String, Vec<protocol::SourceEdit>> =
            indexmap::IndexMap::new();
        let add_decl_edit =
            |file_edits: &mut indexmap::IndexMap<String, Vec<protocol::SourceEdit>>,
             f_path: String,
             g_off: u32,
             g_len: u32,
             n_end: u32| {
                let decl_edits = file_edits.entry(f_path).or_default();
                if !decl_edits
                    .iter()
                    .any(|e| e.offset == n_end as i64 && e.replacement == "()")
                {
                    decl_edits.push(protocol::SourceEdit {
                        offset: n_end as i64,
                        length: 0,
                        replacement: "()".to_string(),
                        id: None,
                        description: None,
                    });
                }
                if !decl_edits
                    .iter()
                    .any(|e| e.offset == g_off as i64 && e.length == g_len as i64)
                {
                    decl_edits.push(protocol::SourceEdit {
                        offset: g_off as i64,
                        length: g_len as i64,
                        replacement: String::new(),
                        id: None,
                        description: None,
                    });
                }
            };

        add_decl_edit(
            &mut file_edits,
            file.to_string(),
            get_offset,
            get_len,
            name_end,
        );

        if let Some(mut engine) = search_engine {
            let field_id = {
                let sink = NoopSink;
                let ctx = resolved.ctx(&sink);
                element
                    .cast::<dartr_element::GetterElement>()
                    .and_then(|g| ctx.get(g).variable.get())
                    .filter(|v| v.raw().tag() == Tag::Field)
                    .map(|v| v.raw())
            };
            let getter_selems: Vec<SElem> = if let Some(fid) = field_id {
                let field_selem = SElem {
                    lib: resolved.library.clone(),
                    unit: resolved.index,
                    id: fid,
                };
                let (members, _) = engine.hierarchy_members_and_parameters(&field_selem);
                members
                    .into_iter()
                    .filter_map(|m| {
                        let gid = m.with(|ctx| {
                            let f = m.id.cast::<dartr_element::FieldElement>()?;
                            let g = ctx.get(f).getter?;
                            let flags =
                                dartr_resolver::element_ext::first_fragment_flags(ctx, g.raw());
                            if flags.contains(
                                dartr_element::FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION,
                            ) {
                                Some(g.raw())
                            } else {
                                None
                            }
                        })?;
                        Some(m.same(gid))
                    })
                    .collect()
            } else {
                vec![SElem {
                    lib: resolved.library.clone(),
                    unit: resolved.index,
                    id: element,
                }]
            };

            for g_selem in getter_selems {
                if let Some((frag_path, g_off, g_len, n_end)) = g_selem.with(|ctx| {
                    let first = ctx.element_data(g_selem.id)?.first_fragment;
                    let frag_path = dartr_server::navigation::fragment_path(ctx, first)?;
                    let fd = ctx.fragment_data(first)?;
                    let name_off = fd.name_offset?;
                    let name_len = fd.name.map(|n| ctx.name_str(n).len())? as u32;
                    let src = dartr_project::fs::read_string(&frag_path)?;
                    let before = src.get(..name_off as usize)?;
                    let get_pos = before.rfind("get")? as u32;
                    Some((frag_path, get_pos, name_off - get_pos, name_off + name_len))
                }) {
                    add_decl_edit(&mut file_edits, frag_path, g_off, g_len, n_end);
                }
                let refs = engine.find_element_references(&g_selem, false);
                for r in refs {
                    let loc = r.location;
                    let after = loc.offset + loc.length;
                    let edits = file_edits.entry(loc.file).or_default();
                    if !edits
                        .iter()
                        .any(|e| e.offset == after && e.replacement == "()")
                    {
                        edits.push(protocol::SourceEdit {
                            offset: after,
                            length: 0,
                            replacement: "()".to_string(),
                            id: None,
                            description: None,
                        });
                    }
                }
            }
        }

        let mut edits_list = Vec::new();
        for (f, mut edits) in file_edits {
            edits.sort_by(|a, b| b.offset.cmp(&a.offset));
            edits_list.push(protocol::SourceFileEdit {
                file: f,
                file_stamp: 0,
                edits,
            });
        }
        change = Some(protocol::SourceChange {
            message: "Convert Getter To Method".to_string(),
            edits: edits_list,
            linked_edit_groups: Vec::new(),
            selection: None,
            selection_length: None,
            id: None,
        });
    }

    Ok(RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems: Vec::new(),
        final_problems: Vec::new(),
        feedback: None,
        change,
        potential_edits: None,
    })
}

fn compute_extract_method_refactoring(
    resolved: &ResolvedUnitRef,
    file: &str,
    offset: u32,
    length: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let ast = &unit.ast;
    let source = &ast.tokens.source;

    if length == 0 {
        return Ok(fatal_initial_problem(
            "Can only extract a single expression or a set of statements.",
        ));
    }
    let expr = find_selected_expression(ast, unit.unit, offset, length);
    let return_type = expr
        .and_then(|e| unit.tables.static_type.get(e).copied())
        .map(|ty| {
            dartr_element::display_string::type_display_string_with(
                &ctx,
                ty,
                dartr_element::display_string::DisplayOptions::default(),
            )
        })
        .unwrap_or_else(|| "void".to_string());

    let names = expr
        .map(|e| suggest_expression_names(&ctx, ast, &unit.tables, e))
        .unwrap_or_else(|| vec!["res".to_string()]);

    let feedback = serde_json::to_value(protocol::ExtractMethodFeedback {
        offset: offset as i64,
        length: length as i64,
        return_type: return_type.clone(),
        names,
        can_create_getter: true,
        parameters: Vec::new(),
        offsets: vec![offset as i64],
        lengths: vec![length as i64],
    })
    .ok();

    let mut options_problems = Vec::new();
    let method_name = options.and_then(|o| o.get("name")).and_then(Value::as_str);
    let create_getter = options
        .and_then(|o| o.get("createGetter"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let opt_return_type = options
        .and_then(|o| o.get("returnType"))
        .and_then(Value::as_str)
        .unwrap_or(&return_type);

    if let Some(name) = method_name {
        if name.is_empty() {
            options_problems.push(protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: "Method name must not be empty.".to_string(),
                location: None,
            });
        } else if !is_valid_identifier(name) {
            options_problems.push(protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: "Method name must be a valid identifier.".to_string(),
                location: None,
            });
        }
    }

    let has_fatal = options_problems
        .iter()
        .any(|p| matches!(p.severity, protocol::RefactoringProblemSeverity::FATAL));

    let mut change = None;
    if !validate_only
        && !has_fatal
        && let Some(name) = method_name
    {
        // Find enclosing declaration (MethodDeclaration or FunctionDeclaration)
        let start_node = expr.or_else(|| ast.node_covering(unit.unit, offset, length));
        let mut enclosing_decl = start_node;
        while let Some(n) = enclosing_decl {
            if matches!(
                ast.kind(n),
                NodeKind::MethodDeclaration | NodeKind::FunctionDeclaration
            ) {
                break;
            }
            enclosing_decl = ast.parent(n);
        }
        if let Some(decl) = enclosing_decl {
            let decl_end = ast.end(decl) as usize;
            let decl_offset = ast.offset(decl) as usize;
            let line_start = source[..decl_offset].rfind('\n').map_or(0, |p| p + 1);
            let indent: String = source[line_start..decl_offset]
                .chars()
                .take_while(|&c| c == ' ' || c == '\t')
                .collect();
            let eol = if source.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let sel_text = &source[offset as usize..(offset + length) as usize];
            let call_text = if create_getter {
                name.to_string()
            } else {
                format!("{name}()")
            };
            let new_decl = if expr.is_some() {
                if create_getter {
                    format!("{eol}{eol}{indent}{opt_return_type} get {name} => {sel_text};")
                } else {
                    format!("{eol}{eol}{indent}{opt_return_type} {name}() => {sel_text};")
                }
            } else if create_getter {
                format!(
                    "{eol}{eol}{indent}{opt_return_type} get {name} {{{eol}{indent}  {sel_text}{eol}{indent}}}"
                )
            } else {
                format!(
                    "{eol}{eol}{indent}{opt_return_type} {name}() {{{eol}{indent}  {sel_text}{eol}{indent}}}"
                )
            };

            let edits = vec![
                protocol::SourceEdit {
                    offset: decl_end as i64,
                    length: 0,
                    replacement: new_decl,
                    id: None,
                    description: None,
                },
                protocol::SourceEdit {
                    offset: offset as i64,
                    length: length as i64,
                    replacement: call_text,
                    id: None,
                    description: None,
                },
            ];
            change = Some(protocol::SourceChange {
                message: "Extract Method".to_string(),
                edits: vec![protocol::SourceFileEdit {
                    file: file.to_string(),
                    file_stamp: 0,
                    edits,
                }],
                linked_edit_groups: Vec::new(),
                selection: None,
                selection_length: None,
                id: None,
            });
        }
    }

    Ok(RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems,
        final_problems: Vec::new(),
        feedback,
        change,
        potential_edits: None,
    })
}

fn compute_extract_widget_refactoring(
    resolved: &ResolvedUnitRef,
    file: &str,
    offset: u32,
    length: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let mut options_problems = Vec::new();
    let widget_name = options.and_then(|o| o.get("name")).and_then(Value::as_str);
    if let Some(name) = widget_name {
        if name.is_empty() {
            options_problems.push(protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: "Class name must not be empty.".to_string(),
                location: None,
            });
        } else if !is_valid_identifier(name) {
            options_problems.push(protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: "Class name must be a valid identifier.".to_string(),
                location: None,
            });
        }
    }

    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let ast = &unit.ast;
    let source = &ast.tokens.source;
    let u = dartr_server::element_locator::Unit {
        ctx: &ctx,
        ast,
        tables: &unit.tables,
    };

    let Some(creation) = find_extract_widget_creation(&u, unit.unit, offset, length) else {
        return Ok(RefactoringResponse {
            initial_problems: vec![protocol::RefactoringProblem {
                severity: protocol::RefactoringProblemSeverity::FATAL,
                message: "Can only extract a widget expression or a method returning widget."
                    .to_string(),
                location: None,
            }],
            options_problems,
            final_problems: Vec::new(),
            feedback: None,
            change: None,
            potential_edits: None,
        });
    };

    let mut enclosing_unit_member = Some(creation.raw());
    while let Some(n) = enclosing_unit_member {
        if ast.parent(n) == Some(unit.unit.raw()) {
            break;
        }
        enclosing_unit_member = ast.parent(n);
    }

    let has_fatal = options_problems
        .iter()
        .any(|p| matches!(p.severity, protocol::RefactoringProblemSeverity::FATAL));

    let mut change = None;
    if !validate_only
        && !has_fatal
        && let Some(name) = widget_name
        && let Some(member) = enclosing_unit_member
    {
        let expr_start = ast.offset(creation.raw()) as usize;
        let expr_end = ast.end(creation.raw()) as usize;
        let line_start = source[..expr_start].rfind('\n').map_or(0, |p| p + 1);
        let indent_old: String = source[line_start..expr_start]
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .collect();
        let indent_new = "    ";
        let raw_code = &source[expr_start..expr_end];
        let eol = if source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let mut reindented = String::new();
        for (idx, line) in raw_code.split(eol).enumerate() {
            if idx == 0 {
                reindented.push_str(line);
            } else {
                reindented.push_str(eol);
                if let Some(stripped) = line.strip_prefix(&indent_old) {
                    reindented.push_str(indent_new);
                    reindented.push_str(stripped);
                } else {
                    reindented.push_str(line);
                }
            }
        }
        let member_end = ast.end(member) as usize;
        let class_code = format!(
            "{eol}{eol}class {name} extends StatelessWidget {{{eol}  const {name}({{{eol}    super.key,{eol}  }});{eol}{eol}  @override{eol}  Widget build(BuildContext context) {{{eol}    return {reindented};{eol}  }}{eol}}}"
        );
        let edits = vec![
            protocol::SourceEdit {
                offset: member_end as i64,
                length: 0,
                replacement: class_code,
                id: None,
                description: None,
            },
            protocol::SourceEdit {
                offset: expr_start as i64,
                length: (expr_end - expr_start) as i64,
                replacement: format!("{name}()"),
                id: None,
                description: None,
            },
        ];
        change = Some(protocol::SourceChange {
            message: "Extract Widget".to_string(),
            edits: vec![protocol::SourceFileEdit {
                file: file.to_string(),
                file_stamp: 0,
                edits,
            }],
            linked_edit_groups: Vec::new(),
            selection: None,
            selection_length: None,
            id: None,
        });
    }

    Ok(RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems,
        final_problems: Vec::new(),
        feedback: Some(serde_json::json!({})),
        change,
        potential_edits: None,
    })
}

fn compute_inline_method_refactoring(
    resolved: &ResolvedUnitRef,
    search_engine: Option<SearchEngine<'_>>,
    file: &str,
    offset: u32,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let (element, method_name, class_name, is_declaration, body_expr_text, decl_range) = {
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let source = &ast.tokens.source;
        let u = dartr_server::element_locator::Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let Some(node) = ast.node_covering(unit.unit, offset, 0) else {
            return Ok(fatal_initial_problem(
                "Method declaration or reference must be selected to activate this refactoring.",
            ));
        };
        let Some(element) = dartr_server::element_locator::get_element(&u, node) else {
            return Ok(fatal_initial_problem(
                "Method declaration or reference must be selected to activate this refactoring.",
            ));
        };
        if !matches!(
            element.tag(),
            Tag::Method | Tag::TopLevelFunction | Tag::Getter | Tag::Setter
        ) {
            return Ok(fatal_initial_problem(
                "Method declaration or reference must be selected to activate this refactoring.",
            ));
        }
        let method_name = display_name(&ctx, element);
        let class_name = ctx
            .element_data(element)
            .and_then(|d| d.enclosing)
            .filter(|e| e.tag() != Tag::Library)
            .map(|e| display_name(&ctx, e));
        let is_declaration =
            ast.is::<MethodDeclaration>(node) || ast.is::<FunctionDeclaration>(node);
        let mut cur = Some(node);
        let mut body_expr_text = None;
        let mut decl_range = None;
        while let Some(n) = cur {
            if let Some(md) = ast.cast::<MethodDeclaration>(n) {
                decl_range = Some((ast.offset(n), ast.length(n)));
                if let Some(efb) = ast.cast::<ExpressionFunctionBody>(ast[md].body) {
                    let expr = ast[efb].expression.raw();
                    body_expr_text =
                        Some(source[ast.offset(expr) as usize..ast.end(expr) as usize].to_string());
                }
                break;
            }
            if let Some(fd) = ast.cast::<FunctionDeclaration>(n) {
                decl_range = Some((ast.offset(n), ast.length(n)));
                let fe = ast[fd].function_expression;
                if let Some(efb) = ast.cast::<ExpressionFunctionBody>(ast[fe].body) {
                    let expr = ast[efb].expression.raw();
                    body_expr_text =
                        Some(source[ast.offset(expr) as usize..ast.end(expr) as usize].to_string());
                }
                break;
            }
            cur = ast.parent(n);
        }
        (
            element,
            method_name,
            class_name,
            is_declaration,
            body_expr_text,
            decl_range,
        )
    };
    let feedback = serde_json::to_value(protocol::InlineMethodFeedback {
        class_name,
        method_name,
        is_declaration,
    })
    .ok();

    let mut change = None;
    if !validate_only && let Some(body_text) = body_expr_text {
        let delete_source = options
            .and_then(|o| o.get("deleteSource"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut file_edits: indexmap::IndexMap<String, Vec<protocol::SourceEdit>> =
            indexmap::IndexMap::new();
        if delete_source && let Some((d_off, d_len)) = decl_range {
            file_edits
                .entry(file.to_string())
                .or_default()
                .push(protocol::SourceEdit {
                    offset: d_off as i64,
                    length: d_len as i64,
                    replacement: String::new(),
                    id: None,
                    description: None,
                });
        }
        if let Some(mut engine) = search_engine {
            let selem = SElem {
                lib: resolved.library.clone(),
                unit: resolved.index,
                id: element,
            };
            let refs = engine.find_element_references(&selem, false);
            for r in refs {
                let loc = r.location;
                if let Some(src) = dartr_project::fs::read_string(&loc.file) {
                    let name_start = loc.offset as usize;
                    let name_end = (loc.offset + loc.length) as usize;
                    let call_end = if src.get(name_end..name_end + 2) == Some("()") {
                        name_end + 2
                    } else {
                        name_end
                    };
                    let (rep_start, replacement) =
                        if name_start > 0 && src.as_bytes().get(name_start - 1) == Some(&b'.') {
                            let mut t_start = name_start - 1;
                            while t_start > 0 {
                                let b = src.as_bytes()[t_start - 1];
                                if b.is_ascii_alphanumeric() || b == b'_' || b == b'$' {
                                    t_start -= 1;
                                } else {
                                    break;
                                }
                            }
                            let target_str = &src[t_start..name_start - 1];
                            (t_start, format!("{target_str}.{body_text}"))
                        } else {
                            (name_start, body_text.clone())
                        };
                    file_edits
                        .entry(loc.file)
                        .or_default()
                        .push(protocol::SourceEdit {
                            offset: rep_start as i64,
                            length: (call_end - rep_start) as i64,
                            replacement,
                            id: None,
                            description: None,
                        });
                }
            }
        }
        let mut edits_list = Vec::new();
        for (f, mut edits) in file_edits {
            edits.sort_by(|a, b| b.offset.cmp(&a.offset));
            edits_list.push(protocol::SourceFileEdit {
                file: f,
                file_stamp: 0,
                edits,
            });
        }
        change = Some(protocol::SourceChange {
            message: "Inline Method".to_string(),
            edits: edits_list,
            linked_edit_groups: Vec::new(),
            selection: None,
            selection_length: None,
            id: None,
        });
    }

    Ok(RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems: Vec::new(),
        final_problems: Vec::new(),
        feedback,
        change,
        potential_edits: None,
    })
}

fn compute_move_file_refactoring(
    search_engine: Option<SearchEngine<'_>>,
    file: &str,
    validate_only: bool,
    options: Option<&Value>,
) -> std::result::Result<RefactoringResponse, (&'static str, String)> {
    let Some(new_file) = options
        .and_then(|o| o.get("newFile"))
        .and_then(Value::as_str)
    else {
        return Ok(fatal_initial_problem("Unable to create a refactoring"));
    };
    let change = if validate_only {
        None
    } else {
        let mut edits_list = Vec::new();
        if let Some(mut engine) = search_engine {
            let old_path = std::path::Path::new(file);
            let new_path = std::path::Path::new(new_file);
            let scope = engine.search_scope();
            for (context, owned) in scope.iter() {
                for f in owned {
                    if f == file {
                        continue;
                    }
                    let Some(f_parent) = std::path::Path::new(f).parent() else {
                        continue;
                    };
                    let Some(resolved_f) = engine.resolved_unit_in(f, Some(*context)) else {
                        continue;
                    };
                    let unit_f = resolved_f.unit();
                    let ast_f = &unit_f.ast;
                    let src_f = &ast_f.tokens.source;
                    let mut file_edits = Vec::new();
                    for &d in ast_f.list_raw(ast_f[unit_f.unit].directives) {
                        let uri_node = if let Some(imp) = ast_f.cast::<ImportDirective>(d) {
                            Some(ast_f[imp].uri.raw())
                        } else if let Some(exp) = ast_f.cast::<ExportDirective>(d) {
                            Some(ast_f[exp].uri.raw())
                        } else if let Some(part) = ast_f.cast::<PartDirective>(d) {
                            Some(ast_f[part].uri.raw())
                        } else {
                            None
                        };
                        let Some(u_node) = uri_node else {
                            continue;
                        };
                        let u_start = ast_f.offset(u_node) as usize;
                        let u_end = ast_f.end(u_node) as usize;
                        let Some(raw_lit) = src_f.get(u_start..u_end) else {
                            continue;
                        };
                        let unquoted = raw_lit.trim_matches(|c| c == '\'' || c == '"');
                        if unquoted.starts_with("dart:") || unquoted.starts_with("package:") {
                            continue;
                        }
                        if f_parent.join(unquoted) == old_path {
                            let new_rel = new_path
                                .strip_prefix(f_parent)
                                .ok()
                                .and_then(|p| p.to_str())
                                .or_else(|| new_path.file_name().and_then(|n| n.to_str()))
                                .unwrap_or(unquoted);
                            file_edits.push(protocol::SourceEdit {
                                offset: u_start as i64,
                                length: (u_end - u_start) as i64,
                                replacement: format!("'{new_rel}'"),
                                id: None,
                                description: None,
                            });
                        }
                    }
                    if !file_edits.is_empty() {
                        file_edits.sort_by(|a, b| b.offset.cmp(&a.offset));
                        edits_list.push(protocol::SourceFileEdit {
                            file: f.clone(),
                            file_stamp: 0,
                            edits: file_edits,
                        });
                    }
                }
            }
        }
        Some(protocol::SourceChange {
            message: "Move File".to_string(),
            edits: edits_list,
            linked_edit_groups: Vec::new(),
            selection: None,
            selection_length: None,
            id: None,
        })
    };
    Ok(RefactoringResponse {
        initial_problems: Vec::new(),
        options_problems: Vec::new(),
        final_problems: Vec::new(),
        feedback: None,
        change,
        potential_edits: None,
    })
}
