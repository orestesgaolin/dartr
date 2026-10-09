// Dart source: pkg/analyzer/lib/src/error/element_usage_detector.dart

//! `ElementUsageDetector`: finds the usages of the elements of a set of
//! usage sets (deprecated, experimental, `@doNotSubmit`) and reports them.
//!
//! Dart parameterizes the detector with `UsageSetAndReporter` objects; the
//! three sets of the analyzer are the variants of [`UsageSet`] (all of them
//! rely only on the element metadata, so Dart's `usagesArbitrary` is
//! empty). The frontier variant (`element_usage_frontier_detector.dart`)
//! adds the element stacks, see
//! [`super::element_usage_frontier_detector`].

use std::sync::Arc;

use dartr_ast::{
    Annotation, ArgumentList, AssignmentExpression, Ast, BinaryExpression, ConstructorDeclaration,
    ConstructorName, DotShorthandConstructorInvocation, DotShorthandInvocation,
    DotShorthandPropertyAccess, ExportDirective, ExtensionOverride, FormalParameterList,
    FunctionExpression, FunctionExpressionInvocation, HideCombinator, Id, ImportDirective,
    IndexExpression, InstanceCreationExpression, MethodDeclaration, MethodInvocation,
    NamedArgument, NamedType, NodeId, PatternField, PostfixExpression, PrefixExpression,
    PrefixedIdentifier, PrimaryConstructorBody, PropertyAccess, RedirectingConstructorInvocation,
    SimpleIdentifier, SuperConstructorInvocation, SuperFormalParameter,
};
use dartr_element::{ElementId, FormalParameterElement, Tag, TypeKind};
use dartr_typesystem::{TypeExt, member};

use super::support::{
    Range, declared_element, display_name, element_of, in_declaration_context, library_export,
    library_import, library_of, node_range, read_element, token_range, uri_library,
    variable_pattern_name, write_element,
};
use super::{UnitVerifier, deprecated_member_use_verifier as deprecated};
use super::{
    do_not_submit_member_use_verifier as do_not_submit,
    experimental_member_use_verifier as experimental,
};
use crate::ast_ext::formal_parameter_parts;
use crate::element_metadata::{UnitAst, WorkspacePackage, accessor_variable};

/// One `UsageSetAndReporter` of the analyzer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageSet {
    /// `DeprecatedElementUsageSet` / `DeprecatedElementUsageReporter`.
    Deprecated,
    /// `ExperimentalElementUsageSet` / `ExperimentalElementUsageReporter`.
    Experimental,
    /// `DoNotSubmitElementUsageSet` / `DoNotSubmitElementUsageReporter`.
    DoNotSubmit,
}

/// The tag information of an element in a [`UsageSet`].
#[derive(Clone, Debug)]
pub enum TagInfo {
    /// The deprecation message (empty: none).
    Deprecated(String),
    Unit,
}

impl UsageSet {
    /// Dart `ElementUsageSet.getTagInfo(element, metadata)`.
    pub fn get_tag_info(self, v: &UnitVerifier<'_>, element: ElementId) -> Option<TagInfo> {
        let unit = Some(UnitAst {
            ast: v.ast,
            tables: v.tables,
        });
        match self {
            UsageSet::Deprecated => {
                deprecated::get_tag_info(&v.ctx, element, unit).map(TagInfo::Deprecated)
            }
            UsageSet::Experimental => {
                experimental::get_tag_info(&v.ctx, element, unit).map(|_| TagInfo::Unit)
            }
            UsageSet::DoNotSubmit => {
                do_not_submit::get_tag_info(&v.ctx, element, unit).map(|_| TagInfo::Unit)
            }
        }
    }

    /// Dart `ElementUsageReporter.report`.
    fn report(
        self,
        v: &mut UnitVerifier<'_>,
        site: Range,
        display_name: &str,
        tag: &TagInfo,
        same_package: bool,
    ) {
        match self {
            UsageSet::Deprecated => {
                let message = match tag {
                    TagInfo::Deprecated(m) => m.as_str(),
                    TagInfo::Unit => "",
                };
                deprecated::report(v, site, display_name, message, same_package)
            }
            UsageSet::Experimental => experimental::report(v, site, display_name, same_package),
            UsageSet::DoNotSubmit => do_not_submit::report(v, site, display_name),
        }
    }
}

