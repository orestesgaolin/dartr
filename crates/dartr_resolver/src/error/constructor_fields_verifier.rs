// Dart source: pkg/analyzer/lib/src/error/constructor_fields_verifier.dart

//! `ConstructorFieldsVerifier`: the initialization of fields by the
//! generative constructors of a class (final and non-nullable fields that
//! a constructor does not initialize, fields initialized twice). One per
//! library (Dart `LibraryVerificationContext.constructorFieldsVerifier`):
//! the error verifier adds the constructors of each class, enum and
//! extension type ([`ConstructorFieldsVerifier::add_constructors`], which
//! reports the double initializations at once), and the library analyzer
//! calls [`ConstructorFieldsVerifier::report`] after all units (the fields
//! that are not initialized).

use dartr_ast::{
    Ast, ClassMember, ClassNamePart, ConstructorDeclaration, ConstructorFieldInitializer,
    ConstructorInitializer, FormalParameterList, Id, NodeId, NodeKind, NodeList,
    PrimaryConstructorBody, PrimaryConstructorDeclaration, RedirectingConstructorInvocation,
};
use dartr_diagnostics::diag;
use dartr_element::{
    Ctx, EId, ElementId, FieldElement, FormalParameterElement, FragmentFlags, InterfaceElement,
    Name, Tag,
};
use dartr_typesystem::{TypeExt, member};
use indexmap::{IndexMap, IndexSet};

use super::{UnitVerifier, VerifierHost};
use crate::ast_ext;
use crate::element_ext;

/// Dart `_InitState`: the four states of a field initialization through a
/// constructor signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InitState {
    /// The field is declared without an initializer.
    NotInit,
    /// The field is declared with an initializer.
    InitInDeclaration,
    /// The field is initialized in a field formal parameter of the
    /// constructor being verified.
    InitInFieldFormal,
    /// The field is initialized in the list of initializers of the
    /// constructor being verified.
    InitInInitializer,
}

/// Dart `_Constructor`.
#[derive(Debug)]
struct Constructor {
    /// The index of the unit of the constructor (Dart `diagnosticReporter`).
    unit: usize,
    /// Dart `errorRange` (offset, length).
    error_range: (usize, usize),
    fields: IndexMap<EId<FieldElement>, InitState>,
    duplicate_field_names: IndexSet<String>,
    is_primary: bool,
    primary_constructors: Vec<Id<PrimaryConstructorDeclaration>>,
    secondary_constructors: Vec<Id<ConstructorDeclaration>>,
    /// Set to `true` if the constructor redirects.
    has_redirecting_constructor_invocation: bool,
}

/// Dart `_Interface`.
#[derive(Debug)]
struct Interface {
    duplicate_field_names: IndexSet<String>,
    /// [`InitState::NotInit`] or [`InitState::InitInDeclaration`] for each
    /// field of the element: the initial state of each [`Constructor`].
    fields: IndexMap<EId<FieldElement>, InitState>,
    constructors: IndexMap<ElementId, Constructor>,
}

/// Dart `ConstructorFieldsVerifier`: verifier for initializing fields in
/// constructors.
#[derive(Debug, Default)]
pub struct ConstructorFieldsVerifier {
    interfaces: IndexMap<EId<InterfaceElement>, Interface>,
    /// Dart `ConstructorDeclaration.notInitializedFields` and
    /// `PrimaryConstructorDeclaration.notInitializedFields` (set by
    /// [`Self::report`]; read by fixes): keyed by the unit index and the
    /// constructor node.
    pub not_initialized_fields: IndexMap<(usize, NodeId), Vec<EId<FieldElement>>>,
}

