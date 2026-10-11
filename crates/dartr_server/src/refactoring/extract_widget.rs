// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/extract_widget.dart

use std::collections::{HashMap, HashSet};

use dartr_ast::*;
use dartr_element::{
    AnyElement, Ctx, EId, ElementId, InterfaceElement, NoopSink, PropertyInducingElement, Tag,
    TypeId, TypeKind,
};
use dartr_resolver::error::support::corresponding_parameter;
use dartr_typesystem::TypeExt;

use super::naming::validate_class_name;
use super::rename::ChangeAcc;
use super::status::{Location, RefactoringStatus};
use crate::completion::candidate::display_name;
use crate::correction::change::SourceChange;
use crate::element_locator::Unit;
use crate::server::ResolvedUnitRef;

const URI_FRAMEWORK: &str = "package:flutter/src/widgets/framework.dart";

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
struct Parameter {
    name: String,
    type_str: String,
    is_method_parameter: bool,
    is_required: bool,
    constructor_name: String,
}

#[derive(Clone, Debug)]
enum ExtractionTarget {
    Expression(Id<InstanceCreationExpression>),
    Statements {
        statements: Vec<Id<Statement>>,
        range: SourceRange,
    },
    Method(Id<MethodDeclaration>),
}

pub struct ExtractWidget {
    pub unit_ref: ResolvedUnitRef,
    pub offset: u32,
    pub length: u32,
    pub name: String,

    enclosing_class_node: Option<Id<ClassDeclaration>>,
    enclosing_class_element: Option<EId<InterfaceElement>>,
    enclosing_unit_member: Option<NodeId>,
    target: Option<ExtractionTarget>,
    parameters: Vec<Parameter>,
}

impl ExtractWidget {
    pub fn new(unit_ref: &ResolvedUnitRef, offset: u32, length: u32) -> Self {
        Self {
            unit_ref: unit_ref.clone(),
            offset,
            length,
            name: String::new(),
            enclosing_class_node: None,
            enclosing_class_element: None,
            enclosing_unit_member: None,
            target: None,
            parameters: Vec::new(),
        }
    }

