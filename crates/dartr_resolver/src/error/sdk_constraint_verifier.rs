// Dart source: pkg/analyzer/lib/src/hint/sdk_constraint_verifier.dart,
// pkg/analyzer/lib/src/dart/element/since_sdk_version.dart,
// pkg/analyzer/lib/src/utilities/extensions/version.dart

//! `SdkConstraintVerifier` and `SinceSdkVersionComputer`: checks references to
//! SDK APIs annotated with `@Since(...)` and the `>>>` operator against the
//! enclosing pub package's `environment: sdk:` version constraint.

use std::cmp::Ordering;
use std::path::Path;

use dartr_ast::{
    ArgumentList, AssignmentExpression, Ast, AstVisitor, BinaryExpression, ConstructorName,
    ExtensionOverride, FunctionExpressionInvocation, HideCombinator, Id, IndexExpression,
    MethodDeclaration, MethodInvocation, NamedArgument, NamedType, NodeId, PrefixedIdentifier,
    PropertyAccess, ShowCombinator, SimpleIdentifier,
};
use dartr_diagnostics::diag;
use dartr_element::{Ctx, ElementId, FormalParameterElement, Tag, TypeKind};
use dartr_syntax::TokenType;
use dartr_typesystem::TypeExt;

use super::correct_override::non_synthetic;
use super::support::{
    Range, corresponding_parameter, element_of, in_declaration_context, library_of, node_range,
    read_element, token_range, write_element,
};
use super::{UnitVerifier, VerifierHost};
use crate::element_metadata::element_annotations;

/// Runs `SdkConstraintVerifier` on [v] if [v] belongs to a `PubPackage` with a
/// valid `environment: sdk:` constraint.
pub fn verify(v: &mut UnitVerifier<'_>) {
    let Some(constraint) = sdk_version_constraint_for_file(v.path) else {
        return;
    };
    let check_triple_shift = constraint.check_triple_shift();
    let mut verifier = SdkConstraintVerifier {
        v,
        constraint,
        check_triple_shift,
    };
    verifier.v.ast.accept(verifier.v.unit, &mut verifier);
}

struct SdkConstraintVerifier<'a, 'v> {
    v: &'v mut UnitVerifier<'a>,
    constraint: SdkVersionConstraint,
    check_triple_shift: bool,
}

impl SdkConstraintVerifier<'_, '_> {
    /// Dart `SdkConstraintVerifier._checkSinceSdkVersion`.
    fn check_since_sdk_version(
        &mut self,
        element: Option<ElementId>,
        target: NodeId,
        error_entity: Option<Range>,
    ) {
        let Some(raw_element) = element else {
            return;
        };
        let ctx = self.v.ctx;
        let element = non_synthetic(&ctx, raw_element);
        let Some(since_sdk_version) = compute_since_sdk_version(&ctx, element) else {
            return;
        };
        if self.constraint.requires_at_least(since_sdk_version) {
            return;
        }
        let ast = self.v.ast;
        let entity = match error_entity {
            Some(range) => range,
            None => {
                if !self.should_report_enum_index(target, element) {
                    return;
                }
                let mut target = target;
                if let Some(assign) = ast.cast::<AssignmentExpression>(target) {
                    target = ast[assign].left_hand_side.raw();
                }
                if let Some(c) = ast.cast::<ConstructorName>(target) {
                    match ast[c].name {
                        Some(name) => token_range(ast, ast[name].token),
                        None => token_range(ast, ast[ast[c].type_].name),
                    }
                } else if let Some(e) = ast.cast::<ExtensionOverride>(target) {
                    token_range(ast, ast[e].name)
                } else if let Some(f) = ast.cast::<FunctionExpressionInvocation>(target) {
                    node_range(ast, ast[f].argument_list)
                } else if let Some(i) = ast.cast::<IndexExpression>(target) {
                    token_range(ast, ast[i].left_bracket)
                } else if let Some(m) = ast.cast::<MethodInvocation>(target) {
                    node_range(ast, ast[m].method_name)
                } else if let Some(n) = ast.cast::<NamedType>(target) {
                    token_range(ast, ast[n].name)
                } else if let Some(p) = ast.cast::<PrefixedIdentifier>(target) {
                    node_range(ast, ast[p].identifier)
                } else if let Some(p) = ast.cast::<PropertyAccess>(target) {
                    node_range(ast, ast[p].property_name)
                } else if ast.is::<SimpleIdentifier>(target) {
                    node_range(ast, target)
                } else {
                    node_range(ast, target)
                }
            }
        };
        let d = diag::sdk_version_since(
            &since_sdk_version.to_string(),
            &self.constraint.display_string(),
        )
        .at_offset(entity.0 as usize, entity.1 as usize);
        self.v.report(d);
    }

