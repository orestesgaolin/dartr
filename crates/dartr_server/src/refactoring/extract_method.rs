// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/extract_method.dart

//! `ExtractMethodRefactoring`: extracts a selected expression, closure, or set
//! of statements into a method or function declaration.

use std::collections::{HashMap, HashSet};

use dartr_ast::*;
use dartr_element::{AnyElement, Ctx, ElementId, NoopSink, Tag, TypeId, TypeKind};
use dartr_format::text::{substring_utf16, utf16_len};
use dartr_syntax::TokenType;

use super::extract_local::{
    count_leading_whitespaces, count_trailing_whitespaces, in_declaration_context, ranges_intersect,
};
use super::host::RefactoringHost;
use super::naming;
use super::status::{Location, RefactoringStatus};
use crate::completion::candidate::display_name;
use crate::correction::change::SourceChange;
use crate::correction::change_builder::{ChangeBuilder, end_of_line};
use crate::correction::utils::CorrectionUtils;
use crate::element_locator::{Unit, get_element};
use crate::server::ResolvedUnitRef;

const ERROR_EXITS: &str = "Selected statements contain a return statement, but not all possible execution flows exit. Semantics may not be preserved.";

/// Dart `RefactoringMethodParameter`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MethodParameter {
    pub id: String,
    pub kind: &'static str, // "REQUIRED", "POSITIONAL", "NAMED"
    pub r#type: String,
    pub name: String,
    pub parameters: Option<String>,
}

#[derive(Clone, Debug)]
struct Occurrence {
    range: (u32, u32),
    is_selection: bool,
    parameter_old_to_occurrence_name: HashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
struct SourcePattern {
    parameter_types: Vec<TypeId>,
    normalized_source: String,
    original_to_pattern_names: HashMap<String, String>,
}

impl SourcePattern {
    fn is_compatible(&self, other: &Self) -> bool {
        self.normalized_source == other.normalized_source
            && self.parameter_types == other.parameter_types
    }
}

/// Dart `ExtractMethodRefactoringImpl`.
pub struct ExtractMethod {
    pub unit: ResolvedUnitRef,
    pub selection_offset: u32,
    pub selection_length: u32,
    pub selection_range: (u32, u32),
    pub return_type: String,
    pub variable_type: Option<String>,
    pub name: String,
    pub extract_all: bool,
    pub can_create_getter: bool,
    pub create_getter: bool,
    pub names: Vec<String>,
    pub offsets: Vec<u32>,
    pub lengths: Vec<u32>,
    pub parameters: Vec<MethodParameter>,
    parameters_map: HashMap<String, MethodParameter>,
    parameter_references_map: HashMap<String, Vec<(u32, u32)>>,
    excluded_names: HashSet<String>,
    local_names: HashMap<String, Vec<(u32, u32)>>,
    unqualified_names: HashSet<String>,
    has_await: bool,
    return_type_id: Option<TypeId>,
    return_variable_name: Option<String>,
    parent_member: Option<NodeId>,
    selection_expression: Option<NodeId>,
    selection_function_expression: Option<Id<FunctionExpression>>,
    selection_statements: Option<Vec<NodeId>>,
    occurrences: Vec<Occurrence>,
    static_context: bool,
}

impl ExtractMethod {
    pub fn new(unit: &ResolvedUnitRef, selection_offset: u32, selection_length: u32) -> Self {
        Self {
            unit: unit.clone(),
            selection_offset,
            selection_length,
            selection_range: (selection_offset, selection_length),
            return_type: String::new(),
            variable_type: None,
            name: String::new(),
            extract_all: true,
            can_create_getter: false,
            create_getter: false,
            names: Vec::new(),
            offsets: Vec::new(),
            lengths: Vec::new(),
            parameters: Vec::new(),
            parameters_map: HashMap::new(),
            parameter_references_map: HashMap::new(),
            excluded_names: HashSet::new(),
            local_names: HashMap::new(),
            unqualified_names: HashSet::new(),
            has_await: false,
            return_type_id: None,
            return_variable_name: None,
            parent_member: None,
            selection_expression: None,
            selection_function_expression: None,
            selection_statements: None,
            occurrences: Vec::new(),
            static_context: false,
        }
    }

    pub fn refactoring_name(&self) -> &'static str {
        let ast = &self.unit.unit().ast;
        if let Some(mut node) = ast.node_covering(self.unit.unit().unit, self.selection_offset, 0) {
            loop {
                if ast.is::<ClassDeclaration>(node) {
                    return "Extract Method";
                }
                let Some(p) = ast.parent(node) else {
                    break;
                };
                node = p;
            }
        }
        "Extract Function"
    }

    pub fn is_available(
        unit: &ResolvedUnitRef,
        selection_offset: u32,
        selection_length: u32,
    ) -> bool {
        let mut r = Self::new(unit, selection_offset, selection_length);
        !r.check_selection().has_fatal_error()
    }

    pub fn check_initial_conditions(&mut self) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        result.add_status(self.check_selection());
        if result.has_fatal_error() {
            return result;
        }
        let param_status = self.initialize_parameters();
        result.add_status(param_status);
        self.initialize_has_await();
        self.initialize_return_type();
        self.initialize_occurrences();
        self.prepare_offsets_lengths();
        self.can_create_getter = self.compute_can_create_getter();
        self.create_getter = self.can_create_getter
            && self
                .selection_expression
                .is_some_and(|e| is_expression_for_getter(&self.unit.unit().ast, e));
        self.prepare_excluded_names();
        self.prepare_names();