    pub fn refactoring_name(&self) -> &'static str {
        "Extract Widget"
    }

    fn file_path(&self) -> String {
        self.unit_ref.path().to_string()
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
        !self.check_selection(&u).has_fatal_error()
    }

    pub fn check_initial_conditions(&mut self) -> RefactoringStatus {
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

        let mut result = RefactoringStatus::new();
        result.add_status(self.check_selection(&u));
        if result.has_fatal_error() {
            return result;
        }

        let ast_node = match &self.target {
            Some(ExtractionTarget::Expression(expr)) => expr.raw(),
            Some(ExtractionTarget::Method(method)) => method.raw(),
            Some(ExtractionTarget::Statements { statements, .. }) => statements[0].raw(),
            None => return result,
        };

        let mut cur = Some(ast_node);
        while let Some(n) = cur {
            if u.ast.parent(n) == Some(root.raw()) {
                self.enclosing_unit_member = Some(n);
                break;
            }
            cur = u.ast.parent(n);
        }

        result.add_status(self.initialize_parameters(&u));
        result
    }

    pub fn check_name(&self) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        result.add_status(validate_class_name(&self.name));
        if !result.has_fatal_error() {
            let sink = NoopSink;
            let ctx = self.unit_ref.ctx(&sink);
            let lib_id = self.unit_ref.library.library.library;
            for child in crate::server::library_children(&ctx, lib_id) {
                let child_name = display_name(&ctx, child);
                if child_name == self.name {
                    let kind_name = child.kind().display_name();
                    let loc = element_problem_location(&ctx, child);
                    result.add_error(
                        format!(
                            "Library already declares {kind_name} with name '{}'.",
                            self.name
                        ),
                        loc,
                    );
                }
            }
        }
        result
    }

    pub fn check_final_conditions(&self) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        result.add_status(validate_class_name(&self.name));
        result
    }

    pub fn check_all_conditions(&mut self) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        result.add_status(self.check_initial_conditions());
        if result.has_fatal_error() {
            return result;
        }
        result.add_status(self.check_name());
        if result.has_fatal_error() {
            return result;
        }
        result.add_status(self.check_final_conditions());
        result
    }

    fn check_selection(&mut self, u: &Unit<'_, '_>) -> RefactoringStatus {
        let root = self.unit_ref.unit().unit;
        let mut node = u.ast.node_covering(root, self.offset, self.length);

        if let Some(n) = node
            && let Some(ret) = u.ast.cast::<ReturnStatement>(n)
        {
            node = u.ast[ret].expression.map(|e| e.raw());
        }

        self.enclosing_class_node = None;
        self.enclosing_class_element = None;
        let mut cur = node;
        while let Some(n) = cur {
            if let Some(cls) = u.ast.cast::<ClassDeclaration>(n) {
                self.enclosing_class_node = Some(cls);
                self.enclosing_class_element = u
                    .declared_element(cls)
                    .and_then(|e| e.cast::<InterfaceElement>());
                break;
            }
            cur = u.ast.parent(n);
        }

        if let Some(n) = node
            && let Some(new_expr) = find_instance_creation_expression(u.ast, n)
            && is_widget_creation(u, new_expr)
        {
            self.target = Some(ExtractionTarget::Expression(new_expr));
            return RefactoringStatus::new();
        }

        if let Some(n) = node
            && let Some(block) = u.ast.cast::<Block>(n)
        {
            let selection_range = SourceRange::new(self.offset, self.length);
            let mut statements = Vec::new();
            for &stmt in u.ast.list(u.ast[block].statements) {
                let stmt_range =
                    SourceRange::new(u.ast.offset(stmt.raw()), u.ast.length(stmt.raw()));
                if ranges_intersect(stmt_range, selection_range) {
                    statements.push(stmt);
                }
            }
            if !statements.is_empty() {
                let last_stmt = *statements.last().unwrap();
                if let Some(ret) = u.ast.cast::<ReturnStatement>(last_stmt.raw())
                    && let Some(ret_expr) = u.ast[ret].expression
                    && is_widget_expression(u, ret_expr.raw())
                {
                    let first = statements[0].raw();
                    let last = last_stmt.raw();
                    let range = SourceRange::new(
                        u.ast.offset(first),
                        u.ast.end(last).saturating_sub(u.ast.offset(first)),
                    );
                    self.target = Some(ExtractionTarget::Statements { statements, range });
                    return RefactoringStatus::new();
                } else {
                    return RefactoringStatus::fatal(
                        "The last selected statement must return a widget.",
                    );
                }
            }
        }

        let mut walk = node;
        while let Some(n) = walk {
            if u.ast.is::<FunctionBody>(n) {
                break;
            }
            if let Some(method) = u.ast.cast::<MethodDeclaration>(n) {
                if let Some(ret_ty_node) = u.ast[method].return_type
                    && let Some(&ty) = u.tables.annotation_type.get(ret_ty_node.raw())
                    && is_widget_type(u.ctx, ty)
                {
                    self.target = Some(ExtractionTarget::Method(method));
                    return RefactoringStatus::new();
                }
                break;
            }
            walk = u.ast.parent(n);
        }

        RefactoringStatus::fatal(
            "Can only extract a widget expression or a method returning widget.",
        )
    }

    fn initialize_parameters(&mut self, u: &Unit<'_, '_>) -> RefactoringStatus {
        let mut collector = ParametersCollector::new(
            u,
            self.enclosing_class_element,
            match &self.target {
                Some(ExtractionTarget::Expression(expr)) => {
                    SourceRange::new(u.ast.offset(expr.raw()), u.ast.length(expr.raw()))
                }
                Some(ExtractionTarget::Statements { range, .. }) => *range,
                Some(ExtractionTarget::Method(method)) => {
                    SourceRange::new(u.ast.offset(method.raw()), u.ast.length(method.raw()))
                }
                None => SourceRange::new(0, 0),
            },
        );

        match &self.target {
            Some(ExtractionTarget::Expression(expr)) => {
                collector.visit(expr.raw());
            }
            Some(ExtractionTarget::Statements { statements, .. }) => {
                for stmt in statements {
                    collector.visit(stmt.raw());
                }
            }
            Some(ExtractionTarget::Method(method)) => {
                collector.visit(u.ast[*method].body.raw());
            }
            None => {}
        }

        let mut status = collector.status;
        self.parameters = collector.parameters;

        if let Some(ExtractionTarget::Method(method)) = &self.target
            && let Some(param_list) = u.ast[*method].parameters
        {
            for &param in u.ast.list(u.ast[param_list].parameters) {
                if let Some(el) = u.declared_element(param.raw())
                    && let AnyElement::FormalParameter(fp) = u.ctx.any(el)
                    && let Some(name_id) = fp.name
                {
                    let name = u.ctx.name_str(name_id).to_string();
                    let is_required = fp.kind.is_required();
                    let type_str = fp
                        .type_
                        .get()
                        .map(|ty| {
                            dartr_element::display_string::type_display_string_with(
                                u.ctx,
                                ty,
                                dartr_element::display_string::DisplayOptions::default(),
                            )
                        })
                        .unwrap_or_else(|| "dynamic".to_string());
                    self.parameters.push(Parameter {
                        name: name.clone(),
                        type_str,
                        is_method_parameter: true,
                        is_required,
                        constructor_name: name,
                    });
                }
            }
        }

        for param in &self.parameters {
            if param.name == "key" {
                status.add_error(
                    "The parameter 'key' will conflict with the widget 'key'.",
                    None,
                );
            }
        }

        let mut used_names: HashSet<String> = HashSet::new();
        for param in &self.parameters {
            if !param.name.starts_with('_') {
                used_names.insert(param.name.clone());
            }
        }

        for param in &mut self.parameters {
            let mut name = param.name.clone();
            if let Some(base_name) = name.strip_prefix('_') {
                let mut i = 1usize;
                loop {
                    let candidate = if i == 1 {
                        base_name.to_string()
                    } else {
                        format!("{base_name}{i}")
                    };
                    if used_names.insert(candidate.clone()) {
                        name = candidate;
                        break;
                    }
                    i += 1;
                }
            }
            param.constructor_name = name;
        }

        status
    }

    pub fn create_change(&self) -> SourceChange {
        let sink = NoopSink;
        let ctx = self.unit_ref.ctx(&sink);
        let cu = self.unit_ref.unit();
        let u = Unit {
            ctx: &ctx,
            ast: &cu.ast,
            tables: &cu.tables,
        };
        let source = &u.ast.tokens.source;
        let file = self.file_path();
        let eol = end_of_line(source);

        let mut acc = ChangeAcc::default();
        let Some(target) = &self.target else {
            return acc.into_change(self.refactoring_name().to_string());
        };
        let Some(enclosing_member) = self.enclosing_unit_member else {
            return acc.into_change(self.refactoring_name().to_string());
        };

        match target {
            ExtractionTarget::Expression(expr) => {
                let range = SourceRange::new(u.ast.offset(expr.raw()), u.ast.length(expr.raw()));
                let inst = self.write_widget_instantiation();
                acc.add(&file, range.offset, range.length, &inst);
            }
            ExtractionTarget::Statements { range, .. } => {
                let inst = self.write_widget_instantiation();
                acc.add(
                    &file,
                    range.offset,
                    range.length,
                    &format!("return {inst};"),
                );
            }
            ExtractionTarget::Method(method) => {
                let method_range =
                    SourceRange::new(u.ast.offset(method.raw()), u.ast.length(method.raw()));
                let lines_range = get_lines_range_skip_leading_empty(source, method_range);
                acc.add(&file, lines_range.offset, lines_range.length, "");

                if let Some(method_el) = u.declared_element(*method)
                    && let Some(class_node) = self.enclosing_class_node
                {
                    let invocations = collect_method_invocations(&u, class_node.raw(), method_el);
                    for inv in invocations {
                        let args = u.ast.list(u.ast[u.ast[inv].argument_list].arguments);
                        let mut args_by_name: HashMap<String, NodeId> = HashMap::new();
                        for &arg in args {
                            if let Some(param_el) =
                                corresponding_parameter(u.ctx, u.ast, u.tables, arg.raw())
                                && let Some(p_name) = u.ctx.element_name(param_el)
                            {
                                let expr_node =
                                    if let Some(named) = u.ast.cast::<NamedArgument>(arg) {
                                        u.ast[named].argument_expression.raw()
                                    } else {
                                        arg.raw()
                                    };
                                args_by_name.insert(p_name.to_string(), expr_node);
                            }
                        }

                        let mut call = format!("{}(", self.name);
                        let mut needs_comma = false;
                        for param in &self.parameters {
                            let arg_expr = args_by_name.get(&param.name).copied();
                            if param.is_method_parameter && arg_expr.is_none() {
                                continue;
                            }
                            if needs_comma {
                                call.push_str(", ");
                            }
                            needs_comma = true;
                            call.push_str(&param.name);
                            call.push_str(": ");
                            if param.is_method_parameter
                                && let Some(arg_node) = arg_expr
                            {
                                let start = u.ast.offset(arg_node) as usize;
                                let end = u.ast.end(arg_node) as usize;
                                call.push_str(source.get(start..end).unwrap_or(""));
                            } else {
                                call.push_str(&param.name);
                            }
                        }
                        call.push(')');
                        acc.add(
                            &file,
                            u.ast.offset(inv.raw()),
                            u.ast.length(inv.raw()),
                            &call,
                        );
                    }
                }
            }
        }

        let member_end = u.ast.end(enclosing_member);
        let decl_code = self.write_widget_declaration(&u, eol);
        acc.add(&file, member_end, 0, &decl_code);

        acc.into_change(self.refactoring_name().to_string())
    }

    fn write_widget_instantiation(&self) -> String {
        let mut out = format!("{}(", self.name);
        for (i, param) in self.parameters.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            let pub_name = corresponding_public_name(&param.constructor_name)
                .unwrap_or(&param.constructor_name);
            out.push_str(pub_name);
            out.push_str(": ");
            out.push_str(&param.name);
        }
        out.push(')');
        out
    }

    fn write_widget_declaration(&self, u: &Unit<'_, '_>, eol: &str) -> String {
        let source = &u.ast.tokens.source;
        let use_super_parameters = is_super_parameters_enabled(source);
        let params_to_init: Vec<&Parameter> = self
            .parameters
            .iter()
            .filter(|p| p.constructor_name != p.name)
            .collect();

        let mut out = String::new();
        out.push_str(eol);
        out.push_str(eol);
        out.push_str(&format!(
            "class {} extends StatelessWidget {{{eol}",
            self.name
        ));

        out.push_str(&format!("  const {}({{{eol}", self.name));
        out.push_str("    ");
        if use_super_parameters {
            out.push_str("super.key");
        } else {
            out.push_str("Key? key");
        }
        out.push(',');
        out.push_str(eol);

        for param in &self.parameters {
            out.push_str("    ");
            if param.is_required {
                out.push_str("required ");
            }
            if param.constructor_name != param.name {
                out.push_str(&param.type_str);
                out.push(' ');
                out.push_str(&param.constructor_name);
            } else {
                out.push_str("this.");
                out.push_str(&param.name);
            }
            out.push(',');
            out.push_str(eol);
        }

        out.push_str("  })");
        if !(use_super_parameters && params_to_init.is_empty()) {
            out.push_str(" : ");
            for (i, param) in params_to_init.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&param.name);
                out.push_str(" = ");
                out.push_str(&param.constructor_name);
            }
            if !use_super_parameters {
                if !params_to_init.is_empty() {
                    out.push_str(", ");
                }
                out.push_str("super(key: key)");
            }
        }
        out.push(';');
        out.push_str(eol);
        out.push_str(eol);

        if !self.parameters.is_empty() {
            for param in &self.parameters {
                out.push_str(&format!("  final {} {};{eol}", param.type_str, param.name));
            }
            out.push_str(eol);
        }

        out.push_str(&format!("  @override{eol}"));
        out.push_str("  Widget build(BuildContext context) ");
        match &self.target {
            Some(ExtractionTarget::Expression(expr)) => {
                let expr_offset = u.ast.offset(expr.raw()) as usize;
                let expr_end = u.ast.end(expr.raw()) as usize;
                let indent_old = get_line_prefix(source, expr_offset);
                let indent_new = "    ";
                let code = source.get(expr_offset..expr_end).unwrap_or("");
                let code = replace_source_indent(code, &indent_old, indent_new, eol);

                out.push_str(&format!("{{{eol}"));
                out.push_str("    return ");
                out.push_str(&code);
                out.push(';');
                out.push_str(eol);
                out.push_str(&format!("  }}{eol}"));
            }
            Some(ExtractionTarget::Statements { range, .. }) => {
                let start = range.offset as usize;
                let end = range.end() as usize;
                let indent_old = get_line_prefix(source, start);
                let indent_new = "    ";
                let code = source.get(start..end).unwrap_or("");
                let code = replace_source_indent(code, &indent_old, indent_new, eol);

                out.push_str(&format!("{{{eol}"));
                out.push_str(indent_new);
                out.push_str(&code);
                out.push_str(eol);
                out.push_str(&format!("  }}{eol}"));
            }
            Some(ExtractionTarget::Method(method)) => {
                let body = u.ast[*method].body.raw();
                let start = u.ast.offset(body) as usize;
                let end = u.ast.end(body) as usize;
                let code = source.get(start..end).unwrap_or("");
                out.push_str(code);
                out.push_str(eol);
            }
            None => {}
        }
        out.push('}');
        out
    }
}

