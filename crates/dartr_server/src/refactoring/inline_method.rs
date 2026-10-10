// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/inline_method.dart

use std::collections::{HashMap, HashSet};

use dartr_ast::*;
use dartr_element::{
    AnyElement, Ctx, ElementId, FragmentFlags, InterfaceElement, NoopSink, Tag, TypeId, TypeKind,
};
use dartr_resolver::error::support::corresponding_parameter;
use dartr_typesystem::TypeExt;
use indexmap::IndexMap;

use super::host::RefactoringHost;
use super::rename::ChangeAcc;
use super::status::{Location, RefactoringStatus};
use crate::completion::candidate::display_name;
use crate::correction::change::{SourceChange, SourceEdit};
use crate::element_locator::Unit;
use crate::server::{ResolvedUnitRef, SElem};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceRange {
    offset: u32,
    length: u32,
}

impl SourceRange {
    fn new(offset: u32, length: u32) -> Self {
        Self { offset, length }
    }

    fn end(self) -> u32 {
        self.offset + self.length
    }
}

#[derive(Clone, Debug)]
struct ParameterOccurrence {
    base_offset: u32,
    identifier_range: SourceRange,
    parent_paren_range: Option<SourceRange>,
    parent_precedence: u8,
    in_unbraced_interpolation: Option<SourceRange>,
}

#[derive(Clone, Debug)]
struct ParameterInfo {
    name: String,
    is_required: bool,
    default_value_code: Option<String>,
    occurrences: Vec<ParameterOccurrence>,
}

#[derive(Clone, Debug)]
struct SourcePart {
    base: u32,
    source: String,
    prefix: String,
    parameters: IndexMap<ElementId, ParameterInfo>,
    variables: IndexMap<ElementId, (String, Vec<SourceRange>)>,
    explicit_this_offsets: Vec<u32>,
    implicit_this_offsets: Vec<u32>,
    implicit_class_name_offsets: IndexMap<String, Vec<u32>>,
}

#[derive(Clone, Debug)]
struct VariableDecl {
    name: String,
    initializer: String,
}

pub struct InlineMethod {
    pub unit_ref: ResolvedUnitRef,
    pub offset: u32,
    pub is_declaration: bool,
    pub delete_source: bool,
    pub inline_all: bool,

    pub class_name: Option<String>,
    pub method_name: Option<String>,

    method_element: Option<ElementId>,
    method_selem: Option<SElem>,
    method_unit_ref: Option<ResolvedUnitRef>,
    method_node: Option<NodeId>,
    method_parameters: Option<Id<FormalParameterList>>,
    method_body: Option<Id<FunctionBody>>,
    method_expression: Option<Id<Expression>>,
    method_expression_part: Option<SourcePart>,
    method_statements_part: Option<SourcePart>,
    built_change: Option<SourceChange>,
}

impl InlineMethod {
    pub fn new(unit_ref: &ResolvedUnitRef, offset: u32) -> Self {
        Self {
            unit_ref: unit_ref.clone(),
            offset,
            is_declaration: false,
            delete_source: false,
            inline_all: true,
            class_name: None,
            method_name: None,
            method_element: None,
            method_selem: None,
            method_unit_ref: None,
            method_node: None,
            method_parameters: None,
            method_body: None,
            method_expression: None,
            method_expression_part: None,
            method_statements_part: None,
            built_change: None,
        }
    }

