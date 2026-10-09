// Dart source: pkg/analyzer/lib/src/error/return_type_verifier.dart,
// pkg/analyzer/lib/src/generated/error_verifier.dart
// (EnclosingExecutableContext)

//! `ReturnTypeVerifier`: the declared return types of `async`, `sync*` and
//! `async*` functions, and the returned values of `return` statements and
//! expression function bodies. The error verifier (and the error handler
//! verifier) call the `verify_*` functions with the context of the
//! enclosing executable (Dart `_returnTypeVerifier.enclosingExecutable`).
//!
//! [`EnclosingExecutableContext`] is the Dart class of
//! `error_verifier.dart`; it is here because the return type verifier is
//! its main reader.

use dartr_ast::{
    Expression, ExpressionFunctionBody, Id, ParenthesizedExpression, ReturnStatement,
    TypeAnnotation,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    ClassElement, Ctx, EId, ElemRef, ElementId, FragmentFlags, Nullability, Tag, TypeId, TypeKind,
};
use dartr_typesystem::{TypeExt, member};

use super::VerifierHost;
use crate::element_ext::first_fragment_flags;

/// Dart `EnclosingExecutableContext` (error_verifier.dart): the executable
/// that encloses the node being verified.
#[derive(Clone, Debug)]
pub struct EnclosingExecutableContext {
    /// Dart `element` (an `InternalExecutableElement`).
    pub element: Option<ElemRef>,
    pub is_asynchronous: bool,
    pub is_const_constructor: bool,
    pub is_generative_constructor: bool,
    pub is_generator: bool,
    pub in_factory_constructor: bool,
    pub in_static_method: bool,
    /// If this context is the first argument in a method invocation of
    /// `Future.catchError`, the return type expected for
    /// `Future<T>.catchError`'s `onError` parameter, which is
    /// `FutureOr<T>`, otherwise `None`.
    pub catch_error_on_error_return_type: Option<TypeId>,
    pub then_on_error_return_type: Option<TypeId>,
    /// The return statements that have a value (Dart `_returnsWith`).
    pub returns_with: Vec<Id<ReturnStatement>>,
    /// The return statements that do not have a value (Dart
    /// `_returnsWithout`).
    pub returns_without: Vec<Id<ReturnStatement>>,
    /// Set to `false` when the declared return type is not legal for the
    /// kind of the function body, e.g. not `Future` for `async`.
    pub has_legal_return_type: bool,
    /// The number of enclosing `CatchClause`s in this executable.
    pub catch_clause_level: u32,
    /// The display name of [element] (Dart `displayName`).
    display_name: Option<String>,
    /// The tag of the base element of [element].
    tag: Option<Tag>,
    /// Dart `element.returnType`.
    element_return_type: Option<TypeId>,
}