struct ParametersCollector<'a, 'b> {
    u: &'a Unit<'b, 'b>,
    enclosing_class: Option<EId<InterfaceElement>>,
    expression_range: SourceRange,
    status: RefactoringStatus,
    unique_elements: HashSet<ElementId>,
    parameters: Vec<Parameter>,
    enclosing_classes: Option<HashSet<ElementId>>,
}

impl<'a, 'b> ParametersCollector<'a, 'b> {
    fn new(
        u: &'a Unit<'b, 'b>,
        enclosing_class: Option<EId<InterfaceElement>>,
        expression_range: SourceRange,
    ) -> Self {
        Self {
            u,
            enclosing_class,
            expression_range,
            status: RefactoringStatus::new(),
            unique_elements: HashSet::new(),
            parameters: Vec::new(),
            enclosing_classes: None,
        }
    }

    fn visit(&mut self, node: NodeId) {
        if let Some(sid) = self.u.ast.cast::<SimpleIdentifier>(node) {
            self.visit_simple_identifier(sid);
        }
        for child in self.u.ast.children(node) {
            self.visit(child);
        }
    }

    fn visit_simple_identifier(&mut self, sid: Id<SimpleIdentifier>) {
        let Some(mut element) = self.u.write_or_read_element(sid) else {
            return;
        };
        let element_name = display_name(self.u.ctx, element);
        let mut param_ty: Option<TypeId> = None;

        match element.tag() {
            Tag::Method => {
                if self.is_member_of_enclosing_class(element) {
                    self.status.add_error(
                        "Reference to an enclosing class method cannot be extracted.",
                        None,
                    );
                }
            }
            Tag::LocalVariable => {
                if let AnyElement::LocalVariable(lv) = self.u.ctx.any(element) {
                    let frag = self.u.ctx.fragment(lv.first_fragment());
                    let name_off = frag.name_offset.or(frag.first_token_offset).unwrap_or(0);
                    if !range_contains(self.expression_range, name_off) {
                        if in_setter_context(self.u.ast, sid.raw()) {
                            self.status.add_error(
                                format!("Write to '{element_name}' cannot be extracted."),
                                None,
                            );
                        } else {
                            param_ty = lv.type_.get();
                        }
                    }
                }
            }
            Tag::Getter | Tag::Setter => {
                let var = match self.u.ctx.any(element) {
                    AnyElement::Getter(g) => g.variable.get(),
                    AnyElement::Setter(s) => s.variable.get(),
                    _ => None,
                };
                if let Some(field) = var
                    && self.is_member_of_enclosing_class(field.raw())
                {
                    if in_setter_context(self.u.ast, sid.raw()) {
                        self.status.add_error(
                            format!("Write to '{element_name}' cannot be extracted."),
                            None,
                        );
                    } else {
                        param_ty = variable_type(self.u.ctx, field);
                        element = field.raw();
                    }
                }
            }
            Tag::Field => {
                if self.is_member_of_enclosing_class(element) {
                    if in_setter_context(self.u.ast, sid.raw()) {
                        self.status.add_error(
                            format!("Write to '{element_name}' cannot be extracted."),
                            None,
                        );
                    } else if let Some(prop) = element.cast::<PropertyInducingElement>() {
                        param_ty = variable_type(self.u.ctx, prop);
                    }
                }
            }
            _ => {}
        }

        if let Some(ty) = param_ty
            && self.unique_elements.insert(element)
        {
            let type_str = dartr_element::display_string::type_display_string_with(
                self.u.ctx,
                ty,
                dartr_element::display_string::DisplayOptions::default(),
            );
            self.parameters.push(Parameter {
                name: element_name.clone(),
                type_str,
                is_method_parameter: false,
                is_required: true,
                constructor_name: element_name,
            });
        }
    }

