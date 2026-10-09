// Dart source: pkg/linter/lib/src/rules/deprecated_member_use_from_same_package.dart
// Dart source: pkg/analyzer/lib/src/error/element_usage_detector.dart
// Dart source: pkg/analyzer/lib/src/error/element_usage_frontier_detector.dart
// Dart source: pkg/analyzer/lib/src/error/deprecated_member_use_verifier.dart (DeprecatedElementUsageSet)

use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{
    AnyElement, ConstructorElement, DirectiveUri, EId, ElemRef, ElementId, FormalParameterElement,
    FragmentFlags, LibraryElement, Tag,
};
use dartr_typesystem::{TypeExt, member};
use std::path::PathBuf;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    registry.add(
        NodeKind::CompilationUnit,
        "deprecated_member_use_from_same_package",
        check,
    );
}

/// Dart `_Visitor.visitCompilationUnit`.
fn check(context: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    let Some(package) = context.package_root() else {
        return;
    };
    let Some(resolved) = context.resolved else {
        return;
    };
    let mut visitor = RecursiveVisitor {
        c: context,
        r: resolved,
        package,
        in_element_stack: vec![false],
        unit: node,
        out,
    };
    visitor.visit_compilation_unit(node);
}

/// Dart `normalizeDeprecationMessage`.
fn normalize_deprecation_message(message: &str) -> Option<String> {
    let message = message.trim();
    if message.is_empty() || message == "." {
        None
    } else if message.ends_with(['.', '?', '!']) {
        Some(message.to_string())
    } else {
        Some(format!("{message}."))
    }
}

struct RecursiveVisitor<'c, 'a, 'o> {
    c: &'c LinterContext<'a>,
    r: crate::ResolvedLintContext<'a>,
    /// Dart `WorkspacePackage` (its root).
    package: PathBuf,
    /// Dart `_inElementStacksMetadataOnly[0]` (the one usage set).
    /// The `CompilationUnit` node.
    unit: NodeId,
    in_element_stack: Vec<bool>,
    out: &'o mut Vec<Diagnostic>,
}

enum Step {
    Enter(NodeId),
    Pop,
}