impl EnclosingExecutableContext {
    /// Dart `EnclosingExecutableContext(element, isAsynchronous:,
    /// isGenerator:, catchErrorOnErrorReturnType:,
    /// thenOnErrorReturnType:)`.
    pub fn new(
        ctx: &Ctx<'_>,
        element: Option<ElemRef>,
        is_asynchronous: bool,
        is_generator: bool,
        catch_error_on_error_return_type: Option<TypeId>,
        then_on_error_return_type: Option<TypeId>,
    ) -> EnclosingExecutableContext {
        let base = element.map(|e| member::base_element(ctx, e));
        let tag = base.map(|e| e.tag());
        let constructor_flags = match base {
            Some(e) if e.tag() == Tag::Constructor => Some(first_fragment_flags(ctx, e)),
            _ => None,
        };
        EnclosingExecutableContext {
            element,
            is_asynchronous,
            is_const_constructor: constructor_flags
                .is_some_and(|f| f.contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)),
            is_generative_constructor: constructor_flags
                .is_some_and(|f| !f.contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)),
            is_generator,
            in_factory_constructor: base.is_some_and(|e| in_factory_constructor(ctx, e)),
            in_static_method: base.is_some_and(|e| in_static_method(ctx, e)),
            catch_error_on_error_return_type,
            then_on_error_return_type,
            returns_with: Vec::new(),
            returns_without: Vec::new(),
            has_legal_return_type: true,
            catch_clause_level: 0,
            display_name: base.map(|e| display_name(ctx, e)),
            tag,
            element_return_type: element.map(|e| member::return_type(ctx, e)),
        }
    }

    /// Dart `EnclosingExecutableContext.empty()`.
    pub fn empty() -> EnclosingExecutableContext {
        EnclosingExecutableContext {
            element: None,
            is_asynchronous: false,
            is_const_constructor: false,
            is_generative_constructor: false,
            is_generator: false,
            in_factory_constructor: false,
            in_static_method: false,
            catch_error_on_error_return_type: None,
            then_on_error_return_type: None,
            returns_with: Vec::new(),
            returns_without: Vec::new(),
            has_legal_return_type: true,
            catch_clause_level: 0,
            display_name: None,
            tag: None,
            element_return_type: None,
        }
    }

    /// Dart `displayName`.
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Dart `isClosure`: a local function without a name.
    pub fn is_closure(&self) -> bool {
        self.tag == Some(Tag::LocalFunction) && self.display_name.as_deref() == Some("")
    }

    /// Dart `isConstructor`.
    pub fn is_constructor(&self) -> bool {
        self.tag == Some(Tag::Constructor)
    }

    /// Dart `isFunction`.
    pub fn is_function(&self) -> bool {
        match self.tag {
            Some(Tag::LocalFunction | Tag::TopLevelFunction) => {
                self.display_name.as_deref().is_some_and(|n| !n.is_empty())
            }
            Some(Tag::Getter | Tag::Setter) => true,
            _ => false,
        }
    }

    /// Dart `isMethod`.
    pub fn is_method(&self) -> bool {
        self.tag == Some(Tag::Method)
    }

    /// Dart `isSynchronous`.
    pub fn is_synchronous(&self) -> bool {
        !self.is_asynchronous
    }

    /// Dart `returnType`.
    pub fn return_type(&self) -> TypeId {
        self.catch_error_on_error_return_type
            .or(self.then_on_error_return_type)
            .or(self.element_return_type)
            .unwrap_or(TypeId::DYNAMIC)
    }
}

/// Dart `ExecutableElement.displayName`: the name (`''` for an unnamed
/// local function); for a constructor, `ClassName` or `ClassName.name`.
fn display_name(ctx: &Ctx<'_>, element: ElementId) -> String {
    let name = ctx.element_name(element);
    if element.tag() == Tag::Constructor {
        let class_name = ctx
            .element_data(element)
            .and_then(|d| d.enclosing)
            .and_then(|e| ctx.element_name(e))
            .unwrap_or("<null>");
        return match name.unwrap_or("<null>") {
            "new" => class_name.to_string(),
            name => format!("{class_name}.{name}"),
        };
    }
    name.unwrap_or("").to_string()
}

/// The enclosing element of the first fragment of [element] (Dart
/// `element.firstFragment.enclosingFragment?.element`); `None` at the
/// library level.
fn enclosing_of_first_fragment(ctx: &Ctx<'_>, element: ElementId) -> Option<ElementId> {
    let data = ctx.element_data(element)?;
    let enclosing = ctx.fragment_data(data.first_fragment)?.enclosing_fragment?;
    ctx.fragment_data(enclosing)?.element.try_get().copied()
}

/// Dart `EnclosingExecutableContext._inFactoryConstructor(element)`.
fn in_factory_constructor(ctx: &Ctx<'_>, element: ElementId) -> bool {
    let Some(enclosing) = enclosing_of_first_fragment(ctx, element) else {
        return false;
    };
    if element.tag() == Tag::Constructor {
        return first_fragment_flags(ctx, element)
            .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY);
    }
    in_factory_constructor(ctx, enclosing)
}