    fn is_member_of_enclosing_class(&mut self, element: ElementId) -> bool {
        let Some(enclosing_class) = self.enclosing_class else {
            return false;
        };
        let classes = self.enclosing_classes.get_or_insert_with(|| {
            let mut set = HashSet::new();
            set.insert(enclosing_class.raw());
            let this_ty = self.u.ctx.interface_this_type(enclosing_class);
            for &sup_ty in self.u.ctx.all_supertypes(this_ty).iter() {
                if let TypeKind::Interface { element: sup, .. } = *self.u.ctx.ty(sup_ty) {
                    set.insert(sup.raw());
                }
            }
            set
        });
        self.u
            .ctx
            .element_data(element)
            .and_then(|d| d.enclosing)
            .is_some_and(|enc| classes.contains(&enc))
    }
}

fn collect_method_invocations(
    u: &Unit<'_, '_>,
    root: NodeId,
    method_element: ElementId,
) -> Vec<Id<MethodInvocation>> {
    let mut out = Vec::new();
    collect_method_invocations_rec(u, root, method_element, &mut out);
    out
}

fn collect_method_invocations_rec(
    u: &Unit<'_, '_>,
    node: NodeId,
    method_element: ElementId,
    out: &mut Vec<Id<MethodInvocation>>,
) {
    if let Some(mi) = u.ast.cast::<MethodInvocation>(node)
        && u.element(u.ast[mi].method_name) == Some(method_element)
    {
        out.push(mi);
        return;
    }
    for child in u.ast.children(node) {
        collect_method_invocations_rec(u, child, method_element, out);
    }
}

