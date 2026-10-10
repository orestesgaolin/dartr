// Dart source: pkg/analyzer/lib/src/analysis_rule/analysis_rule.dart
// Dart source: pkg/analyzer/lib/src/analysis_rule/rule_context.dart
// Dart source: pkg/analyzer/lib/src/lint/linter_visitor.dart
// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_computeLints)

use dartr_ast::{Ast, NodeId, NodeKind};
use dartr_ast_builder::ParsedUnit;
use dartr_diagnostics::{Diagnostic, DiagnosticCode};
use dartr_syntax::TokenId;
use indexmap::{IndexMap, IndexSet};

mod analysis_rule_timers;
pub mod constants;
mod ignore_info;
pub mod rules;
pub use analysis_rule_timers::{AnalysisRuleTimers, RuleTimer};
pub use ignore_info::IgnoreInfo;
mod registry;
mod rule_metadata;
mod subscriptions;
pub use dartr_parser::experimental_flags::ExperimentalFlag;
pub use registry::*;
pub use rule_metadata::ALL_RULES;
pub type LintDiagnostic = Diagnostic;
#[derive(Clone, Copy)]
pub struct RuleContextUnit<'a> {
    pub parsed: &'a ParsedUnit,
    pub source: &'a str,
    pub path: &'a str,
}
/// Semantic results of the visited unit. Its local arena must remain alive.
#[derive(Clone, Copy)]
pub struct ResolvedLintContext<'a> {
    pub ctx: dartr_element::Ctx<'a>,
    pub tables: &'a dartr_element::ResolutionTables,
    /// Resolver `LocalVariableInfo.potentiallyMutatedInScope` facts.
    pub potentially_mutated_in_scope: &'a IndexSet<dartr_element::ElementId>,
    /// Resolver `corresponding_parameter_type`: argument expression → Dart
    /// `correspondingParameter.type`.
    pub corresponding_parameter_type: &'a dartr_ast::NodeMap<dartr_element::TypeId>,
    pub library: dartr_element::EId<dartr_element::LibraryElement>,
    /// Dart `Element.metadata` of elements of any library, with the
    /// annotation values (`None`: no metadata access).
    pub metadata: Option<&'a dyn ElementMetadata>,
}

/// An `Annotation` node of a resolved unit of the analyzed library or of
/// another library (an opaque reference for [`ElementMetadata`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AnnotationRef {
    pub unit: u32,
    pub node: NodeId,
}