impl ConstructorFieldsVerifier {
    /// Dart `addConstructors(diagnosticReporter, element, members,
    /// primaryConstructor)`: [unit] is the index of the unit of [host]
    /// (the unit that declares [element]); [members] are the class members
    /// of the declaration and [primary_constructor] its `namePart`.
    pub fn add_constructors<'a, H: VerifierHost<'a>>(
        &mut self,
        host: &mut H,
        unit: usize,
        element: EId<InterfaceElement>,
        members: &[Id<ClassMember>],
        primary_constructor: Id<ClassNamePart>,
    ) {
        self.for_interface(&host.ctx(), element);
        if let Some(primary_constructor) = host
            .ast()
            .cast::<PrimaryConstructorDeclaration>(primary_constructor)
        {
            self.add_primary_constructor(host, unit, element, members, primary_constructor);
        }
        let constructors: Vec<_> = members
            .iter()
            .filter_map(|&m| host.ast().cast::<ConstructorDeclaration>(m))
            .collect();
        for constructor in constructors {
            self.add_constructor(host, unit, element, constructor);
        }
    }

    /// Dart `report()`: reports the fields that the constructors do not
    /// initialize, into the unit of each constructor.
    pub fn report(&mut self, units: &mut [UnitVerifier<'_>]) {
        for interface in self.interfaces.values() {
            for constructor in interface.constructors.values() {
                let Some(unit) = units.iter_mut().find(|u| u.index == constructor.unit) else {
                    continue;
                };
                let Some(not_initialized) = constructor.report(unit) else {
                    continue;
                };
                for &node in &constructor.primary_constructors {
                    self.not_initialized_fields
                        .insert((constructor.unit, node.raw()), not_initialized.clone());
                }
                for &node in &constructor.secondary_constructors {
                    self.not_initialized_fields
                        .insert((constructor.unit, node.raw()), not_initialized.clone());
                }
            }
        }
    }

    /// Dart `_addConstructor(diagnosticReporter:, interfaceFields:, node:)`.
    fn add_constructor<'a, H: VerifierHost<'a>>(
        &mut self,
        host: &mut H,
        unit: usize,
        element: EId<InterfaceElement>,
        node: Id<ConstructorDeclaration>,
    ) {
        let ctx = host.ctx();
        let ast = host.ast();
        if ast[node].factory_keyword.is_some()
            || ast[node].redirected_constructor.is_some()
            || ast[node].external_keyword.is_some()
        {
            return;
        }

        let Some(fragment_data) = host
            .tables()
            .declared_fragment
            .get(node)
            .and_then(|&f| ctx.fragment_data(f))
        else {
            return;
        };
        let Some(&constructor_element) = fragment_data.element.try_get() else {
            return;
        };
        let is_augmentation = fragment_data
            .flags
            .get()
            .contains(FragmentFlags::FRAGMENT_IS_AUGMENTATION);
        let error_range = constructor_error_range(ast, node);
        let parameters = ast[node].parameters;
        let initializers = ast[node].initializers;

        let interface_fields = &mut self.interfaces[&element];
        let constructor_state =
            interface_fields.for_constructor(unit, constructor_element, error_range, false);
        constructor_state.secondary_constructors.push(node);

        if !is_augmentation {
            constructor_state.update_with_parameters(host, parameters);
        }

        constructor_state.update_with_initializers(host, initializers);
    }

    /// Dart `_addPrimaryConstructor(diagnosticReporter:, interfaceFields:,
    /// primaryConstructor:)`.
    fn add_primary_constructor<'a, H: VerifierHost<'a>>(
        &mut self,
        host: &mut H,
        unit: usize,
        element: EId<InterfaceElement>,
        members: &[Id<ClassMember>],
        primary_constructor: Id<PrimaryConstructorDeclaration>,
    ) {
        let ctx = host.ctx();
        let ast = host.ast();
        let Some(constructor_element) = host
            .tables()
            .declared_fragment
            .get(primary_constructor)
            .and_then(|&f| ctx.fragment_data(f))
            .and_then(|f| f.element.try_get().copied())
        else {
            return;
        };
        let error_range = primary_constructor_error_range(ast, primary_constructor);
        let formal_parameters = ast[primary_constructor].formal_parameters;
        // Dart `primaryConstructor.body`: the first primary constructor body
        // among the class members.
        let body = members
            .iter()
            .find_map(|&m| ast.cast::<PrimaryConstructorBody>(m));
        let initializers = body.map(|b| ast[b].initializers);

        let interface_fields = &mut self.interfaces[&element];
        let constructor_state =
            interface_fields.for_constructor(unit, constructor_element, error_range, true);
        constructor_state
            .primary_constructors
            .push(primary_constructor);

        constructor_state.update_with_parameters(host, formal_parameters);

        if let Some(initializers) = initializers {
            constructor_state.update_with_initializers(host, initializers);
        }
    }

    /// Dart `_forInterface(element)`.
    fn for_interface(&mut self, ctx: &Ctx<'_>, element: EId<InterfaceElement>) {
        if self.interfaces.contains_key(&element) {
            return;
        }

        let mut field_map = IndexMap::new();
        let mut field_name_counts: IndexMap<&str, usize> = IndexMap::new();

        for &field in &ctx.interface(element).fields {
            let flags = element_ext::first_fragment_flags(ctx, field.raw());
            if flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER) {
                continue;
            }
            let name = ctx.element_data(field.raw()).and_then(|d| d.name);
            if element.tag() == Tag::Enum && name == Some(Name::INDEX) {
                continue;
            }
            let state = if has_initializer(ctx, field) {
                InitState::InitInDeclaration
            } else {
                InitState::NotInit
            };
            field_map.insert(field, state);
            if let Some(name) = name {
                *field_name_counts.entry(ctx.name_str(name)).or_insert(0) += 1;
            }
        }

        let duplicate_field_names = field_name_counts
            .into_iter()
            .filter(|&(_, count)| count > 1)
            .map(|(name, _)| name.to_string())
            .collect();
        self.interfaces.insert(
            element,
            Interface {
                duplicate_field_names,
                fields: field_map,
                constructors: IndexMap::new(),
            },
        );
    }
}

