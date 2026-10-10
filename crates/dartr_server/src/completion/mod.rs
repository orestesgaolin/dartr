// Dart source: pkg/analysis_server/lib/src/services/completion/dart/completion_manager.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/completion_state.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/suggestion_collector.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/not_imported_completion_pass.dart
// Dart source: pkg/analyzer/lib/src/dart/analysis/file_state_filter.dart
// Dart source: pkg/analysis_server/lib/src/utilities/extensions/completion_request.dart

//! Code completion of Dart files (Dart `DartCompletionManager`): the
//! candidate suggestions at an offset, ranked by relevance.
//!
//! - [`target`]: the completion target and the replacement range.
//! - [`pass`]: the in-scope pass (Dart `InScopeCompletionPass`).
//! - [`declaration`], [`keyword`], [`uri`]: the helpers of the pass.
//! - [`relevance`]: the context type, the features and the relevance.
//! - [`candidate`]: the candidate suggestions.
//! - [`lsp`]: the LSP completion items.

pub mod candidate;
pub mod declaration;
pub mod elem;
pub mod keyword;
pub mod imports;
pub mod known;
pub mod lsp;
pub mod overrides;
pub mod pass;
pub mod relevance;
mod relevance_tables;
pub mod snippets;
pub mod target;
pub mod uri;

use std::time::Instant;