impl RecursiveVisitor<'_, '_, '_> {
    fn ctx(&self) -> &dartr_element::Ctx<'_> {
        &self.r.ctx
    }

    // ------------------------------------------------------------------
    // DeprecatedElementUsageSet
    // ------------------------------------------------------------------

    /// Dart `ElementAnnotation.isDeprecated`.
    fn is_deprecated_annotation(&self, element: ElementId) -> bool {
        let ctx = self.ctx();
        let is_dart_core = member::library(ctx, ElemRef::Base(element))
            .is_some_and(|l| ctx.library_uri(l) == "dart:core");
        if !is_dart_core {
            return false;
        }
        match element.tag() {
            Tag::Constructor => {
                ctx.element_data(element)
                    .and_then(|d| d.enclosing)
                    .and_then(|e| ctx.element_name(e))
                    == Some("Deprecated")
            }
            Tag::Getter | Tag::Setter => ctx.element_name(element) == Some("deprecated"),
            _ => false,
        }
    }

    /// Dart `DeprecatedElementUsageSet.getTagInfo`.
    fn get_tag_info(&self, element: ElementId) -> Option<String> {
        let metadata = self.r.metadata?;
        for annotation in metadata.annotations(element) {
            let Some(annotation_element) = metadata.annotation_element(annotation) else {
                continue;
            };
            if !self.is_deprecated_annotation(annotation_element) {
                continue;
            }
            let Some(value) = metadata.annotation_value(annotation) else {
                continue;
            };
            if let Some(kind) = value.get_field("_kind") {
                let kind = kind.get_field("_name").and_then(|n| n.to_string_value());
                if kind != Some("use") {
                    continue;
                }
            }
            if matches!(annotation_element.tag(), Tag::Getter | Tag::Setter) {
                return Some(String::new());
            }
            return Some(
                value
                    .get_field("message")
                    .and_then(|m| m.to_string_value())
                    .or_else(|| value.get_field("expires").and_then(|m| m.to_string_value()))
                    .unwrap_or("")
                    .to_string(),
            );
        }
        None
    }

    // ------------------------------------------------------------------
    // ElementUsageFrontierDetector
    // ------------------------------------------------------------------

    /// Dart `pushElement`.
    fn push_element(&mut self, element: Option<ElementId>) {
        let mut new_value = *self.in_element_stack.last().unwrap();
        if !new_value && let Some(element) = element {
            new_value = self.get_tag_info(element).is_some();
        }
        self.in_element_stack.push(new_value);
    }

    /// Dart `popElement`.
    fn pop_element(&mut self) {
        self.in_element_stack.pop();
    }

    /// Dart `ElementUsageFrontierDetector.checkUsage` and
    /// `ElementUsageDetector.checkUsage`.
    fn check_usage(&mut self, element: Option<ElemRef>, node: NodeId) {
        if *self.in_element_stack.last().unwrap() {
            return;
        }
        let Some(element) = element else {
            return;
        };
        let ctx = self.r.ctx;
        let mut element = member::base_element(&ctx, element);
        // Implicit getters/setters.
        if matches!(element.tag(), Tag::Getter | Tag::Setter) {
            let accessor = EId::<dartr_element::PropertyAccessorElement>::from_raw(element);
            let data = ctx.property_accessor(accessor);
            let is_origin_variable = ctx.fragment_data(data.first_fragment).is_some_and(|f| {
                f.flags
                    .has(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
            });
            if is_origin_variable && let Some(variable) = data.variable.get() {
                element = variable.raw();
            }
        }
        if let Some(parameter) = element.cast::<FormalParameterElement>()
            && ctx.get(parameter).kind.is_required()
        {
            return;
        }
        if self
            .r
            .metadata
            .is_none_or(|m| m.annotations(element).is_empty())
        {
            return;
        }
        let Some(tag_info) = self.get_tag_info(element) else {
            return;
        };
        if self.is_local_parameter(element, node) {
            return;
        }

        let ast = self.c.ast;
        // The error entity: a node, or a token.
        let mut entity: Result<NodeId, dartr_syntax::TokenId> = Ok(node);
        let parent = ast.parent(node);
        if let Some(parent) = parent
            && let Some(assignment) = ast.cast::<AssignmentExpression>(parent)
            && ast[assignment].left_hand_side.raw() == node
        {
            if let Some(prefixed) = ast.cast::<PrefixedIdentifier>(node) {
                entity = Ok(ast[prefixed].identifier.raw());
            } else if let Some(access) = ast.cast::<PropertyAccess>(node) {
                entity = Ok(ast[access].property_name.raw());
            }
        } else if let Some(n) = ast.cast::<ExtensionOverride>(node) {
            entity = Err(ast[n].name);
        } else if let Some(n) = ast.cast::<NamedType>(node) {
            entity = Err(ast[n].name);
        } else if let Some(n) = ast.cast::<NamedArgument>(node) {
            entity = Err(ast[n].name);
        } else if let Some(n) = ast.cast::<PatternField>(node)
            && let Some(field_name) = ast[n].name
        {
            match ast[field_name].name {
                None => {
                    if let Some(name) = variable_pattern_name(ast, ast[n].pattern.raw()) {
                        entity = Err(name);
                    }
                }
                Some(name) => entity = Err(name),
            }
        }

        let display_name = if let Some(constructor) = element.cast::<ConstructorElement>() {
            let class_name = ctx
                .element_data(constructor.raw())
                .and_then(|d| d.enclosing)
                .and_then(|e| ctx.element_name(e))
                .unwrap_or("<null>");
            match ctx.element_name(constructor.raw()) {
                None | Some("") | Some("new") => class_name.to_string(),
                Some(name) => format!("{class_name}.{name}"),
            }
        } else if let Some(library) = element.cast::<LibraryElement>() {
            ctx.library_uri(library).to_string()
        } else {
            ctx.element_name(element).unwrap_or("").to_string()
        };

        // Dart `_DeprecatedElementUsageReporter.report`.
        if !self.is_library_in_workspace_package(element) {
            return;
        }
        let (offset, length) = match entity {
            Ok(node) => (ast.offset(node) as usize, ast.length(node) as usize),
            Err(token) => {
                let t = ast.tokens.get(token);
                (t.offset as usize, t.length as usize)
            }
        };
        if let Some(message) = normalize_deprecation_message(&tag_info) {
            self.c.report_offset(
                self.out,
                &diag::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITH_MESSAGE,
                offset,
                length,
                &[&display_name, &message],
            );
        } else {
            self.c.report_offset(
                self.out,
                &diag::DEPRECATED_MEMBER_USE_FROM_SAME_PACKAGE_WITHOUT_MESSAGE,
                offset,
                length,
                &[&display_name],
            );
        }
    }

    /// Dart `_isLibraryInWorkspacePackage(element.library)`.
    fn is_library_in_workspace_package(&self, element: ElementId) -> bool {
        let ctx = self.ctx();
        let library = match element.cast::<LibraryElement>() {
            Some(library) => Some(library),
            None => member::library(ctx, ElemRef::Base(element)),
        };
        let Some(library) = library else {
            return false;
        };
        let fragment = ctx.get(library).first_fragment();
        let path = ctx.fragment(fragment).source.path.clone();
        std::path::Path::new(&*path).starts_with(&self.package)
    }

    /// Dart `ElementUsageDetector._isLocalParameter`.
    fn is_local_parameter(&self, element: ElementId, node: NodeId) -> bool {
        if element.cast::<FormalParameterElement>().is_none() {
            return false;
        }
        let ctx = self.ctx();
        let defining_function = ctx.element_data(element).and_then(|d| d.enclosing);
        let ast = self.c.ast;
        let mut current = Some(node);
        while let Some(n) = current {
            let declaration = match ast.kind(n) {
                NodeKind::ConstructorDeclaration
                | NodeKind::FunctionExpression
                | NodeKind::MethodDeclaration => Some(n),
                NodeKind::PrimaryConstructorBody => primary_constructor_declaration(ast, n),
                _ => None,
            };
            if let Some(declaration) = declaration
                && let Some(declared) = self.c.declared_element(declaration)
                && Some(declared) == defining_function
            {
                return true;
            }
            current = ast.parent(n);
        }
        false
    }

    /// Dart `_invocationArguments`.
    fn invocation_arguments(&mut self, element: Option<ElemRef>, arguments: Id<ArgumentList>) {
        let Some(element) = element else {
            return;
        };
        let ctx = self.r.ctx;
        let element = member::base_element(&ctx, element);
        if !matches!(
            ctx.any(element),
            AnyElement::Constructor(_)
                | AnyElement::Method(_)
                | AnyElement::Getter(_)
                | AnyElement::Setter(_)
                | AnyElement::TopLevelFunction(_)
                | AnyElement::LocalFunction(_)
        ) {
            return;
        }
        let parameters: Vec<ElementId> = member::formal_parameters(&ctx, ElemRef::Base(element))
            .into_iter()
            .map(|p| member::base_element(&ctx, p))
            .collect();
        let ast = self.c.ast;
        let arguments: Vec<NodeId> = ast.list_raw(ast[arguments].arguments).to_vec();
        self.visit_parameters_and_arguments(&parameters, &arguments);
    }

    /// Dart `_visitParametersAndArguments`.
    fn visit_parameters_and_arguments(&mut self, parameters: &[ElementId], arguments: &[NodeId]) {
        let ctx = self.r.ctx;
        let kind = |p: ElementId| ctx.get(EId::<FormalParameterElement>::from_raw(p)).kind;
        let mut positional_index = 0;
        for &argument in arguments {
            if let Some(named) = self.c.ast.cast::<NamedArgument>(argument) {
                let name = self.c.ast.tokens.lexeme(self.c.ast[named].name);
                let parameter = parameters
                    .iter()
                    .copied()
                    .filter(|&p| kind(p).is_named())
                    .find(|&p| ctx.element_name(p) == Some(name));
                if let Some(parameter) = parameter {
                    self.check_usage(Some(ElemRef::Base(parameter)), argument);
                }
            } else if positional_index < parameters.len() {
                let parameter = parameters[positional_index];
                positional_index += 1;
                if kind(parameter).is_positional() {
                    self.check_usage(Some(ElemRef::Base(parameter)), argument);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // _RecursiveVisitor
    // ------------------------------------------------------------------

    fn element(&self, node: impl Into<NodeId>) -> Option<ElemRef> {
        self.c.element(node)
    }

    /// Dart `_RecursiveVisitor.visitCompilationUnit` and the recursion.
    fn visit_compilation_unit(&mut self, node: NodeId) {
        // Dart `node.declaredFragment?.element`: the library.
        let Some(fragment) = self.r.tables.declared_fragment.get(node).copied() else {
            return;
        };
        let Some(fragment) = fragment.cast::<dartr_element::LibraryFragment>() else {
            return;
        };
        let library = self.ctx().fragment(fragment).library;
        self.push_element(Some(library.raw()));
        let mut stack: Vec<Step> = self
            .c
            .ast
            .children(node)
            .into_iter()
            .rev()
            .map(Step::Enter)
            .collect();
        while let Some(step) = stack.pop() {
            match step {
                Step::Pop => self.pop_element(),
                Step::Enter(n) => {
                    if self.before_children(n) {
                        stack.push(Step::Pop);
                    }
                    stack.extend(self.c.ast.children(n).into_iter().rev().map(Step::Enter));
                }
            }
        }
    }

    /// The part of the visit methods before `super.visitX(node)`; returns
    /// whether an element was pushed (popped after the children).
    fn before_children(&mut self, node: NodeId) -> bool {
        let ast = self.c.ast;
        match ast.kind(node) {
            NodeKind::AssignmentExpression => {
                let lhs = ast[Id::<AssignmentExpression>::from_raw(node)]
                    .left_hand_side
                    .raw();
                let tables = self.r.tables;
                self.check_usage(tables.read_element.get(node).copied(), lhs);
                self.check_usage(tables.write_element.get(node).copied(), lhs);
                self.check_usage(self.element(node), node);
                false
            }
            NodeKind::BinaryExpression
            | NodeKind::ConstructorName
            | NodeKind::ExtensionOverride
            | NodeKind::IndexExpression
            | NodeKind::NamedType
            | NodeKind::PatternField => {
                self.check_usage(self.element(node), node);
                false
            }
            NodeKind::PrefixExpression | NodeKind::PostfixExpression => {
                let operand = match ast.kind(node) {
                    NodeKind::PrefixExpression => {
                        ast[Id::<PrefixExpression>::from_raw(node)].operand
                    }
                    _ => ast[Id::<PostfixExpression>::from_raw(node)].operand,
                }
                .raw();
                let tables = self.r.tables;
                self.check_usage(tables.read_element.get(node).copied(), operand);
                self.check_usage(tables.write_element.get(node).copied(), operand);
                self.check_usage(self.element(node), node);
                false
            }
            NodeKind::ConstructorDeclaration => {
                let element = self.c.declared_element(node);
                self.push_element(element);
                self.constructor_declaration(node, element);
                true
            }
            NodeKind::ClassDeclaration
            | NodeKind::ClassTypeAlias
            | NodeKind::EnumDeclaration
            | NodeKind::ExtensionDeclaration
            | NodeKind::ExtensionTypeDeclaration
            | NodeKind::FunctionDeclaration
            | NodeKind::FunctionTypeAlias
            | NodeKind::GenericTypeAlias
            | NodeKind::MethodDeclaration
            | NodeKind::MixinDeclaration
            | NodeKind::FieldFormalParameter
            | NodeKind::RegularFormalParameter
            | NodeKind::SuperFormalParameter => {
                let element = self.c.declared_element(node);
                self.push_element(element);
                true
            }
            NodeKind::FieldDeclaration | NodeKind::TopLevelVariableDeclaration => {
                let list = match ast.kind(node) {
                    NodeKind::FieldDeclaration => {
                        ast[Id::<FieldDeclaration>::from_raw(node)].fields
                    }
                    _ => ast[Id::<TopLevelVariableDeclaration>::from_raw(node)].variables,
                };
                let first = ast.list(ast[list].variables).first().copied();
                let element = first.and_then(|v| self.c.declared_element(v));
                self.push_element(element);
                true
            }
            NodeKind::ExportDirective => {
                let keyword = ast[Id::<ExportDirective>::from_raw(node)].export_keyword;
                let offset = ast.tokens.get(keyword).offset as i32;
                let library = self.unit_fragment().and_then(|fragment| {
                    self.ctx()
                        .fragment(fragment)
                        .library_exports
                        .iter()
                        .find(|e| e.export_keyword_offset == offset)
                        .and_then(|e| match &e.directive.uri {
                            DirectiveUri::Library { library, .. } => Some(*library),
                            _ => None,
                        })
                });
                self.check_usage(library.map(|l| ElemRef::Base(l.raw())), node);
                false
            }
            NodeKind::ImportDirective => {
                let keyword = ast[Id::<ImportDirective>::from_raw(node)].import_keyword;
                let offset = ast.tokens.get(keyword).offset as i32;
                let library = self.unit_fragment().and_then(|fragment| {
                    self.ctx()
                        .fragment(fragment)
                        .library_imports
                        .iter()
                        .find(|i| !i.is_synthetic && i.import_keyword_offset == offset)
                        .and_then(|i| match &i.directive.uri {
                            DirectiveUri::Library { library, .. } => Some(*library),
                            _ => None,
                        })
                });
                self.check_usage(library.map(|l| ElemRef::Base(l.raw())), node);
                false
            }
            NodeKind::FunctionExpressionInvocation => {
                if let Some(call) = self.element(node) {
                    let base = member::base_element(&self.r.ctx, call);
                    if base.tag() == Tag::Method && self.ctx().element_name(base) == Some("call") {
                        self.check_usage(Some(call), node);
                    }
                }
                false
            }
            NodeKind::InstanceCreationExpression => {
                let n = &ast[Id::<InstanceCreationExpression>::from_raw(node)];
                let (constructor_name, arguments) = (n.constructor_name, n.argument_list);
                self.invocation_arguments(self.element(constructor_name), arguments);
                false
            }
            NodeKind::MethodInvocation => {
                let n = &ast[Id::<MethodInvocation>::from_raw(node)];
                let (method_name, arguments) = (n.method_name, n.argument_list);
                self.invocation_arguments(self.element(method_name), arguments);
                false
            }
            NodeKind::RedirectingConstructorInvocation => {
                let arguments =
                    ast[Id::<RedirectingConstructorInvocation>::from_raw(node)].argument_list;
                let element = self.element(node);
                self.check_usage(element, node);
                self.invocation_arguments(element, arguments);
                false
            }
            NodeKind::SuperConstructorInvocation => {
                let arguments = ast[Id::<SuperConstructorInvocation>::from_raw(node)].argument_list;
                let element = self.element(node);
                self.check_usage(element, node);
                self.invocation_arguments(element, arguments);
                false
            }
            NodeKind::SimpleIdentifier => {
                self.simple_identifier(node);
                false
            }
            _ => false,
        }
    }

    /// The library fragment of the visited unit.
    fn unit_fragment(&self) -> Option<dartr_element::FId<dartr_element::LibraryFragment>> {
        let fragment = *self.r.tables.declared_fragment.get(self.unit)?;
        fragment.cast()
    }

    /// Dart `ElementUsageDetector.constructorDeclaration`.
    fn constructor_declaration(&mut self, node: NodeId, element: Option<ElementId>) {
        let ast = self.c.ast;
        let n = &ast[Id::<ConstructorDeclaration>::from_raw(node)];
        if n.factory_keyword.is_some() {
            return;
        }
        let has_constructor_invocation = ast.list_raw(n.initializers).iter().any(|&i| {
            matches!(
                ast.kind(i),
                NodeKind::SuperConstructorInvocation | NodeKind::RedirectingConstructorInvocation
            )
        });
        if has_constructor_invocation {
            return;
        }
        let Some(constructor) = element.and_then(|e| e.cast::<ConstructorElement>()) else {
            return;
        };
        let super_constructor = self.ctx().get(constructor).super_constructor.get();
        self.check_usage(super_constructor, node);
    }

    /// Dart `ElementUsageDetector.simpleIdentifier`.
    fn simple_identifier(&mut self, node: NodeId) {
        let ast = self.c.ast;
        if let Some(parent) = ast.parent(node) {
            match ast.kind(parent) {
                // Dart `inDeclarationContext()`.
                NodeKind::ImportDirective => {
                    if ast[Id::<ImportDirective>::from_raw(parent)]
                        .prefix
                        .is_some_and(|p| p.raw() == node)
                    {
                        return;
                    }
                }
                NodeKind::Label => {
                    if let Some(parent2) = ast.parent(parent)
                        && (Statement::test(ast.kind(parent2))
                            || matches!(
                                ast.kind(parent2),
                                NodeKind::SwitchCase
                                    | NodeKind::SwitchDefault
                                    | NodeKind::SwitchPatternCase
                            ))
                    {
                        return;
                    }
                }
                NodeKind::ConstructorName => {
                    if ast[Id::<ConstructorName>::from_raw(parent)]
                        .name
                        .is_some_and(|n| n.raw() == node)
                    {
                        return;
                    }
                }
                NodeKind::SuperConstructorInvocation => {
                    if ast[Id::<SuperConstructorInvocation>::from_raw(parent)]
                        .constructor_name
                        .is_some_and(|n| n.raw() == node)
                    {
                        return;
                    }
                }
                NodeKind::HideCombinator => return,
                _ => {}
            }
        }
        self.check_usage(self.element(node), node);
    }
}

/// Dart `PrimaryConstructorBody.declaration`.
fn primary_constructor_declaration(ast: &Ast, body: NodeId) -> Option<NodeId> {
    let declaration = ast.parent(ast.parent(body)?)?;
    let name_part = match ast.kind(declaration) {
        NodeKind::ClassDeclaration => ast[Id::<ClassDeclaration>::from_raw(declaration)].name_part,
        NodeKind::EnumDeclaration => ast[Id::<EnumDeclaration>::from_raw(declaration)].name_part,
        NodeKind::ExtensionTypeDeclaration => {
            ast[Id::<ExtensionTypeDeclaration>::from_raw(declaration)].name_part
        }
        _ => return None,
    };
    ast.cast::<PrimaryConstructorDeclaration>(name_part)
        .map(|p| p.raw())
}

/// Dart `DartPattern.variablePattern` (the name token of the variable).
fn variable_pattern_name(ast: &Ast, pattern: NodeId) -> Option<dartr_syntax::TokenId> {
    match ast.kind(pattern) {
        NodeKind::DeclaredVariablePattern => {
            Some(ast[Id::<DeclaredVariablePattern>::from_raw(pattern)].name)
        }
        NodeKind::AssignedVariablePattern => {
            Some(ast[Id::<AssignedVariablePattern>::from_raw(pattern)].name)
        }
        NodeKind::CastPattern => {
            variable_pattern_name(ast, ast[Id::<CastPattern>::from_raw(pattern)].pattern.raw())
        }
        NodeKind::NullAssertPattern => variable_pattern_name(
            ast,
            ast[Id::<NullAssertPattern>::from_raw(pattern)]
                .pattern
                .raw(),
        ),
        NodeKind::NullCheckPattern => variable_pattern_name(
            ast,
            ast[Id::<NullCheckPattern>::from_raw(pattern)].pattern.raw(),
        ),
        NodeKind::ParenthesizedPattern => variable_pattern_name(
            ast,
            ast[Id::<ParenthesizedPattern>::from_raw(pattern)]
                .pattern
                .raw(),
        ),
        _ => None,
    }
}