fn find_instance_creation_expression(
    ast: &Ast,
    mut node: NodeId,
) -> Option<Id<InstanceCreationExpression>> {
    if ast.is::<ImportPrefixReference>(node)
        && let Some(p) = ast.parent(node)
    {
        node = p;
    }
    if ast.is::<SimpleIdentifier>(node)
        && let Some(p) = ast.parent(node)
    {
        node = p;
    }
    if ast.is::<PrefixedIdentifier>(node)
        && let Some(p) = ast.parent(node)
    {
        node = p;
    }
    if ast.is::<NamedType>(node)
        && let Some(p) = ast.parent(node)
    {
        node = p;
    }
    if ast.is::<ConstructorName>(node)
        && let Some(p) = ast.parent(node)
    {
        node = p;
    }
    ast.cast::<InstanceCreationExpression>(node)
}

fn is_widget_creation(u: &Unit<'_, '_>, node: Id<InstanceCreationExpression>) -> bool {
    let ctor_name = u.ast[node].constructor_name;
    let ctor_el = u
        .element(ctor_name)
        .or_else(|| u.ast[ctor_name].name.and_then(|id| u.element(id)))
        .or_else(|| u.element(node));
    if let Some(ctor) = ctor_el
        && let Some(enc) = u.ctx.element_data(ctor).and_then(|d| d.enclosing)
        && let Some(iface) = enc.cast::<InterfaceElement>()
    {
        return is_widget_interface(u.ctx, iface);
    }
    if let Some(&ty) = u.tables.static_type.get(node.raw()) {
        return is_widget_type(u.ctx, ty);
    }
    false
}

