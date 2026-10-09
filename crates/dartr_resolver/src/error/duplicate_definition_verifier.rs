// Dart source: pkg/analyzer/lib/src/error/duplicate_definition_verifier.dart

//! `DuplicateDefinitionVerifier`: names defined twice in one scope
//! (statements, formal parameters, type parameters, variable lists, catch
//! clauses, the top-level declarations of a unit).
//!
//! The error verifier creates one [`DuplicateDefinitionVerifier`] per unit
//! (Dart `_duplicateDefinitionVerifier`) and calls its methods on the
//! nodes: [`DuplicateDefinitionVerifier::check_unit`] (`visitCompilationUnit`),
//! [`DuplicateDefinitionVerifier::check_statements`] (blocks, switch
//! members), [`DuplicateDefinitionVerifier::check_catch_clause`],
//! [`DuplicateDefinitionVerifier::check_parameters`],
//! [`DuplicateDefinitionVerifier::check_for_variables`] and
//! [`DuplicateDefinitionVerifier::check_type_parameters`].

use dartr_ast::{
    CatchClause, ClassDeclaration, CompilationUnit, EnumDeclaration, ExtensionDeclaration,
    ExtensionTypeDeclaration, FieldFormalParameter, FormalParameterList, FunctionDeclaration,
    FunctionDeclarationStatement, Id, MixinDeclaration, NodeId, NodeList,
    PatternVariableDeclarationStatement, Statement, TopLevelVariableDeclaration, TypeParameterList,
    VariableDeclarationList, VariableDeclarationStatement,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{ElementId, FieldFormalParameterElement, FragmentId, Tag};
use dartr_syntax::TokenId;
use dartr_typesystem::type_ext::TypeExt;
use indexmap::{IndexMap, IndexSet};

use super::VerifierHost;
use super::correct_override::{
    declared_fragment, duplicate_definition, duplicate_definition_for_nodes, fragment_element,
    has_wildcard_variables_feature_enabled, is_augmentation, is_wildcard_variable, lookup_name,
    node_range, token_is_synthetic, token_range,
};
use crate::ast_ext::formal_parameter_parts;
use crate::scope::LibraryScopes;

/// Dart `DuplicateDefinitionVerifier`.
#[derive(Default)]
pub struct DuplicateDefinitionVerifier {
    /// `_reportedTokens`.
    reported_tokens: IndexSet<TokenId>,
}

/// The kind of a [`DuplicateIdentifierScope`] (Dart `_DuplicateIdentifierScope`
/// and `_FormalParameterDuplicateIdentifierScope`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    Plain,
    FormalParameters,
}

/// Dart `_DuplicateIdentifierScope`.
struct DuplicateIdentifierScope {
    kind: ScopeKind,
    /// `_scope`: name to (node, token).
    scope: IndexMap<String, (NodeId, TokenId)>,
}

impl DuplicateIdentifierScope {
    fn new(kind: ScopeKind) -> Self {
        DuplicateIdentifierScope {
            kind,
            scope: IndexMap::new(),
        }
    }
}