/// Dart `EnclosingExecutableContext._inStaticMethod(element)`.
fn in_static_method(ctx: &Ctx<'_>, element: ElementId) -> bool {
    let Some(enclosing) = enclosing_of_first_fragment(ctx, element) else {
        return false;
    };
    if matches!(
        enclosing.tag(),
        Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType | Tag::Extension
    ) {
        if matches!(
            element.tag(),
            Tag::Constructor
                | Tag::Method
                | Tag::Getter
                | Tag::Setter
                | Tag::TopLevelFunction
                | Tag::LocalFunction
        ) {
            return member::is_static(ctx, ElemRef::Base(element));
        }
    }
    in_static_method(ctx, enclosing)
}

/// Dart `ReturnTypeVerifier._flattenedReturnType`.
fn flattened_return_type<'a, H: VerifierHost<'a>>(
    host: &H,
    enclosing_executable: &EnclosingExecutableContext,
) -> TypeId {
    let return_type = enclosing_executable.return_type();
    if enclosing_executable.is_synchronous() {
        return_type
    } else {
        host.type_system().flatten(return_type)
    }
}

/// Dart `ReturnTypeVerifier.verifyExpressionFunctionBody(node)`.
pub fn verify_expression_function_body<'a, H: VerifierHost<'a>>(
    host: &mut H,
    enclosing_executable: &EnclosingExecutableContext,
    node: Id<ExpressionFunctionBody>,
) {
    let ctx = host.ctx();
    // This enables concise declarations of void functions.
    if matches!(
        ctx.ty(flattened_return_type(host, enclosing_executable)),
        TypeKind::Void
    ) {
        return;
    }

    let expression = host.ast()[node].expression;
    check_return_expression(host, enclosing_executable, expression);
}

/// Dart `ReturnTypeVerifier.verifyReturnStatement(statement)`.
pub fn verify_return_statement<'a, H: VerifierHost<'a>>(
    host: &mut H,
    enclosing_executable: &EnclosingExecutableContext,
    statement: Id<ReturnStatement>,
) {
    let expression = host.ast()[statement].expression;

    if enclosing_executable.is_generative_constructor {
        if let Some(expression) = expression {
            let d = host.at(diag::return_in_generative_constructor(), expression);
            host.report(d);
        }
        return;
    }

    if enclosing_executable.is_generator {
        return;
    }

    let Some(expression) = expression else {
        check_return_without_value(host, enclosing_executable, statement);
        return;
    };

    check_return_expression(host, enclosing_executable, expression);
}

/// Dart `ReturnTypeVerifier.verifyReturnType(returnType)`: the declared
/// return type of an `async`, `sync*` or `async*` function. Sets
/// `enclosingExecutable.hasLegalReturnType` to `false` when it reports.
pub fn verify_return_type<'a, H: VerifierHost<'a>>(
    host: &mut H,
    enclosing_executable: &mut EnclosingExecutableContext,
    return_type: Option<Id<TypeAnnotation>>,
) {
    // If no declared type, then the type is `dynamic`, which is valid.
    let Some(return_type) = return_type else {
        return;
    };
    let tp = host.ctx().tp;

    if enclosing_executable.is_asynchronous {
        if enclosing_executable.is_generator {
            check_element(
                host,
                enclosing_executable,
                return_type,
                tp.stream_element(),
                diag::illegal_async_generator_return_type(),
            );
        } else {
            check_element(
                host,
                enclosing_executable,
                return_type,
                tp.future_element(),
                diag::illegal_async_return_type(),
            );
        }
    } else if enclosing_executable.is_generator {
        check_element(
            host,
            enclosing_executable,
            return_type,
            tp.iterable_element(),
            diag::illegal_sync_generator_return_type(),
        );
    }
}