/// Dart `ElementUsageDetector`.
pub struct ElementUsageDetector {
    /// Dart `_workspacePackage`.
    pub workspace_package: Option<Arc<WorkspacePackage>>,
    /// Dart `usagesMetadataOnly`.
    pub usages_metadata_only: Vec<UsageSet>,
    /// Dart `ElementUsageFrontierDetector._inElementStacksMetadataOnly`;
    /// `None` for a plain detector.
    pub in_element_stacks: Option<Vec<Vec<bool>>>,
}

impl ElementUsageDetector {
    pub fn new(workspace_package: Option<Arc<WorkspacePackage>>, usages: Vec<UsageSet>) -> Self {
        ElementUsageDetector {
            workspace_package,
            usages_metadata_only: usages,
            in_element_stacks: None,
        }
    }

    /// Dart `shouldCheckMetadataOnlyForIndex(i)`.
    fn should_check_metadata_only_for_index(&self, i: usize) -> bool {
        match &self.in_element_stacks {
            Some(stacks) => !stacks[i].last().copied().unwrap_or(false),
            None => true,
        }
    }

    /// Dart `annotation(node)`.
    pub fn annotation(&mut self, v: &mut UnitVerifier<'_>, node: Id<Annotation>) {
        let ast = v.ast;
        let element = element_of(&v.ctx, v.tables, node)
            .or_else(|| crate::element_metadata::annotation_element(&v.ctx, ast, node, v.fragment));
        self.check_usage(v, element, ast[node].name.raw());
        if let Some(arguments) = ast[node].arguments {
            self.invocation_arguments(v, element, arguments);
        }
    }