impl Interface {
    /// Dart `_Interface.forConstructor(diagnosticReporter:, errorRange:,
    /// element:, isPrimary:)`.
    fn for_constructor(
        &mut self,
        unit: usize,
        element: ElementId,
        error_range: (usize, usize),
        is_primary: bool,
    ) -> &mut Constructor {
        let fields = &self.fields;
        let duplicate_field_names = &self.duplicate_field_names;
        self.constructors
            .entry(element)
            .or_insert_with(|| Constructor {
                unit,
                error_range,
                fields: fields.clone(),
                duplicate_field_names: duplicate_field_names.clone(),
                is_primary,
                primary_constructors: Vec::new(),
                secondary_constructors: Vec::new(),
                has_redirecting_constructor_invocation: false,
            })
    }
}

impl Constructor {
    /// Dart `_Constructor.report()`: returns the fields that are not
    /// initialized (Dart `node.notInitializedFields = allNotInitialized`),
    /// `None` for a redirecting constructor.
    fn report(&self, unit: &mut UnitVerifier<'_>) -> Option<Vec<EId<FieldElement>>> {
        if self.has_redirecting_constructor_invocation {
            return None;
        }
        let ctx = unit.ctx;
        let type_system = unit.type_system;

        // Prepare lists of not initialized fields.
        let mut not_init_final_fields: Vec<(EId<FieldElement>, String)> = Vec::new();
        let mut not_init_non_nullable_fields: Vec<(EId<FieldElement>, String)> = Vec::new();
        for (&field, &state) in &self.fields {
            if state != InitState::NotInit {
                continue;
            }
            let e = field.raw();
            let flags = element_ext::first_fragment_flags(&ctx, e);
            if element_ext::is_late(&ctx, e) {
                continue;
            }
            if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT)
                || flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL)
            {
                continue;
            }
            if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC) {
                continue;
            }

            let Some(name) = ctx.element_name(e) else {
                continue;
            };
            if self.duplicate_field_names.contains(name) {
                continue;
            }

            if element_ext::is_final(&ctx, e) {
                not_init_final_fields.push((field, name.to_string()));
            } else if type_system.is_potentially_non_nullable(element_ext::variable_type(&ctx, e)) {
                not_init_non_nullable_fields.push((field, name.to_string()));
            }
        }

        let all_not_initialized = not_init_final_fields
            .iter()
            .chain(&not_init_non_nullable_fields)
            .map(|&(f, _)| f)
            .collect();

        self.report_not_initialized_final(unit, &not_init_final_fields);
        self.report_not_initialized_non_nullable(unit, &not_init_non_nullable_fields);
        Some(all_not_initialized)
    }

    /// Dart `reportNotInitializedFinal(notInitFinalFields)`.
    fn report_not_initialized_final(
        &self,
        unit: &mut UnitVerifier<'_>,
        not_init_final_fields: &[(EId<FieldElement>, String)],
    ) {
        let mut names: Vec<&str> = not_init_final_fields
            .iter()
            .map(|(_, n)| n.as_str())
            .collect();
        names.sort();

        let d = match names.as_slice() {
            [] => return,
            [name] => diag::final_not_initialized_constructor_1(name),
            [name1, name2] => diag::final_not_initialized_constructor_2(name1, name2),
            [name1, name2, remaining @ ..] => {
                diag::final_not_initialized_constructor_3_plus(name1, name2, remaining.len() as i64)
            }
        };
        let (offset, length) = self.error_range;
        unit.report(d.at_offset(offset, length));
    }

    /// Dart `reportNotInitializedNonNullable(notInitNonNullableFields)`.
    fn report_not_initialized_non_nullable(
        &self,
        unit: &mut UnitVerifier<'_>,
        not_init_non_nullable_fields: &[(EId<FieldElement>, String)],
    ) {
        let mut names: Vec<&str> = not_init_non_nullable_fields
            .iter()
            .map(|(_, n)| n.as_str())
            .collect();
        names.sort();

        let (offset, length) = self.error_range;
        for name in names {
            let d = diag::not_initialized_non_nullable_instance_field_constructor(name);
            unit.report(d.at_offset(offset, length));
        }
    }

    /// Dart `updateWithInitializers(diagnosticReporter, initializers)`.
    fn update_with_initializers<'a, H: VerifierHost<'a>>(
        &mut self,
        host: &mut H,
        initializers: NodeList<ConstructorInitializer>,
    ) {
        let ctx = host.ctx();
        let initializers = host.ast().list(initializers).to_vec();
        for initializer in initializers {
            let ast = host.ast();
            if ast.is::<RedirectingConstructorInvocation>(initializer) {
                self.has_redirecting_constructor_invocation = true;
            }
            let Some(initializer) = ast.cast::<ConstructorFieldInitializer>(initializer) else {
                continue;
            };
            let field_name = ast[initializer].field_name;
            let Some(field_element) = host
                .element(field_name)
                .map(|e| member::base_element(&ctx, e))
                .and_then(|e| e.cast::<FieldElement>())
            else {
                continue;
            };
            let field = field_element.raw();
            match self.fields.get(&field_element).copied() {
                Some(InitState::NotInit) => {
                    self.fields
                        .insert(field_element, InitState::InitInInitializer);
                }
                Some(InitState::InitInDeclaration) => {
                    if self.is_primary {
                        let d = host.at(
                            diag::field_initialized_in_declaration_and_initializer_of_primary_constructor(),
                            field_name,
                        );
                        host.report(d);
                    } else if element_ext::is_final(&ctx, field)
                        || element_ext::is_const(&ctx, field)
                    {
                        let d = host.at(
                            diag::field_initialized_in_initializer_and_declaration(),
                            field_name,
                        );
                        host.report(d);
                    }
                    self.fields
                        .insert(field_element, InitState::InitInInitializer);
                }
                Some(InitState::InitInFieldFormal) => {
                    let d = host.at(
                        diag::field_initialized_in_parameter_and_initializer(),
                        field_name,
                    );
                    host.report(d);
                }
                Some(InitState::InitInInitializer) => {
                    let name = ctx.element_name(field).unwrap_or("");
                    let d = host.at(
                        diag::field_initialized_by_multiple_initializers(name),
                        field_name,
                    );
                    host.report(d);
                }
                None => {}
            }
        }
    }

    /// Dart `updateWithParameters(formalParameters)`.
    fn update_with_parameters<'a, H: VerifierHost<'a>>(
        &mut self,
        host: &mut H,
        formal_parameters: Id<FormalParameterList>,
    ) {
        let ctx = host.ctx();
        let ast = host.ast();
        let parameters = ast.list(ast[formal_parameters].parameters).to_vec();
        for formal_parameter in parameters {
            let ast = host.ast();
            if !matches!(
                ast.kind(formal_parameter.raw()),
                NodeKind::RegularFormalParameter
                    | NodeKind::FieldFormalParameter
                    | NodeKind::SuperFormalParameter
            ) {
                continue;
            }
            let Some(parameter_element) = host
                .tables()
                .declared_fragment
                .get(formal_parameter)
                .and_then(|&f| ctx.fragment_data(f))
                .and_then(|f| f.element.try_get().copied())
            else {
                continue;
            };
            // Dart `parameterElement is FieldFormalParameterElement`.
            if parameter_element.tag() != Tag::FieldFormalParameter {
                continue;
            }
            let Some(parameter_element) = parameter_element.cast::<FormalParameterElement>() else {
                continue;
            };
            let Some(field_element) = ctx.get(parameter_element).field.get() else {
                continue;
            };
            let field = field_element.raw();
            let name = ast_ext::formal_parameter_parts(ast, formal_parameter.raw()).name;
            match self.fields.get(&field_element).copied() {
                Some(InitState::NotInit) => {
                    self.fields
                        .insert(field_element, InitState::InitInFieldFormal);
                }
                Some(InitState::InitInDeclaration) => {
                    if self.is_primary {
                        if let Some(name) = name {
                            let d = host.at_token(
                                diag::field_initialized_in_declaration_and_parameter_of_primary_constructor(),
                                name,
                            );
                            host.report(d);
                        }
                    } else if (element_ext::is_final(&ctx, field)
                        || element_ext::is_const(&ctx, field))
                        && let Some(name) = name
                    {
                        let field_name = ctx.element_name(field).unwrap_or("");
                        let d = host.at_token(
                            diag::final_initialized_in_declaration_and_constructor(field_name),
                            name,
                        );
                        host.report(d);
                    }
                    self.fields
                        .insert(field_element, InitState::InitInFieldFormal);
                }
                // `InitInFieldFormal`: reported in
                // DuplicateDefinitionVerifier._checkDuplicateIdentifier.
                _ => {}
            }
        }
    }
}