/// Dart `checkElement(expectedElement, locatableDiagnostic)` (local
/// function of `verifyReturnType`).
fn check_element<'a, H: VerifierHost<'a>>(
    host: &mut H,
    enclosing_executable: &mut EnclosingExecutableContext,
    return_type: Id<TypeAnnotation>,
    expected_element: EId<ClassElement>,
    locatable_diagnostic: LocatableDiagnostic,
) {
    let ctx = host.ctx();
    // It is a compile-time error if the declared return type of a function
    // marked `sync*` or `async*` is `void`; it is a compile-time error if
    // the declared return type of a function marked `...` is not a
    // supertype of `...`.
    let illegal = (enclosing_executable.is_generator
        && matches!(ctx.ty(enclosing_executable.return_type()), TypeKind::Void))
        || !is_legal_return_type(host, enclosing_executable, expected_element);
    if illegal {
        // Dart `reportError()`.
        enclosing_executable.has_legal_return_type = false;
        let d = host.at(locatable_diagnostic, return_type);
        host.report(d);
    }
}

/// Dart `_checkReturnExpression(expression)`: checks for a type mismatch
/// between the type of [expression] and the expected return type of the
/// enclosing executable.
fn check_return_expression<'a, H: VerifierHost<'a>>(
    host: &mut H,
    enclosing_executable: &EnclosingExecutableContext,
    expression: Id<Expression>,
) {
    if !enclosing_executable.has_legal_return_type {
        // ILLEGAL_ASYNC_RETURN_TYPE has already been reported, meaning the
        // _declared_ return type is illegal; don't confuse by also
        // reporting that the type being returned here does not match that
        // illegal return type.
        return;
    }

    if enclosing_executable.is_generator {
        // RETURN_IN_GENERATOR has already been reported; do not report a
        // duplicate error.
        return;
    }

    let ctx = host.ctx();
    let type_system = host.type_system();
    let strict_casts = host.options().strict_casts;
    let is_void = |t: TypeId| matches!(ctx.ty(t), TypeKind::Void);

    // `T` is the declared return type.
    // `S` is the static type of the expression.
    let t = enclosing_executable.return_type();
    let s = host.static_type(expression).unwrap_or(TypeId::DYNAMIC);

    if enclosing_executable.is_synchronous() {
        // It is a compile-time error if `T` is `void`, and `S` is neither
        // `void`, `dynamic`, nor `Null`.
        if is_void(t) && !is_void_dynamic_or_null(&ctx, s) {
            report_type_error(host, enclosing_executable, expression, s, t);
            return;
        }
        // It is a compile-time error if `S` is `void`, and `T` is neither
        // `void` nor `dynamic`.
        if is_void(s) && !is_void_dynamic(&ctx, t) {
            report_type_error(host, enclosing_executable, expression, s, t);
            return;
        }
        // It is a compile-time error if `S` is not `void`, and `S` is not
        // assignable to `T`.
        if !is_void(s) {
            if let TypeKind::Record { positional, .. } = *ctx.ty(t)
                && ctx.list(positional).len() == 1
                && !matches!(ctx.ty(s), TypeKind::Record { .. })
                && host.ast().is::<ParenthesizedExpression>(expression)
            {
                let field = ctx.list(positional)[0];
                if type_system.is_assignable_to(field, s, strict_casts) {
                    let d = host.at(
                        diag::record_literal_one_positional_no_trailing_comma_by_type(),
                        expression,
                    );
                    host.report(d);
                    return;
                }
            }

            if !type_system.is_assignable_to(s, t, strict_casts) {
                report_type_error(host, enclosing_executable, expression, s, t);
                return;
            }
        }
        // OK
        return;
    }

    if enclosing_executable.is_asynchronous {
        let t_v = type_system.future_value_type(t);
        let flatten_s = type_system.flatten(s);
        // It is a compile-time error if `flatten(T)` is `void`, and
        // `flatten(S)` is neither `void`, `dynamic`, nor `Null`.
        if is_void(t_v) && !is_void_dynamic_or_null(&ctx, flatten_s) {
            report_type_error(host, enclosing_executable, expression, s, t);
            return;
        }
        // It is a compile-time error if `flatten(S)` is `void`, and
        // `flatten(T)` is neither `void`, `dynamic`.
        if is_void(flatten_s) && !is_void_dynamic(&ctx, t_v) {
            report_type_error(host, enclosing_executable, expression, s, t);
            return;
        }
        // It is a compile-time error if `flatten(S)` is not `void`, and
        // `Future<flatten(S)>` is not assignable to `T`.
        if !is_void(flatten_s)
            && !type_system.is_assignable_to(s, t_v, strict_casts)
            && !type_system.is_subtype_of(flatten_s, t_v)
        {
            report_type_error(host, enclosing_executable, expression, s, t);
        }
    }
}