impl DuplicateDefinitionVerifier {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dart `checkCatchClause(node)`: the exception and stack trace
    /// parameters have different names.
    pub fn check_catch_clause<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        node: Id<CatchClause>,
    ) {
        let ast = host.ast();
        let (Some(exception_parameter), Some(stack_trace_parameter)) = (
            ast[node].exception_parameter,
            ast[node].stack_trace_parameter,
        ) else {
            return;
        };
        let ctx = host.ctx();
        if let Some(fragment) = declared_fragment(host, exception_parameter)
            && let Some(element) = fragment_element(&ctx, fragment)
            && is_wildcard_variable(&ctx, element)
        {
            return;
        }
        let ast = host.ast();
        let exception_name = ast.tokens.lexeme(ast[exception_parameter].name).to_string();
        if exception_name == ast.tokens.lexeme(ast[stack_trace_parameter].name) {
            let duplicate = node_range(ast, stack_trace_parameter);
            let original = node_range(ast, exception_parameter);
            let d = duplicate_definition_for_nodes(
                &ctx,
                host.fragment(),
                diag::duplicate_definition(&exception_name),
                duplicate,
                original,
            );
            host.report(d);
        }
    }

    /// Dart `checkForVariables(node)`: the list does not define two
    /// variables with the same name.
    pub fn check_for_variables<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        node: Id<VariableDeclarationList>,
    ) {
        let mut scope = DuplicateIdentifierScope::new(ScopeKind::Plain);
        let variables = host.ast().list(host.ast()[node].variables).to_vec();
        for variable in variables {
            let name = host.ast()[variable].name;
            self.scope_add(host, &mut scope, name, variable.raw());
        }
    }

    /// Dart `checkParameters(node)`: the parameters have unique names.
    pub fn check_parameters<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        node: Id<FormalParameterList>,
    ) {
        let mut scope = DuplicateIdentifierScope::new(ScopeKind::FormalParameters);
        let parameters = host.ast().list(host.ast()[node].parameters).to_vec();
        for &parameter in &parameters {
            // The identifier can be null if this is a parameter list for a
            // generic function type.
            let Some(identifier) = formal_parameter_parts(host.ast(), parameter.raw()).name else {
                continue;
            };
            self.scope_add(host, &mut scope, identifier, parameter.raw());
        }

        // For private named parameters, also look for collisions with their
        // public name and other parameters.
        let ctx = host.ctx();
        for &parameter in &parameters {
            let Some(fragment) = declared_fragment(host, parameter) else {
                continue;
            };
            if fragment.tag() != Tag::FieldFormalParameter {
                continue;
            }
            let Some(fid) = fragment.cast::<dartr_element::FormalParameterFragment>() else {
                continue;
            };
            let data = ctx.fragment(fid);
            if data.private_name.is_none() {
                continue;
            }
            let (Some(private_name), Some(public_name)) = (
                formal_parameter_parts(host.ast(), parameter.raw()).name,
                data.name,
            ) else {
                continue;
            };
            let public_name = ctx.name_str(public_name).to_string();
            self.check_public_name(host, &scope, private_name, &public_name);
        }
    }

    /// Dart `checkStatements(statements)`: the variables declared by the
    /// statements have unique names.
    pub fn check_statements<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        statements: NodeList<Statement>,
    ) {
        let mut scope = DuplicateIdentifierScope::new(ScopeKind::Plain);
        let statements = host.ast().list(statements).to_vec();
        for statement in statements {
            let ast = host.ast();
            if let Some(s) = ast.cast::<VariableDeclarationStatement>(statement) {
                let list = ast[s].variables;
                let variables = ast.list(ast[list].variables).to_vec();
                for variable in variables {
                    let name = host.ast()[variable].name;
                    self.scope_add(host, &mut scope, name, variable.raw());
                }
            } else if let Some(s) = ast.cast::<FunctionDeclarationStatement>(statement) {
                if !self.is_wild_card_function(host, s) {
                    let declaration = ast[s].function_declaration;
                    let name = ast[declaration].name;
                    self.scope_add(host, &mut scope, name, declaration.raw());
                }
            } else if let Some(s) = ast.cast::<PatternVariableDeclarationStatement>(statement) {
                let declaration = ast[s].declaration;
                let ctx = host.ctx();
                let elements = host
                    .rt()
                    .pattern_variable_declaration_elements
                    .get(declaration)
                    .cloned()
                    .unwrap_or_default();
                for element in elements {
                    let Some(node) = crate::element_ext::bind_pattern_variable_node(&ctx, element)
                    else {
                        continue;
                    };
                    let Some(pattern) = host.ast().cast::<dartr_ast::DeclaredVariablePattern>(node)
                    else {
                        continue;
                    };
                    let name = host.ast()[pattern].name;
                    self.scope_add(host, &mut scope, name, node);
                }
            }
        }
    }

    /// Dart `checkTypeParameters(node)`: the type parameters have unique
    /// names.
    pub fn check_type_parameters<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        node: Id<TypeParameterList>,
    ) {
        let mut scope = DuplicateIdentifierScope::new(ScopeKind::Plain);
        let parameters = host.ast().list(host.ast()[node].type_parameters).to_vec();
        for parameter in parameters {
            let name = host.ast()[parameter].name;
            self.scope_add(host, &mut scope, name, parameter.raw());
        }
    }

    /// Dart `checkUnit(node)`: no two top-level declarations of the library
    /// have the same name, and no import prefix has the name of a
    /// top-level declaration. [scopes] gives Dart
    /// `_currentLibrary.libraryDeclarations` and the order of the library
    /// fragments.
    pub fn check_unit<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        scopes: &LibraryScopes,
        node: Id<CompilationUnit>,
    ) {
        let ctx = host.ctx();
        let library = host.library();
        let fragment = host.fragment();
        let mut defined_getters: IndexMap<String, ElementId> = IndexMap::new();
        let mut defined_setters: IndexMap<String, ElementId> = IndexMap::new();

        let add_without_checking =
            |getters: &mut IndexMap<String, ElementId>,
             setters: &mut IndexMap<String, ElementId>,
             library_fragment: dartr_element::FId<dartr_element::LibraryFragment>| {
                let lf = ctx.fragment(library_fragment);
                let add = |map: &mut IndexMap<String, ElementId>, f: FragmentId| {
                    if let Some(element) = fragment_element(&ctx, f)
                        && let Some(name) = lookup_name(&ctx, element)
                    {
                        map.insert(name, element);
                    }
                };
                for f in &lf.getters {
                    add(getters, f.raw());
                }
                for f in &lf.setters {
                    add(setters, f.raw());
                }
                for f in &lf.classes {
                    add(getters, f.raw());
                }
                for f in &lf.enums {
                    add(getters, f.raw());
                }
                for f in &lf.extensions {
                    add(getters, f.raw());
                }
                for f in &lf.extension_types {
                    add(getters, f.raw());
                }
                for f in &lf.functions {
                    add(getters, f.raw());
                }
                for f in &lf.mixins {
                    add(getters, f.raw());
                }
                for f in &lf.type_aliases {
                    add(getters, f.raw());
                }
            };

        // `fragment.prefixes`: the prefix elements of the imports of this
        // fragment.
        let prefixes = ctx.fragment(fragment).library_import_prefixes.clone();
        for prefix in prefixes {
            let Some(name) = ctx.element_name(prefix.raw()) else {
                continue;
            };
            if let Some(existing) = scopes.library_declaration_with_name(name) {
                let prefix_fragment = ctx.get(prefix).first_fragment().raw();
                let d = duplicate_definition(
                    &ctx,
                    diag::prefix_collides_with_top_level_member(name),
                    prefix_fragment,
                    existing,
                );
                host.report(d);
            }
        }

        // TODO(scheglov): carry across resolved units
        for (library_fragment, _) in crate::scope::library_fragments(&ctx, library) {
            if library_fragment == fragment {
                break;
            }
            add_without_checking(&mut defined_getters, &mut defined_setters, library_fragment);
        }

        let declarations = host.ast().list(host.ast()[node].declarations).to_vec();
        for member in declarations {
            let ast = host.ast();
            let member = member.raw();
            if let Some(n) = ast.cast::<TopLevelVariableDeclaration>(member) {
                let list = ast[n].variables;
                let variables = ast.list(ast[list].variables).to_vec();
                for variable in variables {
                    let Some(declared) = declared_fragment(host, variable) else {
                        continue;
                    };
                    if is_augmentation(&ctx, declared) {
                        continue;
                    }
                    let Some(element) = fragment_element(&ctx, declared)
                        .and_then(|e| e.cast::<dartr_element::PropertyInducingElement>())
                    else {
                        continue;
                    };
                    let name = host.ast()[variable].name;
                    let data = ctx.property_inducing(element);
                    if let Some(getter) = data.getter {
                        let getter = ctx.get(getter).first_fragment().raw();
                        self.check_duplicate_fragment_identifier(
                            host,
                            &mut defined_getters,
                            name,
                            Some(declared),
                            getter,
                        );
                    }
                    // `declaredElement.definesSetter`.
                    if defines_setter(&ctx, element.raw())
                        && let Some(setter) = data.setter
                    {
                        let setter = ctx.get(setter).first_fragment().raw();
                        self.check_duplicate_fragment_identifier(
                            host,
                            &mut defined_setters,
                            name,
                            Some(declared),
                            setter,
                        );
                    }
                }
                continue;
            }
            let Some(declared) = declared_fragment(host, member) else {
                continue;
            };
            if is_augmentation(&ctx, declared) {
                continue;
            }
            let (name, setter) = if let Some(n) = ast.cast::<ClassDeclaration>(member) {
                (
                    Some(class_name_part_type_name(ast, ast[n].name_part.raw())),
                    false,
                )
            } else if let Some(n) = ast.cast::<EnumDeclaration>(member) {
                (
                    Some(class_name_part_type_name(ast, ast[n].name_part.raw())),
                    false,
                )
            } else if let Some(n) = ast.cast::<ExtensionDeclaration>(member) {
                (ast[n].name, false)
            } else if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(member) {
                (
                    Some(class_name_part_type_name(ast, ast[n].name_part.raw())),
                    false,
                )
            } else if let Some(n) = ast.cast::<FunctionDeclaration>(member) {
                (Some(ast[n].name), declared.tag() == Tag::Setter)
            } else if let Some(n) = ast.cast::<MixinDeclaration>(member) {
                (Some(ast[n].name), false)
            } else {
                (type_alias_name(ast, member), false)
            };
            let Some(name) = name else {
                continue;
            };
            let scope = if setter {
                &mut defined_setters
            } else {
                &mut defined_getters
            };
            self.check_duplicate_fragment_identifier(host, scope, name, None, declared);
        }
    }

    /// `_checkDuplicateFragmentIdentifier(scope, identifier, originFragment:,
    /// fragment:)`.
    fn check_duplicate_fragment_identifier<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        scope: &mut IndexMap<String, ElementId>,
        identifier: TokenId,
        origin_fragment: Option<FragmentId>,
        fragment: FragmentId,
    ) {
        if token_is_synthetic(host.ast(), identifier) {
            return;
        }
        let ctx = host.ctx();
        let Some(element) = fragment_element(&ctx, fragment) else {
            return;
        };
        let Some(lookup_name) = lookup_name(&ctx, element) else {
            return;
        };
        if self.reported_tokens.contains(&identifier) {
            return;
        }
        if let Some(&previous) = scope.get(&lookup_name) {
            self.reported_tokens.insert(identifier);
            let d = duplicate_definition(
                &ctx,
                diag::duplicate_definition(&lookup_name),
                origin_fragment.unwrap_or(fragment),
                previous,
            );
            host.report(d);
        } else {
            scope.insert(lookup_name, element);
        }
    }

    /// `_isWildCardFunction(statement)`.
    fn is_wild_card_function<'h, H: VerifierHost<'h>>(
        &self,
        host: &H,
        statement: Id<FunctionDeclarationStatement>,
    ) -> bool {
        let ast = host.ast();
        let declaration = ast[statement].function_declaration;
        ast.tokens.lexeme(ast[declaration].name) == "_"
            && has_wildcard_variables_feature_enabled(&host.ctx(), host.library())
    }

    /// `_DuplicateIdentifierScope.add(identifier, node:)`: reports a
    /// diagnostic if the scope already has [identifier], else adds [node].
    fn scope_add<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        scope: &mut DuplicateIdentifierScope,
        identifier: TokenId,
        node: NodeId,
    ) {
        let ast = host.ast();
        if token_is_synthetic(ast, identifier) {
            return;
        }
        if self.reported_tokens.contains(&identifier) {
            return;
        }
        // Wildcards do not collide.
        if self.is_wildcard(host, scope.kind, identifier, node) {
            return;
        }
        let ast = host.ast();
        let lexeme = ast.tokens.lexeme(identifier).to_string();
        if let Some(&(previous_node, previous_token)) = scope.scope.get(&lexeme) {
            self.reported_tokens.insert(identifier);
            let code = get_diagnostic(host, scope.kind, previous_node, node, &lexeme);
            let duplicate = token_range(ast, identifier);
            let original = token_range(ast, previous_token);
            let d = duplicate_definition_for_nodes(
                &host.ctx(),
                host.fragment(),
                code,
                duplicate,
                original,
            );
            host.report(d);
        } else {
            scope.scope.insert(lexeme, (node, identifier));
        }
    }

    /// `isWildcard(identifier, node)` of the scope kinds.
    fn is_wildcard<'h, H: VerifierHost<'h>>(
        &self,
        host: &H,
        kind: ScopeKind,
        identifier: TokenId,
        node: NodeId,
    ) -> bool {
        let is_wildcard = host.ast().tokens.lexeme(identifier) == "_"
            && has_wildcard_variables_feature_enabled(&host.ctx(), host.library());
        if !is_wildcard {
            return false;
        }
        if kind == ScopeKind::FormalParameters {
            // Since fields can be named `_`, initializing formals are not
            // considered wildcards.
            let ctx = host.ctx();
            let element = declared_fragment(host, node).and_then(|f| fragment_element(&ctx, f));
            if element.is_some_and(|e| e.is::<FieldFormalParameterElement>()) {
                return false;
            }
        }
        true
    }

    /// `_FormalParameterDuplicateIdentifierScope.checkPublicName(
    /// privateName:, publicName:)`: the public name of a private named
    /// parameter does not collide with another parameter.
    fn check_public_name<'h, H: VerifierHost<'h>>(
        &mut self,
        host: &mut H,
        scope: &DuplicateIdentifierScope,
        private_name: TokenId,
        public_name: &str,
    ) {
        if let Some(&(_, previous_token)) = scope.scope.get(public_name) {
            let ast = host.ast();
            let duplicate = token_range(ast, private_name);
            let original = token_range(ast, previous_token);
            let d = duplicate_definition_for_nodes(
                &host.ctx(),
                host.fragment(),
                diag::private_named_parameter_duplicate_public_name(public_name),
                duplicate,
                original,
            );
            host.report(d);
        }
    }
}