fn is_widget_expression(u: &Unit<'_, '_>, node: NodeId) -> bool {
    if u.ast.is::<NamedArgument>(node) {
        return false;
    }
    if let Some(p) = u.ast.parent(node) {
        if u.ast.is::<NamedType>(p) || u.ast.is::<ConstructorName>(p) {
            return false;
        }
        if let Some(pp) = u.ast.parent(p)
            && u.ast.is::<NamedType>(pp)
        {
            return false;
        }
    }
    if u.ast.is::<Expression>(node)
        && let Some(&ty) = u.tables.static_type.get(node)
    {
        return is_widget_type(u.ctx, ty);
    }
    false
}

fn is_widget_type(ctx: &Ctx<'_>, ty: TypeId) -> bool {
    let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
        return false;
    };
    is_widget_interface(ctx, element)
}

fn is_widget_interface(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> bool {
    if element.raw().tag() != Tag::Class {
        return false;
    }
    if is_exact_widget_class(ctx, element) {
        return true;
    }
    for &sup_ty in ctx.all_supertypes(ctx.interface_this_type(element)).iter() {
        if let TypeKind::Interface { element: sup, .. } = *ctx.ty(sup_ty)
            && is_exact_widget_class(ctx, sup)
        {
            return true;
        }
    }
    false
}

fn is_exact_widget_class(ctx: &Ctx<'_>, element: EId<InterfaceElement>) -> bool {
    if element.raw().tag() != Tag::Class {
        return false;
    }
    let iface = ctx.interface(element);
    let Some(el_name) = iface.name else {
        return false;
    };
    if ctx.name_str(el_name) != "Widget" {
        return false;
    }
    let Some(lib_id) = ctx.element_data(element.raw()).and_then(|d| d.library) else {
        return false;
    };
    let first_frag = ctx.get(lib_id).first_fragment();
    let uri = &*ctx.fragment(first_frag).source.uri;
    uri == URI_FRAMEWORK
}

fn variable_type(ctx: &Ctx<'_>, prop: EId<PropertyInducingElement>) -> Option<TypeId> {
    match ctx.any(prop.raw()) {
        AnyElement::Field(f) => f.property.type_.get(),
        AnyElement::TopLevelVariable(v) => v.property.type_.get(),
        _ => None,
    }
}

fn in_setter_context(ast: &Ast, node: NodeId) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(pre) = ast.cast::<PrefixExpression>(parent) {
        let op = ast.tokens.lexeme(ast[pre].operator);
        return op == "++" || op == "--";
    }
    if let Some(post) = ast.cast::<PostfixExpression>(parent) {
        let op = ast.tokens.lexeme(ast[post].operator);
        return op == "++" || op == "--";
    }
    if let Some(assign) = ast.cast::<AssignmentExpression>(parent) {
        return ast[assign].left_hand_side.raw() == node;
    }
    if let Some(for_each) = ast.cast::<ForEachPartsWithIdentifier>(parent) {
        return ast[for_each].identifier.raw() == node;
    }
    false
}