/// Dart `reportTypeError()` (local function of `_checkReturnExpression`).
fn report_type_error<'a, H: VerifierHost<'a>>(
    host: &mut H,
    enclosing_executable: &EnclosingExecutableContext,
    expression: Id<Expression>,
    s: TypeId,
    t: TypeId,
) {
    let ctx = host.ctx();
    let (s_arg, t_arg) = (type_arg(&ctx, s), type_arg(&ctx, t));
    let display_name = enclosing_executable.display_name().unwrap_or("");
    let d = if enclosing_executable
        .catch_error_on_error_return_type
        .is_some()
    {
        diag::return_of_invalid_type_from_catch_error(s_arg, t_arg)
    } else if enclosing_executable.then_on_error_return_type.is_some() {
        diag::return_of_invalid_type_from_then(s_arg, t_arg)
    } else if enclosing_executable.is_closure() {
        diag::return_of_invalid_type_from_closure(s_arg, t_arg)
    } else if enclosing_executable.is_constructor() {
        diag::return_of_invalid_type_from_constructor(s_arg, t_arg, display_name)
    } else if enclosing_executable.is_function() {
        diag::return_of_invalid_type_from_function(s_arg, t_arg, display_name)
    } else if enclosing_executable.is_method() {
        diag::return_of_invalid_type_from_method(s_arg, t_arg, display_name)
    } else {
        return;
    };
    let d = host.at(d, expression);
    host.report(d);
}

/// Dart `_checkReturnWithoutValue(statement)`.
fn check_return_without_value<'a, H: VerifierHost<'a>>(
    host: &mut H,
    enclosing_executable: &EnclosingExecutableContext,
    statement: Id<ReturnStatement>,
) {
    let ctx = host.ctx();
    let t = enclosing_executable.return_type();
    if enclosing_executable.is_synchronous() {
        if is_void_dynamic_or_null(&ctx, t) {
            return;
        }
    } else {
        let t_v = host.type_system().future_value_type(t);
        if is_void_dynamic_or_null(&ctx, t_v) {
            return;
        }
    }

    let return_keyword = host.ast()[statement].return_keyword;
    let d = host.at_token(diag::return_without_value(), return_keyword);
    host.report(d);
}

/// Dart `_isLegalReturnType(expectedElement)`: whether
/// `expectedElement<Never>` is a subtype of the declared return type (an
/// `async` function returns a `Future<T>`, a `sync*` function an
/// `Iterable<T>`, an `async*` function a `Stream<T>`).
fn is_legal_return_type<'a, H: VerifierHost<'a>>(
    host: &H,
    enclosing_executable: &EnclosingExecutableContext,
    expected_element: EId<ClassElement>,
) -> bool {
    let ctx = host.ctx();
    let return_type = enclosing_executable.return_type();
    let lower_bound = ctx.interface_type(
        expected_element.upcast(),
        &[TypeId::NEVER],
        Nullability::None,
    );
    host.type_system().is_subtype_of(lower_bound, return_type)
}

/// Dart `_isVoidDynamic(type)`.
fn is_void_dynamic(ctx: &Ctx<'_>, t: TypeId) -> bool {
    matches!(
        ctx.ty(t),
        TypeKind::Void | TypeKind::Dynamic | TypeKind::Invalid
    )
}

/// Dart `_isVoidDynamicOrNull(type)`.
fn is_void_dynamic_or_null(ctx: &Ctx<'_>, t: TypeId) -> bool {
    is_void_dynamic(ctx, t) || ctx.is_dart_core_null(t)
}