    /// Dart `SdkConstraintVerifier._shouldReportEnumIndex`.
    fn should_report_enum_index(&self, node: NodeId, element: ElementId) -> bool {
        let ctx = self.v.ctx;
        if matches!(element.tag(), Tag::Getter | Tag::Setter)
            && ctx.element_name(element) == Some("index")
        {
            let ast = self.v.ast;
            let target_type = if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
                self.v.tables.static_type.get(ast[p].prefix.raw()).copied()
            } else if let Some(p) = ast.cast::<PropertyAccess>(node) {
                let real_target = if crate::ast_ext::property_access_is_cascaded(ast, p) {
                    crate::ast_ext::ancestor_cascade_target(ast, p.raw())
                } else {
                    ast[p].target
                };
                real_target.and_then(|t| self.v.tables.static_type.get(t.raw()).copied())
            } else {
                None
            };
            if let Some(target_type) = target_type {
                if let TypeKind::Interface {
                    element: target_elem,
                    ..
                } = *ctx.ty(target_type)
                {
                    return target_elem.raw().tag() == Tag::Class
                        && ctx.is_dart_core_enum(target_type);
                }
                return false;
            }
            false
        } else {
            true
        }
    }
}

impl AstVisitor for SdkConstraintVerifier<'_, '_> {
    fn visit_argument_list(&mut self, ast: &Ast, node: Id<ArgumentList>) {
        let ctx = self.v.ctx;
        let tables = self.v.tables;
        for &argument in ast.list(ast[node].arguments) {
            if !ast.is::<NamedArgument>(argument) {
                let parameter = corresponding_parameter(&ctx, ast, tables, argument.raw());
                self.check_since_sdk_version(
                    parameter,
                    node.raw(),
                    Some(node_range(ast, argument)),
                );
            }
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_assignment_expression(&mut self, ast: &Ast, node: Id<AssignmentExpression>) {
        let read = read_element(&self.v.ctx, self.v.tables, node);
        self.check_since_sdk_version(read, node.raw(), None);
        let write = write_element(&self.v.ctx, self.v.tables, node);
        self.check_since_sdk_version(write, node.raw(), None);
        self.visit_node(ast, node.raw());
    }

    fn visit_binary_expression(&mut self, ast: &Ast, node: Id<BinaryExpression>) {
        if self.check_triple_shift && ast.tokens.ty(ast[node].operator) == TokenType::GT_GT_GT {
            let d = self
                .v
                .at_token(diag::sdk_version_gt_gt_gt_operator(), ast[node].operator);
            self.v.report(d);
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_constructor_name(&mut self, ast: &Ast, node: Id<ConstructorName>) {
        let element = element_of(&self.v.ctx, self.v.tables, node);
        self.check_since_sdk_version(element, node.raw(), None);
        self.visit_node(ast, node.raw());
    }

    fn visit_function_expression_invocation(
        &mut self,
        ast: &Ast,
        node: Id<FunctionExpressionInvocation>,
    ) {
        let element = element_of(&self.v.ctx, self.v.tables, node);
        self.check_since_sdk_version(element, node.raw(), None);
        self.visit_node(ast, node.raw());
    }

    fn visit_hide_combinator(&mut self, _ast: &Ast, _node: Id<HideCombinator>) {
        // Don't flag references within a combinator.
    }

    fn visit_index_expression(&mut self, ast: &Ast, node: Id<IndexExpression>) {
        let element = element_of(&self.v.ctx, self.v.tables, node);
        self.check_since_sdk_version(element, node.raw(), None);
        self.visit_node(ast, node.raw());
    }

    fn visit_method_declaration(&mut self, ast: &Ast, node: Id<MethodDeclaration>) {
        if self.check_triple_shift
            && ast[node].operator_keyword.is_some()
            && ast.tokens.lexeme(ast[node].name) == ">>>"
        {
            let d = self
                .v
                .at_token(diag::sdk_version_gt_gt_gt_operator(), ast[node].name);
            self.v.report(d);
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_method_invocation(&mut self, ast: &Ast, node: Id<MethodInvocation>) {
        let element = element_of(&self.v.ctx, self.v.tables, ast[node].method_name);
        self.check_since_sdk_version(element, node.raw(), None);
        self.visit_node(ast, node.raw());
    }

    fn visit_named_argument(&mut self, ast: &Ast, node: Id<NamedArgument>) {
        let parameter = corresponding_parameter(&self.v.ctx, ast, self.v.tables, node.raw());
        self.check_since_sdk_version(
            parameter,
            node.raw(),
            Some(token_range(ast, ast[node].name)),
        );
        self.visit_node(ast, node.raw());
    }

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        let element = element_of(&self.v.ctx, self.v.tables, node);
        self.check_since_sdk_version(element, node.raw(), None);
        self.visit_node(ast, node.raw());
    }

    fn visit_prefixed_identifier(&mut self, ast: &Ast, node: Id<PrefixedIdentifier>) {
        let element = element_of(&self.v.ctx, self.v.tables, node)
            .or_else(|| element_of(&self.v.ctx, self.v.tables, ast[node].identifier));
        self.check_since_sdk_version(element, node.raw(), None);
        self.visit_node(ast, node.raw());
    }

    fn visit_property_access(&mut self, ast: &Ast, node: Id<PropertyAccess>) {
        let element = element_of(&self.v.ctx, self.v.tables, ast[node].property_name);
        self.check_since_sdk_version(element, node.raw(), None);
        self.visit_node(ast, node.raw());
    }

    fn visit_show_combinator(&mut self, _ast: &Ast, _node: Id<ShowCombinator>) {
        // Don't flag references within a combinator.
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if in_declaration_context(ast, node) {
            return;
        }
        let element = element_of(&self.v.ctx, self.v.tables, node);
        self.check_since_sdk_version(element, node.raw(), None);
    }
}

// ------------------------------------------------------------------ @Since

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SimpleVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

impl std::fmt::Display for SimpleVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Dart `SinceSdkVersionComputer.compute(element)`.
fn compute_since_sdk_version(ctx: &Ctx<'_>, element: ElementId) -> Option<SimpleVersion> {
    let library = library_of(ctx, element)?;
    if !ctx.library_uri(library).starts_with("dart:") {
        return None;
    }
    if let Some(p) = element.cast::<FormalParameterElement>()
        && dartr_typesystem::type_ext::is_required(ctx.get(p).kind)
    {
        return None;
    }
    let specified = specified_since_version(ctx, element);
    let enclosing = if element.tag() == Tag::Library {
        None
    } else {
        ctx.element_data(element)
            .and_then(|d| d.enclosing.or_else(|| d.library.map(|l| l.raw())))
    };
    let enclosing_version = enclosing.and_then(|e| compute_since_sdk_version(ctx, e));
    match (specified, enclosing_version) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Dart `SinceSdkVersionComputer._specifiedVersion(element)`.
fn specified_since_version(ctx: &Ctx<'_>, element: ElementId) -> Option<SimpleVersion> {
    let mut result: Option<SimpleVersion> = None;
    for annotation in element_annotations(ctx, element, None) {
        if is_since_annotation(ctx, &annotation)
            && let Some(version_str) = annotation.string_argument(Some(0), None)
            && let Some(version) = parse_since_version(&version_str)
        {
            result = Some(match result {
                Some(prev) => prev.max(version),
                None => version,
            });
        }
    }
    result
}

fn is_since_annotation(
    ctx: &Ctx<'_>,
    annotation: &crate::element_metadata::AnnotationRef<'_>,
) -> bool {
    let Some(element) = annotation.element else {
        return false;
    };
    if element.tag() != Tag::Constructor {
        return false;
    }
    let Some(data) = ctx.element_data(element) else {
        return false;
    };
    let class_name = data
        .enclosing
        .and_then(|c| ctx.element_name(c))
        .unwrap_or("");
    let library_uri = data.library.map(|l| ctx.library_uri(l)).unwrap_or("");
    class_name == "Since" && library_uri == "dart:_internal"
}

/// Dart `SinceSdkVersionComputer._parseVersion(versionStr)` (ignoring
/// pre-release/build since SDK `@Since` annotations and `withoutPreRelease`
/// compare `major.minor.patch`).
fn parse_since_version(version_str: &str) -> Option<SimpleVersion> {
    // 2.15 -> 2.15.0
    if let Some((major_s, minor_s)) = version_str.split_once('.')
        && !minor_s.contains('.')
        && !major_s.is_empty()
        && !minor_s.is_empty()
        && major_s.bytes().all(|b| b.is_ascii_digit())
        && minor_s.bytes().all(|b| b.is_ascii_digit())
    {
        return Some(SimpleVersion {
            major: major_s.parse().ok()?,
            minor: minor_s.parse().ok()?,
            patch: 0,
        });
    }
    let v = parse_full_version(version_str)?;
    Some(v.without_pre_release())
}

// ------------------------------------------------------------------ constraint

#[derive(Clone, Debug, PartialEq, Eq)]
struct FullVersion {
    major: u64,
    minor: u64,
    patch: u64,
    pre_release: Vec<VersionIdentifier>,
    build: Vec<VersionIdentifier>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum VersionIdentifier {
    Number(u64),
    Text(String),
}

impl FullVersion {
    fn without_pre_release(&self) -> SimpleVersion {
        SimpleVersion {
            major: self.major,
            minor: self.minor,
            patch: self.patch,
        }
    }

    fn next_breaking(&self) -> SimpleVersion {
        if self.major > 0 {
            SimpleVersion {
                major: self.major + 1,
                minor: 0,
                patch: 0,
            }
        } else {
            SimpleVersion {
                major: 0,
                minor: self.minor + 1,
                patch: 0,
            }
        }
    }

    fn compare_to(&self, other: &FullVersion) -> Ordering {
        let ord =
            (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch));
        if ord != Ordering::Equal {
            return ord;
        }
        if self.pre_release.is_empty() && !other.pre_release.is_empty() {
            return Ordering::Greater;
        }
        if other.pre_release.is_empty() && !self.pre_release.is_empty() {
            return Ordering::Less;
        }
        let ord = compare_identifiers(&self.pre_release, &other.pre_release);
        if ord != Ordering::Equal {
            return ord;
        }
        compare_identifiers(&self.build, &other.build)
    }
}

fn compare_identifiers(a: &[VersionIdentifier], b: &[VersionIdentifier]) -> Ordering {
    for i in 0..a.len().max(b.len()) {
        let ord = match (a.get(i), b.get(i)) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(VersionIdentifier::Number(x)), Some(VersionIdentifier::Number(y))) => x.cmp(y),
            (Some(VersionIdentifier::Number(_)), Some(VersionIdentifier::Text(_))) => {
                Ordering::Less
            }
            (Some(VersionIdentifier::Text(_)), Some(VersionIdentifier::Number(_))) => {
                Ordering::Greater
            }
            (Some(VersionIdentifier::Text(x)), Some(VersionIdentifier::Text(y))) => x.cmp(y),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

/// Dart `VersionConstraint` after `.withoutPreRelease`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum SdkVersionConstraint {
    Empty,
    Version(SimpleVersion),
    Range {
        min: Option<SimpleVersion>,
        include_min: bool,
        max: Option<SimpleVersion>,
        include_max: bool,
    },
}

impl SdkVersionConstraint {
    /// Dart `VersionConstraintExtension.requiresAtLeast(version)`.
    fn requires_at_least(&self, version: SimpleVersion) -> bool {
        match *self {
            SdkVersionConstraint::Empty => true,
            SdkVersionConstraint::Version(v) => v >= version,
            SdkVersionConstraint::Range { min, .. } => min.is_some_and(|m| m >= version),
        }
    }

    /// Dart `!before_2_14_0.intersect(_versionConstraint).isEmpty`.
    fn check_triple_shift(&self) -> bool {
        let v2_14_0 = SimpleVersion {
            major: 2,
            minor: 14,
            patch: 0,
        };
        match *self {
            SdkVersionConstraint::Empty => false,
            SdkVersionConstraint::Version(v) => v < v2_14_0,
            SdkVersionConstraint::Range { min, .. } => min.is_none_or(|m| m < v2_14_0),
        }
    }

    /// Dart `_versionConstraint.toString()` after `.withoutPreRelease`.
    fn display_string(&self) -> String {
        match *self {
            SdkVersionConstraint::Empty => "<empty>".to_string(),
            SdkVersionConstraint::Version(v) => v.to_string(),
            SdkVersionConstraint::Range {
                min,
                include_min,
                max,
                include_max,
            } => {
                if min.is_none() && max.is_none() {
                    return "any".to_string();
                }
                let mut out = String::new();
                if let Some(min) = min {
                    out.push_str(if include_min { ">=" } else { ">" });
                    out.push_str(&min.to_string());
                }
                if let Some(max) = max {
                    if min.is_some() {
                        out.push(' ');
                    }
                    out.push_str(if include_max { "<=" } else { "<" });
                    out.push_str(&max.to_string());
                }
                out
            }
        }
    }
}

/// Parses a `pub_semver` `VersionConstraint` and applies `.withoutPreRelease`.
fn parse_sdk_constraint_without_pre_release(text: &str) -> Option<SdkVersionConstraint> {
    let text = text.trim();
    if text.is_empty() || text.contains("||") {
        return None;
    }
    if text == "any" {
        return Some(SdkVersionConstraint::Range {
            min: None,
            include_min: false,
            max: None,
            include_max: false,
        });
    }
    if let Some(rest) = text.strip_prefix('^') {
        let rest = rest.trim();
        let v = parse_full_version(rest)?;
        let min = v.without_pre_release();
        let max = v.next_breaking();
        if min > max {
            return None;
        }
        return Some(SdkVersionConstraint::Range {
            min: Some(min),
            include_min: true,
            max: Some(max),
            include_max: false,
        });
    }

    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let mut terms = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i];
        let (op, ver_str) = if matches!(tok, ">=" | ">" | "<=" | "<") {
            i += 1;
            (tok, *tokens.get(i)?)
        } else if let Some(v) = tok.strip_prefix(">=") {
            (">=", v)
        } else if let Some(v) = tok.strip_prefix("<=") {
            ("<=", v)
        } else if let Some(v) = tok.strip_prefix('>') {
            (">", v)
        } else if let Some(v) = tok.strip_prefix('<') {
            ("<", v)
        } else if tok.starts_with('^') {
            return None;
        } else {
            ("=", tok)
        };
        if ver_str.is_empty() {
            return None;
        }
        terms.push((op, parse_full_version(ver_str)?));
        i += 1;
    }

    let mut min: Option<(FullVersion, bool)> = None;
    let mut max: Option<(FullVersion, bool)> = None;
    for (op, ver) in terms {
        match op {
            ">=" => update_min(&mut min, ver, true),
            ">" => update_min(&mut min, ver, false),
            "<=" => update_max(&mut max, ver, true),
            "<" => update_max(&mut max, ver, false),
            "=" => {
                update_min(&mut min, ver.clone(), true);
                update_max(&mut max, ver, true);
            }
            _ => return None,
        }
    }

    if let (Some((min_v, inc_min)), Some((max_v, inc_max))) = (&min, &max) {
        let ord = min_v.compare_to(max_v);
        if ord == Ordering::Greater {
            return Some(SdkVersionConstraint::Empty);
        }
        if ord == Ordering::Equal {
            if *inc_min && *inc_max {
                return Some(SdkVersionConstraint::Version(min_v.without_pre_release()));
            }
            return Some(SdkVersionConstraint::Empty);
        }
    }

    let min_simple = min.as_ref().map(|(v, _)| v.without_pre_release());
    let inc_min = min.as_ref().is_some_and(|(_, inc)| *inc);
    let max_simple = max.as_ref().map(|(v, _)| v.without_pre_release());
    let inc_max = max.as_ref().is_some_and(|(_, inc)| *inc);
    if let (Some(mn), Some(mx)) = (min_simple, max_simple)
        && mn > mx
    {
        return None;
    }
    Some(SdkVersionConstraint::Range {
        min: min_simple,
        include_min: inc_min,
        max: max_simple,
        include_max: inc_max,
    })
}

fn update_min(current: &mut Option<(FullVersion, bool)>, cand: FullVersion, inc: bool) {
    if current.as_ref().is_none_or(|(v, cur_inc)| {
        let ord = cand.compare_to(v);
        ord == Ordering::Greater || (ord == Ordering::Equal && !inc && *cur_inc)
    }) {
        *current = Some((cand, inc));
    }
}

fn update_max(current: &mut Option<(FullVersion, bool)>, cand: FullVersion, inc: bool) {
    if current.as_ref().is_none_or(|(v, cur_inc)| {
        let ord = cand.compare_to(v);
        ord == Ordering::Less || (ord == Ordering::Equal && !inc && *cur_inc)
    }) {
        *current = Some((cand, inc));
    }
}

fn parse_full_version(version: &str) -> Option<FullVersion> {
    let numeric_end = version
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(version.len());
    let suffix = &version[numeric_end..];
    let valid_ids = |text: &str| {
        !text.is_empty()
            && text.split('.').all(|id| {
                !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
    };
    let (pre_release, build) = if let Some(rest) = suffix.strip_prefix('-') {
        if let Some((pre, b)) = rest.split_once('+') {
            if !valid_ids(pre) || !valid_ids(b) || b.contains('+') {
                return None;
            }
            (pre, b)
        } else {
            if !valid_ids(rest) {
                return None;
            }
            (rest, "")
        }
    } else if let Some(b) = suffix.strip_prefix('+') {
        if !valid_ids(b) || b.contains('+') {
            return None;
        }
        ("", b)
    } else if suffix.is_empty() {
        ("", "")
    } else {
        return None;
    };
    let parts: Vec<&str> = version[..numeric_end].split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    Some(FullVersion {
        major: parts[0].parse().ok()?,
        minor: parts[1].parse().ok()?,
        patch: parts[2].parse().ok()?,
        pre_release: parse_identifiers(pre_release),
        build: parse_identifiers(build),
    })
}

fn parse_identifiers(text: &str) -> Vec<VersionIdentifier> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split('.')
        .map(|id| {
            id.parse().map_or_else(
                |_| VersionIdentifier::Text(id.to_string()),
                VersionIdentifier::Number,
            )
        })
        .collect()
}

// ------------------------------------------------------------------ pubspec lookup

/// Finds the enclosing `PubPackage`'s `sdkVersionConstraint.withoutPreRelease`
/// for [file_path], matching `PackageConfigWorkspace.findPackageFor`.
fn sdk_version_constraint_for_file(file_path: &str) -> Option<SdkVersionConstraint> {
    let parent = Path::new(file_path).parent()?;
    // Must be inside a `PackageConfigWorkspace` (innermost ancestor with
    // `.dart_tool/package_config.json`).
    let workspace_root = parent
        .ancestors()
        .find(|dir| dir.join(".dart_tool/package_config.json").is_file())?;
    for current in parent.ancestors() {
        let pubspec_path = current.join("pubspec.yaml");
        if pubspec_path.is_file() {
            if is_in_third_party_dart(&pubspec_path) {
                return None;
            }
            let content = std::fs::read_to_string(&pubspec_path).ok()?;
            let sdk_str = extract_environment_sdk(&content)?;
            return parse_sdk_constraint_without_pre_release(&sdk_str);
        }
        if current == workspace_root {
            return None;
        }
    }
    None
}

fn is_in_third_party_dart(pubspec_path: &Path) -> bool {
    let comps: Vec<_> = pubspec_path
        .components()
        .map(|c| c.as_os_str().to_str().unwrap_or(""))
        .collect();
    comps.len() > 4 && comps[comps.len() - 3] == "dart" && comps[comps.len() - 4] == "third_party"
}

fn extract_environment_sdk(yaml: &str) -> Option<String> {
    let mut lines = yaml.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = strip_yaml_comment(line).trim_end();
        if trimmed.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let content = trimmed.trim_start();
        if let Some(rest) = content.strip_prefix("environment:") {
            let rest = rest.trim();
            if let Some(flow) = rest.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                for entry in flow.split(',') {
                    if let Some(val) = entry.trim().strip_prefix("sdk:") {
                        return unquote_yaml_scalar(val.trim());
                    }
                }
                return None;
            }
            if !rest.is_empty() {
                return None;
            }
            let mut sdk_indent = None;
            while let Some(&next_line) = lines.peek() {
                let next_trimmed = strip_yaml_comment(next_line).trim_end();
                if next_trimmed.trim().is_empty() {
                    lines.next();
                    continue;
                }
                let next_indent = next_line.len() - next_line.trim_start().len();
                if next_indent <= indent {
                    break;
                }
                lines.next();
                if let Some(si) = sdk_indent
                    && next_indent != si
                {
                    continue;
                }
                sdk_indent.get_or_insert(next_indent);
                let next_content = next_trimmed.trim_start();
                if let Some(sdk_rest) = next_content.strip_prefix("sdk:") {
                    let sdk_val = sdk_rest.trim();
                    if !sdk_val.is_empty() {
                        return unquote_yaml_scalar(sdk_val);
                    }
                    while let Some(&val_line) = lines.peek() {
                        let val_trimmed = strip_yaml_comment(val_line).trim_end();
                        if val_trimmed.trim().is_empty() {
                            lines.next();
                            continue;
                        }
                        let val_indent = val_line.len() - val_line.trim_start().len();
                        if val_indent <= next_indent {
                            return None;
                        }
                        lines.next();
                        return unquote_yaml_scalar(val_trimmed.trim());
                    }
                    return None;
                }
            }
        }
    }
    None
}

fn strip_yaml_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b'\\' if in_double => {
                i += 1;
            }
            b'#' if !in_single && !in_double && (i == 0 || bytes[i - 1].is_ascii_whitespace()) => {
                return &line[..i];
            }
            _ => {}
        }
        i += 1;
    }
    line
}

fn unquote_yaml_scalar(s: &str) -> Option<String> {
    if s.is_empty() || s == "null" || s == "~" {
        return None;
    }
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        return Some(s[1..s.len() - 1].replace("''", "'"));
    }
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        return Some(s[1..s.len() - 1].to_string());
    }
    Some(s.to_string())
}