/// Dart `FieldElement.hasInitializer`: whether a fragment of [field] has
/// an initializer.
fn has_initializer(ctx: &Ctx<'_>, field: EId<FieldElement>) -> bool {
    let mut fragment = ctx.element_data(field.raw()).map(|d| d.first_fragment);
    while let Some(f) = fragment {
        let Some(data) = ctx.fragment_data(f) else {
            break;
        };
        if data
            .flags
            .get()
            .contains(FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER)
        {
            return true;
        }
        fragment = data.next_fragment;
    }
    false
}

/// Dart `ConstructorDeclarationImpl.errorRange`: from the type name (or
/// `new` / `factory`) to the end of the constructor name.
fn constructor_error_range(ast: &Ast, node: Id<ConstructorDeclaration>) -> (usize, usize) {
    let n = &ast[node];
    let (start, start_end) = match n.type_name {
        Some(type_name) => (ast.offset(type_name), ast.end(type_name)),
        None => match n.new_keyword.or(n.factory_keyword) {
            Some(t) => (ast.tokens.offset(t), ast_ext::token_end(ast, t)),
            None => (ast.offset(node), ast.offset(node)),
        },
    };
    let end = match n.name {
        Some(name) => ast_ext::token_end(ast, name),
        None => start_end,
    };
    (start as usize, end.saturating_sub(start) as usize)
}

/// Dart `PrimaryConstructorDeclarationImpl.errorRange`: from the first
/// token to the end of the constructor name.
fn primary_constructor_error_range(
    ast: &Ast,
    node: Id<PrimaryConstructorDeclaration>,
) -> (usize, usize) {
    let n = &ast[node];
    let begin = n.const_keyword.unwrap_or(n.type_name);
    let start = ast.tokens.offset(begin);
    let end = match n.constructor_name {
        Some(name) => ast.end(name),
        None => ast_ext::token_end(ast, begin),
    };
    (start as usize, end.saturating_sub(start) as usize)
}