    /// Dart `assignmentExpression(node)`.
    pub fn assignment_expression(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<AssignmentExpression>,
    ) {
        let lhs = v.ast[node].left_hand_side.raw();
        let read = read_element(&v.ctx, v.tables, node);
        self.check_usage(v, read, lhs);
        let write = write_element(&v.ctx, v.tables, node);
        self.check_usage(v, write, lhs);
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `binaryExpression(node)`.
    pub fn binary_expression(&mut self, v: &mut UnitVerifier<'_>, node: Id<BinaryExpression>) {
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `checkUsage(element, node)`: reports the usage of [element] at
    /// [node].
    pub fn check_usage(
        &mut self,
        v: &mut UnitVerifier<'_>,
        element: Option<ElementId>,
        node: NodeId,
    ) {
        let Some(mut element) = element else {
            return;
        };
        // Implicit getters/setters.
        if let Some(variable) = accessor_variable(&v.ctx, element) {
            element = variable;
        }
        if let Some(p) = element.cast::<FormalParameterElement>()
            && dartr_typesystem::type_ext::is_required(v.ctx.get(p).kind)
        {
            return;
        }

        let mut gives_non_null_results: Vec<(UsageSet, TagInfo)> = Vec::new();
        for i in 0..self.usages_metadata_only.len() {
            if !self.should_check_metadata_only_for_index(i) {
                continue;
            }
            let usage = self.usages_metadata_only[i];
            if let Some(tag) = usage.get_tag_info(v, element) {
                gives_non_null_results.push((usage, tag));
            }
        }
        if gives_non_null_results.is_empty() {
            return;
        }

        let ast = v.ast;
        if is_local_parameter(v, element, Some(node)) {
            return;
        }

        let mut error_entity = node_range(ast, node);
        let parent = ast.parent(node);
        if let Some(assignment) = parent.and_then(|p| ast.cast::<AssignmentExpression>(p))
            && ast[assignment].left_hand_side.raw() == node
        {
            if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
                error_entity = node_range(ast, ast[p].identifier);
            } else if let Some(p) = ast.cast::<PropertyAccess>(node) {
                error_entity = node_range(ast, ast[p].property_name);
            }
        } else if let Some(n) = ast.cast::<ExtensionOverride>(node) {
            error_entity = token_range(ast, ast[n].name);
        } else if let Some(n) = ast.cast::<NamedType>(node) {
            error_entity = token_range(ast, ast[n].name);
        } else if let Some(n) = ast.cast::<NamedArgument>(node) {
            error_entity = token_range(ast, ast[n].name);
        } else if let Some(n) = ast.cast::<PatternField>(node)
            && let Some(field_name) = ast[n].name
        {
            match ast[field_name].name {
                None => {
                    if let Some(name) = variable_pattern_name(ast, ast[n].pattern.raw()) {
                        error_entity = token_range(ast, name);
                    }
                }
                Some(name) => error_entity = token_range(ast, name),
            }
        }

        let ctx = v.ctx;
        let mut name = display_name(&ctx, element);
        if element.tag() == Tag::Library {
            if let Some(library) = library_of(&ctx, element) {
                name = ctx.library_uri(library).to_string();
            }
        } else if element.tag() != Tag::Constructor
            && name == "call"
            && let Some(invocation) = ast.cast::<MethodInvocation>(node)
            && let Some(&invoke_type) = v.tables.invoke_type.get(invocation.raw())
            && let TypeKind::Interface { element: class, .. } = *ctx.ty(invoke_type)
        {
            name = format!("{}.{name}", ctx.element_name(class.raw()).unwrap_or(""));
        }

        let same_package = match (&self.workspace_package, library_of(&ctx, element)) {
            (Some(p), Some(library)) => p.contains_library(&ctx, library),
            _ => false,
        };
        for (usage, tag) in gives_non_null_results {
            usage.report(v, error_entity, &name, &tag, same_package);
        }
    }

    /// Dart `constructorDeclaration(node)`.
    pub fn constructor_declaration(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<ConstructorDeclaration>,
    ) {
        let ast = v.ast;
        if ast[node].factory_keyword.is_some() {
            return;
        }
        let has_constructor_invocation = ast.list(ast[node].initializers).iter().any(|&i| {
            ast.is::<SuperConstructorInvocation>(i) || ast.is::<RedirectingConstructorInvocation>(i)
        });
        if has_constructor_invocation {
            return;
        }
        let super_constructor = declared_element(&v.ctx, v.tables, node)
            .and_then(|e| e.cast::<dartr_element::ConstructorElement>())
            .and_then(|c| v.ctx.get(c).super_constructor.get())
            .map(|c| member::base_element(&v.ctx, c));
        self.check_usage(v, super_constructor, node.raw());
    }

    /// Dart `constructorName(node)`.
    pub fn constructor_name(&mut self, v: &mut UnitVerifier<'_>, node: Id<ConstructorName>) {
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `dotShorthandConstructorInvocation(node)`.
    pub fn dot_shorthand_constructor_invocation(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        let ast = v.ast;
        let element = element_of(&v.ctx, v.tables, node);
        if let Some(interface) = element
            .and_then(|e| v.ctx.element_data(e))
            .and_then(|d| d.enclosing)
        {
            self.check_usage(v, Some(interface), node.raw());
        }
        let constructor = element_of(&v.ctx, v.tables, ast[node].constructor_name);
        self.invocation_arguments(v, constructor, ast[node].argument_list);
    }

    /// Dart `dotShorthandInvocation(node)`.
    pub fn dot_shorthand_invocation(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<DotShorthandInvocation>,
    ) {
        let ast = v.ast;
        let element = element_of(&v.ctx, v.tables, ast[node].member_name);
        if let Some(interface) = element
            .and_then(|e| v.ctx.element_data(e))
            .and_then(|d| d.enclosing)
        {
            self.check_usage(v, Some(interface), node.raw());
        }
        self.invocation_arguments(v, element, ast[node].argument_list);
    }

    /// Dart `dotShorthandPropertyAccess(node)`.
    pub fn dot_shorthand_property_access(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<DotShorthandPropertyAccess>,
    ) {
        let ast = v.ast;
        let element = element_of(&v.ctx, v.tables, ast[node].property_name);
        if let Some(interface) = element
            .and_then(|e| v.ctx.element_data(e))
            .and_then(|d| d.enclosing)
        {
            self.check_usage(v, Some(interface), node.raw());
        }
    }

    /// Dart `exportDirective(node)`.
    pub fn export_directive(&mut self, v: &mut UnitVerifier<'_>, node: Id<ExportDirective>) {
        let library = library_export(v, node).and_then(|e| uri_library(&e.directive.uri));
        self.check_usage(v, library.map(|l| l.raw()), node.raw());
    }

    /// Dart `extensionOverride(node)`.
    pub fn extension_override(&mut self, v: &mut UnitVerifier<'_>, node: Id<ExtensionOverride>) {
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `formalParameter(node)`.
    pub fn formal_parameter(&mut self, v: &mut UnitVerifier<'_>, node: NodeId) {
        let ast = v.ast;
        let Some(parent) = ast
            .parent(node)
            .and_then(|p| ast.cast::<FormalParameterList>(p))
        else {
            return;
        };
        let Some(constructor) = ast
            .parent(parent)
            .and_then(|p| ast.cast::<ConstructorDeclaration>(p))
        else {
            return;
        };
        let Some(redirected) = ast[constructor].redirected_constructor else {
            return;
        };
        let Some(redirected_constructor) = v.tables.element.get(redirected.raw()).copied() else {
            return;
        };
        let ctx = v.ctx;
        let parameters: Vec<ElementId> = member::formal_parameters(&ctx, redirected_constructor)
            .into_iter()
            .map(|p| member::base_element(&ctx, p))
            .collect();
        let parts = formal_parameter_parts(ast, node);
        let is_named = matches!(
            parts.kind,
            dartr_ast::ParameterKind::Named | dartr_ast::ParameterKind::NamedRequired
        );
        if is_named {
            let name = parts.name.map(|t| ast.tokens.lexeme(t));
            let redirected_parameter = parameters
                .iter()
                .copied()
                .find(|&p| parameter_is_named(&ctx, p) && ctx.element_name(p) == name);
            self.check_usage(v, redirected_parameter, node);
        } else {
            let Some(position) = ast
                .list(ast[parent].parameters)
                .iter()
                .position(|p| p.raw() == node)
            else {
                return;
            };
            if position >= parameters.len() {
                return;
            }
            let redirected_parameter = parameters[position];
            if parameter_is_named(&ctx, redirected_parameter) {
                return;
            }
            self.check_usage(v, Some(redirected_parameter), node);
        }
    }

    /// Dart `functionExpressionInvocation(node)`.
    pub fn function_expression_invocation(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<FunctionExpressionInvocation>,
    ) {
        let element = element_of(&v.ctx, v.tables, node);
        if let Some(e) = element
            && e.tag() == Tag::Method
            && v.ctx.element_name(e) == Some("call")
        {
            self.check_usage(v, Some(e), node.raw());
        }
    }

    /// Dart `importDirective(node)`.
    pub fn import_directive(&mut self, v: &mut UnitVerifier<'_>, node: Id<ImportDirective>) {
        let library = library_import(v, node).and_then(|i| uri_library(&i.directive.uri));
        self.check_usage(v, library.map(|l| l.raw()), node.raw());
    }

    /// Dart `indexExpression(node)`.
    pub fn index_expression(&mut self, v: &mut UnitVerifier<'_>, node: Id<IndexExpression>) {
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `instanceCreationExpression(node)`.
    pub fn instance_creation_expression(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<InstanceCreationExpression>,
    ) {
        let ast = v.ast;
        let element = element_of(&v.ctx, v.tables, ast[node].constructor_name);
        self.invocation_arguments(v, element, ast[node].argument_list);
    }

    /// Dart `methodInvocation(node)`.
    pub fn method_invocation(&mut self, v: &mut UnitVerifier<'_>, node: Id<MethodInvocation>) {
        let ast = v.ast;
        let element = element_of(&v.ctx, v.tables, ast[node].method_name);
        self.invocation_arguments(v, element, ast[node].argument_list);
    }

    /// Dart `namedType(node)`.
    pub fn named_type(&mut self, v: &mut UnitVerifier<'_>, node: Id<NamedType>) {
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `patternField(node)`.
    pub fn pattern_field(&mut self, v: &mut UnitVerifier<'_>, node: Id<PatternField>) {
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `postfixExpression(node)`.
    pub fn postfix_expression(&mut self, v: &mut UnitVerifier<'_>, node: Id<PostfixExpression>) {
        let operand = v.ast[node].operand.raw();
        let read = read_element(&v.ctx, v.tables, node);
        self.check_usage(v, read, operand);
        let write = write_element(&v.ctx, v.tables, node);
        self.check_usage(v, write, operand);
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `prefixExpression(node)`.
    pub fn prefix_expression(&mut self, v: &mut UnitVerifier<'_>, node: Id<PrefixExpression>) {
        let operand = v.ast[node].operand.raw();
        let read = read_element(&v.ctx, v.tables, node);
        self.check_usage(v, read, operand);
        let write = write_element(&v.ctx, v.tables, node);
        self.check_usage(v, write, operand);
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `redirectingConstructorInvocation(node)`.
    pub fn redirecting_constructor_invocation(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
        self.invocation_arguments(v, element, v.ast[node].argument_list);
    }

    /// Dart `simpleIdentifier(node)`.
    pub fn simple_identifier(&mut self, v: &mut UnitVerifier<'_>, node: Id<SimpleIdentifier>) {
        let ast = v.ast;
        // Don't report declared identifiers.
        if in_declaration_context(ast, node) {
            return;
        }
        if let Some(parent) = ast.parent(node) {
            // Report full ConstructorName, not just the constructor name.
            if let Some(c) = ast.cast::<ConstructorName>(parent)
                && ast[c].name == Some(node)
            {
                return;
            }
            // Report full SuperConstructorInvocation, not just the
            // constructor name.
            if let Some(s) = ast.cast::<SuperConstructorInvocation>(parent)
                && ast[s].constructor_name == Some(node)
            {
                return;
            }
            // HideCombinator is forgiving.
            if ast.is::<HideCombinator>(parent) {
                return;
            }
        }
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
    }

    /// Dart `superConstructorInvocation(node)`.
    pub fn super_constructor_invocation(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<SuperConstructorInvocation>,
    ) {
        let element = element_of(&v.ctx, v.tables, node);
        self.check_usage(v, element, node.raw());
        self.invocation_arguments(v, element, v.ast[node].argument_list);
    }

    /// Dart `superFormalParameter(node)`.
    pub fn super_formal_parameter(
        &mut self,
        v: &mut UnitVerifier<'_>,
        node: Id<SuperFormalParameter>,
    ) {
        let element = declared_element(&v.ctx, v.tables, node);
        if let Some(p) = element.and_then(|e| super_constructor_parameter(v, e)) {
            self.check_usage(v, Some(p), node.raw());
        }
    }

    /// Dart `_invocationArguments(element, arguments)`.
    fn invocation_arguments(
        &mut self,
        v: &mut UnitVerifier<'_>,
        element: Option<ElementId>,
        arguments: Id<ArgumentList>,
    ) {
        let Some(element) = element else {
            return;
        };
        if !matches!(
            element.tag(),
            Tag::Constructor
                | Tag::Method
                | Tag::Getter
                | Tag::Setter
                | Tag::TopLevelFunction
                | Tag::LocalFunction
        ) {
            return;
        }
        let ctx = v.ctx;
        let parameters: Vec<ElementId> = member::formal_parameters(&ctx, element.into())
            .into_iter()
            .map(|p| member::base_element(&ctx, p))
            .collect();
        let args: Vec<NodeId> = v
            .ast
            .list(v.ast[arguments].arguments)
            .iter()
            .map(|a| a.raw())
            .collect();
        self.visit_parameters_and_arguments(v, &parameters, &args);
    }

    /// Dart `_visitParametersAndArguments(parameters, arguments)`.
    fn visit_parameters_and_arguments(
        &mut self,
        v: &mut UnitVerifier<'_>,
        parameters: &[ElementId],
        arguments: &[NodeId],
    ) {
        let ast = v.ast;
        let ctx = v.ctx;
        let mut positional_index = 0;
        for &argument in arguments {
            if let Some(named) = ast.cast::<NamedArgument>(argument) {
                let name = ast.tokens.lexeme(ast[named].name);
                let parameter = parameters
                    .iter()
                    .copied()
                    .find(|&p| parameter_is_named(&ctx, p) && ctx.element_name(p) == Some(name));
                if parameter.is_some() {
                    self.check_usage(v, parameter, argument);
                }
            } else if positional_index < parameters.len() {
                let parameter = parameters[positional_index];
                positional_index += 1;
                if !parameter_is_named(&ctx, parameter) {
                    self.check_usage(v, Some(parameter), argument);
                }
            }
        }
    }
}

fn parameter_is_named(ctx: &dartr_element::Ctx<'_>, p: ElementId) -> bool {
    p.cast::<FormalParameterElement>()
        .is_some_and(|p| dartr_typesystem::type_ext::is_named(ctx.get(p).kind))
}

/// Dart `SuperFormalParameterElement.superConstructorParameter`.
fn super_constructor_parameter(v: &UnitVerifier<'_>, parameter: ElementId) -> Option<ElementId> {
    if parameter.tag() != Tag::SuperFormalParameter {
        return None;
    }
    let ctx = v.ctx;
    let constructor = ctx
        .element_data(parameter)?
        .enclosing?
        .cast::<dartr_element::ConstructorElement>()?;
    let super_constructor = ctx.get(constructor).super_constructor.get()?;
    let parameters: Vec<ElementId> = member::formal_parameters(&ctx, super_constructor)
        .into_iter()
        .map(|p| member::base_element(&ctx, p))
        .collect();
    if parameter_is_named(&ctx, parameter) {
        let name = ctx.element_name(parameter);
        return parameters
            .into_iter()
            .find(|&s| parameter_is_named(&ctx, s) && ctx.element_name(s) == name);
    }
    // The index among the positional super parameters.
    let own: Vec<ElementId> = member::formal_parameters(&ctx, constructor.raw().into())
        .into_iter()
        .map(|p| member::base_element(&ctx, p))
        .collect();
    let index = own
        .iter()
        .filter(|&&q| q.tag() == Tag::SuperFormalParameter && !parameter_is_named(&ctx, q))
        .position(|&q| q == parameter)?;
    parameters
        .into_iter()
        .filter(|&s| !parameter_is_named(&ctx, s))
        .nth(index)
}

/// Dart `_isLocalParameter(element, node)`: whether [element] is a formal
/// parameter declared in an enclosing function of [node].
fn is_local_parameter(v: &UnitVerifier<'_>, element: ElementId, mut node: Option<NodeId>) -> bool {
    if element.cast::<FormalParameterElement>().is_none() {
        return false;
    }
    let ast: &Ast = v.ast;
    let Some(defining_function) = v.ctx.element_data(element).and_then(|d| d.enclosing) else {
        return false;
    };
    while let Some(n) = node {
        let declaration = if ast.is::<ConstructorDeclaration>(n)
            || ast.is::<FunctionExpression>(n)
            || ast.is::<MethodDeclaration>(n)
        {
            Some(n)
        } else if let Some(body) = ast.cast::<PrimaryConstructorBody>(n) {
            primary_constructor_declaration(ast, body)
        } else {
            None
        };
        if let Some(d) = declaration
            && declared_element(&v.ctx, v.tables, d) == Some(defining_function)
        {
            return true;
        }
        node = ast.parent(n);
    }
    false
}

/// Dart `PrimaryConstructorBody.declaration`: the primary constructor of
/// the enclosing declaration.
pub fn primary_constructor_declaration(
    ast: &Ast,
    body: Id<PrimaryConstructorBody>,
) -> Option<NodeId> {
    let class_body = ast.parent(body)?;
    let declaration = ast.parent(class_body)?;
    let part = if let Some(c) = ast.cast::<dartr_ast::ClassDeclaration>(declaration) {
        ast[c].name_part.raw()
    } else if let Some(e) = ast.cast::<dartr_ast::EnumDeclaration>(declaration) {
        ast[e].name_part.raw()
    } else if let Some(e) = ast.cast::<dartr_ast::ExtensionTypeDeclaration>(declaration) {
        ast[e].name_part.raw()
    } else {
        return None;
    };
    ast.is::<dartr_ast::PrimaryConstructorDeclaration>(part)
        .then_some(part)
}