/// `getDiagnostic(previous, current)` of the scope kinds: when two
/// initializing formals collide, the user cannot initialize the same field
/// twice.
fn get_diagnostic<'h, H: VerifierHost<'h>>(
    host: &H,
    kind: ScopeKind,
    previous: NodeId,
    current: NodeId,
    name: &str,
) -> LocatableDiagnostic {
    let ast = host.ast();
    if kind == ScopeKind::FormalParameters
        && ast.is::<FieldFormalParameter>(previous)
        && ast.is::<FieldFormalParameter>(current)
    {
        return diag::duplicate_field_formal_parameter(name);
    }
    diag::duplicate_definition(name)
}

/// The `name` of a `TypeAlias` node (`ClassTypeAlias`, `FunctionTypeAlias`
/// or `GenericTypeAlias`).
fn type_alias_name(ast: &dartr_ast::Ast, node: NodeId) -> Option<TokenId> {
    if let Some(n) = ast.cast::<dartr_ast::ClassTypeAlias>(node) {
        return Some(ast[n].name);
    }
    if let Some(n) = ast.cast::<dartr_ast::FunctionTypeAlias>(node) {
        return Some(ast[n].name);
    }
    if let Some(n) = ast.cast::<dartr_ast::GenericTypeAlias>(node) {
        return Some(ast[n].name);
    }
    None
}