/// Dart `Element.metadata`: the annotations of an element, their elements
/// (`ElementAnnotation.element`) and values
/// (`ElementAnnotation.computeConstantValue()`).
pub trait ElementMetadata {
    /// Dart `element.metadata.annotations`.
    fn annotations(&self, element: dartr_element::ElementId) -> Vec<AnnotationRef>;
    /// Dart `ElementAnnotation.element`, as a base element.
    fn annotation_element(&self, annotation: AnnotationRef) -> Option<dartr_element::ElementId>;
    /// Dart `ElementAnnotation.computeConstantValue()`.
    fn annotation_value(&self, annotation: AnnotationRef)
    -> Option<dartr_constant::DartObjectImpl>;
    /// Dart `VariableElement.computeConstantValue()` (also the default value
    /// of a formal parameter).
    fn element_constant_value(
        &self,
        element: dartr_element::ElementId,
    ) -> Option<dartr_constant::DartObjectImpl>;
    /// Dart `Expression.computeConstantValue()?.value` of [node] in the unit
    /// with the index [unit] of the analyzed library.
    fn expression_constant_value(
        &self,
        unit: u32,
        node: NodeId,
    ) -> Option<dartr_constant::DartObjectImpl>;
}
/// A resolved AST and its original parse metadata. Resolution can rewrite nodes.
#[derive(Clone, Copy)]
pub struct ResolvedRuleContextUnit<'a> {
    pub parsed: &'a ParsedUnit,
    pub ast: &'a Ast,
    pub unit: NodeId,
    pub source: &'a str,
    pub path: &'a str,
    pub resolved: Option<ResolvedLintContext<'a>>,
}
#[derive(Clone, Copy)]
pub struct LinterContext<'a> {
    pub parsed: &'a ParsedUnit,
    pub ast: &'a Ast,
    pub source: &'a str,
    pub path: &'a str,
    pub all_units: &'a [RuleContextUnit<'a>],
    pub current_unit: usize,
    pub resolved: Option<ResolvedLintContext<'a>>,
    pub resolved_units: &'a [ResolvedRuleContextUnit<'a>],
}
pub type RuleContext<'a> = LinterContext<'a>;
impl<'a> LinterContext<'a> {
    /// The same library context, using the selected unit's AST and local arena.
    pub fn resolved_unit(&self, index: usize) -> Option<Self> {
        let unit = self.resolved_units.get(index)?;
        Some(Self {
            parsed: unit.parsed,
            ast: unit.ast,
            source: unit.source,
            path: unit.path,
            current_unit: index,
            resolved: unit.resolved,
            ..*self
        })
    }
    pub fn constant_value(
        &self,
        node: impl Into<NodeId>,
    ) -> Option<dartr_constant::DartObjectImpl> {
        constants::constant_value(self, node.into())
    }
    pub fn default_value(
        &self,
        parameter: dartr_element::ElemRef,
    ) -> Option<dartr_constant::DartObjectImpl> {
        constants::default_value(self, parameter)
    }
    pub fn constant_type_system(&self) -> Option<constants::ConstantTypeSystem<'a>> {
        Some(constants::ConstantTypeSystem(self.resolved?.ctx))
    }
    /// The texts of `DartType` diagnostic arguments: Dart `convertTypeNames`
    /// (`getDisplayString(preferTypeAlias: true)`, disambiguated when two
    /// types have the same display string).
    pub fn type_argument_texts(&self, types: &[dartr_element::TypeId]) -> Vec<String> {
        let Some(resolved) = self.resolved else {
            return Vec::new();
        };
        let arguments: Vec<dartr_diagnostics::DiagnosticArg> = types
            .iter()
            .map(|&ty| {
                dartr_diagnostics::DiagnosticArg::Type(dartr_element::diagnostics::type_arg(
                    &resolved.ctx,
                    ty,
                ))
            })
            .collect();
        dartr_diagnostics::convert_type_names(&arguments).0
    }
    /// The text of one `DartType` diagnostic argument.
    pub fn type_argument_text(&self, ty: dartr_element::TypeId) -> String {
        self.type_argument_texts(&[ty]).pop().unwrap_or_default()
    }
    /// Dart `writeOrReadElement` of an identifier (`_writeElement(node) ??
    /// element`, analyzer `ast/extensions.dart`).
    pub fn write_or_read_element(&self, node: impl Into<NodeId>) -> Option<dartr_element::ElemRef> {
        fn write_element(c: &LinterContext<'_>, node: NodeId) -> Option<dartr_element::ElemRef> {
            use dartr_ast::*;
            let parent = c.ast.parent(node)?;
            let tables = c.resolved?.tables;
            let is = |child: NodeId| child == node;
            match c.ast.kind(parent) {
                NodeKind::AssignmentExpression
                    if is(c.ast[Id::<AssignmentExpression>::from_raw(parent)]
                        .left_hand_side
                        .raw()) =>
                {
                    tables.write_element.get(parent).copied()
                }
                NodeKind::PostfixExpression
                    if is(c.ast[Id::<PostfixExpression>::from_raw(parent)]
                        .operand
                        .raw()) =>
                {
                    tables.write_element.get(parent).copied()
                }
                NodeKind::PrefixExpression
                    if is(c.ast[Id::<PrefixExpression>::from_raw(parent)]
                        .operand
                        .raw()) =>
                {
                    tables.write_element.get(parent).copied()
                }
                NodeKind::PrefixedIdentifier
                    if is(c.ast[Id::<PrefixedIdentifier>::from_raw(parent)]
                        .identifier
                        .raw()) =>
                {
                    write_element(c, parent)
                }
                NodeKind::PropertyAccess
                    if is(c.ast[Id::<PropertyAccess>::from_raw(parent)]
                        .property_name
                        .raw()) =>
                {
                    write_element(c, parent)
                }
                _ => None,
            }
        }
        let node = node.into();
        write_element(self, node).or_else(|| self.element(node))
    }
    /// Dart `ElementExtension.canonicalElement2` of the base element:
    /// a property accessor is replaced with its variable.
    pub fn canonical_element2(
        &self,
        element: dartr_element::ElemRef,
    ) -> Option<dartr_element::ElementId> {
        let resolved = self.resolved?;
        let base = dartr_typesystem::member::base_element(&resolved.ctx, element);
        Some(match base.tag() {
            dartr_element::Tag::Getter | dartr_element::Tag::Setter => resolved
                .ctx
                .property_accessor(dartr_element::EId::from_raw(base))
                .variable
                .get()
                .map(|v| v.raw())
                .unwrap_or(base),
            _ => base,
        })
    }
    /// Whether an annotation of [element] (its `ElementAnnotation.element`,
    /// a base element) satisfies [predicate].
    pub fn has_annotation_where(
        &self,
        element: dartr_element::ElementId,
        mut predicate: impl FnMut(&dartr_element::Ctx<'a>, dartr_element::ElementId) -> bool,
    ) -> bool {
        let Some(resolved) = self.resolved else {
            return false;
        };
        let Some(metadata) = resolved.metadata else {
            return false;
        };
        metadata.annotations(element).into_iter().any(|annotation| {
            metadata
                .annotation_element(annotation)
                .is_some_and(|a| predicate(&resolved.ctx, a))
        })
    }
    /// Dart `ElementAnnotationImpl._isConstructor(libraryName:, className:)`
    /// for an annotation of [element].
    pub fn has_constructor_annotation(
        &self,
        element: dartr_element::ElementId,
        library_name: &str,
        class_name: &str,
    ) -> bool {
        use dartr_typesystem::TypeExt;
        self.has_annotation_where(element, |ctx, a| {
            a.tag() == dartr_element::Tag::Constructor
                && ctx
                    .element_data(a)
                    .and_then(|d| d.enclosing)
                    .and_then(|e| ctx.element_name(e))
                    == Some(class_name)
                && dartr_typesystem::member::library(ctx, dartr_element::ElemRef::Base(a))
                    .is_some_and(|library| ctx.element_name(library.raw()) == Some(library_name))
        })
    }
    /// Dart `Metadata.hasImmutable` (`isImmutable`: the `immutable` getter or
    /// the `Immutable` constructor of package:meta).
    pub fn has_immutable(&self, element: dartr_element::ElementId) -> bool {
        self.has_package_meta_getter(element, "immutable")
            || self.has_constructor_annotation(element, "meta", "Immutable")
    }
    /// Dart `element.metadata.annotations.any((a) => a._isPackageMetaGetter(name))`
    /// (`hasAwaitNotRequired`, `hasOptionalTypeArgs`, ...).
    pub fn has_package_meta_getter(&self, element: dartr_element::ElementId, name: &str) -> bool {
        self.has_top_getter_annotation(element, "meta", name)
    }
    /// Dart `ElementAnnotationImpl._isTopGetter(libraryName:, name:)` for an
    /// annotation of [element]: the top-level getter [name] of the library
    /// named [library_name] (`hasOverride`: `dart.core`, `override`).
    pub fn has_top_getter_annotation(
        &self,
        element: dartr_element::ElementId,
        library_name: &str,
        name: &str,
    ) -> bool {
        use dartr_typesystem::TypeExt;
        let Some(resolved) = self.resolved else {
            return false;
        };
        let Some(metadata) = resolved.metadata else {
            return false;
        };
        let ctx = &resolved.ctx;
        metadata.annotations(element).into_iter().any(|annotation| {
            metadata.annotation_element(annotation).is_some_and(|a| {
                a.tag() == dartr_element::Tag::Getter
                    && ctx.element_name(a) == Some(name)
                    && dartr_typesystem::member::library(ctx, dartr_element::ElemRef::Base(a))
                        .is_some_and(|library| {
                            ctx.element_name(library.raw()) == Some(library_name)
                        })
            })
        })
    }
    pub fn static_type(&self, node: impl Into<NodeId>) -> Option<dartr_element::TypeId> {
        self.resolved?.tables.static_type.get(node.into()).copied()
    }
    pub fn element(&self, node: impl Into<NodeId>) -> Option<dartr_element::ElemRef> {
        self.resolved?.tables.element.get(node.into()).copied()
    }
    pub fn corresponding_parameter(
        &self,
        node: impl Into<NodeId>,
    ) -> Option<dartr_element::ElemRef> {
        let node = node.into();
        let tables = self.resolved?.tables;
        tables.param_element.get(node).copied().or_else(|| {
            let named = self.ast.cast::<dartr_ast::NamedArgument>(node)?;
            tables
                .param_element
                .get(self.ast[named].argument_expression.raw())
                .copied()
        })
    }
    /// Dart `correspondingParameter?.type` of an argument (the expression,
    /// or a `NamedArgument`).
    pub fn corresponding_parameter_type(
        &self,
        node: impl Into<NodeId>,
    ) -> Option<dartr_element::TypeId> {
        let node = node.into();
        let types = self.resolved?.corresponding_parameter_type;
        types.get(node).copied().or_else(|| {
            let named = self.ast.cast::<dartr_ast::NamedArgument>(node)?;
            types
                .get(self.ast[named].argument_expression.raw())
                .copied()
        })
    }
    pub fn declared_element(&self, node: impl Into<NodeId>) -> Option<dartr_element::ElementId> {
        let resolved = self.resolved?;
        let fragment = *resolved.tables.declared_fragment.get(node.into())?;
        resolved
            .ctx
            .fragment_data(fragment)?
            .element
            .try_get()
            .copied()
    }
    pub fn type_system(&self) -> Option<dartr_typesystem::TypeSystem<'a>> {
        Some(dartr_typesystem::TypeSystem::new(self.resolved?.ctx))
    }
    /// The library's defining unit, including when a processor visits a part.
    pub fn defining_unit(&self) -> &RuleContextUnit<'a> {
        &self.all_units[0]
    }
    /// Workspace package root for the defining unit, if a pubspec exists.
    pub fn package_root(&self) -> Option<std::path::PathBuf> {
        let path = self.all_units.first()?.path;
        let parent = std::path::Path::new(path).parent()?;
        parent
            .ancestors()
            .find(|root| root.join("pubspec.yaml").is_file())
            .map(std::path::Path::to_path_buf)
    }
    pub fn is_in_lib_dir(&self) -> bool {
        self.package_root().is_some_and(|root| {
            std::path::Path::new(self.all_units[0].path).starts_with(root.join("lib"))
        })
    }
    pub fn is_in_test_directory(&self) -> bool {
        self.package_root().is_some_and(|root| {
            std::path::Path::new(self.all_units[0].path).starts_with(root.join("test"))
        })
    }
    pub fn is_feature_enabled(&self, feature: ExperimentalFlag) -> bool {
        self.defining_unit()
            .parsed
            .feature_set
            .is_experiment_enabled(feature)
    }
    /// The visited unit's source URI. The analyzer uses a
    /// package URI for files mapped by package_config, and a file URI otherwise.
    pub fn source_uri(&self) -> String {
        let file = std::path::Path::new(self.path);
        for root in file.ancestors().skip(1) {
            let config = root.join(".dart_tool/package_config.json");
            let Ok(content) = std::fs::read_to_string(&config) else {
                continue;
            };
            let Ok(config_value) = serde_json::from_str::<serde_json::Value>(&content) else {
                break;
            };
            if let Some(packages) = config_value["packages"].as_array() {
                for package in packages {
                    let (Some(name), Some(root_uri)) =
                        (package["name"].as_str(), package["rootUri"].as_str())
                    else {
                        continue;
                    };
                    let package_root = if let Some(path) = root_uri.strip_prefix("file://") {
                        std::path::PathBuf::from(path)
                    } else if root_uri.contains(':') {
                        continue;
                    } else {
                        config.parent().unwrap().join(root_uri)
                    };
                    let package_dir =
                        package_root.join(package["packageUri"].as_str().unwrap_or(""));
                    if let Ok(package_dir) = package_dir.canonicalize()
                        && let Ok(relative) = file.strip_prefix(&package_dir)
                    {
                        return format!(
                            "package:{name}/{}",
                            relative.to_string_lossy().replace('\\', "/")
                        );
                    }
                }
            }
            break;
        }
        if file.is_absolute() {
            format!("file://{}", self.path.replace('\\', "/"))
        } else {
            self.path.replace('\\', "/")
        }
    }
    pub fn report_node(
        &self,
        out: &mut Vec<Diagnostic>,
        code: &'static DiagnosticCode,
        node: impl Into<NodeId>,
        args: &[&str],
    ) {
        let node = node.into();
        if !self.is_synthetic(node) {
            self.report_offset(
                out,
                code,
                self.ast.offset(node) as usize,
                self.ast.length(node) as usize,
                args,
            );
        }
    }
    pub fn report_token(
        &self,
        out: &mut Vec<Diagnostic>,
        code: &'static DiagnosticCode,
        token: TokenId,
        args: &[&str],
    ) {
        let token = self.ast.tokens.get(token);
        if !token.is_synthetic() {
            self.report_offset(
                out,
                code,
                token.offset as usize,
                token.length as usize,
                args,
            );
        }
    }
    pub fn report_offset(
        &self,
        out: &mut Vec<Diagnostic>,
        code: &'static DiagnosticCode,
        offset: usize,
        length: usize,
        args: &[&str],
    ) {
        out.push(Diagnostic::new(code, offset, length, args, vec![]));
    }
    pub fn is_synthetic(&self, node: impl Into<NodeId>) -> bool {
        use dartr_ast::{ExpressionStatement, Id, NamedType};
        let node = node.into();
        match self.ast.kind(node) {
            NodeKind::BooleanLiteral
            | NodeKind::SimpleIdentifier
            | NodeKind::SimpleStringLiteral
            | NodeKind::EmptyStatement => self
                .ast
                .tokens
                .get(self.ast.begin_token(node))
                .is_synthetic(),
            NodeKind::NamedType => {
                let n = &self.ast[Id::<NamedType>::from_raw(node)];
                self.ast.tokens.get(n.name).is_synthetic() && n.type_arguments.is_none()
            }
            NodeKind::ExpressionStatement => {
                let n = &self.ast[Id::<ExpressionStatement>::from_raw(node)];
                self.is_synthetic(n.expression)
                    && n.semicolon
                        .is_none_or(|token| self.ast.tokens.get(token).is_synthetic())
            }
            _ => false,
        }
    }
    pub fn text(&self, node: impl Into<NodeId>) -> String {
        let node = node.into();
        if self.ast.kind(node) == NodeKind::CompilationUnit {
            return self.source.to_owned();
        }
        let start = self.ast.tokens.byte_range(self.ast.begin_token(node)).start;
        let end = self.ast.tokens.byte_range(self.ast.end_token(node)).end;
        if let Some(text) = self.source.get(start..end) {
            return text.to_owned();
        }
        String::from_utf16_lossy(
            &self
                .source
                .encode_utf16()
                .skip(self.ast.offset(node) as usize)
                .take(self.ast.length(node) as usize)
                .collect::<Vec<_>>(),
        )
    }
}
pub type NodeProcessor = fn(&LinterContext<'_>, NodeId, &mut Vec<Diagnostic>);
pub type AfterLibraryProcessor = fn(&LinterContext<'_>, &mut Vec<Diagnostic>);
#[derive(Default)]
pub struct RuleVisitorRegistry {
    pub enable_timing: bool,
    pub timers: std::cell::RefCell<AnalysisRuleTimers>,
    subscriptions: IndexMap<NodeKind, Vec<(&'static str, NodeProcessor)>>,
    after_library: Vec<(&'static str, AfterLibraryProcessor)>,
}
pub type NodeLintRegistry = RuleVisitorRegistry;
impl RuleVisitorRegistry {
    pub fn with_timing(enable_timing: bool) -> Self {
        Self {
            enable_timing,
            ..Self::default()
        }
    }
    pub fn add(&mut self, kind: NodeKind, rule: &'static str, processor: NodeProcessor) {
        self.subscriptions
            .entry(kind)
            .or_default()
            .push((rule, processor));
    }
    pub fn add_after_library(&mut self, rule: &'static str, processor: AfterLibraryProcessor) {
        self.after_library.push((rule, processor));
    }
}
pub fn lint(parsed: &ParsedUnit, source: &str, path: &str, enabled: &[&str]) -> Vec<Diagnostic> {
    lint_library(
        &[RuleContextUnit {
            parsed,
            source,
            path,
        }],
        enabled,
    )
    .remove(0)
}

/// Register once per library, visit each unit in source order, then run callbacks.
/// The first unit is the defining unit. All units must exist and be parsed.
pub fn lint_library(units: &[RuleContextUnit<'_>], enabled: &[&str]) -> Vec<Vec<Diagnostic>> {
    let mut out = lint_library_unfiltered(units, enabled);
    for (index, diagnostics) in out.iter_mut().enumerate() {
        let ctx = LinterContext {
            parsed: units[index].parsed,
            ast: &units[index].parsed.ast,
            source: units[index].source,
            path: units[index].path,
            all_units: units,
            current_unit: index,
            resolved: None,
            resolved_units: &[],
        };
        let ignores = IgnoreInfo::for_dart(&ctx);
        diagnostics.retain(|diagnostic| !ignores.ignored(&ctx, diagnostic));
    }
    out
}

/// Like [lint_library], without the ignore-comment filtering. The analyzer
/// filters all diagnostics of a unit together, with the `cannot-ignore`
/// codes of the analysis options: callers that do that use this function.
pub fn lint_library_unfiltered(
    units: &[RuleContextUnit<'_>],
    enabled: &[&str],
) -> Vec<Vec<Diagnostic>> {
    let Some(_) = units.first() else {
        return vec![];
    };
    let context = |index: usize| {
        let unit = &units[index];
        LinterContext {
            parsed: unit.parsed,
            ast: &unit.parsed.ast,
            source: unit.source,
            path: unit.path,
            all_units: units,
            current_unit: index,
            resolved: None,
            resolved_units: &[],
        }
    };
    let mut registry = RuleVisitorRegistry::default();
    let enabled: IndexSet<_> = enabled.iter().map(|s| s.to_ascii_lowercase()).collect();
    let defining_context = context(0);
    for rule in ALL_RULES {
        if enabled.contains(rule.name) {
            rules::register(rule.name, &mut registry, &defining_context);
        }
    }
    let visitor = AnalysisRuleVisitor::new(&registry);
    let mut out = Vec::with_capacity(units.len());
    for (index, unit) in units.iter().enumerate() {
        let ctx = context(index);
        let mut diagnostics = vec![];
        visitor.visit(&ctx, unit.parsed.unit.raw(), &mut diagnostics);
        out.push(diagnostics);
    }
    let last_index = units.len() - 1;
    visitor.after_library(&context(last_index), &mut out[last_index]);
    deduplicate_diagnostics(&mut out);
    out
}

/// Run rules after body resolution; retain defining-unit metadata for parts.
/// Callers can defer ignore filtering until all analyzer diagnostics are available.
pub fn lint_resolved_library_unfiltered(
    units: &[ResolvedRuleContextUnit<'_>],
    enabled: &[&str],
) -> Vec<Vec<Diagnostic>> {
    if units.is_empty() {
        return vec![];
    }
    let parsed_units: Vec<_> = units
        .iter()
        .map(|unit| RuleContextUnit {
            parsed: unit.parsed,
            source: unit.source,
            path: unit.path,
        })
        .collect();
    let context = |index: usize| {
        let unit = &units[index];
        LinterContext {
            parsed: unit.parsed,
            ast: unit.ast,
            source: unit.source,
            path: unit.path,
            all_units: &parsed_units,
            current_unit: index,
            resolved: unit.resolved,
            resolved_units: units,
        }
    };
    let mut registry = RuleVisitorRegistry::default();
    let enabled: IndexSet<_> = enabled.iter().map(|s| s.to_ascii_lowercase()).collect();
    for rule in ALL_RULES {
        if enabled.contains(rule.name) {
            rules::register(rule.name, &mut registry, &context(0));
        }
    }
    let visitor = AnalysisRuleVisitor::new(&registry);
    let mut out = vec![vec![]; units.len()];
    for (index, unit) in units.iter().enumerate() {
        visitor.visit(&context(index), unit.unit, &mut out[index]);
    }
    let last = units.len() - 1;
    visitor.after_library(&context(last), &mut out[last]);
    deduplicate_diagnostics(&mut out);
    out
}

// Dart source: pkg/analyzer/lib/error/listener.dart (RecordingDiagnosticListener)
fn deduplicate_diagnostics(units: &mut [Vec<Diagnostic>]) {
    for diagnostics in units {
        let mut seen = IndexSet::new();
        diagnostics.retain(|diagnostic| {
            seen.insert((
                diagnostic.code.unique_name,
                diagnostic.offset,
                diagnostic.length,
                diagnostic.message.clone(),
            ))
        });
    }
}

/// Resolved counterpart of [lint_library], including ignore comments.
pub fn lint_resolved_library(
    units: &[ResolvedRuleContextUnit<'_>],
    enabled: &[&str],
) -> Vec<Vec<Diagnostic>> {
    let mut out = lint_resolved_library_unfiltered(units, enabled);
    let parsed_units: Vec<_> = units
        .iter()
        .map(|unit| RuleContextUnit {
            parsed: unit.parsed,
            source: unit.source,
            path: unit.path,
        })
        .collect();
    for (index, diagnostics) in out.iter_mut().enumerate() {
        let unit = &units[index];
        let ctx = LinterContext {
            parsed: unit.parsed,
            ast: unit.ast,
            source: unit.source,
            path: unit.path,
            all_units: &parsed_units,
            current_unit: index,
            resolved: unit.resolved,
            resolved_units: units,
        };
        let ignores = IgnoreInfo::for_dart(&ctx);
        diagnostics.retain(|diagnostic| !ignores.ignored(&ctx, diagnostic));
    }
    out
}

/// The generated Dart visitor runs each node's subscriptions before children.
/// Panics propagate, so a failed rule cannot silently produce incomplete output.
pub struct AnalysisRuleVisitor<'a> {
    registry: &'a RuleVisitorRegistry,
}
impl<'a> AnalysisRuleVisitor<'a> {
    pub fn new(registry: &'a RuleVisitorRegistry) -> Self {
        Self { registry }
    }
    pub fn visit(&self, ctx: &LinterContext<'_>, root: NodeId, out: &mut Vec<Diagnostic>) {
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            if let Some(subscriptions) = self.registry.subscriptions.get(&ctx.ast.kind(node)) {
                for (rule, processor) in subscriptions {
                    if self.registry.enable_timing {
                        self.registry.timers.borrow_mut().get_timer(rule).start();
                    }
                    processor(ctx, node, out);
                    if self.registry.enable_timing {
                        self.registry.timers.borrow_mut().get_timer(rule).stop();
                    }
                }
            }
            pending.extend(ctx.ast.children(node).into_iter().rev());
        }
    }
    pub fn after_library(&self, ctx: &LinterContext<'_>, out: &mut Vec<Diagnostic>) {
        for (rule, callback) in &self.registry.after_library {
            if self.registry.enable_timing {
                self.registry.timers.borrow_mut().get_timer(rule).start();
            }
            callback(ctx, out);
            if self.registry.enable_timing {
                self.registry.timers.borrow_mut().get_timer(rule).stop();
            }
        }
    }
}

/// Run enabled AST rules using the parsed unit's source text.
/// File-sensitive rules should use `lint` with the actual file path.
pub fn run_lints(parsed: &ParsedUnit, enabled: &[&str]) -> Vec<Diagnostic> {
    lint(parsed, &parsed.ast.tokens.source, "", enabled)
}

/// Apply explicitly enabled configurations and severity overrides.
pub fn lint_with_config(
    parsed: &ParsedUnit,
    source: &str,
    path: &str,
    configs: &IndexMap<String, RuleConfig>,
) -> Vec<Diagnostic> {
    let registry = Registry::builtin();
    let enabled: Vec<_> = registry.enabled(configs).map(|rule| rule.name).collect();
    let mut diagnostics = lint(parsed, source, path, &enabled);
    for diagnostic in &mut diagnostics {
        if let Some(config) = configs.get(diagnostic.code.name)
            && let Some(severity) = config.diagnostic_severity()
        {
            diagnostic.severity = severity;
        }
    }
    diagnostics
}