        if self.selection_function_expression.is_some() && !self.parameters.is_empty() {
            let vars: Vec<String> = self
                .parameters
                .iter()
                .map(|p| format!("'{}'", p.name))
                .collect();
            let count = vars.len();
            let noun = if count == 1 { "variable" } else { "variables" };
            let list = quoted_and_comma_separated(&vars);
            let msg = format!(
                "Cannot extract the closure as a method,it references the external {noun} {list}."
            );
            return RefactoringStatus::fatal(msg);
        }
        result
    }

    pub fn check_name(&self) -> RefactoringStatus {
        naming::validate_method_name(&self.name)
    }

    pub fn check_final_conditions(&self) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        result.add_status(naming::validate_method_name(&self.name));
        result.add_status(self.check_parameter_names());
        result.add_status(self.check_possible_conflicts());
        result
    }

    pub fn check_all_conditions(&mut self) -> RefactoringStatus {
        let mut result = self.check_initial_conditions();
        if result.has_fatal_error() {
            return result;
        }
        result.add_status(self.check_final_conditions());
        result
    }

    fn check_parameter_names(&self) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        for (i, param) in self.parameters.iter().enumerate() {
            result.add_status(naming::validate_parameter_name(&param.name));
            for (j, other) in self.parameters.iter().enumerate() {
                if i != j && other.name == param.name {
                    result.add_error(format!("Parameter '{}' already exists", param.name), None);
                    return result;
                }
            }
            if self.is_parameter_name_conflict_with_body(param) {
                result.add_error(
                    format!(
                        "'{}' is already used as a name in the selected code",
                        param.name
                    ),
                    None,
                );
                return result;
            }
        }
        result
    }

    fn is_parameter_name_conflict_with_body(&self, param: &MethodParameter) -> bool {
        let id = &param.id;
        let name = &param.name;
        if let Some(param_ranges) = self.parameter_references_map.get(id)
            && let Some(other_ranges) = self.local_names.get(name)
        {
            for &pr in param_ranges {
                for &or in other_ranges {
                    if ranges_intersect(pr, or) {
                        return true;
                    }
                }
            }
        }
        self.unqualified_names.contains(name)
    }

    fn check_possible_conflicts(&self) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        let Some(parent_member) = self.parent_member else {
            return result;
        };
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let u = Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let Some(mut parent) = ast.parent(parent_member) else {
            return result;
        };
        if ast.is::<CompilationUnit>(parent) {
            for &decl in ast.list_raw(ast[unit.unit].declarations) {
                if let Some(el) = u.declared_element(decl)
                    && display_name(&ctx, el) == self.name
                {
                    let kind_name = el.kind().display_name();
                    let lib_name = self.unit.path();
                    result.add_error(
                        format!(
                            "Library '{lib_name}' already declares {kind_name} with name '{}'.",
                            self.name
                        ),
                        None,
                    );
                }
            }
            return result;
        }
        if matches!(
            ast.kind(parent),
            NodeKind::BlockClassBody | NodeKind::BlockEnumBody
        ) && let Some(gp) = ast.parent(parent)
        {
            parent = gp;
        }
        if let Some(el) = u.declared_element(parent)
            && let Some(iface) = el.cast::<dartr_element::InterfaceElement>()
        {
            let iface_name = display_name(&ctx, el);
            let iface_data = ctx.interface(iface);
            for &m in &iface_data.methods {
                if display_name(&ctx, m.raw()) == self.name {
                    result.add_error(
                        format!(
                            "Class '{iface_name}' already declares method with name '{}'.",
                            self.name
                        ),
                        None,
                    );
                }
            }
            for &f in &iface_data.fields {
                if display_name(&ctx, f.raw()) == self.name {
                    result.add_error(
                        format!(
                            "Class '{iface_name}' already declares field with name '{}'.",
                            self.name
                        ),
                        None,
                    );
                }
            }
        }
        result
    }

    pub fn signature(&self) -> String {
        let mut sb = String::new();
        if self.create_getter {
            sb.push_str("get ");
            sb.push_str(&self.name);
        } else {
            sb.push_str(&self.name);
            sb.push('(');
            for (i, param) in self.parameters.iter().enumerate() {
                if i > 0 {
                    sb.push_str(", ");
                }
                if param.r#type != "dynamic" && !param.r#type.is_empty() {
                    sb.push_str(&param.r#type);
                    sb.push(' ');
                }
                if let Some(params) = &param.parameters {
                    sb.push_str("Function");
                    sb.push_str(params);
                    sb.push(' ');
                }
                sb.push_str(&param.name);
            }
            sb.push(')');
        }
        sb
    }

    pub fn create_change(&self, host: &mut dyn RefactoringHost) -> SourceChange {
        let path = self.unit.path().to_string();
        let source = &self.unit.unit().ast.tokens.source;
        let default_eol = end_of_line(source);
        let ref_name = self.refactoring_name().to_string();
        let mut out = SourceChange::default();
        host.with_change_workspace(&mut |workspace| {
            let mut builder = ChangeBuilder::new(workspace, default_eol);
            self.build_change(&mut builder, &path);
            out = builder.source_change();
            out.message = ref_name.clone();
        });
        out
    }

    fn build_change(&self, builder: &mut ChangeBuilder<'_>, path: &str) {
        let Some(parent_member) = self.parent_member else {
            return;
        };
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let utils = CorrectionUtils::new(ast, self.unit.line_info());
        let eol = &utils.end_of_line;
        let prefix = utils.get_node_prefix(parent_member);
        let modifiers = if self.static_context { "static " } else { "" };
        let return_expr_source = self.get_method_body_source(&utils);
        let sig = self.signature();
        let expected_closure_ret = self.get_expected_closure_return_type_code();

        builder.add_dart_file_edit(path, |b| {
            for occ in &self.occurrences {
                if !self.extract_all && !occ.is_selection {
                    continue;
                }
                b.add_replacement(occ.range.0, occ.range.1, |eb| {
                    if self.selection_function_expression.is_some() {
                        eb.write(&self.name);
                    } else {
                        if self.selection_statements.is_some()
                            && let Some(var_type) = &self.variable_type
                        {
                            if let Some(ret_var) = &self.return_variable_name {
                                let occ_name = occ
                                    .parameter_old_to_occurrence_name
                                    .get(ret_var)
                                    .map(String::as_str)
                                    .unwrap_or(ret_var);
                                if !self.parameters_map.contains_key(ret_var) {
                                    if var_type.is_empty() {
                                        eb.write("var ");
                                    } else {
                                        eb.write(var_type);
                                        eb.write(" ");
                                    }
                                }
                                eb.write(occ_name);
                                eb.write(" = ");
                            } else {
                                eb.write("return ");
                            }
                        }
                        if self.has_await {
                            eb.write("await ");
                        }
                        eb.write(&self.name);
                        if !self.create_getter {
                            eb.write("(");
                            for (i, param) in self.parameters.iter().enumerate() {
                                if i > 0 {
                                    eb.write(", ");
                                }
                                if let Some(arg_name) =
                                    occ.parameter_old_to_occurrence_name.get(&param.id)
                                {
                                    eb.write(arg_name);
                                } else {
                                    eb.write(&param.name);
                                }
                            }
                            eb.write(")");
                        }
                        if self.selection_statements.is_some() {
                            eb.write(";");
                        }
                    }
                });
            }

            let member_end = ast.end(parent_member);
            b.add_insertion(member_end, |eb| {
                eb.write(&format!("{eol}{eol}{prefix}"));
                if let Some(fe) = self.selection_function_expression {
                    eb.write(modifiers);
                    eb.write(&expected_closure_ret);
                    eb.write(&self.name);
                    eb.write(&return_expr_source);
                    if ast.is::<ExpressionFunctionBody>(ast[fe].body) {
                        eb.write(";");
                    }
                } else if self.selection_expression.is_some() {
                    let async_kw = if self.has_await { " async" } else { "" };
                    let is_multi_line = return_expr_source.contains(eol.as_str());
                    if !is_multi_line {
                        eb.write(modifiers);
                        if !self.return_type.is_empty() {
                            eb.write(&format!("{} ", self.return_type));
                        }
                        eb.write(&sig);
                        eb.write(async_kw);
                        eb.write(" => ");
                        eb.write(&return_expr_source);
                        eb.write(";");
                    } else {
                        let indented = indent_source_left_right(
                            &format!("{};", return_expr_source.trim()),
                            eol,
                        )
                        .trim()
                        .to_string();
                        eb.write(modifiers);
                        if !self.return_type.is_empty() {
                            eb.write(&format!("{} ", self.return_type));
                        }
                        eb.write(&sig);
                        eb.write(async_kw);
                        eb.write(&format!(" {{{eol}{prefix}  "));
                        if !self.return_type.is_empty() {
                            eb.write("return ");
                        }
                        eb.write(&indented);
                        eb.write(&format!("{eol}{prefix}}}"));
                    }
                } else if self.selection_statements.is_some() {
                    let async_kw = if self.has_await { " async" } else { "" };
                    eb.write(modifiers);
                    if !self.return_type.is_empty() {
                        eb.write(&format!("{} ", self.return_type));
                    }
                    eb.write(&sig);
                    eb.write(async_kw);
                    eb.write(&format!(" {{{eol}"));
                    eb.write(&return_expr_source);
                    if let Some(ret_var) = &self.return_variable_name {
                        eb.write(&format!("{prefix}  return {ret_var};{eol}"));
                    }
                    eb.write(&format!("{prefix}}}"));
                }
            });
        });
    }

    fn get_method_body_source(&self, utils: &CorrectionUtils<'_>) -> String {
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let source = &ast.tokens.source;
        let (sel_off, sel_len) = self.selection_range;
        let mut body =
            substring_utf16(source, sel_off as usize, (sel_off + sel_len) as usize).to_string();

        let mut replace_edits: Vec<(u32, u32, String)> = Vec::new();
        for param in &self.parameters {
            if let Some(ranges) = self.parameter_references_map.get(&param.id) {
                for &(r_off, r_len) in ranges {
                    if r_off >= sel_off && r_off + r_len <= sel_off + sel_len {
                        replace_edits.push((r_off - sel_off, r_len, param.name.clone()));
                    }
                }
            }
        }
        replace_edits.sort_by(|a, b| b.0.cmp(&a.0));
        for (off, len, repl) in replace_edits {
            let new_body = {
                let before = substring_utf16(&body, 0, off as usize);
                let after = substring_utf16(&body, (off + len) as usize, usize::MAX);
                format!("{before}{repl}{after}")
            };
            body = new_body;
        }

        if let Some(fe) = self.selection_function_expression
            && let Some(parent_member) = self.parent_member
        {
            let mut stmt = Some(fe.raw());
            while let Some(n) = stmt {
                if ast.is::<Statement>(n) {
                    break;
                }
                stmt = ast.parent(n);
            }
            if let Some(base_node) = stmt {
                let base_indent = utils.get_node_prefix(base_node);
                let target_indent = utils.get_node_prefix(parent_member);
                body = replace_source_indent(
                    &body,
                    &base_indent,
                    &target_indent,
                    &utils.end_of_line,
                    false,
                    false,
                )
                .trim()
                .to_string();
            }
        }
        if let Some(stmts) = &self.selection_statements
            && let Some(&first_stmt) = stmts.first()
            && let Some(parent_member) = self.parent_member
        {
            let sel_indent = utils.get_node_prefix(first_stmt);
            let target_indent = format!("{}  ", utils.get_node_prefix(parent_member));
            body = replace_source_indent(
                &body,
                &sel_indent,
                &target_indent,
                &utils.end_of_line,
                true,
                true,
            );
        }
        body
    }

    fn get_expected_closure_return_type_code(&self) -> String {
        let Some(fe) = self.selection_function_expression else {
            return String::new();
        };
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let mut arg_node = fe.raw();
        if let Some(p) = ast.parent(arg_node)
            && ast.is::<NamedArgument>(p)
        {
            arg_node = p;
        }
        if let Some(param) = dartr_resolver::error::support::corresponding_parameter(
            &ctx,
            ast,
            &unit.tables,
            arg_node,
        ) && let AnyElement::FormalParameter(fp) = ctx.any(param)
            && let Some(param_ty) = fp.type_.get()
            && let TypeKind::Function(ft) = ctx.ty(param_ty)
        {
            let ret_str = type_code(&ctx, ft.ret);
            if ret_str != "dynamic" && !ret_str.is_empty() {
                return format!("{ret_str} ");
            }
        }
        String::new()
    }

    fn check_selection(&mut self) -> RefactoringStatus {
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let source = &ast.tokens.source;
        let content_len = utf16_len(source) as u32;

        if self.selection_offset == 0 {
            return RefactoringStatus::fatal("The selection offset must be greater than zero.");
        }
        if self.selection_offset + self.selection_length >= content_len {
            return RefactoringStatus::fatal(
                "The selection end offset must be less than the length of the file.",
            );
        }

        let sel_str = substring_utf16(
            source,
            self.selection_offset as usize,
            (self.selection_offset + self.selection_length) as usize,
        );
        let num_leading = count_leading_whitespaces(sel_str);
        let num_trailing = count_trailing_whitespaces(sel_str);
        let offset = self.selection_offset + num_leading;
        let end = (self.selection_offset + self.selection_length).saturating_sub(num_trailing);
        let length = end.saturating_sub(offset);
        self.selection_range = (offset, length);

        let u = Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };

        if let Some(fe) = find_function_expression(ast, unit.unit, self.selection_range) {
            self.selection_function_expression = Some(fe);
            self.selection_range = (ast.offset(fe.raw()), ast.length(fe.raw()));
            self.parent_member = enclosing_class_or_unit_member(ast, fe.raw());
            return RefactoringStatus::new();
        }

        let file_path = self.unit.path().to_string();
        let analysis = analyze_selection(&u, unit.unit, self.selection_range, &file_path);
        if analysis.status.has_fatal_error() {
            return analysis.status;
        }
        let mut selected_nodes = analysis.selected_nodes;
        if selected_nodes.is_empty() {
            let mut cur = analysis.covering_node;
            while let Some(node) = cur {
                if ast.is::<Statement>(node) {
                    break;
                }
                if ast.is::<Expression>(node) {
                    let r = (ast.offset(node), ast.length(node));
                    if is_extractable(&u, unit.unit, r, &file_path) {
                        selected_nodes.push(node);
                        self.selection_range = r;
                        break;
                    }
                }
                cur = ast.parent(node);
            }
        }

        if let Some(&first_node) = selected_nodes.first() {
            self.parent_member = enclosing_class_or_unit_member(ast, first_node);
            if selected_nodes.len() == 1
                && !selection_includes_non_whitespace_outside_node(
                    source,
                    ast,
                    self.selection_range,
                    first_node,
                )
                && ast.is::<Expression>(first_node)
            {
                if let Some(fe) = ast.cast::<FunctionExpression>(first_node) {
                    self.selection_function_expression = Some(fe);
                    self.selection_expression = None;
                } else {
                    self.selection_expression = Some(first_node);
                }
                return RefactoringStatus::new();
            }
            if selected_nodes.iter().all(|&n| ast.is::<Statement>(n)) {
                self.selection_statements = Some(selected_nodes);
                return RefactoringStatus::new();
            }
        }

        RefactoringStatus::fatal("Can only extract a single expression or a set of statements.")
    }

    fn initialize_parameters(&mut self) -> RefactoringStatus {
        self.parameters.clear();
        self.parameters_map.clear();
        self.parameter_references_map.clear();
        self.local_names.clear();
        self.unqualified_names.clear();

        let mut result = RefactoringStatus::new();
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let u = Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };

        let Some(parent_member) = self.parent_member else {
            return result;
        };
        let (sel_off, sel_len) = self.selection_range;
        let sel_end = sel_off + sel_len;

        // First collect local variable/parameter declaration offsets & visible ranges
        let mut decl_offsets: HashMap<ElementId, u32> = HashMap::new();
        let mut visible_ranges: HashMap<ElementId, (u32, u32)> = HashMap::new();
        let mut stack = vec![parent_member];
        while let Some(cur) = stack.pop() {
            if let Some(vd) = ast.cast::<VariableDeclaration>(cur)
                && let Some(el) = u.declared_element(vd)
            {
                decl_offsets.insert(el, ast.tokens.offset(ast[vd].name));
                if let Some(blk) = enclosing_block_or_body(ast, cur) {
                    visible_ranges.insert(el, (ast.offset(blk), ast.length(blk)));
                }
            } else if ast.is::<FormalParameter>(cur)
                && let Some(el) = u.declared_element(cur)
            {
                decl_offsets.insert(el, ast.offset(cur));
                if let Some(blk) = enclosing_block_or_body(ast, cur) {
                    visible_ranges.insert(el, (ast.offset(blk), ast.length(blk)));
                }
            } else if let Some(fd) = ast.cast::<FunctionDeclaration>(cur)
                && let Some(el) = u.declared_element(fd)
            {
                decl_offsets.insert(el, ast.tokens.offset(ast[fd].name));
                if let Some(blk) = enclosing_block_or_body(ast, cur) {
                    visible_ranges.insert(el, (ast.offset(blk), ast.length(blk)));
                }
            }
            for child in ast.children(cur) {
                stack.push(child);
            }
        }

        let mut assigned_used_vars: Vec<ElementId> = Vec::new();

        // Walk parent_member in pre-order to collect parameters and assigned variables
        let mut walk = Vec::new();
        collect_preorder(ast, parent_member, &mut walk);
        for node in walk {
            let n_off = ast.offset(node);
            let n_len = ast.length(node);
            let n_end = n_off + n_len;
            let in_selection = n_off >= sel_off && n_end <= sel_end;

            if let Some(vd) = ast.cast::<VariableDeclaration>(node)
                && in_selection
                && let Some(el) = u.declared_element(vd)
            {
                if is_used_after_selection(&u, parent_member, el, sel_end)
                    && !assigned_used_vars.contains(&el)
                {
                    assigned_used_vars.push(el);
                }
                if el.tag() == Tag::LocalVariable
                    && let Some(&vr) = visible_ranges.get(&el)
                {
                    let name = ast.tokens.lexeme(ast[vd].name).to_string();
                    self.local_names.entry(name).or_default().push(vr);
                }
            }

            if let Some(sid) = ast.cast::<SimpleIdentifier>(node)
                && in_selection
            {
                let name = ast.tokens.lexeme(ast[sid].token).to_string();
                let el = get_element(&u, sid.raw());
                let is_local = el.is_some_and(|e| {
                    matches!(
                        e.tag(),
                        Tag::LocalVariable
                            | Tag::FormalParameter
                            | Tag::FieldFormalParameter
                            | Tag::SuperFormalParameter
                            | Tag::LocalFunction
                    )
                });
                if let Some(el) = el
                    && is_local
                {
                    let declared_in_sel = decl_offsets
                        .get(&el)
                        .is_some_and(|&d_off| d_off >= sel_off && d_off < sel_end);
                    if !declared_in_sel {
                        if !self.parameters_map.contains_key(&name) {
                            let param_ty = u
                                .tables
                                .static_type
                                .get(sid.raw())
                                .copied()
                                .or_else(|| element_type(&ctx, el));
                            let (ty_code, params_code) = match param_ty {
                                Some(ty) => parameter_type_and_params_code(&ctx, ty),
                                None => ("dynamic".to_string(), None),
                            };
                            let mp = MethodParameter {
                                id: name.clone(),
                                kind: "REQUIRED",
                                r#type: ty_code,
                                name: name.clone(),
                                parameters: params_code,
                            };
                            self.parameters.push(mp.clone());
                            self.parameters_map.insert(name.clone(), mp);
                        }
                        self.parameter_references_map
                            .entry(name.clone())
                            .or_default()
                            .push((n_off, n_len));
                    }
                    if is_lhs_of_assignment(ast, sid.raw())
                        && is_used_after_selection(&u, parent_member, el, sel_end)
                        && matches!(
                            el.tag(),
                            Tag::LocalVariable
                                | Tag::FormalParameter
                                | Tag::FieldFormalParameter
                                | Tag::SuperFormalParameter
                        )
                        && !assigned_used_vars.contains(&el)
                    {
                        assigned_used_vars.push(el);
                    }
                    if in_declaration_context(ast, sid)
                        && let Some(&vr) = visible_ranges.get(&el)
                    {
                        self.local_names.entry(name).or_default().push(vr);
                    }
                } else if !is_qualified_identifier(ast, sid) {
                    self.unqualified_names.insert(name);
                }
            }
        }

        if let Some(expr) = self.selection_expression {
            self.return_type_id = unit.tables.static_type.get(expr).copied();
        }
        if let Some(stmts) = &self.selection_statements {
            let has_return = stmts.iter().any(|&s| may_end_with_return(ast, s));
            if has_return
                && stmts
                    .last()
                    .is_some_and(|&last| !statement_definitely_exits(ast, last))
            {
                result.add_error(ERROR_EXITS, None);
            }
            self.return_type_id = compute_statements_return_type(&u, stmts);
        }

        if assigned_used_vars.len() == 1 {
            if self.return_type_id.is_some() {
                result.add_fatal_error(
                    "Ambiguous return value: Selected block contains assignment(s) to local variables and return statement.",
                    None,
                );
                return result;
            }
            let ret_var = assigned_used_vars[0];
            self.return_type_id = element_type(&ctx, ret_var);
            self.return_variable_name = Some(display_name(&ctx, ret_var));
        } else if assigned_used_vars.len() > 1 {
            let mut sb = String::new();
            for v in assigned_used_vars {
                sb.push_str(&display_name(&ctx, v));
                sb.push('\n');
            }
            result.add_fatal_error(
                format!(
                    "Ambiguous return value: Selected block contains more than one assignment to local variables. Affected variables are:\n\n{}",
                    sb.trim()
                ),
                None,
            );
        }
        result
    }

    fn initialize_has_await(&mut self) {
        let ast = &self.unit.unit().ast;
        let mut found = false;
        if let Some(expr) = self.selection_expression {
            found = has_await_in_node(ast, expr);
        } else if let Some(stmts) = &self.selection_statements {
            for &s in stmts {
                if has_await_in_node(ast, s) {
                    found = true;
                    break;
                }
            }
        }
        self.has_await = found;
    }

    fn initialize_return_type(&mut self) {
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        if self.selection_function_expression.is_some() {
            self.variable_type = Some(String::new());
            self.return_type = String::new();
            return;
        }
        let Some(ret_ty) = self.return_type_id else {
            self.variable_type = None;
            self.return_type = if self.has_await {
                "Future<void>".to_string()
            } else {
                "void".to_string()
            };
            return;
        };
        if matches!(ctx.ty(ret_ty), TypeKind::Dynamic | TypeKind::Invalid) {
            self.variable_type = Some(String::new());
            self.return_type = if self.has_await {
                "Future".to_string()
            } else {
                String::new()
            };
            return;
        }
        let var_ty = type_code(&ctx, ret_ty);
        self.variable_type = Some(var_ty.clone());
        if self.has_await {
            if var_ty.starts_with("Future<") || var_ty == "Future" {
                self.return_type = var_ty;
            } else {
                self.return_type = format!("Future<{var_ty}>");
            }
        } else {
            self.return_type = var_ty;
        }
    }

    fn initialize_occurrences(&mut self) {
        self.occurrences.clear();
        let Some(parent_member) = self.parent_member else {
            return;
        };
        let unit_ref = self.unit.clone();
        let sink = NoopSink;
        let ctx = unit_ref.ctx(&sink);
        let unit = unit_ref.unit();
        let ast = &unit.ast;
        let u = Unit {
            ctx: &ctx,
            ast,
            tables: &unit.tables,
        };
        let Some(enclosing_parent) = ast.parent(parent_member) else {
            return;
        };

        let sel_pattern = get_source_pattern(&u, self.selection_range);
        let mut pattern_to_sel_name: HashMap<String, String> = HashMap::new();
        for (orig, pat) in &sel_pattern.original_to_pattern_names {
            pattern_to_sel_name.insert(pat.clone(), orig.clone());
        }

        let file_path = unit_ref.path().to_string();
        self.visit_occurrences_node(
            &u,
            unit.unit,
            enclosing_parent,
            &sel_pattern,
            &pattern_to_sel_name,
            &file_path,
            false,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_occurrences_node(
        &mut self,
        u: &Unit<'_, '_>,
        root: Id<CompilationUnit>,
        node: NodeId,
        sel_pattern: &SourcePattern,
        pattern_to_sel_name: &HashMap<String, String>,
        file_path: &str,
        mut force_static: bool,
    ) {
        let ast = u.ast;
        if ast.is::<ConstructorInitializer>(node) {
            force_static = true;
        } else if let Some(fd) = ast.cast::<FieldDeclaration>(node) {
            force_static = ast[fd].static_keyword.is_some();
        } else if let Some(md) = ast.cast::<MethodDeclaration>(node) {
            force_static = ast[md].modifier_keyword.is_some();
        }

        if let Some(blk) = ast.cast::<Block>(node)
            && let Some(sel_stmts) = &self.selection_statements
        {
            let stmts = ast.list_raw(ast[blk].statements);
            let sel_count = sel_stmts.len();
            let mut i = 0;
            while i + sel_count <= stmts.len() {
                let first = stmts[i];
                let last = stmts[i + sel_count - 1];
                let r = (ast.offset(first), ast.end(last) - ast.offset(first));
                if self.try_to_find_occurrence(
                    u,
                    root,
                    r,
                    sel_pattern,
                    pattern_to_sel_name,
                    file_path,
                    force_static,
                ) {
                    i += sel_count;
                } else {
                    i += 1;
                }
            }
        } else if (self.selection_function_expression.is_some()
            || self
                .selection_expression
                .is_some_and(|se| ast.kind(se) == ast.kind(node)))
            && ast.is::<Expression>(node)
        {
            let r = (ast.offset(node), ast.length(node));
            self.try_to_find_occurrence(
                u,
                root,
                r,
                sel_pattern,
                pattern_to_sel_name,
                file_path,
                force_static,
            );
        }

        for child in ast.children(node) {
            self.visit_occurrences_node(
                u,
                root,
                child,
                sel_pattern,
                pattern_to_sel_name,
                file_path,
                force_static,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn try_to_find_occurrence(
        &mut self,
        u: &Unit<'_, '_>,
        root: Id<CompilationUnit>,
        node_range: (u32, u32),
        sel_pattern: &SourcePattern,
        pattern_to_sel_name: &HashMap<String, String>,
        file_path: &str,
        force_static: bool,
    ) -> bool {
        if !is_extractable(u, root, node_range, file_path) {
            return false;
        }
        let node_pattern = get_source_pattern(u, node_range);
        if sel_pattern.is_compatible(&node_pattern) {
            let mut map = HashMap::new();
            for (orig_name, pat_name) in &node_pattern.original_to_pattern_names {
                if let Some(sel_name) = pattern_to_sel_name.get(pat_name) {
                    map.insert(sel_name.clone(), orig_name.clone());
                }
            }
            self.occurrences.push(Occurrence {
                range: node_range,
                is_selection: ranges_intersect(self.selection_range, node_range),
                parameter_old_to_occurrence_name: map,
            });
            if force_static {
                self.static_context = true;
            }
            return true;
        }
        false
    }

    fn prepare_offsets_lengths(&mut self) {
        self.offsets.clear();
        self.lengths.clear();
        for occ in &self.occurrences {
            self.offsets.push(occ.range.0);
            self.lengths.push(occ.range.1);
        }
    }

    fn compute_can_create_getter(&self) -> bool {
        if self.selection_function_expression.is_some() {
            return false;
        }
        if !self.parameters.is_empty() {
            return false;
        }
        if let Some(expr) = self.selection_expression
            && self.unit.unit().ast.is::<AssignmentExpression>(expr)
        {
            return false;
        }
        if self.selection_statements.is_some() {
            return self.return_type != "void";
        }
        true
    }

    fn prepare_excluded_names(&mut self) {
        self.excluded_names.clear();
        let Some(parent_member) = self.parent_member else {
            return;
        };
        let ast = &self.unit.unit().ast;
        let mut stack = vec![parent_member];
        while let Some(cur) = stack.pop() {
            if let Some(vd) = ast.cast::<VariableDeclaration>(cur) {
                let lex = ast.tokens.lexeme(ast[vd].name);
                if !lex.is_empty() {
                    self.excluded_names.insert(lex.to_string());
                }
            }
            for child in ast.children(cur) {
                stack.push(child);
            }
        }
    }

    fn prepare_names(&mut self) {
        self.names.clear();
        let Some(expr) = self.selection_expression else {
            return;
        };
        let sink = NoopSink;
        let ctx = self.unit.ctx(&sink);
        let unit = self.unit.unit();
        let ast = &unit.ast;
        let static_type = unit.tables.static_type.get(expr).copied();
        self.names
            .extend(naming::get_variable_name_suggestions_for_expression(
                &ctx,
                ast,
                &unit.tables,
                static_type,
                Some(expr),
                &self.excluded_names,
                true,
            ));
    }
}

struct SelectionAnalysis {
    status: RefactoringStatus,
    covering_node: Option<NodeId>,
    selected_nodes: Vec<NodeId>,
}

fn is_extractable(
    u: &Unit<'_, '_>,
    root: Id<CompilationUnit>,
    range: (u32, u32),
    file_path: &str,
) -> bool {
    analyze_selection(u, root, range, file_path).status.is_ok()
}

fn analyze_selection(
    u: &Unit<'_, '_>,
    root: Id<CompilationUnit>,
    selection: (u32, u32),
    file_path: &str,
) -> SelectionAnalysis {
    let ast = u.ast;
    let sel_start = selection.0;
    let sel_end = selection.0 + selection.1;

    // Find deepest covering node and direct children inside selection
    let covering_node = ast.node_covering(root, sel_start, selection.1);
    let mut selected_nodes = Vec::new();
    let mut status = RefactoringStatus::new();

    if let Some(cov) = covering_node {
        if ast.offset(cov) >= sel_start && ast.end(cov) <= sel_end && selection.1 > 0 {
            selected_nodes.push(cov);
        } else {
            for child in ast.children(cov) {
                let c_start = ast.offset(child);
                let c_end = ast.end(child);
                if c_start >= sel_start && c_end <= sel_end {
                    selected_nodes.push(child);
                } else if c_start < sel_end && c_end > sel_start {
                    // Selection partially overlaps a child
                    if c_start < sel_start && c_end <= sel_end {
                        status.add_fatal_error(
                            "The selection does not cover a set of statements or an expression. Extend selection to a valid range.",
                            None,
                        );
                    } else if c_start >= sel_start && c_end > sel_end {
                        status.add_fatal_error(
                            "The selection does not cover a set of statements or an expression. Extend selection to a valid range.",
                            None,
                        );
                    }
                }
            }
        }
    }

    let first_selected = selected_nodes.first().copied();
    let last_selected = selected_nodes.last().copied();

    // Check parent consistency if multiple selected nodes
    if selected_nodes.len() > 1
        && let Some(first) = first_selected
        && let Some(first_parent) = ast.parent(first)
    {
        for &n in &selected_nodes[1..] {
            if ast.parent(n) != Some(first_parent) {
                status.add_fatal_error(
                    "Not all selected statements are enclosed by the same parent statement.",
                    None,
                );
                break;
            }
        }
    }

    // Check statements validity (tokens outside selected statements, break/continue targets)
    if let (Some(first), Some(last)) = (first_selected, last_selected)
        && selected_nodes.iter().all(|&n| ast.is::<Statement>(n))
    {
        // Check break / continue targets
        for &stmt in &selected_nodes {
            if has_uncontained_break_or_continue(ast, stmt, sel_start, sel_end) {
                status.add_fatal_error(
                    "Selection contains 'break' or 'continue' statement with target outside of selection.",
                    None,
                );
            }
        }
        let _ = (first, last);
    }

    // Walk covering_node subtree to check node-specific restrictions
    if let Some(cov) = covering_node {
        let mut stack = vec![cov];
        while let Some(node) = stack.pop() {
            let n_off = ast.offset(node);
            let n_len = ast.length(node);
            let n_end = n_off + n_len;
            if n_end < sel_start || n_off > sel_end {
                continue;
            }

            if ast.is::<Directive>(node) && ranges_intersect(selection, (n_off, n_len)) {
                status.add_fatal_error(
                    "Cannot extract a directive.",
                    Some(Location {
                        file: file_path.to_string(),
                        offset: n_off,
                        length: n_len,
                    }),
                );
                break;
            }
            if ast.is::<ImportPrefixReference>(node) {
                status.add_fatal_error("Cannot extract an import prefix.", None);
            }
            if ast.is::<NamedType>(node)
                && (first_selected == Some(node)
                    || (covering_node == Some(node) && selection.1 != 0))
            {
                status.add_fatal_error("Cannot extract a single type reference.", None);
            }
            if let Some(assign) = ast.cast::<AssignmentExpression>(node) {
                let lhs = ast[assign].left_hand_side.raw();
                if first_selected == Some(lhs) {
                    status.add_fatal_error(
                        "Cannot extract the left-hand side of an assignment.",
                        Some(Location {
                            file: file_path.to_string(),
                            offset: ast.offset(lhs),
                            length: ast.length(lhs),
                        }),
                    );
                }
            }
            if first_selected == Some(node) {
                if ast.is::<CatchClauseParameter>(node) {
                    status.add_fatal_error("Cannot extract the name part of a declaration.", None);
                }
                if ast.is::<ConstructorInitializer>(node) {
                    status.add_fatal_error(
                        "Cannot extract a constructor initializer. Select expression part of initializer.",
                        Some(Location {
                            file: file_path.to_string(),
                            offset: n_off,
                            length: n_len,
                        }),
                    );
                }
                if ast.is::<FormalParameterList>(node) {
                    status.add_fatal_error(
                        "Cannot extract a parameter list.",
                        Some(Location {
                            file: file_path.to_string(),
                            offset: n_off,
                            length: n_len,
                        }),
                    );
                }
                if ast.is::<FunctionDeclaration>(node)
                    || (ast.is::<FunctionExpression>(node)
                        && ast
                            .parent(node)
                            .is_some_and(|p| ast.is::<FunctionDeclaration>(p)))
                {
                    status.add_fatal_error(
                        "Cannot extract a function declaration.",
                        Some(Location {
                            file: file_path.to_string(),
                            offset: n_off,
                            length: n_len,
                        }),
                    );
                }
                if ast.is::<GenericFunctionType>(node) {
                    status.add_fatal_error("Cannot extract a single type reference.", None);
                }
                if ast.is::<VariableDeclaration>(node) {
                    status.add_fatal_error(
                        "Cannot extract a variable declaration fragment. Select whole declaration statement.",
                        Some(Location {
                            file: file_path.to_string(),
                            offset: n_off,
                            length: n_len,
                        }),
                    );
                }
                if let Some(sid) = ast.cast::<SimpleIdentifier>(node) {
                    if in_declaration_context(ast, sid) {
                        status.add_fatal_error(
                            "Cannot extract the name part of a declaration.",
                            None,
                        );
                    }
                    if let Some(el) = get_element(u, sid.raw()) {
                        if matches!(
                            el.tag(),
                            Tag::LocalFunction | Tag::Method | Tag::TopLevelFunction
                        ) {
                            status.add_fatal_error("Cannot extract a single method name.", None);
                        }
                        if el.tag() == Tag::Prefix {
                            status.add_fatal_error("Cannot extract an import prefix.", None);
                        }
                    }
                    if let Some(p) = ast.parent(node) {
                        if let Some(pi) = ast.cast::<PrefixedIdentifier>(p)
                            && ast[pi].identifier.raw() == node
                        {
                            status.add_fatal_error(
                                "Cannot extract name part of a property access.",
                                None,
                            );
                        }
                        if let Some(pa) = ast.cast::<PropertyAccess>(p)
                            && ast[pa].property_name.raw() == node
                        {
                            status.add_fatal_error(
                                "Cannot extract name part of a property access.",
                                None,
                            );
                        }
                        if ast.is::<NamedType>(p) {
                            status.add_fatal_error("Cannot extract a single type reference.", None);
                        }
                    }
                }
            }
            if let Some(for_stmt) = ast.cast::<ForStatement>(node) {
                let parts = ast[for_stmt].for_loop_parts;
                if let Some(fpd) = ast.cast::<ForPartsWithDeclarations>(parts)
                    && first_selected == Some(ast[fpd].variables.raw())
                {
                    status.add_fatal_error(
                        "Cannot extract initialization part of a 'for' statement.",
                        None,
                    );
                }
                if let Some(fpd) = ast.cast::<ForPartsWithDeclarations>(parts)
                    && last_selected.is_some_and(|l| ast.list_raw(ast[fpd].updaters).contains(&l))
                {
                    status.add_fatal_error(
                        "Cannot extract increment part of a 'for' statement.",
                        None,
                    );
                }
                if let Some(fpe) = ast.cast::<ForPartsWithExpression>(parts)
                    && last_selected.is_some_and(|l| ast.list_raw(ast[fpe].updaters).contains(&l))
                {
                    status.add_fatal_error(
                        "Cannot extract increment part of a 'for' statement.",
                        None,
                    );
                }
            }

            let children = ast.children(node);
            for child in children.into_iter().rev() {
                stack.push(child);
            }
        }
    }

    SelectionAnalysis {
        status,
        covering_node,
        selected_nodes,
    }
}

fn find_function_expression(
    ast: &Ast,
    root: Id<CompilationUnit>,
    sel: (u32, u32),
) -> Option<Id<FunctionExpression>> {
    if sel.1 != 0 {
        return None;
    }
    let offset = sel.0;
    let node = ast.node_covering(root, offset, 0)?;
    if let Some(named) = ast.cast::<NamedArgument>(node)
        && let Some(fe) = ast.cast::<FunctionExpression>(ast[named].argument_expression)
    {
        return Some(fe);
    }
    let mut cur = Some(node);
    while let Some(n) = cur {
        if let Some(fe) = ast.cast::<FunctionExpression>(n) {
            if let Some(params) = ast[fe].parameters {
                let p_off = ast.offset(params.raw());
                let p_end = ast.end(params.raw());
                if p_off <= offset
                    && offset <= p_end
                    && ast
                        .parent(fe.raw())
                        .is_none_or(|p| !ast.is::<FunctionDeclaration>(p))
                {
                    return Some(fe);
                }
            }
            break;
        }
        cur = ast.parent(n);
    }
    None
}

fn enclosing_class_or_unit_member(ast: &Ast, input: NodeId) -> Option<NodeId> {
    let mut member = input;
    let mut cur = Some(input);
    while let Some(node) = cur {
        if matches!(
            ast.kind(node),
            NodeKind::BlockClassBody | NodeKind::BlockEnumBody | NodeKind::CompilationUnit
        ) {
            return Some(member);
        }
        member = node;
        cur = ast.parent(node);
    }
    None
}

fn selection_includes_non_whitespace_outside_node(
    source: &str,
    ast: &Ast,
    selection: (u32, u32),
    node: NodeId,
) -> bool {
    let n_off = ast.offset(node);
    let n_end = ast.end(node);
    let sel_off = selection.0;
    let sel_end = selection.0 + selection.1;
    if !(sel_off <= n_off && n_end <= sel_end) {
        return false;
    }
    if !is_just_whitespace(source, sel_off, n_off) {
        return true;
    }
    if !is_just_whitespace(source, n_end, sel_end) {
        return true;
    }
    false
}

fn is_just_whitespace(source: &str, start: u32, end: u32) -> bool {
    if start >= end {
        return true;
    }
    substring_utf16(source, start as usize, end as usize)
        .trim()
        .is_empty()
}

fn get_source_pattern(u: &Unit<'_, '_>, range: (u32, u32)) -> SourcePattern {
    let ast = u.ast;
    let source = &ast.tokens.source;
    let r_start = range.0;
    let r_end = range.0 + range.1;
    let orig_source = substring_utf16(source, r_start as usize, r_end as usize).to_string();
    let mut pattern = SourcePattern::default();
    let mut replace_edits: Vec<(u32, u32, String)> = Vec::new();

    struct Visitor<'a, 'b, 'c> {
        u: &'a Unit<'b, 'c>,
        r_start: u32,
        r_end: u32,
        pattern: &'a mut SourcePattern,
        edits: &'a mut Vec<(u32, u32, String)>,
    }
    impl Visitor<'_, '_, '_> {
        fn add_token(&mut self, tok_id: dartr_syntax::TokenId, el: Option<ElementId>) {
            let tok = self.u.ast.tokens.get(tok_id);
            if tok.offset >= self.r_start && tok.end() <= self.r_end {
                if let Some(el) = el
                    && matches!(
                        el.tag(),
                        Tag::LocalVariable
                            | Tag::FormalParameter
                            | Tag::FieldFormalParameter
                            | Tag::SuperFormalParameter
                            | Tag::LocalFunction
                    )
                {
                    let orig_name = display_name(self.u.ctx, el);
                    let pat_name = if let Some(p) =
                        self.pattern.original_to_pattern_names.get(&orig_name)
                    {
                        p.clone()
                    } else {
                        if let Some(ty) = element_type(self.u.ctx, el) {
                            self.pattern.parameter_types.push(ty);
                        }
                        let p = format!("__refVar{}", self.pattern.original_to_pattern_names.len());
                        self.pattern
                            .original_to_pattern_names
                            .insert(orig_name, p.clone());
                        p
                    };
                    self.edits
                        .push((tok.offset - self.r_start, tok.length, pat_name));
                }
            }
        }
        fn visit(&mut self, node: NodeId) {
            let ast = self.u.ast;
            if ast.end(node) < self.r_start || ast.offset(node) > self.r_end {
                return;
            }
            if let Some(named) = ast.cast::<NamedArgument>(node) {
                self.visit(ast[named].argument_expression.raw());
                return;
            }
            if let Some(sid) = ast.cast::<SimpleIdentifier>(node) {
                let el = get_element(self.u, sid.raw());
                self.add_token(ast[sid].token, el);
            } else if let Some(vd) = ast.cast::<VariableDeclaration>(node) {
                let el = self.u.declared_element(vd);
                self.add_token(ast[vd].name, el);
            }
            for child in ast.children(node) {
                self.visit(child);
            }
        }
    }

    if let Some(cov) = ast.node_covering(ast.root(NodeId::from_index(0)), r_start, range.1) {
        let mut v = Visitor {
            u,
            r_start,
            r_end,
            pattern: &mut pattern,
            edits: &mut replace_edits,
        };
        v.visit(cov);
    }

    replace_edits.sort_by(|a, b| b.0.cmp(&a.0));
    let mut mod_src = orig_source;
    for (off, len, repl) in replace_edits {
        let new_src = {
            let before = substring_utf16(&mod_src, 0, off as usize);
            let after = substring_utf16(&mod_src, (off + len) as usize, usize::MAX);
            format!("{before}{repl}{after}")
        };
        mod_src = new_src;
    }
    pattern.normalized_source = normalize_whitespace(&mod_src);
    pattern
}

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_expression_for_getter(ast: &Ast, expr: NodeId) -> bool {
    if let Some(bin) = ast.cast::<BinaryExpression>(expr) {
        return is_expression_for_getter(ast, ast[bin].left_operand.raw())
            && is_expression_for_getter(ast, ast[bin].right_operand.raw());
    }
    if ast.is::<Literal>(expr) || ast.is::<SimpleIdentifier>(expr) {
        return true;
    }
    if let Some(pre) = ast.cast::<PrefixExpression>(expr) {
        return is_expression_for_getter(ast, ast[pre].operand.raw());
    }
    if let Some(pi) = ast.cast::<PrefixedIdentifier>(expr) {
        return is_expression_for_getter(ast, ast[pi].prefix.raw());
    }
    if let Some(pa) = ast.cast::<PropertyAccess>(expr) {
        return ast[pa]
            .target
            .is_some_and(|t| is_expression_for_getter(ast, t.raw()));
    }
    false
}

fn collect_preorder(ast: &Ast, node: NodeId, out: &mut Vec<NodeId>) {
    out.push(node);
    for child in ast.children(node) {
        collect_preorder(ast, child, out);
    }
}

fn enclosing_block_or_body(ast: &Ast, mut node: NodeId) -> Option<NodeId> {
    while let Some(p) = ast.parent(node) {
        if ast.is::<Block>(p) || ast.is::<FunctionBody>(p) {
            return Some(p);
        }
        node = p;
    }
    None
}

fn is_lhs_of_assignment(ast: &Ast, node: NodeId) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(assign) = ast.cast::<AssignmentExpression>(parent)
        && ast[assign].left_hand_side.raw() == node
    {
        return true;
    }
    if let Some(pre) = ast.cast::<PrefixExpression>(parent)
        && ast[pre].operand.raw() == node
        && matches!(
            ast.tokens.ty(ast[pre].operator),
            TokenType::PLUS_PLUS | TokenType::MINUS_MINUS
        )
    {
        return true;
    }
    if let Some(post) = ast.cast::<PostfixExpression>(parent)
        && ast[post].operand.raw() == node
        && matches!(
            ast.tokens.ty(ast[post].operator),
            TokenType::PLUS_PLUS | TokenType::MINUS_MINUS
        )
    {
        return true;
    }
    false
}

fn is_qualified_identifier(ast: &Ast, sid: Id<SimpleIdentifier>) -> bool {
    let node = sid.raw();
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(pa) = ast.cast::<PropertyAccess>(parent) {
        return ast[pa].property_name == sid;
    }
    if let Some(pi) = ast.cast::<PrefixedIdentifier>(parent) {
        return ast[pi].identifier == sid;
    }
    if let Some(mi) = ast.cast::<MethodInvocation>(parent) {
        return ast[mi].method_name == sid && ast[mi].target.is_some();
    }
    false
}

fn is_used_after_selection(
    u: &Unit<'_, '_>,
    parent_member: NodeId,
    element: ElementId,
    sel_end: u32,
) -> bool {
    let ast = u.ast;
    let mut stack = vec![parent_member];
    while let Some(cur) = stack.pop() {
        if ast.is::<SimpleIdentifier>(cur)
            && ast.offset(cur) > sel_end
            && get_element(u, cur) == Some(element)
        {
            return true;
        }
        for child in ast.children(cur) {
            stack.push(child);
        }
    }
    false
}

fn element_type(ctx: &Ctx<'_>, el: ElementId) -> Option<TypeId> {
    match ctx.any(el) {
        AnyElement::LocalVariable(v) => v.type_.get(),
        AnyElement::FormalParameter(p) => p.type_.get(),
        AnyElement::Field(f) => f.property.type_.get(),
        AnyElement::TopLevelVariable(v) => v.property.type_.get(),
        AnyElement::LocalFunction(f) => f.executable.type_.get(),
        _ => None,
    }
}

fn type_code(ctx: &Ctx<'_>, ty: TypeId) -> String {
    if matches!(ctx.ty(ty), TypeKind::Dynamic | TypeKind::Invalid) {
        return "dynamic".to_string();
    }
    dartr_element::display_string::type_display_string_with(
        ctx,
        ty,
        dartr_element::display_string::DisplayOptions::default(),
    )
}

fn parameter_type_and_params_code(ctx: &Ctx<'_>, ty: TypeId) -> (String, Option<String>) {
    if let TypeKind::Function(ft) = ctx.ty(ty) {
        let mut buf = String::from("(");
        for (i, p) in ctx.list(ft.params).iter().enumerate() {
            if i > 0 {
                buf.push_str(", ");
            }
            buf.push_str(&type_code(ctx, p.ty));
            if let Some(n) = p.name {
                let s = ctx.name_str(n);
                if !s.is_empty() {
                    buf.push(' ');
                    buf.push_str(s);
                }
            }
        }
        buf.push(')');
        (type_code(ctx, ft.ret), Some(buf))
    } else {
        (type_code(ctx, ty), None)
    }
}

fn has_await_in_node(ast: &Ast, node: NodeId) -> bool {
    if ast.is::<FunctionExpression>(node) {
        return false;
    }
    if ast.is::<AwaitExpression>(node) {
        return true;
    }
    if let Some(fs) = ast.cast::<ForStatement>(node)
        && ast[fs].await_keyword.is_some()
    {
        return true;
    }
    for child in ast.children(node) {
        if has_await_in_node(ast, child) {
            return true;
        }
    }
    false
}

fn may_end_with_return(ast: &Ast, node: NodeId) -> bool {
    if ast.is::<BlockFunctionBody>(node) {
        return false;
    }
    if ast.is::<ReturnStatement>(node) {
        return true;
    }
    for child in ast.children(node) {
        if may_end_with_return(ast, child) {
            return true;
        }
    }
    false
}

fn statement_definitely_exits(ast: &Ast, stmt: NodeId) -> bool {
    if ast.is::<ReturnStatement>(stmt) || ast.is::<RethrowExpression>(stmt) {
        return true;
    }
    if let Some(es) = ast.cast::<ExpressionStatement>(stmt)
        && ast.is::<ThrowExpression>(ast[es].expression)
    {
        return true;
    }
    if let Some(blk) = ast.cast::<Block>(stmt) {
        return ast
            .list_raw(ast[blk].statements)
            .iter()
            .any(|&s| statement_definitely_exits(ast, s));
    }
    if let Some(ifs) = ast.cast::<IfStatement>(stmt) {
        if let Some(else_s) = ast[ifs].else_statement {
            return statement_definitely_exits(ast, ast[ifs].then_statement.raw())
                && statement_definitely_exits(ast, else_s.raw());
        }
        return false;
    }
    false
}

fn compute_statements_return_type(u: &Unit<'_, '_>, stmts: &[NodeId]) -> Option<TypeId> {
    let mut ret_ty: Option<TypeId> = None;
    fn visit(u: &Unit<'_, '_>, node: NodeId, ret_ty: &mut Option<TypeId>) {
        let ast = u.ast;
        if ast.is::<BlockFunctionBody>(node) {
            return;
        }
        if let Some(rs) = ast.cast::<ReturnStatement>(node) {
            if let Some(expr) = ast[rs].expression
                && let Some(&ty) = u.tables.static_type.get(expr.raw())
                && !matches!(u.ctx.ty(ty), TypeKind::Never(_))
            {
                if ret_ty.is_none() {
                    *ret_ty = Some(ty);
                }
            }
            return;
        }
        for child in ast.children(node) {
            visit(u, child, ret_ty);
        }
    }
    for &s in stmts {
        visit(u, s, &mut ret_ty);
    }
    ret_ty
}

fn has_uncontained_break_or_continue(
    ast: &Ast,
    stmt: NodeId,
    sel_start: u32,
    sel_end: u32,
) -> bool {
    let mut stack = vec![stmt];
    while let Some(cur) = stack.pop() {
        if ast.is::<FunctionExpression>(cur) {
            continue;
        }
        if ast.is::<BreakStatement>(cur) || ast.is::<ContinueStatement>(cur) {
            // Find enclosing loop or switch
            let mut p = ast.parent(cur);
            let mut target = None;
            while let Some(n) = p {
                if matches!(
                    ast.kind(n),
                    NodeKind::ForStatement
                        | NodeKind::WhileStatement
                        | NodeKind::DoStatement
                        | NodeKind::SwitchStatement
                ) {
                    target = Some(n);
                    break;
                }
                p = ast.parent(n);
            }
            if let Some(t) = target {
                if ast.offset(t) < sel_start || ast.end(t) > sel_end {
                    return true;
                }
            } else {
                return true;
            }
        }
        for child in ast.children(cur) {
            stack.push(child);
        }
    }
    false
}

pub(crate) fn replace_source_indent(
    source: &str,
    old_indent: &str,
    new_indent: &str,
    eol: &str,
    include_leading: bool,
    ensure_trailing_newline: bool,
) -> String {
    let lines: Vec<&str> = source.split(eol).collect();
    let mut sb = String::new();
    for (i, line) in lines.iter().enumerate() {
        let is_last = i + 1 == lines.len();
        if is_last && line.is_empty() {
            break;
        }
        let do_replace_whitespace = i != 0 || include_leading;
        let do_append_eol = !is_last || ensure_trailing_newline;
        if do_replace_whitespace {
            let rest = line.strip_prefix(old_indent).unwrap_or(line);
            sb.push_str(new_indent);
            sb.push_str(rest);
        } else {
            sb.push_str(line);
        }
        if do_append_eol {
            sb.push_str(eol);
        }
    }
    sb
}

fn indent_source_left_right(source: &str, eol: &str) -> String {
    let lines: Vec<&str> = source.split(eol).collect();
    let mut sb = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            sb.push_str(eol);
        }
        sb.push_str("  ");
        sb.push_str(line);
    }
    sb
}

fn quoted_and_comma_separated(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        2 => format!("{} and {}", items[0], items[1]),
        n => {
            let head = items[..n - 1].join(", ");
            format!("{head}, and {}", items[n - 1])
        }
    }
}