/// Dart `ClassNamePart.typeName`.
pub(crate) fn class_name_part_type_name(ast: &dartr_ast::Ast, name_part: NodeId) -> TokenId {
    if let Some(n) = ast.cast::<dartr_ast::NameWithTypeParameters>(name_part) {
        return ast[n].type_name;
    }
    let n = ast
        .cast::<dartr_ast::PrimaryConstructorDeclaration>(name_part)
        .expect("a class name part");
    ast[n].type_name
}

/// Dart `PropertyInducingElementExtension.definesSetter`
/// (utilities/extensions/element.dart): a const variable defines no setter,
/// a final variable defines one only when it is late and has no initializer.
fn defines_setter(ctx: &dartr_element::Ctx<'_>, element: ElementId) -> bool {
    use crate::element_ext::{is_const, is_final, is_late};
    if is_const(ctx, element) {
        return false;
    }
    if is_final(ctx, element) {
        is_late(ctx, element) && !has_initializer(ctx, element)
    } else {
        true
    }
}

/// Dart `PropertyInducingElement.hasInitializer`: a fragment of the
/// variable has an initializer.
fn has_initializer(ctx: &dartr_element::Ctx<'_>, element: ElementId) -> bool {
    let mut fragment = ctx.element_data(element).map(|d| d.first_fragment);
    while let Some(f) = fragment {
        let Some(data) = ctx.fragment_data(f) else {
            break;
        };
        if data
            .flags
            .get()
            .contains(dartr_element::FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER)
        {
            return true;
        }
        fragment = data.next_fragment;
    }
    false
}
