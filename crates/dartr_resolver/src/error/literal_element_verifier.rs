// Dart source: pkg/analyzer/lib/src/error/literal_element_verifier.dart

//! `LiteralElementVerifier`: the elements of list, set and map literals
//! are assignable to the element, key and value types of the literal. The
//! error verifier creates a [`LiteralElementVerifier`] per literal and
//! calls [`LiteralElementVerifier::verify`] for each element (Dart
//! `_checkForListElementTypeNotAssignable`, `_checkForMapTypeNotAssignable`,
//! `_checkForSetElementTypeNotAssignable3`).
//!
//! The Dart verifier calls back into the error verifier
//! (`checkForUseOfVoidResult`, `getImplicitCallMethod`, the diagnostics of
//! `inferFunctionTypeInstantiation`); these are the methods of
//! [`LiteralElementHost`].

use dartr_ast::{
    CollectionElement, Expression, ForElement, Id, IfElement, MapLiteralEntry, MethodInvocation,
    NodeId, NullAwareElement, SpreadElement,
};
use dartr_diagnostics::{Diagnostic, LocatableDiagnostic, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{ElemRef, MethodElement, Nullability, TypeId, TypeKind};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::TokenType;
use dartr_typesystem::generic_inferrer::{
    InferenceErrorEntity, InferenceErrorEntityKind, InferenceFlags,
};
use dartr_typesystem::inheritance_manager3::{GetMemberOptions, InheritanceManager3, Name};
use dartr_typesystem::type_system_operations::TypeSystemOperations;
use dartr_typesystem::{TypeExt, member};

use super::{UnitVerifier, VerifierHost};

/// What the literal element verifier needs from the error verifier (Dart
/// `ErrorVerifier` / `ErrorDetectionHelpers`).
pub trait LiteralElementHost<'a>: VerifierHost<'a> {
    /// Reports a diagnostic that is already located (the diagnostics of
    /// `TypeSystem.inferFunctionTypeInstantiation`).
    fn report_diagnostic(&mut self, diagnostic: Diagnostic);

    /// Dart `ErrorDetectionHelpers.checkForUseOfVoidResult(expression)`:
    /// whether [expression] has the type `void` (reports
    /// `useOfVoidResult`).
    fn check_for_use_of_void_result(&mut self, expression: Id<Expression>) -> bool
    where
        Self: Sized,
    {
        let ctx = self.ctx();
        if !self
            .static_type(expression)
            .is_some_and(|t| matches!(ctx.ty(t), TypeKind::Void))
        {
            return false;
        }
        let d = match self.ast().cast::<MethodInvocation>(expression) {
            Some(invocation) => {
                let method_name = self.ast()[invocation].method_name;
                self.at(diag::use_of_void_result(), method_name)
            }
            None => self.at(diag::use_of_void_result(), expression),
        };
        self.report(d);
        true
    }

    /// Dart `ErrorDetectionHelpers.getImplicitCallMethod(type, context,
    /// errorNode)`: the `call` method of [ty] when it is torn off
    /// implicitly in the [context] type.
    fn get_implicit_call_method(&self, ty: TypeId, context: TypeId) -> Option<ElemRef> {
        let ctx = self.ctx();
        let type_system = self.type_system();
        let mut ty = ty;
        let mut visited_types = vec![ty];
        while let TypeKind::TypeParameter {
            param,
            promoted_bound,
            ..
        } = *ctx.ty(ty)
        {
            if ctx.nullability_suffix(ty) != Nullability::None {
                // The value might be `null`, so implicit `.call` tearoff is
                // invalid.
                return None;
            }
            // Dart `TypeParameterType.bound`.
            ty = promoted_bound
                .or_else(|| ctx.type_parameter_bound(param))
                .unwrap_or(ctx.tp.object_question_type());
            if visited_types.contains(&ty) {
                // A cycle!
                return None;
            }
            visited_types.push(ty);
        }
        if type_system.accepts_function_type(context)
            && let TypeKind::Interface { element, .. } = *ctx.ty(ty)
            && ctx.nullability_suffix(ty) != Nullability::Question
        {
            let library = ctx.element_data(element.raw()).and_then(|d| d.library);
            let name = Name::for_library(&ctx, library, "call");
            let member =
                InheritanceManager3::new(ctx).get_member3(ty, name, GetMemberOptions::default())?;
            // Dart `.tryCast<InternalMethodElement>()`.
            member::base_element(&ctx, member)
                .is::<MethodElement>()
                .then_some(member)
        } else {
            None
        }
    }
}

impl<'a> LiteralElementHost<'a> for UnitVerifier<'a> {
    fn report_diagnostic(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

/// Dart `LiteralElementVerifier`: the verifier for the collection elements
/// of one list, set, or map literal.
#[derive(Clone, Copy, Debug, Default)]
pub struct LiteralElementVerifier {
    pub for_list: bool,
    pub for_set: bool,
    pub element_type: Option<TypeId>,
    pub for_map: bool,
    pub map_key_type: Option<TypeId>,
    pub map_value_type: Option<TypeId>,
}

impl LiteralElementVerifier {
    /// Dart `LiteralElementVerifier.verify(element)`.
    pub fn verify<'a, H: LiteralElementHost<'a>>(
        &self,
        host: &mut H,
        element: Id<CollectionElement>,
    ) {
        self.verify_element(host, Some(element));
    }

    /// Dart `_checkAssignableToElementType(type, errorNode)`: checks that
    /// [ty] is assignable to the [element_type], otherwise reports the list
    /// or set error on the [error_node].
    fn check_assignable_to_element_type<'a, H: LiteralElementHost<'a>>(
        &self,
        host: &mut H,
        ty: TypeId,
        error_node: NodeId,
    ) {
        let ctx = host.ctx();
        let type_system = host.type_system();
        let strict_casts = host.options().strict_casts;
        let element_type = self.element_type.unwrap_or(TypeId::DYNAMIC);

        if !type_system.is_assignable_to(ty, element_type, strict_casts) {
            let assignable_when_nullable = type_system.is_assignable_to(
                ty,
                type_system.make_nullable(element_type),
                strict_casts,
            );
            let error_code: fn(_, _) -> LocatableDiagnostic =
                match (self.for_list, assignable_when_nullable) {
                    (false, false) => diag::set_element_type_not_assignable,
                    (false, true) => diag::set_element_type_not_assignable_nullability,
                    (true, false) => diag::list_element_type_not_assignable,
                    (true, true) => diag::list_element_type_not_assignable_nullability,
                };
            let d = host.at(
                error_code(type_arg(&ctx, ty), type_arg(&ctx, element_type)),
                error_node,
            );
            host.report(d);
        }
    }

    /// Dart `_verifyElement(element)`: verifies that [element] can be
    /// assigned to the [element_type] of the enclosing list, set, or map
    /// literal.
    fn verify_element<'a, H: LiteralElementHost<'a>>(
        &self,
        host: &mut H,
        element: Option<Id<CollectionElement>>,
    ) {
        let Some(element) = element else {
            return;
        };
        let ctx = host.ctx();
        let ast = host.ast();
        if let Some(for_element) = ast.cast::<ForElement>(element) {
            let body = ast[for_element].body;
            self.verify_element(host, Some(body));
        } else if let Some(if_element) = ast.cast::<IfElement>(element) {
            let then_element = ast[if_element].then_element;
            let else_element = ast[if_element].else_element;
            self.verify_element(host, Some(then_element));
            self.verify_element(host, else_element);
        } else if let Some(entry) = ast.cast::<MapLiteralEntry>(element) {
            if self.for_map {
                self.verify_map_literal_entry(host, entry);
            } else {
                let d = host.at(diag::map_entry_not_in_map(), element);
                host.report(d);
            }
        } else if let Some(spread) = ast.cast::<SpreadElement>(element) {
            let is_null_aware = ast.tokens.ty(ast[spread].spread_operator)
                == TokenType::PERIOD_PERIOD_PERIOD_QUESTION;
            let expression = ast[spread].expression;
            if self.for_list || self.for_set {
                self.verify_spread_for_list_or_set(host, is_null_aware, expression);
            } else if self.for_map {
                self.verify_spread_for_map(host, is_null_aware, expression);
            }
        } else if let Some(null_aware) = ast.cast::<NullAwareElement>(element) {
            if self.for_list || self.for_set {
                let value = ast[null_aware].value;
                let value_type = type_or_throw(host, value);
                // A null-aware marker tests this expression for `null`, so a
                // `void` value is used even when the stored element type is
                // also `void`.
                if matches!(ctx.ty(value_type), TypeKind::Void) {
                    host.check_for_use_of_void_result(value);
                    return;
                }
                let promoted = host.type_system().promote_to_non_null(value_type);
                self.check_assignable_to_element_type(host, promoted, element.raw());
            } else {
                let d = host.at(diag::expression_in_map(), element);
                host.report(d);
            }
        } else if let Some(expression) = ast.cast::<Expression>(element) {
            if self.for_list || self.for_set {
                let element_type_is_void = self
                    .element_type
                    .is_some_and(|t| matches!(ctx.ty(t), TypeKind::Void));
                if !element_type_is_void && host.check_for_use_of_void_result(expression) {
                    return;
                }
                let ty = type_or_throw(host, expression);
                self.check_assignable_to_element_type(host, ty, expression.raw());
            } else {
                let d = host.at(diag::expression_in_map(), expression);
                host.report(d);
            }
        }
    }

    /// Dart `_verifyMapLiteralEntry(entry)`: verifies that the key and the
    /// value of [entry] are assignable to [map_key_type] and
    /// [map_value_type].
    fn verify_map_literal_entry<'a, H: LiteralElementHost<'a>>(
        &self,
        host: &mut H,
        entry: Id<MapLiteralEntry>,
    ) {
        let ctx = host.ctx();
        let type_system = host.type_system();
        let strict_casts = host.options().strict_casts;
        let is_void = |t: TypeId| matches!(ctx.ty(t), TypeKind::Void);
        let ast = host.ast();
        let key = ast[entry].key;
        let value = ast[entry].value;
        let key_question = ast[entry].key_question.is_some();
        let value_question = ast[entry].value_question.is_some();

        let map_key_type = self.map_key_type.unwrap_or(TypeId::DYNAMIC);
        let mut key_type = type_or_throw(host, key);

        // A null-aware marker tests this expression for `null`, so a `void`
        // value is used even when the stored key type is also `void`.
        if key_question && is_void(key_type) {
            host.check_for_use_of_void_result(key);
            return;
        }

        if !is_void(map_key_type) && host.check_for_use_of_void_result(key) {
            return;
        }

        let map_value_type = self.map_value_type.unwrap_or(TypeId::DYNAMIC);
        let mut value_type = type_or_throw(host, value);

        // A null-aware marker tests this expression for `null`, so a `void`
        // value is used even when the stored value type is also `void`.
        if value_question && is_void(value_type) {
            host.check_for_use_of_void_result(value);
            return;
        }

        if !is_void(map_value_type) && host.check_for_use_of_void_result(value) {
            return;
        }

        // If the key is null-aware, the entry is only added when the key is
        // not `null`, so the key type to check should be promoted to
        // non-null.
        if key_question {
            key_type = type_system.promote_to_non_null(key_type);
        }
        if !type_system.is_assignable_to(key_type, map_key_type, strict_casts) {
            let d = if !key_question
                && type_system.is_assignable_to(
                    key_type,
                    type_system.make_nullable(map_key_type),
                    strict_casts,
                ) {
                diag::map_key_type_not_assignable_nullability(
                    type_arg(&ctx, key_type),
                    type_arg(&ctx, map_key_type),
                )
            } else {
                diag::map_key_type_not_assignable(
                    type_arg(&ctx, key_type),
                    type_arg(&ctx, map_key_type),
                )
            };
            let d = host.at(d, key);
            host.report(d);
        }

        // If the value is null-aware, the entry is only added when the value
        // is not `null`, so the value type to check should be promoted to
        // non-null.
        if value_question {
            value_type = type_system.promote_to_non_null(value_type);
        }
        if !type_system.is_assignable_to(value_type, map_value_type, strict_casts) {
            let d = if !value_question
                && type_system.is_assignable_to(
                    value_type,
                    type_system.make_nullable(map_value_type),
                    strict_casts,
                ) {
                diag::map_value_type_not_assignable_nullability(
                    type_arg(&ctx, value_type),
                    type_arg(&ctx, map_value_type),
                )
            } else {
                diag::map_value_type_not_assignable(
                    type_arg(&ctx, value_type),
                    type_arg(&ctx, map_value_type),
                )
            };
            let d = host.at(d, value);
            host.report(d);
        }
    }

    /// Dart `_verifySpreadForListOrSet(isNullAware, expression)`: verifies
    /// that the type of the elements of [expression] can be assigned to the
    /// [element_type] of the enclosing collection.
    fn verify_spread_for_list_or_set<'a, H: LiteralElementHost<'a>>(
        &self,
        host: &mut H,
        is_null_aware: bool,
        expression: Id<Expression>,
    ) {
        let ctx = host.ctx();
        let type_system = host.type_system();
        let strict_casts = host.options().strict_casts;
        let expression_type = type_or_throw(host, expression);
        if matches!(ctx.ty(expression_type), TypeKind::Dynamic) {
            if strict_casts {
                let d = host.at(diag::not_iterable_spread(), expression);
                host.report(d);
            }
            return;
        }

        if type_system.is_subtype_of(expression_type, TypeId::NEVER) {
            return;
        }

        if type_system.is_subtype_of(expression_type, type_system.null_none()) {
            if is_null_aware {
                return;
            }
            let d = host.at(diag::not_null_aware_null_spread(), expression);
            host.report(d);
            return;
        }

        let Some(iterable_type) =
            ctx.as_instance_of(expression_type, ctx.tp.iterable_element().upcast())
        else {
            let d = host.at(diag::not_iterable_spread(), expression);
            host.report(d);
            return;
        };

        let iterable_element_type = ctx
            .type_arguments(iterable_type)
            .first()
            .copied()
            .unwrap_or(TypeId::DYNAMIC);
        let element_type = self.element_type.unwrap_or(TypeId::DYNAMIC);
        if type_system.is_assignable_to(iterable_element_type, element_type, strict_casts) {
            return;
        }
        let error_code: fn(_, _) -> LocatableDiagnostic = if self.for_list {
            diag::list_element_type_not_assignable
        } else {
            diag::set_element_type_not_assignable
        };
        // Also check for an "implicit tear-off conversion" which would be
        // applied after desugaring a spread element.
        let Some(implicit_call_method) =
            host.get_implicit_call_method(iterable_element_type, element_type)
        else {
            let d = host.at(
                error_code(
                    type_arg(&ctx, iterable_element_type),
                    type_arg(&ctx, element_type),
                ),
                expression,
            );
            host.report(d);
            return;
        };
        let mut tearoff_type = member::type_(&ctx, implicit_call_method);
        if host
            .features()
            .is_experiment_enabled(ExperimentalFlag::ConstructorTearoffs)
        {
            let type_arguments =
                infer_function_type_instantiation(host, element_type, tearoff_type, expression);
            if !type_arguments.is_empty() {
                tearoff_type = ctx.instantiate_function_type(tearoff_type, &type_arguments);
            }
        }

        if !type_system.is_assignable_to(tearoff_type, element_type, strict_casts) {
            let d = host.at(
                error_code(
                    type_arg(&ctx, iterable_element_type),
                    type_arg(&ctx, element_type),
                ),
                expression,
            );
            host.report(d);
        }
    }

    /// Dart `_verifySpreadForMap(isNullAware, expression)`: verifies that
    /// [expression] is a subtype of `Map<Object, Object>`, and its keys and
    /// values are assignable to [map_key_type] and [map_value_type].
    fn verify_spread_for_map<'a, H: LiteralElementHost<'a>>(
        &self,
        host: &mut H,
        is_null_aware: bool,
        expression: Id<Expression>,
    ) {
        let ctx = host.ctx();
        let type_system = host.type_system();
        let strict_casts = host.options().strict_casts;
        let expression_type = type_or_throw(host, expression);
        if matches!(ctx.ty(expression_type), TypeKind::Dynamic) {
            if strict_casts {
                let d = host.at(diag::not_map_spread(), expression);
                host.report(d);
            }
            return;
        }

        if type_system.is_subtype_of(expression_type, TypeId::NEVER) {
            return;
        }

        if type_system.is_subtype_of(expression_type, type_system.null_none()) {
            if is_null_aware {
                return;
            }
            let d = host.at(diag::not_null_aware_null_spread(), expression);
            host.report(d);
            return;
        }

        let Some(map_type) = ctx.as_instance_of(expression_type, ctx.tp.map_element().upcast())
        else {
            let d = host.at(diag::not_map_spread(), expression);
            host.report(d);
            return;
        };

        let type_arguments = ctx.type_arguments(map_type);
        let key_type = type_arguments.first().copied().unwrap_or(TypeId::DYNAMIC);
        let map_key_type = self.map_key_type.unwrap_or(TypeId::DYNAMIC);
        if !type_system.is_assignable_to(key_type, map_key_type, strict_casts) {
            let d = host.at(
                diag::map_key_type_not_assignable(
                    type_arg(&ctx, key_type),
                    type_arg(&ctx, map_key_type),
                ),
                expression,
            );
            host.report(d);
        }

        let value_type = type_arguments.get(1).copied().unwrap_or(TypeId::DYNAMIC);
        let map_value_type = self.map_value_type.unwrap_or(TypeId::DYNAMIC);
        if !type_system.is_assignable_to(value_type, map_value_type, strict_casts) {
            let d = host.at(
                diag::map_value_type_not_assignable(
                    type_arg(&ctx, value_type),
                    type_arg(&ctx, map_value_type),
                ),
                expression,
            );
            host.report(d);
        }
    }
}

/// Dart `typeSystem.inferFunctionTypeInstantiation(elementType,
/// tearoffType, diagnosticReporter:, errorNode: expression,
/// genericMetadataIsEnabled: true, ...)`.
fn infer_function_type_instantiation<'a, H: LiteralElementHost<'a>>(
    host: &mut H,
    context: TypeId,
    fn_type: TypeId,
    error_node: Id<Expression>,
) -> Vec<TypeId> {
    let options = host.options();
    let type_system = host.type_system();
    let flags = InferenceFlags {
        generic_metadata_is_enabled: true,
        inference_using_bounds_is_enabled: host
            .features()
            .is_experiment_enabled(ExperimentalFlag::InferenceUsingBounds),
        strict_inference: options.strict_inference,
    };
    let entity = InferenceErrorEntity {
        offset: host.ast().offset(error_node) as usize,
        length: host.ast().length(error_node) as usize,
        is_invocation_in_as_expression: false,
        kind: InferenceErrorEntityKind::Expression {
            static_type: host.static_type(error_node),
        },
    };
    let mut reported = Vec::new();
    let result = {
        let mut listener = |d: Diagnostic| reported.push(d);
        let mut reporter = dartr_diagnostics::DiagnosticReporter::new(&mut listener);
        type_system.infer_function_type_instantiation(
            context,
            fn_type,
            Some(&mut reporter),
            Some(entity),
            TypeSystemOperations::new(type_system, options.strict_casts),
            flags,
            None,
            None,
        )
    };
    for d in reported {
        host.report_diagnostic(d);
    }
    result
}

/// Dart `expression.typeOrThrow` (`dynamic` when not resolved).
fn type_or_throw<'a, H: VerifierHost<'a>>(host: &H, expression: Id<Expression>) -> TypeId {
    host.static_type(expression).unwrap_or(TypeId::DYNAMIC)
}