    pub fn refactoring_name(&self) -> &'static str {
        if self.method_element.is_some_and(|e| e.tag() == Tag::Method) {
            "Inline Method"
        } else {
            "Inline Function"
        }
    }

    pub fn is_available(&mut self) -> bool {
        let unit_ref = self.unit_ref.clone();
        let sink = NoopSink;
        let ctx = unit_ref.ctx(&sink);
        let cu = unit_ref.unit();
        let u = Unit {
            ctx: &ctx,
            ast: &cu.ast,
            tables: &cu.tables,
        };
        !self.check_offset(&u).has_fatal_error()
    }

    fn check_offset(&mut self, u: &Unit<'_, '_>) -> RefactoringStatus {
        let fatal = RefactoringStatus::fatal(
            "Method declaration or reference must be selected to activate this refactoring.",
        );
        let root = self.unit_ref.unit().unit;
        let Some(selected_node) = u.ast.node_covering(root, self.offset, 0) else {
            return fatal;
        };

        let element = if let Some(fd) = u.ast.cast::<FunctionDeclaration>(selected_node) {
            self.is_declaration = true;
            u.declared_element(fd)
        } else if let Some(md) = u.ast.cast::<MethodDeclaration>(selected_node) {
            self.is_declaration = true;
            u.declared_element(md)
        } else if let Some(sid) = u.ast.cast::<SimpleIdentifier>(selected_node) {
            if u.ast
                .parent(sid.raw())
                .is_some_and(|p| u.ast.is::<Combinator>(p))
            {
                return fatal;
            }
            u.write_or_read_element(sid)
        } else {
            return fatal;
        };

        let Some(element) = element else {
            return fatal;
        };
        if !matches!(
            element.tag(),
            Tag::Method | Tag::TopLevelFunction | Tag::LocalFunction | Tag::Getter | Tag::Setter
        ) {
            return fatal;
        }
        if matches!(element.tag(), Tag::Getter | Tag::Setter)
            && is_synthetic_accessor(u.ctx, element)
        {
            return fatal;
        }
        if is_operator_method(u.ctx, element) {
            return RefactoringStatus::fatal("Can't inline an operator.");
        }
        if is_generator_executable(u.ctx, element) {
            return RefactoringStatus::fatal("Can't inline a generator.");
        }
        RefactoringStatus::new()
    }

    pub fn check_initial_conditions(
        &mut self,
        host: &mut dyn RefactoringHost,
    ) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        let prep_status = self.prepare_method(host);
        result.add_status(prep_status);
        if result.has_fatal_error() {
            return result;
        }

        let Some(method_el) = self.method_element else {
            return result;
        };

        {
            let sink = NoopSink;
            let ctx = self.unit_ref.ctx(&sink);
            if let Some(lib_id) = ctx.element_data(method_el).and_then(|d| d.library) {
                let first_frag = ctx.get(lib_id).first_fragment();
                let uri = &*ctx.fragment(first_frag).source.uri;
                if uri.starts_with("dart:") {
                    return RefactoringStatus::fatal("Can't inline SDK code.");
                }
            }
            if let Some(d) = ctx.element_data(method_el)
                && let Some(fd) = ctx.fragment_data(d.first_fragment)
                && fd.next_fragment.is_some()
            {
                return RefactoringStatus::fatal("Can't inline an augmented method.");
            }
            if is_operator_method(&ctx, method_el) {
                return RefactoringStatus::fatal("Can't inline an operator.");
            }
            if is_generator_executable(&ctx, method_el) {
                return RefactoringStatus::fatal("Can't inline a generator.");
            }
        }

        let parts_status = self.prepare_method_parts();
        result.add_status(parts_status);
        result
    }

    fn prepare_method(&mut self, host: &mut dyn RefactoringHost) -> RefactoringStatus {
        self.method_element = None;
        self.method_parameters = None;
        self.method_body = None;
        self.delete_source = false;
        self.inline_all = false;
        self.is_declaration = false;

        let fatal = RefactoringStatus::fatal(
            "Method declaration or reference must be selected to activate this refactoring.",
        );

        let (element, decl_file, decl_offset) = {
            let unit_ref = self.unit_ref.clone();
            let sink = NoopSink;
            let ctx = unit_ref.ctx(&sink);
            let cu = unit_ref.unit();
            let u = Unit {
                ctx: &ctx,
                ast: &cu.ast,
                tables: &cu.tables,
            };
            let root = cu.unit;
            let Some(selected_node) = u.ast.node_covering(root, self.offset, 0) else {
                return fatal;
            };

            let element = if let Some(fd) = u.ast.cast::<FunctionDeclaration>(selected_node) {
                self.is_declaration = true;
                u.declared_element(fd)
            } else if let Some(md) = u.ast.cast::<MethodDeclaration>(selected_node) {
                self.is_declaration = true;
                u.declared_element(md)
            } else if let Some(sid) = u.ast.cast::<SimpleIdentifier>(selected_node) {
                if u.ast
                    .parent(sid.raw())
                    .is_some_and(|p| u.ast.is::<Combinator>(p))
                {
                    return fatal;
                }
                u.write_or_read_element(sid)
            } else {
                return fatal;
            };

            let Some(element) = element else {
                return fatal;
            };
            if !matches!(
                element.tag(),
                Tag::Method
                    | Tag::TopLevelFunction
                    | Tag::LocalFunction
                    | Tag::Getter
                    | Tag::Setter
            ) {
                return fatal;
            }
            if matches!(element.tag(), Tag::Getter | Tag::Setter)
                && is_synthetic_accessor(&ctx, element)
            {
                return fatal;
            }

            self.method_name = Some(display_name(&ctx, element));
            self.class_name = ctx
                .element_data(element)
                .and_then(|d| d.enclosing)
                .filter(|e| e.cast::<InterfaceElement>().is_some())
                .map(|e| display_name(&ctx, e));

            let Some(ed) = ctx.element_data(element) else {
                return fatal;
            };
            let first_frag = ed.first_fragment;
            let Some(decl_file) = crate::navigation::fragment_path(&ctx, first_frag) else {
                return fatal;
            };
            let Some(fd) = ctx.fragment_data(first_frag) else {
                return fatal;
            };
            let decl_offset = fd.name_offset.or(fd.first_token_offset).unwrap_or(0);
            (element, decl_file, decl_offset)
        };

        let decl_unit_ref = if self.unit_ref.path() == decl_file {
            self.unit_ref.clone()
        } else if let Some(resolved) =
            host.resolved_unit_in(&decl_file, Some(self.unit_ref.library.context))
        {
            resolved
        } else {
            return fatal;
        };

        let cu = decl_unit_ref.unit();
        let ast = &cu.ast;
        let Some(cov) = ast.node_covering(cu.unit, decl_offset, 0) else {
            return fatal;
        };

        let mut cur = Some(cov);
        let mut found_node = None;
        let mut found_params = None;
        let mut found_body = None;
        while let Some(n) = cur {
            if let Some(md) = ast.cast::<MethodDeclaration>(n) {
                found_node = Some(n);
                found_params = ast[md].parameters;
                found_body = Some(ast[md].body);
                break;
            }
            if let Some(fd) = ast.cast::<FunctionDeclaration>(n) {
                found_node = Some(n);
                let fe = ast[fd].function_expression;
                found_params = ast[fe].parameters;
                found_body = Some(ast[fe].body);
                break;
            }
            cur = ast.parent(n);
        }

        let (Some(method_node), Some(method_body)) = (found_node, found_body) else {
            return fatal;
        };

        let sink = NoopSink;
        let decl_ctx = decl_unit_ref.ctx(&sink);
        let decl_u = Unit {
            ctx: &decl_ctx,
            ast: &cu.ast,
            tables: &cu.tables,
        };
        let decl_el = decl_u.declared_element(method_node).unwrap_or(element);

        self.method_element = Some(decl_el);
        self.method_selem = Some(SElem {
            lib: decl_unit_ref.library.clone(),
            unit: decl_unit_ref.index,
            id: decl_el,
        });
        self.method_unit_ref = Some(decl_unit_ref);
        self.method_node = Some(method_node);
        self.method_parameters = found_params;
        self.method_body = Some(method_body);
        self.delete_source = self.is_declaration;
        self.inline_all = self.delete_source;
        RefactoringStatus::new()
    }

    fn prepare_method_parts(&mut self) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        let Some(decl_unit_ref) = self.method_unit_ref.clone() else {
            return result;
        };
        let Some(method_body) = self.method_body else {
            return RefactoringStatus::fatal("Can't inline a method without a body.");
        };

        let sink = NoopSink;
        let ctx = decl_unit_ref.ctx(&sink);
        let cu = decl_unit_ref.unit();
        let u = Unit {
            ctx: &ctx,
            ast: &cu.ast,
            tables: &cu.tables,
        };
        let file = decl_unit_ref.path().to_string();

        if let Some(efb) = u.ast.cast::<ExpressionFunctionBody>(method_body.raw()) {
            let expr = u.ast[efb].expression;
            self.method_expression = Some(expr);
            let expr_range = SourceRange::new(u.ast.offset(expr.raw()), u.ast.length(expr.raw()));
            self.method_expression_part = Some(self.create_source_part(&u, expr_range));
        } else if let Some(bfb) = u.ast.cast::<BlockFunctionBody>(method_body.raw()) {
            let block = u.ast[bfb].block;
            let stmts_slice = u.ast.list(u.ast[block].statements);
            let mut stmts: Vec<Id<Statement>> = stmts_slice.to_vec();
            if let Some(&last_stmt) = stmts.last()
                && let Some(ret) = u.ast.cast::<ReturnStatement>(last_stmt.raw())
            {
                self.method_expression = u.ast[ret].expression;
                if let Some(expr) = self.method_expression {
                    let expr_range =
                        SourceRange::new(u.ast.offset(expr.raw()), u.ast.length(expr.raw()));
                    self.method_expression_part = Some(self.create_source_part(&u, expr_range));
                }
                stmts.pop();
            }
            if !stmts.is_empty() {
                let first = stmts[0].raw();
                let last = stmts.last().unwrap().raw();
                let raw_range = SourceRange::new(
                    u.ast.offset(first),
                    u.ast.end(last).saturating_sub(u.ast.offset(first)),
                );
                let stmts_range = get_lines_range_statements(&u.ast.tokens.source, raw_range);
                self.method_statements_part = Some(self.create_source_part(&u, stmts_range));
            }
            let mut num_returns = 0usize;
            validate_returns(&u, block.raw(), &file, &mut num_returns, &mut result);
        } else {
            return RefactoringStatus::fatal("Can't inline a method without a body.");
        }

        result
    }

    fn create_source_part(&self, u: &Unit<'_, '_>, range: SourceRange) -> SourcePart {
        let source_str = &u.ast.tokens.source;
        let part_src = source_str
            .get(range.offset as usize..range.end() as usize)
            .unwrap_or("")
            .to_string();
        let prefix = get_line_prefix(source_str, range.offset as usize);
        let mut part = SourcePart {
            base: range.offset,
            source: part_src,
            prefix,
            parameters: IndexMap::new(),
            variables: IndexMap::new(),
            explicit_this_offsets: Vec::new(),
            implicit_this_offsets: Vec::new(),
            implicit_class_name_offsets: IndexMap::new(),
        };

        if let Some(param_list) = self.method_parameters {
            for &param_node in u.ast.list(u.ast[param_list].parameters) {
                if let Some(param_el) = u.declared_element(param_node.raw())
                    && let AnyElement::FormalParameter(fp) = u.ctx.any(param_el)
                {
                    let name = fp
                        .name
                        .map(|n| u.ctx.name_str(n).to_string())
                        .unwrap_or_default();
                    let is_required = fp.kind.is_required();
                    let default_value_code = default_value_for_param(u.ast, param_node.raw());
                    part.parameters.insert(
                        param_el,
                        ParameterInfo {
                            name,
                            is_required,
                            default_value_code,
                            occurrences: Vec::new(),
                        },
                    );
                }
            }
        }

        let cu = self.method_unit_ref.as_ref().unwrap().unit();
        collect_variables_in_part(u, cu.unit.raw(), range, &mut part);
        part
    }

    pub fn check_final_conditions(&mut self, host: &mut dyn RefactoringHost) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        if self.delete_source && !self.inline_all {
            result.add_error("All references must be inlined to remove the source.", None);
        }

        let mut acc = ChangeAcc::default();
        let Some(selem) = self.method_selem.clone() else {
            self.built_change = Some(acc.into_change(self.refactoring_name().to_string()));
            return result;
        };

        let refs = host.search_references(&selem);
        let mut introduced_by_block: HashMap<(String, u32), HashSet<String>> = HashMap::new();
        let mut already_made_async: HashSet<(String, u32)> = HashSet::new();

        for r in &refs {
            let ref_file = &r.path;
            let ref_offset = r.offset;
            let Some(ref_unit) = self.resolve_unit(host, ref_file, Some(r.context)) else {
                continue;
            };
            self.process_reference(
                &ref_unit,
                ref_file,
                ref_offset,
                &mut result,
                &mut acc,
                &mut introduced_by_block,
                &mut already_made_async,
            );
        }

        if self.delete_source
            && self.inline_all
            && let (Some(decl_unit_ref), Some(method_node)) =
                (&self.method_unit_ref, self.method_node)
        {
            let cu = decl_unit_ref.unit();
            let decl_file = decl_unit_ref.path().to_string();
            let node_range =
                SourceRange::new(cu.ast.offset(method_node), cu.ast.length(method_node));
            let lines_range = get_lines_range_skip_leading_empty(&cu.ast.tokens.source, node_range);
            acc.add(&decl_file, lines_range.offset, lines_range.length, "");
        }

        self.built_change = Some(acc.into_change(self.refactoring_name().to_string()));
        result
    }

    pub fn check_all_conditions(&mut self, host: &mut dyn RefactoringHost) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        result.add_status(self.check_initial_conditions(host));
        if result.has_fatal_error() {
            return result;
        }
        result.add_status(self.check_final_conditions(host));
        result
    }

    pub fn create_change(&self) -> SourceChange {
        self.built_change.clone().unwrap_or_else(|| SourceChange {
            message: self.refactoring_name().to_string(),
            ..Default::default()
        })
    }

    fn resolve_unit(
        &self,
        host: &mut dyn RefactoringHost,
        file: &str,
        context: Option<usize>,
    ) -> Option<ResolvedUnitRef> {
        if self.unit_ref.path() == file {
            return Some(self.unit_ref.clone());
        }
        if let Some(m_ref) = &self.method_unit_ref
            && m_ref.path() == file
        {
            return Some(m_ref.clone());
        }
        host.resolved_unit_in(file, context)
    }

    #[allow(clippy::too_many_arguments)]
    fn process_reference(
        &self,
        ref_unit: &ResolvedUnitRef,
        ref_file: &str,
        ref_offset: u32,
        status: &mut RefactoringStatus,
        acc: &mut ChangeAcc,
        introduced_by_block: &mut HashMap<(String, u32), HashSet<String>>,
        already_made_async: &mut HashSet<(String, u32)>,
    ) {
        let sink = NoopSink;
        let ctx = ref_unit.ctx(&sink);
        let cu = ref_unit.unit();
        let u = Unit {
            ctx: &ctx,
            ast: &cu.ast,
            tables: &cu.tables,
        };
        let source_str = &u.ast.tokens.source;
        let eol = end_of_line(source_str);

        let Some(cov) = u.ast.node_covering(cu.unit, ref_offset, 0) else {
            return;
        };
        let Some(sid) = u.ast.cast::<SimpleIdentifier>(cov) else {
            return;
        };
        let node = sid.raw();

        if !self.inline_all {
            let node_range = SourceRange::new(u.ast.offset(node), u.ast.length(node));
            if !range_contains(node_range, self.offset) {
                return;
            }
        }

        let Some(node_parent) = u.ast.parent(node) else {
            return;
        };
        if u.ast.is::<Combinator>(node_parent) {
            return;
        }

        let mut ref_stmt = Some(node);
        while let Some(n) = ref_stmt {
            if u.ast.is::<Statement>(n) && !u.ast.is::<Block>(n) {
                break;
            }
            ref_stmt = u.ast.parent(n);
        }
        let (ref_line_range, ref_prefix) = if let Some(stmt) = ref_stmt {
            let stmt_range = SourceRange::new(u.ast.offset(stmt), u.ast.length(stmt));
            (
                Some(get_lines_range_statements(source_str, stmt_range)),
                get_line_prefix(source_str, u.ast.offset(stmt) as usize),
            )
        } else {
            (
                None,
                get_line_prefix(source_str, u.ast.offset(node) as usize),
            )
        };

        let Some(method_el) = self.method_element else {
            return;
        };

        if is_async_executable(&ctx, method_el)
            && let Some(enclosing_body) = find_ancestor_function_body(u.ast, node)
            && is_sync_body(u.ast, enclosing_body)
        {
            if is_generator_body(u.ast, enclosing_body) {
                status.add_fatal_error(
                    "Can't inline an 'async' method into a 'sync*' method.",
                    Some(node_location(&u, ref_file, node)),
                );
                return;
            }
            if let Some(enclosing_exec) = find_enclosing_executable_element(&u, enclosing_body)
                && !returns_future(&ctx, enclosing_exec)
            {
                status.add_fatal_error(
                    "Can't inline an 'async' method into a function that doesn't return a 'Future'.",
                    Some(node_location(&u, ref_file, node)),
                );
                return;
            }
            let body_off = u.ast.offset(enclosing_body);
            if already_made_async.insert((ref_file.to_string(), body_off)) {
                acc.add(ref_file, body_off, 0, "async ");
            }
        }

        if let Some(mi) = u.ast.cast::<MethodInvocation>(node_parent)
            && u.ast[mi].method_name.raw() == node
        {
            let target = u.ast[mi].target.map(|t| t.raw());
            let cascaded = is_cascaded_method_invocation(u.ast, mi);
            let args: Vec<NodeId> = u
                .ast
                .list(u.ast[u.ast[mi].argument_list].arguments)
                .iter()
                .map(|a| a.raw())
                .collect();
            self.inline_method_invocation(
                &u,
                ref_file,
                status,
                acc,
                mi.raw(),
                node,
                cascaded,
                target,
                &args,
                ref_line_range,
                &ref_prefix,
                eol,
                introduced_by_block,
            );
        } else {
            if method_el.tag() == Tag::Method {
                status.add_fatal_error(
                    "Can't inline a class method reference.",
                    Some(node_location(&u, ref_file, node)),
                );
                return;
            }
            if matches!(method_el.tag(), Tag::Getter | Tag::Setter) {
                let mut usage = node;
                let mut target = None;
                let mut cascade = false;
                if let Some(pi) = u.ast.cast::<PrefixedIdentifier>(node_parent) {
                    if u.ast[pi].prefix.raw() == node {
                        usage = u.ast[pi].prefix.raw();
                    } else {
                        usage = pi.raw();
                        target = Some(u.ast[pi].prefix.raw());
                    }
                } else if let Some(pa) = u.ast.cast::<PropertyAccess>(node_parent) {
                    usage = pa.raw();
                    target = u.ast[pa].target.map(|t| t.raw());
                    cascade = is_cascaded_property_access(u.ast, pa);
                }
                let mut args = Vec::new();
                if in_setter_context(u.ast, node)
                    && let Some(assign) = find_ancestor_assignment(u.ast, node)
                {
                    usage = assign.raw();
                    args.push(u.ast[assign].right_hand_side.raw());
                }
                self.inline_method_invocation(
                    &u,
                    ref_file,
                    status,
                    acc,
                    usage,
                    node,
                    cascade,
                    target,
                    &args,
                    ref_line_range,
                    &ref_prefix,
                    eol,
                    introduced_by_block,
                );
                return;
            }

            if let Some(closure_src) = self.extract_closure_source(&ref_prefix, eol, true) {
                acc.add(
                    ref_file,
                    u.ast.offset(node),
                    u.ast.length(node),
                    &closure_src,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn inline_method_invocation(
        &self,
        u: &Unit<'_, '_>,
        ref_file: &str,
        status: &mut RefactoringStatus,
        acc: &mut ChangeAcc,
        usage: NodeId,
        node: NodeId,
        cascaded: bool,
        target: Option<NodeId>,
        arguments: &[NodeId],
        ref_line_range: Option<SourceRange>,
        ref_prefix: &str,
        eol: &str,
        introduced_by_block: &mut HashMap<(String, u32), HashSet<String>>,
    ) {
        if cascaded {
            status.add_error(
                "Can't inline a cascade invocation.",
                Some(node_location(u, ref_file, usage)),
            );
        }

        if self.can_inline_body(u.ast, usage) {
            let block_key = (
                ref_file.to_string(),
                find_surrounding_block_offset(u.ast, usage),
            );
            let introduced_vars = introduced_by_block.entry(block_key).or_default();
            let mut param_to_var_name: HashMap<ElementId, String> = HashMap::new();

            if let Some(stmts_part) = &self.method_statements_part {
                let (part_source, var_decls) = self.get_method_source_for_invocation(
                    u,
                    ref_file,
                    status,
                    stmts_part,
                    usage,
                    target,
                    arguments,
                    introduced_vars,
                    &mut param_to_var_name,
                );
                self.insert_variable_declarations(
                    u,
                    ref_file,
                    acc,
                    &var_decls,
                    introduced_vars,
                    ref_line_range,
                    node,
                    ref_prefix,
                    eol,
                );
                let replaced = replace_source_indent_full(
                    &part_source,
                    &stmts_part.prefix,
                    ref_prefix,
                    true,
                    true,
                    eol,
                );
                if let Some(line_range) = ref_line_range {
                    acc.add(ref_file, line_range.offset, 0, &replaced);
                }
            }

            if let Some(expr_part) = &self.method_expression_part {
                let (mut source, var_decls) = self.get_method_source_for_invocation(
                    u,
                    ref_file,
                    status,
                    expr_part,
                    usage,
                    target,
                    arguments,
                    introduced_vars,
                    &mut param_to_var_name,
                );
                self.insert_variable_declarations(
                    u,
                    ref_file,
                    acc,
                    &var_decls,
                    introduced_vars,
                    ref_line_range,
                    node,
                    ref_prefix,
                    eol,
                );

                let mut node_to_replace = usage;
                let method_expr = self.method_expression.unwrap().raw();
                let decl_ast = &self.method_unit_ref.as_ref().unwrap().unit().ast;

                if let Some(p) = u.ast.parent(usage)
                    && let Some(interp) = u.ast.cast::<InterpolationExpression>(p)
                    && u.ast.tokens.lexeme(u.ast[interp].left_bracket) == "$"
                {
                    if decl_ast.is::<SimpleIdentifier>(method_expr) {
                        // ok
                    } else if can_discard_single_quotes(decl_ast, method_expr) && source.len() >= 2
                    {
                        node_to_replace = interp.raw();
                        source = source[1..source.len() - 1].to_string();
                    } else {
                        source = format!("{{{source}}}");
                    }
                }

                if get_expression_precedence(decl_ast, method_expr)
                    < get_expression_parent_precedence(u.ast, usage)
                {
                    source = format!("({source})");
                }

                if let Some(p) = u.ast.parent(usage)
                    && u.ast.is::<AwaitExpression>(p)
                    && let Some(rest) = source.strip_prefix("await ")
                {
                    source = rest.to_string();
                }

                source = replace_source_indent_full(
                    &source,
                    &expr_part.prefix,
                    ref_prefix,
                    false,
                    false,
                    eol,
                );
                acc.add(
                    ref_file,
                    u.ast.offset(node_to_replace),
                    u.ast.length(node_to_replace),
                    &source,
                );
            } else if let Some(line_range) = ref_line_range {
                acc.add(ref_file, line_range.offset, line_range.length, "");
            }
            return;
        }

        if let Some(closure_src) = self.extract_closure_source(ref_prefix, eol, false) {
            acc.add(
                ref_file,
                u.ast.offset(node),
                u.ast.length(node),
                &closure_src,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_variable_declarations(
        &self,
        u: &Unit<'_, '_>,
        ref_file: &str,
        acc: &mut ChangeAcc,
        var_decls: &[VariableDecl],
        introduced_vars: &mut HashSet<String>,
        ref_line_range: Option<SourceRange>,
        node: NodeId,
        ref_prefix: &str,
        eol: &str,
    ) {
        if var_decls.is_empty() {
            return;
        }
        let offset = ref_line_range.map_or_else(|| u.ast.offset(node), |r| r.offset);
        let insert_offset = get_line_this(&u.ast.tokens.source, offset as usize) as u32;
        let mut buf = String::new();
        for v in var_decls {
            introduced_vars.insert(v.name.clone());
            buf.push_str(&format!(
                "{ref_prefix}var {} = {};{eol}",
                v.name, v.initializer
            ));
        }
        acc.add(ref_file, insert_offset, 0, &buf);
    }

    fn can_inline_body(&self, ast: &Ast, usage: NodeId) -> bool {
        if self.method_statements_part.is_none() {
            return self.method_expression_part.is_some();
        }
        let Some(parent) = ast.parent(usage) else {
            return false;
        };
        let parent2 = ast.parent(parent);
        if ast.is::<Statement>(parent) {
            return parent2.is_some_and(|p2| ast.is::<Block>(p2));
        }
        if ast.is::<AwaitExpression>(parent)
            && let Some(p2) = parent2
            && ast.is::<ExpressionStatement>(p2)
        {
            return ast.parent(p2).is_some_and(|p3| ast.is::<Block>(p3));
        }
        if let Some(assign) = ast.cast::<AssignmentExpression>(parent) {
            if ast[assign].left_hand_side.raw() == usage {
                return parent2.is_some_and(|p2| {
                    ast.is::<Statement>(p2) && ast.parent(p2).is_some_and(|p3| ast.is::<Block>(p3))
                });
            }
            return self.method_expression_part.is_some();
        }
        if self.method_expression_part.is_some()
            && ast.is::<VariableDeclaration>(parent)
            && let Some(p2) = parent2
            && ast.is::<VariableDeclarationList>(p2)
            && let Some(p3) = ast.parent(p2)
            && ast.is::<VariableDeclarationStatement>(p3)
        {
            return ast.parent(p3).is_some_and(|p4| ast.is::<Block>(p4));
        }
        false
    }

    fn extract_closure_source(
        &self,
        ref_prefix: &str,
        eol: &str,
        strip_semi: bool,
    ) -> Option<String> {
        let decl_unit = self.method_unit_ref.as_ref()?.unit();
        let ast = &decl_unit.ast;
        let params = self.method_parameters?;
        let method_node = self.method_node?;
        let start = ast.tokens.get(ast[params].left_parenthesis).offset as usize;
        let end = ast.end(method_node) as usize;
        let raw = ast.tokens.source.get(start..end)?;
        let method_prefix = get_line_prefix(&ast.tokens.source, ast.offset(method_node) as usize);
        let mut s = replace_source_indent_full(raw, &method_prefix, ref_prefix, false, false, eol);
        s = s.trim().to_string();
        if strip_semi && let Some(stripped) = s.strip_suffix(';') {
            s = stripped.to_string();
        }
        Some(s)
    }

    #[allow(clippy::too_many_arguments)]
    fn get_method_source_for_invocation(
        &self,
        u: &Unit<'_, '_>,
        ref_file: &str,
        status: &mut RefactoringStatus,
        part: &SourcePart,
        context_node: NodeId,
        target_expression: Option<NodeId>,
        arguments: &[NodeId],
        previously_introduced: &HashSet<String>,
        param_to_var_name: &mut HashMap<ElementId, String>,
    ) -> (String, Vec<VariableDecl>) {
        let mut conflicting_names = get_names_conflicting_at(u, context_node);
        for name in previously_introduced {
            conflicting_names.insert(name.clone());
        }

        let mut edits: Vec<SourceEdit> = Vec::new();
        let mut var_decls: Vec<VariableDecl> = Vec::new();
        let source_str = &u.ast.tokens.source;

        for (&param_el, param_info) in &part.parameters {
            let mut arg_expr_node: Option<NodeId> = None;
            if arguments.len() == 1 && self.method_element.is_some_and(|e| e.tag() == Tag::Setter) {
                arg_expr_node = Some(arguments[0]);
            } else {
                for &arg in arguments {
                    if let Some(cp) = corresponding_parameter(u.ctx, u.ast, u.tables, arg)
                        && let Some(cp_name) = u.ctx.element_name(cp)
                        && cp_name == param_info.name
                    {
                        if let Some(named) = u.ast.cast::<NamedArgument>(arg) {
                            arg_expr_node = Some(u.ast[named].argument_expression.raw());
                        } else {
                            arg_expr_node = Some(arg);
                        }
                        break;
                    }
                }
            }

            let (mut arg_prec, mut arg_source) = if let Some(arg_node) = arg_expr_node {
                let start = u.ast.offset(arg_node) as usize;
                let end = u.ast.end(arg_node) as usize;
                (
                    get_expression_precedence(u.ast, arg_node),
                    source_str.get(start..end).unwrap_or("").to_string(),
                )
            } else {
                if param_info.is_required {
                    status.add_error(
                        format!("No argument for the parameter \"{}\".", param_info.name),
                        Some(node_location(u, ref_file, context_node)),
                    );
                    continue;
                }
                (
                    0,
                    param_info
                        .default_value_code
                        .clone()
                        .unwrap_or_else(|| "null".to_string()),
                )
            };

            let existing_var_name = param_to_var_name.get(&param_el).cloned();
            let add_variable = existing_var_name.is_some()
                || should_introduce_variable(u, arg_expr_node, param_info.occurrences.len());

            let mut effective_arg_node = arg_expr_node;
            if add_variable {
                let var_name = if let Some(existing) = existing_var_name {
                    existing
                } else {
                    let orig = if param_info.name.is_empty() {
                        "p"
                    } else {
                        &param_info.name
                    };
                    let unique = get_unique_name(orig, &conflicting_names);
                    conflicting_names.insert(unique.clone());
                    param_to_var_name.insert(param_el, unique.clone());
                    var_decls.push(VariableDecl {
                        name: unique.clone(),
                        initializer: arg_source.clone(),
                    });
                    unique
                };
                arg_prec = 20;
                arg_source = var_name;
                effective_arg_node = None;
            }

            for occ in &param_info.occurrences {
                let mut range_to_replace = occ.identifier_range;
                let occ_arg_source = if let Some(interp_range) = occ.in_unbraced_interpolation {
                    match effective_arg_node {
                        None => arg_source.clone(),
                        Some(n) if u.ast.is::<SimpleIdentifier>(n) => arg_source.clone(),
                        Some(n) if can_discard_single_quotes(u.ast, n) && arg_source.len() >= 2 => {
                            range_to_replace = interp_range;
                            arg_source[1..arg_source.len() - 1].to_string()
                        }
                        _ => format!("{{{arg_source}}}"),
                    }
                } else if arg_prec < occ.parent_precedence {
                    format!("({arg_source})")
                } else {
                    arg_source.clone()
                };

                if add_variable && let Some(paren_range) = occ.parent_paren_range {
                    range_to_replace = paren_range;
                }

                let rel_offset = range_to_replace.offset.saturating_sub(occ.base_offset);
                edits.push(SourceEdit {
                    offset: rel_offset,
                    length: range_to_replace.length,
                    replacement: occ_arg_source,
                    id: 0,
                    description: None,
                });
            }
        }

        for (cls_name, offsets) in &part.implicit_class_name_offsets {
            for &off in offsets {
                edits.push(SourceEdit {
                    offset: off,
                    length: 0,
                    replacement: format!("{cls_name}."),
                    id: 0,
                    description: None,
                });
            }
        }

        if let Some(target_node) = target_expression {
            let start = u.ast.offset(target_node) as usize;
            let end = u.ast.end(target_node) as usize;
            let target_src = source_str.get(start..end).unwrap_or("");
            for &off in &part.explicit_this_offsets {
                edits.push(SourceEdit {
                    offset: off,
                    length: 4,
                    replacement: target_src.to_string(),
                    id: 0,
                    description: None,
                });
            }
            let target_dot = format!("{target_src}.");
            for &off in &part.implicit_this_offsets {
                edits.push(SourceEdit {
                    offset: off,
                    length: 0,
                    replacement: target_dot.clone(),
                    id: 0,
                    description: None,
                });
            }
        }

        for (_var_el, (orig_name, ranges)) in &part.variables {
            let unique_name = get_unique_name(orig_name, &conflicting_names);
            if unique_name != *orig_name {
                for r in ranges {
                    edits.push(SourceEdit {
                        offset: r.offset,
                        length: r.length,
                        replacement: unique_name.clone(),
                        id: 0,
                        description: None,
                    });
                }
            }
        }

        edits.sort_by(|a, b| b.offset.cmp(&a.offset));
        let mut out = part.source.clone();
        for edit in edits {
            let start = (edit.offset as usize).min(out.len());
            let end = (start + edit.length as usize).min(out.len());
            out.replace_range(start..end, &edit.replacement);
        }

        (out, var_decls)
    }
}

fn collect_variables_in_part(
    u: &Unit<'_, '_>,
    node: NodeId,
    body_range: SourceRange,
    part: &mut SourcePart,
) {
    let node_range = SourceRange::new(u.ast.offset(node), u.ast.length(node));
    if !ranges_intersect(body_range, node_range) {
        return;
    }

    if let Some(sid) = u.ast.cast::<SimpleIdentifier>(node) {
        if range_covers(body_range, node_range) {
            add_member_qualifier(u, sid, part);
            add_parameter_occurrence(u, sid, part);
            if let Some(el) = u.element(sid)
                && el.tag() == Tag::LocalVariable
            {
                let name = display_name(u.ctx, el);
                let rel = SourceRange::new(
                    node_range.offset.saturating_sub(part.base),
                    node_range.length,
                );
                part.variables
                    .entry(el)
                    .or_insert_with(|| (name, Vec::new()))
                    .1
                    .push(rel);
            }
        }
    } else if u.ast.is::<ThisExpression>(node) {
        let off = u.ast.offset(node);
        if range_contains(body_range, off) {
            part.explicit_this_offsets
                .push(off.saturating_sub(part.base));
        }
    } else if let Some(vd) = u.ast.cast::<VariableDeclaration>(node) {
        let tok = u.ast.tokens.get(u.ast[vd].name);
        let name_range = SourceRange::new(tok.offset, tok.length);
        if range_covers(body_range, name_range)
            && let Some(el) = u.declared_element(vd)
            && el.tag() == Tag::LocalVariable
        {
            let name = display_name(u.ctx, el);
            let rel = SourceRange::new(
                name_range.offset.saturating_sub(part.base),
                name_range.length,
            );
            part.variables
                .entry(el)
                .or_insert_with(|| (name, Vec::new()))
                .1
                .push(rel);
        }
    }

    for child in u.ast.children(node) {
        collect_variables_in_part(u, child, body_range, part);
    }
}

fn add_member_qualifier(u: &Unit<'_, '_>, sid: Id<SimpleIdentifier>, part: &mut SourcePart) {
    if is_qualified_identifier(u.ast, sid.raw()) {
        return;
    }
    let Some(el) = u.write_or_read_element(sid) else {
        return;
    };
    if !matches!(
        el.tag(),
        Tag::Method | Tag::Getter | Tag::Setter | Tag::Field
    ) {
        return;
    }
    let Some(enc) = u.ctx.element_data(el).and_then(|d| d.enclosing) else {
        return;
    };
    if enc.cast::<InterfaceElement>().is_none() {
        return;
    }
    let off = u.ast.offset(sid.raw()).saturating_sub(part.base);
    if is_static_member(u.ctx, el) {
        let cls_name = display_name(u.ctx, enc);
        part.implicit_class_name_offsets
            .entry(cls_name)
            .or_default()
            .push(off);
    } else {
        part.implicit_this_offsets.push(off);
    }
}

fn add_parameter_occurrence(u: &Unit<'_, '_>, sid: Id<SimpleIdentifier>, part: &mut SourcePart) {
    let Some(el) = u.write_or_read_element(sid) else {
        return;
    };
    let Some(param_info) = part.parameters.get_mut(&el) else {
        return;
    };
    let id_range = SourceRange::new(u.ast.offset(sid.raw()), u.ast.length(sid.raw()));
    let parent_prec = get_expression_parent_precedence(u.ast, sid.raw());
    let mut in_unbraced_interpolation = None;
    let mut parent_paren_range = None;
    if let Some(p) = u.ast.parent(sid.raw()) {
        if let Some(interp) = u.ast.cast::<InterpolationExpression>(p)
            && u.ast.tokens.lexeme(u.ast[interp].left_bracket) == "$"
        {
            in_unbraced_interpolation = Some(SourceRange::new(u.ast.offset(p), u.ast.length(p)));
        }
        if let Some(paren) = u.ast.cast::<ParenthesizedExpression>(p)
            && u.ast[paren].expression.raw() == sid.raw()
        {
            parent_paren_range = Some(SourceRange::new(u.ast.offset(p), u.ast.length(p)));
        }
    }
    param_info.occurrences.push(ParameterOccurrence {
        base_offset: part.base,
        identifier_range: id_range,
        parent_paren_range,
        parent_precedence: parent_prec,
        in_unbraced_interpolation,
    });
}

fn validate_returns(
    u: &Unit<'_, '_>,
    node: NodeId,
    file: &str,
    num_returns: &mut usize,
    status: &mut RefactoringStatus,
) {
    if u.ast.is::<FunctionExpression>(node) {
        return;
    }
    if u.ast.is::<ReturnStatement>(node) {
        *num_returns += 1;
        if *num_returns == 2 {
            status.add_error(
                "Ambiguous return value.",
                Some(node_location(u, file, node)),
            );
        }
    }
    for child in u.ast.children(node) {
        validate_returns(u, child, file, num_returns, status);
    }
}

fn should_introduce_variable(
    u: &Unit<'_, '_>,
    expr: Option<NodeId>,
    occurrence_count: usize,
) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    if let Some(sid) = u.ast.cast::<SimpleIdentifier>(expr)
        && let Some(el) = u.element(sid)
        && matches!(
            el.tag(),
            Tag::LocalVariable | Tag::FormalParameter | Tag::TopLevelVariable | Tag::Field
        )
    {
        return false;
    }
    if u.ast.is::<Literal>(expr) {
        return occurrence_count > 1;
    }
    if let Some(paren) = u.ast.cast::<ParenthesizedExpression>(expr) {
        return should_introduce_variable(u, Some(u.ast[paren].expression.raw()), occurrence_count);
    }
    if let Some(bin) = u.ast.cast::<BinaryExpression>(expr) {
        return should_introduce_variable(u, Some(u.ast[bin].left_operand.raw()), occurrence_count)
            || should_introduce_variable(
                u,
                Some(u.ast[bin].right_operand.raw()),
                occurrence_count,
            );
    }
    true
}

fn get_names_conflicting_at(u: &Unit<'_, '_>, node: NodeId) -> HashSet<String> {
    let mut result = HashSet::new();
    let mut cur = Some(node);
    while let Some(n) = cur {
        if u.ast.is::<FunctionBody>(n) {
            collect_local_names(u, n, &mut result);
            break;
        }
        cur = u.ast.parent(n);
    }
    let mut cls_cur = Some(node);
    while let Some(n) = cls_cur {
        if let Some(cls) = u.ast.cast::<ClassDeclaration>(n)
            && let Some(el) = u.declared_element(cls)
            && let Some(iface) = el.cast::<InterfaceElement>()
        {
            for child in crate::server::class_members(u.ctx, iface.raw(), None) {
                let name = display_name(u.ctx, child);
                if !name.is_empty() {
                    result.insert(name);
                }
            }
            let this_ty = u.ctx.interface_this_type(iface);
            for &sup_ty in u.ctx.all_supertypes(this_ty).iter() {
                if let TypeKind::Interface { element: sup, .. } = *u.ctx.ty(sup_ty) {
                    for child in crate::server::class_members(u.ctx, sup.raw(), None) {
                        let name = display_name(u.ctx, child);
                        if !name.is_empty() {
                            result.insert(name);
                        }
                    }
                }
            }
            break;
        }
        cls_cur = u.ast.parent(n);
    }
    result
}

fn collect_local_names(u: &Unit<'_, '_>, node: NodeId, out: &mut HashSet<String>) {
    if let Some(vd) = u.ast.cast::<VariableDeclaration>(node) {
        out.insert(u.ast.tokens.lexeme(u.ast[vd].name).to_string());
    } else if u.ast.is::<FormalParameter>(node)
        && let Some(el) = u.declared_element(node)
    {
        let name = display_name(u.ctx, el);
        if !name.is_empty() {
            out.insert(name);
        }
    }
    for child in u.ast.children(node) {
        collect_local_names(u, child, out);
    }
}

fn get_unique_name(original_name: &str, conflicting: &HashSet<String>) -> String {
    let mut unique = original_name.to_string();
    let mut idx = 2usize;
    while conflicting.contains(&unique) {
        unique = format!("{original_name}{idx}");
        idx += 1;
    }
    unique
}

fn default_value_for_param(ast: &Ast, param_node: NodeId) -> Option<String> {
    if let Some(rfp) = ast.cast::<RegularFormalParameter>(param_node)
        && let Some(clause) = ast[rfp].default_clause
    {
        let val = ast[clause].value.raw();
        let start = ast.offset(val) as usize;
        let end = ast.end(val) as usize;
        return ast.tokens.source.get(start..end).map(|s| s.to_string());
    }
    if let Some(ffp) = ast.cast::<FieldFormalParameter>(param_node)
        && let Some(clause) = ast[ffp].default_clause
    {
        let val = ast[clause].value.raw();
        let start = ast.offset(val) as usize;
        let end = ast.end(val) as usize;
        return ast.tokens.source.get(start..end).map(|s| s.to_string());
    }
    if let Some(sfp) = ast.cast::<SuperFormalParameter>(param_node)
        && let Some(clause) = ast[sfp].default_clause
    {
        let val = ast[clause].value.raw();
        let start = ast.offset(val) as usize;
        let end = ast.end(val) as usize;
        return ast.tokens.source.get(start..end).map(|s| s.to_string());
    }
    None
}

fn is_qualified_identifier(ast: &Ast, id: NodeId) -> bool {
    let Some(parent) = ast.parent(id) else {
        return false;
    };
    if let Some(pa) = ast.cast::<PropertyAccess>(parent) {
        return ast[pa].property_name.raw() == id;
    }
    if let Some(pi) = ast.cast::<PrefixedIdentifier>(parent) {
        return ast[pi].identifier.raw() == id;
    }
    if let Some(mi) = ast.cast::<MethodInvocation>(parent) {
        return ast[mi].target.is_some() && ast[mi].method_name.raw() == id;
    }
    false
}

fn is_static_member(ctx: &Ctx<'_>, id: ElementId) -> bool {
    let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, id);
    flags.contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
        || flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
}

fn is_synthetic_accessor(ctx: &Ctx<'_>, id: ElementId) -> bool {
    dartr_resolver::element_ext::first_fragment_flags(ctx, id)
        .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
}

fn is_operator_method(ctx: &Ctx<'_>, id: ElementId) -> bool {
    if id.tag() != Tag::Method {
        return false;
    }
    ctx.element_name(id).is_some_and(|n| {
        n.chars()
            .next()
            .is_some_and(|c| !(c.is_alphabetic() || c == '_' || c == '$'))
    })
}

fn is_generator_executable(ctx: &Ctx<'_>, id: ElementId) -> bool {
    dartr_resolver::element_ext::first_fragment_flags(ctx, id)
        .contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR)
}

fn is_async_executable(ctx: &Ctx<'_>, id: ElementId) -> bool {
    dartr_resolver::element_ext::first_fragment_flags(ctx, id)
        .contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS)
}

fn find_ancestor_function_body(ast: &Ast, mut node: NodeId) -> Option<NodeId> {
    while let Some(p) = ast.parent(node) {
        if ast.is::<FunctionBody>(p) {
            return Some(p);
        }
        node = p;
    }
    None
}

fn is_sync_body(ast: &Ast, body: NodeId) -> bool {
    let kw = if let Some(bfb) = ast.cast::<BlockFunctionBody>(body) {
        ast[bfb].keyword
    } else if let Some(efb) = ast.cast::<ExpressionFunctionBody>(body) {
        ast[efb].keyword
    } else {
        None
    };
    match kw {
        None => true,
        Some(tok) => ast.tokens.lexeme(tok) != "async",
    }
}

fn is_generator_body(ast: &Ast, body: NodeId) -> bool {
    if let Some(bfb) = ast.cast::<BlockFunctionBody>(body) {
        return ast[bfb].star.is_some();
    }
    false
}

fn find_enclosing_executable_element(u: &Unit<'_, '_>, body: NodeId) -> Option<ElementId> {
    let p = u.ast.parent(body)?;
    if let Some(md) = u.ast.cast::<MethodDeclaration>(p) {
        return u.declared_element(md);
    }
    if let Some(fe) = u.ast.cast::<FunctionExpression>(p)
        && let Some(pp) = u.ast.parent(fe.raw())
        && let Some(fd) = u.ast.cast::<FunctionDeclaration>(pp)
    {
        return u.declared_element(fd);
    }
    None
}

fn returns_future(ctx: &Ctx<'_>, exec: ElementId) -> bool {
    let ret_ty: Option<TypeId> = match ctx.any(exec) {
        AnyElement::Method(m) => m.return_type.get(),
        AnyElement::TopLevelFunction(f) => f.return_type.get(),
        AnyElement::LocalFunction(f) => f.return_type.get(),
        AnyElement::Getter(g) => g.return_type.get(),
        _ => None,
    };
    let Some(ty) = ret_ty else {
        return true;
    };
    if matches!(ctx.ty(ty), TypeKind::Dynamic) {
        return true;
    }
    if let TypeKind::Interface { element, .. } = *ctx.ty(ty)
        && let Some(name) = ctx.interface(element).name
    {
        let n = ctx.name_str(name);
        return n == "Future" || n == "FutureOr";
    }
    false
}

fn is_cascaded_method_invocation(ast: &Ast, mi: Id<MethodInvocation>) -> bool {
    ast[mi]
        .operator
        .is_some_and(|op| ast.tokens.lexeme(op).starts_with(".."))
}

fn is_cascaded_property_access(ast: &Ast, pa: Id<PropertyAccess>) -> bool {
    ast.tokens.lexeme(ast[pa].operator).starts_with("..")
}

fn find_ancestor_assignment(ast: &Ast, mut node: NodeId) -> Option<Id<AssignmentExpression>> {
    while let Some(p) = ast.parent(node) {
        if let Some(assign) = ast.cast::<AssignmentExpression>(p) {
            return Some(assign);
        }
        node = p;
    }
    None
}

fn find_surrounding_block_offset(ast: &Ast, mut node: NodeId) -> u32 {
    while let Some(p) = ast.parent(node) {
        if ast.is::<Block>(p) || ast.is::<CompilationUnit>(p) {
            return ast.offset(p);
        }
        node = p;
    }
    0
}

fn in_setter_context(ast: &Ast, node: NodeId) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(assign) = ast.cast::<AssignmentExpression>(parent) {
        return ast[assign].left_hand_side.raw() == node;
    }
    if let Some(p2) = ast.parent(parent)
        && let Some(assign) = ast.cast::<AssignmentExpression>(p2)
        && (ast.is::<PrefixedIdentifier>(parent) || ast.is::<PropertyAccess>(parent))
    {
        return ast[assign].left_hand_side.raw() == parent;
    }
    false
}

fn can_discard_single_quotes(ast: &Ast, expr: NodeId) -> bool {
    let lexeme = if let Some(ssl) = ast.cast::<SimpleStringLiteral>(expr) {
        ast.tokens.lexeme(ast[ssl].literal)
    } else if ast.is::<StringInterpolation>(expr) {
        let start = ast.offset(expr) as usize;
        let end = ast.end(expr) as usize;
        ast.tokens.source.get(start..end).unwrap_or("")
    } else {
        return false;
    };
    if lexeme.starts_with('r') || lexeme.starts_with("'''") || lexeme.starts_with("\"\"\"") {
        return false;
    }
    (lexeme.starts_with('\'') && lexeme.ends_with('\''))
        || (lexeme.starts_with('"') && lexeme.ends_with('"'))
}

fn get_expression_precedence(ast: &Ast, node: NodeId) -> u8 {
    if let Some(bin) = ast.cast::<BinaryExpression>(node) {
        return binary_op_precedence(ast.tokens.lexeme(ast[bin].operator));
    }
    if ast.is::<PrefixExpression>(node) || ast.is::<AwaitExpression>(node) {
        return 15;
    }
    if ast.is::<PostfixExpression>(node)
        || ast.is::<MethodInvocation>(node)
        || ast.is::<FunctionExpressionInvocation>(node)
        || ast.is::<PropertyAccess>(node)
        || ast.is::<PrefixedIdentifier>(node)
        || ast.is::<IndexExpression>(node)
    {
        return 16;
    }
    if ast.is::<AsExpression>(node) || ast.is::<IsExpression>(node) {
        return 10;
    }
    if ast.is::<ConditionalExpression>(node) {
        return 3;
    }
    if ast.is::<CascadeExpression>(node) {
        return 2;
    }
    if ast.is::<AssignmentExpression>(node)
        || ast.is::<FunctionExpression>(node)
        || ast.is::<ThrowExpression>(node)
    {
        return 1;
    }
    20
}

fn get_expression_parent_precedence(ast: &Ast, node: NodeId) -> u8 {
    let Some(parent) = ast.parent(node) else {
        return 0;
    };
    if ast.is::<ParenthesizedExpression>(parent)
        || ast.is::<IndexExpression>(parent)
        || ast.is::<ArgumentList>(parent)
        || ast.is::<NamedArgument>(parent)
    {
        return 0;
    }
    if let Some(bin) = ast.cast::<BinaryExpression>(parent) {
        let op = ast.tokens.lexeme(ast[bin].operator);
        let prec = binary_op_precedence(op);
        if ast[bin].right_operand.raw() == node && op != "??" {
            return prec + 1;
        }
        return prec;
    }
    if ast.is::<Expression>(parent) {
        return get_expression_precedence(ast, parent);
    }
    0
}

fn binary_op_precedence(op: &str) -> u8 {
    match op {
        "??" => 4,
        "||" => 5,
        "&&" => 6,
        "==" | "!=" => 7,
        "<" | ">" | "<=" | ">=" => 8,
        "|" => 9,
        "^" => 10,
        "&" => 11,
        "<<" | ">>" | ">>>" => 12,
        "+" | "-" => 13,
        "*" | "/" | "%" | "~/" => 14,
        _ => 1,
    }
}

fn ranges_intersect(a: SourceRange, b: SourceRange) -> bool {
    a.end() >= b.offset && b.end() >= a.offset
}

fn range_covers(a: SourceRange, b: SourceRange) -> bool {
    a.offset <= b.offset && b.end() <= a.end()
}

fn range_contains(r: SourceRange, offset: u32) -> bool {
    r.offset <= offset && offset <= r.end()
}

fn end_of_line(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

fn get_line_this(source: &str, offset: usize) -> usize {
    let offset = offset.min(source.len());
    source[..offset].rfind('\n').map_or(0, |p| p + 1)
}

fn get_line_prefix(source: &str, offset: usize) -> String {
    let offset = offset.min(source.len());
    let line_start = get_line_this(source, offset);
    source[line_start..offset]
        .chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .collect()
}

fn get_lines_range_statements(source: &str, range: SourceRange) -> SourceRange {
    let bytes = source.as_bytes();
    let mut start = (range.offset as usize).min(bytes.len());
    while start > 0 && (bytes[start - 1] == b' ' || bytes[start - 1] == b'\t') {
        start -= 1;
    }
    let mut end = (range.end() as usize).min(bytes.len());
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'\r' {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }
    SourceRange::new(start as u32, (end - start) as u32)
}

fn get_lines_range_skip_leading_empty(source: &str, range: SourceRange) -> SourceRange {
    let bytes = source.as_bytes();
    let mut start = (range.offset as usize).min(bytes.len());
    while start > 0 && (bytes[start - 1] == b' ' || bytes[start - 1] == b'\t') {
        start -= 1;
    }
    loop {
        let mut prev = start;
        if prev > 0 && bytes[prev - 1] == b'\n' {
            prev -= 1;
            if prev > 0 && bytes[prev - 1] == b'\r' {
                prev -= 1;
            }
            let mut line_begin = prev;
            while line_begin > 0
                && (bytes[line_begin - 1] == b' ' || bytes[line_begin - 1] == b'\t')
            {
                line_begin -= 1;
            }
            if line_begin == 0 || bytes[line_begin - 1] == b'\n' {
                start = line_begin;
                continue;
            }
        }
        break;
    }
    let mut end = (range.end() as usize).min(bytes.len());
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'\r' {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'\n' {
        end += 1;
    }
    SourceRange::new(start as u32, (end - start) as u32)
}

fn replace_source_indent_full(
    source: &str,
    old_indent: &str,
    new_indent: &str,
    include_leading: bool,
    ensure_trailing_newline: bool,
    eol: &str,
) -> String {
    let lines: Vec<&str> = source.split(eol).collect();
    if lines.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        let is_last = i == lines.len() - 1;
        if is_last && line.is_empty() {
            break;
        }
        if i == 0 && !include_leading {
            out.push_str(line);
        } else if let Some(rest) = line.strip_prefix(old_indent) {
            out.push_str(new_indent);
            out.push_str(rest);
        } else {
            out.push_str(line);
        }
        if !is_last || ensure_trailing_newline {
            out.push_str(eol);
        }
    }
    out
}

fn node_location(u: &Unit<'_, '_>, file: &str, node: NodeId) -> Location {
    Location {
        file: file.to_string(),
        offset: u.ast.offset(node),
        length: u.ast.length(node),
    }
}