use dartr_ast::*;
use dartr_element::{
    Ctx, EId, ElementId, ExtensionElement, FId, InterfaceElement, LibraryElement, LibraryFragment,
    Nullability, ResolutionTables, TypeId, TypeKind,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_resolver::scope::LibraryScopes;
use dartr_syntax::LineInfo;
use dartr_typesystem::{TypeExt, TypeSystem};

use crate::fuzzy::FuzzyMatcher;
use candidate::Candidate;
use relevance::{ContextInput, RelevanceComputer, compute_context_type};
use target::{CompletionTarget, TokenExt};

/// The code style options that completion reads (Dart `CodeStyleOptions`).
#[derive(Clone, Copy, Debug)]
pub struct CodeStyle {
    /// Dart `specifyTypes` (the `always_specify_types` lint).
    pub specify_types: bool,
    /// Dart `makeLocalsFinal` (the `prefer_final_locals` lint).
    pub make_locals_final: bool,
    /// Dart `preferredQuoteForStrings`.
    pub quote: char,
    /// The quote of the `prefer_single_quotes` or `prefer_double_quotes`
    /// lint (Dart `CodeStyleOptions._lintQuote`).
    pub lint_quote: Option<char>,
}

impl Default for CodeStyle {
    fn default() -> Self {
        CodeStyle {
            specify_types: false,
            make_locals_final: false,
            quote: '\'',
            lint_quote: None,
        }
    }
}

/// Dart `CompletionMatcher`.
pub enum Matcher {
    NoPrefix,
    Fuzzy(Box<FuzzyMatcher>),
}

/// Dart `SuggestionCollector`.
#[derive(Default)]
pub struct Collector {
    pub suggestions: Vec<Candidate>,
    pub completion_location: Option<String>,
    pub is_incomplete: bool,
    pub prefer_constants: bool,
    pub max_suggestions: i64,
}

/// The state that the helpers write: the collector, the matcher, and the
/// time budget.
pub struct Out {
    pub collector: Collector,
    pub matcher: Matcher,
    pub deadline: Instant,
}

impl Out {
    /// Dart `matcher.score(candidate)`.
    pub fn score(&mut self, candidate: &str) -> f64 {
        match &mut self.matcher {
            Matcher::NoPrefix => 0.0,
            Matcher::Fuzzy(m) => m.score(candidate),
        }
    }

    /// Dart `CompletionBudget.isEmpty`.
    pub fn budget_is_empty(&self) -> bool {
        Instant::now() > self.deadline
    }

    /// Dart `SuggestionCollector.addSuggestion`.
    pub fn add(&mut self, suggestion: Candidate) {
        let suggestions = &mut self.collector.suggestions;
        if suggestions.is_empty() {
            suggestions.push(suggestion);
            return;
        }
        let score = suggestion.matcher_score;
        let mut index = 0;
        for i in (0..suggestions.len()).rev() {
            if suggestions[i].matcher_score >= score {
                index = i + 1;
                break;
            }
        }
        suggestions.insert(index, suggestion);
        let max = self.collector.max_suggestions;
        if max >= 0 && suggestions.len() as i64 > max {
            let min_score = suggestions[max as usize].matcher_score;
            while suggestions.len() as i64 > max {
                if suggestions.last().unwrap().matcher_score < min_score {
                    suggestions.pop();
                } else {
                    break;
                }
            }
        }
    }
}

/// A completion request (Dart `DartCompletionRequest` with the parts of
/// `CompletionState`).
pub struct Request<'r, 'a> {
    pub ctx: &'r Ctx<'a>,
    pub ast: &'r Ast,
    pub tables: &'r ResolutionTables,
    pub root: Id<CompilationUnit>,
    pub content: &'r str,
    pub path: &'r str,
    pub offset: u32,
    pub library: EId<LibraryElement>,
    pub fragment: FId<LibraryFragment>,
    pub target: CompletionTarget,
    /// Dart `selection.coveringNode`.
    pub covering: NodeId,
    pub context_type: Option<TypeId>,
    /// Dart `replacementRange`: (offset, length).
    pub replacement: (u32, u32),
    pub line_info: &'r LineInfo,
    pub scopes: LibraryScopes,
    pub style: CodeStyle,
    /// The root of the pub package of the file, if any.
    pub package_root: Option<String>,
    /// Whether the file is in the `test` folder of its package.
    pub in_test_directory: bool,
    /// The SDK libraries: (short name, internal, implementation).
    pub sdk_libraries: Vec<(String, bool, bool)>,
    /// The packages: (name, path of the `lib` folder).
    pub packages: Vec<(String, String)>,
}

impl<'r, 'a> Request<'r, 'a> {
    pub fn ci(&self) -> ContextInput<'r, 'a> {
        ContextInput {
            ctx: self.ctx,
            ast: self.ast,
            tables: self.tables,
        }
    }

    pub fn unit(&self) -> crate::element_locator::Unit<'r, 'a> {
        crate::element_locator::Unit {
            ctx: self.ctx,
            ast: self.ast,
            tables: self.tables,
        }
    }

    /// Whether [flag] is enabled in the library.
    pub fn feature_enabled(&self, flag: ExperimentalFlag) -> bool {
        self.ctx.get(self.library).feature_set.is_enabled(flag.name())
    }

    /// Dart `isWildcardVariable` support (the `wildcard-variables` feature).
    pub fn wildcard_variables(&self) -> bool {
        self.feature_enabled(ExperimentalFlag::WildcardVariables)
    }

    /// `node.declaredFragment?.element`.
    pub fn declared_element(&self, node: NodeId) -> Option<ElementId> {
        declared_element(&self.ci(), node)
    }

    /// `node.element` (the base element).
    pub fn element(&self, node: NodeId) -> Option<ElementId> {
        dartr_resolver::error::support::element_of(self.ctx, self.tables, node)
    }

    /// The fragments of the library.
    pub fn library_fragments(&self) -> Vec<(FId<LibraryFragment>, Option<FId<LibraryFragment>>)> {
        let mut out = Vec::new();
        let first = self.ctx.get(self.library).first_fragment();
        let mut stack = vec![(first, None)];
        while let Some((f, parent)) = stack.pop() {
            out.push((f, parent));
            for part in self.ctx.fragment(f).parts.iter().rev() {
                if let dartr_element::DirectiveUri::Unit { library_fragment, .. } = &part.directive.uri {
                    stack.push((*library_fragment, Some(f)));
                }
            }
        }
        out
    }

    /// Dart `libraryFragment.accessibleExtensions`.
    pub fn accessible_extensions(&self) -> Vec<EId<ExtensionElement>> {
        self.scopes.accessible_extensions(self.fragment).to_vec()
    }

    /// The path of the defining unit of [library].
    pub fn library_path(&self, library: EId<LibraryElement>) -> String {
        let first = self.ctx.get(library).first_fragment();
        self.ctx.fragment(first).source.path.to_string()
    }

    /// Dart `target.enclosingInterfaceElement`.
    pub fn enclosing_interface_element(&self) -> Option<EId<InterfaceElement>> {
        let ast = self.ast;
        let member = ast.this_or_ancestor_of_type::<CompilationUnitMember>(self.target.containing_node)?;
        if ast.is::<ClassDeclaration>(member.raw()) || ast.is::<MixinDeclaration>(member.raw()) {
            return self.declared_element(member.raw())?.cast::<InterfaceElement>();
        }
        None
    }

    /// Dart `getRequestLineIndent`.
    pub fn indent(&self) -> String {
        let units: Vec<u16> = self.content.encode_utf16().collect();
        let mut line_start = self.offset as usize;
        let mut not_whitespace = self.offset as usize;
        while line_start > 0 {
            let c = units[line_start - 1];
            if c == b'\n' as u16 {
                break;
            }
            if c != b' ' as u16 && c != b'\t' as u16 {
                not_whitespace = line_start - 1;
            }
            line_start -= 1;
        }
        String::from_utf16_lossy(&units[line_start..not_whitespace])
    }

    /// Dart `CompletionState.endOfLine`.
    pub fn end_of_line(&self) -> String {
        match self.content.find('\n') {
            Some(i) if i > 0 && self.content.as_bytes()[i - 1] == b'\r' => "\r\n".to_string(),
            Some(_) => "\n".to_string(),
            None => "\n".to_string(),
        }
    }

    /// Dart `isReplacingKeywordOrIdentifier`.
    pub fn is_replacing_keyword_or_identifier(&self) -> bool {
        let ast = self.ast;
        let (r_offset, r_length) = self.replacement;
        let r_end = r_offset + r_length;
        match self.target.entity {
            Some(Entity::Token(t)) => {
                (ast.t_kw_or_ident(t) && r_offset >= ast.t_offset(t)) || r_end >= ast.t_offset(t)
            }
            Some(Entity::Node(n)) => {
                if ast.is::<SimpleIdentifier>(n) && !ast.n_synthetic(n) {
                    r_offset >= ast.offset(n) || r_end >= ast.offset(n)
                } else {
                    false
                }
            }
            None => false,
        }
    }

    /// Dart `shouldSuggestTearOff(element)`.
    pub fn should_suggest_tear_off(&self, element: ElementId) -> bool {
        if !self.feature_enabled(ExperimentalFlag::ConstructorTearoffs) {
            return false;
        }
        let Some(context) = self.context_type else {
            return false;
        };
        let TypeKind::Function(f) = self.ctx.ty(context) else {
            return false;
        };
        let Some(interface) = element.cast::<InterfaceElement>() else {
            return false;
        };
        let count = self.ctx.interface_type_parameters(interface).len();
        let args = vec![TypeId::NEVER; count];
        let bottom = self.ctx.interface_type(interface, &args, Nullability::None);
        TypeSystem::new(*self.ctx).is_subtype_of(bottom, f.ret)
    }
}

/// Dart `List.sort` of [items] (the sort of the Dart VM, so that equal
/// items end in the same order).
pub fn dart_sort_vec<T>(items: Vec<T>, mut compare: impl FnMut(&T, &T) -> i64) -> Vec<T> {
    let mut indexes: Vec<usize> = (0..items.len()).collect();
    dartr_ast::sort::dart_sort(&mut indexes, |a, b| compare(&items[*a], &items[*b]));
    let mut slots: Vec<Option<T>> = items.into_iter().map(Some).collect();
    indexes.into_iter().map(|i| slots[i].take().unwrap()).collect()
}

/// `node.declaredFragment?.element`.
pub fn declared_element(ci: &ContextInput<'_, '_>, node: NodeId) -> Option<ElementId> {
    dartr_resolver::error::support::declared_element(ci.ctx, ci.tables, node)
}

/// The inputs of a request that come from the resolved unit.
pub struct RequestInputs<'r, 'a> {
    pub ctx: &'r Ctx<'a>,
    pub ast: &'r Ast,
    pub tables: &'r ResolutionTables,
    pub root: Id<CompilationUnit>,
    pub content: &'r str,
    pub path: &'r str,
    pub offset: u32,
    pub line_info: &'r LineInfo,
    pub style: CodeStyle,
    pub package_root: Option<String>,
    pub in_test_directory: bool,
    pub sdk_libraries: Vec<(String, bool, bool)>,
    pub packages: Vec<(String, String)>,
}

/// Dart `DartCompletionRequest(...)`: `None` when the unit has no library.
pub fn build_request<'r, 'a>(i: RequestInputs<'r, 'a>) -> Option<Request<'r, 'a>> {
    let ast = i.ast;
    let fragment = i
        .tables
        .declared_fragment
        .get(i.root.raw())?
        .cast::<LibraryFragment>()?;
    let library = i.ctx.fragment(fragment).library;
    let target = CompletionTarget::for_offset(ast, i.root.raw(), i.offset);
    let ci = ContextInput {
        ctx: i.ctx,
        ast,
        tables: i.tables,
    };
    let context_type = compute_context_type(&ci, target.containing_node, i.offset);
    let dot_shorthands = i
        .ctx
        .get(library)
        .feature_set
        .is_enabled(ExperimentalFlag::DotShorthands.name());
    let replacement = target.compute_replacement_range(ast, i.offset, dot_shorthands);
    let covering = ast.node_covering(i.root.raw(), i.offset, 0)?;
    let scopes = LibraryScopes::build(i.ctx, library);
    Some(Request {
        ctx: i.ctx,
        ast,
        tables: i.tables,
        root: i.root,
        content: i.content,
        path: i.path,
        offset: i.offset,
        library,
        fragment,
        target,
        covering,
        context_type,
        replacement,
        line_info: i.line_info,
        scopes,
        style: i.style,
        package_root: i.package_root,
        in_test_directory: i.in_test_directory,
        sdk_libraries: i.sdk_libraries,
        packages: i.packages,
    })
}

/// The result of [`compute`].
pub struct CompletionResult {
    /// The candidates, sorted and with their relevance.
    pub candidates: Vec<Candidate>,
    /// Dart `NotImportedSuggestions.isIncomplete || isTruncated`.
    pub is_incomplete: bool,
}

/// A library that the not-imported pass looks at (Dart `knownFiles`
/// after `FileStateFilter`).
#[derive(Clone, Copy, Debug)]
pub struct KnownLibrary {
    pub element: EId<LibraryElement>,
}

/// Dart `DartCompletionManager.computeFinalizedCandidateSuggestions`.
/// [known] is `None` when the not-imported pass does not run.
pub fn compute(
    q: &Request<'_, '_>,
    budget_ms: u64,
    max_suggestions: i64,
    known: Option<&dyn Fn() -> Vec<KnownLibrary>>,
) -> CompletionResult {
    let ast = q.ast;
    let mut collector = Collector {
        max_suggestions,
        ..Collector::default()
    };
    if q.target.is_comment_text {
        return CompletionResult {
            candidates: Vec::new(),
            is_incomplete: false,
        };
    }
    let prefix = target::token_data_prefix(ast, q.covering, q.offset)
        .map(|(_, p)| p)
        .unwrap_or_default();
    let matcher = if prefix.is_empty() {
        Matcher::NoPrefix
    } else {
        Matcher::Fuzzy(Box::new(FuzzyMatcher::new(&prefix)))
    };
    collector.completion_location = None;
    let mut out = Out {
        collector,
        matcher,
        deadline: Instant::now() + std::time::Duration::from_millis(budget_ms),
    };
    let mut p = pass::Pass::new(q, out, false, true, true);
    p.compute_suggestions();
    let ops = p.decl.as_ref().map(|d| d.ops.clone()).unwrap_or_default();
    let mut decl = p.decl.take();
    out = p.out;
    let mut not_imported_incomplete = false;
    if let (Some(known), Some(d)) = (known, decl.as_mut()) {
        if !ops.is_empty() {
            not_imported_incomplete = not_imported_pass(q, &mut out, d, &ops, known);
        }
    }
    let mut collector = out.collector;
    let is_truncated = max_suggestions >= 0 && collector.suggestions.len() as i64 > max_suggestions;
    // Dart `SuggestionCollector.finalize`.
    let mut computer = RelevanceComputer::new(q, &target::target_prefix(ast, &q.target, q.offset));
    computer.completion_location = collector.completion_location.clone();
    for c in collector.suggestions.iter_mut() {
        c.relevance = computer.compute_relevance(c);
    }
    if std::env::var_os("DARTR_DEBUG_COMPLETION").is_some() {
        let ctx_type = q
            .context_type
            .map(|t| lsp::type_display(q.ctx, t))
            .unwrap_or_default();
        eprintln!(
            "COMPLETION location={:?} context_type={ctx_type} containing={:?}",
            collector.completion_location,
            ast.kind(q.target.containing_node)
        );
        for c in &collector.suggestions {
            eprintln!("  CAND {} {}", c.completion(q.ctx), c.relevance);
        }
    }
    let suggestions = std::mem::take(&mut collector.suggestions);
    collector.suggestions = dart_sort_vec(suggestions, |a, b| {
        if a.matcher_score == b.matcher_score {
            (b.relevance - a.relevance) as i64
        } else if b.matcher_score > a.matcher_score {
            1
        } else {
            -1
        }
    });
    if max_suggestions >= 0 && collector.suggestions.len() as i64 >= max_suggestions {
        collector.suggestions.truncate(max_suggestions as usize);
    }
    CompletionResult {
        candidates: collector.suggestions,
        is_incomplete: collector.is_incomplete || not_imported_incomplete || is_truncated,
    }
}

/// Dart `NotImportedCompletionPass.computeSuggestions`. Returns whether
/// the budget ran out.
fn not_imported_pass(
    q: &Request<'_, '_>,
    out: &mut Out,
    helper: &mut declaration::DeclarationHelper,
    ops: &[declaration::NotImportedOp],
    known: &dyn Fn() -> Vec<KnownLibrary>,
) -> bool {
    use declaration::NotImportedOp;
    let ctx = q.ctx;
    // Dart `_ImportSummary`: the libraries imported without combinators.
    let mut imported: Vec<EId<LibraryElement>> = Vec::new();
    for (fragment, _) in q.library_fragments() {
        for import in &ctx.fragment(fragment).library_imports {
            if let dartr_element::DirectiveUri::Library { library, .. } = &import.directive.uri {
                if import.combinators.is_empty() && !imported.contains(library) {
                    imported.push(*library);
                }
            }
        }
    }
    for library in known() {
        if out.budget_is_empty() {
            out.collector.is_incomplete = true;
            return true;
        }
        let library = library.element;
        if library == q.library {
            continue;
        }
        for op in ops {
            match op {
                NotImportedOp::Constructors => {
                    if !imported.contains(&library) {
                        helper.add_not_imported_constructors(q, out, library);
                    }
                }
                NotImportedOp::InstanceExtensionMembers {
                    ty,
                    excluded_getters,
                    include_methods,
                    include_setters,
                } => {
                    helper.add_not_imported_extension_methods(
                        q,
                        out,
                        library,
                        *ty,
                        excluded_getters,
                        *include_methods,
                        *include_setters,
                    );
                }
                NotImportedOp::StaticMembers => {
                    if !imported.contains(&library) {
                        helper.add_not_imported_top_level_declarations(q, out, library);
                    }
                }
            }
        }
    }
    false
}

/// Dart `FileStateFilter` of the completion file.
pub struct FileFilter {
    /// `None` for `_AnyFilter`.
    pub pub_package: Option<PubFilter>,
}

/// Dart `_PubFilter`.
pub struct PubFilter {
    pub root: String,
    pub name: Option<String>,
    pub friend_of_analyzer: bool,
    pub in_lib_or_entry_point: bool,
    pub dependencies: Vec<String>,
}

impl FileFilter {
    /// Dart `FileStateFilter(file)`.
    pub fn new(package: Option<(String, Option<String>)>, path: &str) -> FileFilter {
        let Some((root, name)) = package else {
            return FileFilter { pub_package: None };
        };
        let in_folder = |f: &str| path.starts_with(&format!("{root}/{f}/"));
        let in_lib = in_folder("lib") || in_folder("bin") || in_folder("web");
        let mut dependencies = Vec::new();
        if let Some(pubspec) = dartr_project::pubspec::Pubspec::read(&format!("{root}/pubspec.yaml")) {
            dependencies.extend(pubspec.dependencies.iter().cloned());
            if !in_lib {
                dependencies.extend(pubspec.dev_dependencies.iter().cloned());
            }
        }
        FileFilter {
            pub_package: Some(PubFilter {
                friend_of_analyzer: matches!(name.as_deref(), Some("analysis_server" | "linter")),
                root,
                name,
                in_lib_or_entry_point: in_lib,
                dependencies,
            }),
        }
    }

    /// Dart `shouldInclude(file)`.
    pub fn should_include(
        &self,
        library: &dartr_cli::driver_provider::KnownLibrary,
        file_package_root: &dyn Fn(&str) -> Option<String>,
    ) -> bool {
        if library.is_dart {
            if library.is_dart_internal {
                return false;
            }
            return !matches!(
                library.uri.as_str(),
                "dart:html" | "dart:indexed_db" | "dart:js" | "dart:js_util" | "dart:svg" | "dart:web_audio" | "dart:web_gl"
            );
        }
        let Some(pubf) = &self.pub_package else {
            return true;
        };
        let Some(package_name) = &library.package_name else {
            if pubf.in_lib_or_entry_point {
                return false;
            }
            return file_package_root(&library.path).as_deref() == Some(pubf.root.as_str());
        };
        if Some(package_name) == pubf.name.as_ref() {
            return true;
        }
        if library.is_src {
            return pubf.friend_of_analyzer && package_name == "analyzer";
        }
        pubf.dependencies.contains(package_name)
    }
}