fn corresponding_public_name(name: &str) -> Option<&str> {
    let stripped = name.strip_prefix('_')?;
    if stripped.is_empty() || stripped.starts_with('_') {
        None
    } else {
        Some(stripped)
    }
}

fn is_super_parameters_enabled(source: &str) -> bool {
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("//") {
            let rest = rest.trim();
            if let Some(ver) = rest.strip_prefix("@dart") {
                let ver = ver.trim().strip_prefix('=').unwrap_or(ver).trim();
                let mut parts = ver.split('.');
                if let (Some(maj), Some(min)) = (
                    parts.next().and_then(|s| s.parse::<u32>().ok()),
                    parts.next().and_then(|s| s.parse::<u32>().ok()),
                ) {
                    return (maj, min) >= (2, 17);
                }
            }
        } else if !trimmed.is_empty() {
            break;
        }
    }
    true
}

fn ranges_intersect(a: SourceRange, b: SourceRange) -> bool {
    a.end() >= b.offset && b.end() >= a.offset
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

fn get_line_prefix(source: &str, offset: usize) -> String {
    let offset = offset.min(source.len());
    let line_start = source[..offset].rfind('\n').map_or(0, |p| p + 1);
    source[line_start..offset]
        .chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .collect()
}

fn replace_source_indent(source: &str, old_indent: &str, new_indent: &str, eol: &str) -> String {
    let lines: Vec<&str> = source.split(eol).collect();
    if lines.len() <= 1 {
        return source.to_string();
    }
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            out.push_str(line);
        } else {
            out.push_str(eol);
            if let Some(rest) = line.strip_prefix(old_indent) {
                out.push_str(new_indent);
                out.push_str(rest);
            } else {
                out.push_str(line);
            }
        }
    }
    out
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

fn element_problem_location(ctx: &Ctx<'_>, id: ElementId) -> Option<Location> {
    let first = ctx.element_data(id)?.first_fragment;
    let file = crate::navigation::fragment_path(ctx, first)?;
    let data = ctx.fragment_data(first)?;
    let offset = data.name_offset.or(data.first_token_offset).unwrap_or(0);
    let length = data.name.map(|n| ctx.name_str(n).len() as u32).unwrap_or(0);
    Some(Location {
        file,
        offset,
        length,
    })
}
