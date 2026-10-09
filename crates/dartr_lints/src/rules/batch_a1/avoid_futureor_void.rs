// Dart source: pkg/linter/lib/src/rules/avoid_futureor_void.dart

use super::helpers::{base_element, is_void, node_type};
use crate::{LinterContext, RuleVisitorRegistry};
use dartr_ast::*;
use dartr_diagnostics::{Diagnostic, diag};
use dartr_element::{AnyElement, TypeKind, Variance as ElementVariance};
use dartr_typesystem::TypeExt;

pub fn register(registry: &mut RuleVisitorRegistry, _: &LinterContext<'_>) {
    for kind in [
        NodeKind::AsExpression,
        NodeKind::CastPattern,
        NodeKind::ExtendsClause,
        NodeKind::ExtensionOnClause,
        NodeKind::FunctionDeclaration,
        NodeKind::ImplementsClause,
        NodeKind::IsExpression,
        NodeKind::MethodDeclaration,
        NodeKind::MixinOnClause,
        NodeKind::ObjectPattern,
        NodeKind::PrimaryConstructorDeclaration,
        NodeKind::TypeParameter,
        NodeKind::VariableDeclarationList,
        NodeKind::WithClause,
    ] {
        registry.add(kind, "avoid_futureor_void", check);
    }
}

#[derive(Clone, Copy)]
enum Variance {
    Out,
    In,
    InOut,
}

impl Variance {
    fn inverse(self) -> Self {
        match self {
            Self::Out => Self::In,
            Self::In => Self::Out,
            Self::InOut => Self::InOut,
        }
    }

    fn compose(self, parameter: Option<ElementVariance>) -> Self {
        match parameter {
            None | Some(ElementVariance::Unrelated | ElementVariance::Covariant) => self,
            Some(ElementVariance::Contravariant) => self.inverse(),
            Some(ElementVariance::Invariant) => Self::InOut,
        }
    }
}

fn type_parameter_variances(
    c: &LinterContext<'_>,
    node: Id<NamedType>,
) -> Vec<Option<ElementVariance>> {
    let Some(r) = c.resolved else {
        return Vec::new();
    };
    let Some(element) = c.element(node).and_then(|e| base_element(c, e)) else {
        return Vec::new();
    };
    let parameters = match r.ctx.any(element) {
        AnyElement::Class(e) => &e.type_params,
        AnyElement::Enum(e) => &e.type_params,
        AnyElement::Mixin(e) => &e.type_params,
        AnyElement::ExtensionType(e) => &e.type_params,
        AnyElement::TypeAlias(e) => &e.type_params,
        _ => return Vec::new(),
    };
    parameters
        .iter()
        .map(|parameter| r.ctx.get(*parameter).variance)
        .collect()
}

fn check_type(
    c: &LinterContext<'_>,
    node: Id<TypeAnnotation>,
    variance: Variance,
    out: &mut Vec<Diagnostic>,
) {
    match c.ast.kind(node) {
        NodeKind::NamedType => {
            let node = Id::<NamedType>::from_raw(node.raw());
            if let Some(arguments) = c.ast[node].type_arguments {
                let arguments = c.ast.list(c.ast[arguments].arguments);
                let parameter_variances = type_parameter_variances(c, node);
                for (index, argument) in arguments.iter().enumerate() {
                    let parameter_variance = if parameter_variances.len() == arguments.len() {
                        parameter_variances[index]
                    } else {
                        None
                    };
                    check_type(c, *argument, variance.compose(parameter_variance), out);
                }
            }

            if matches!(variance, Variance::In) {
                return;
            }
            let Some(r) = c.resolved else { return };
            let Some(ty) = node_type(c, node) else { return };
            if r.ctx.is_dart_async_future_or(ty)
                && let TypeKind::Interface { args, .. } = *r.ctx.ty(ty)
                && r.ctx.list(args).first().is_some_and(|&arg| is_void(c, arg))
            {
                c.report_node(out, &diag::AVOID_FUTUREOR_VOID, node, &[]);
            }
        }
        NodeKind::GenericFunctionType => {
            let node = &c.ast[Id::<GenericFunctionType>::from_raw(node.raw())];
            if let Some(return_type) = node.return_type {
                check_type(c, return_type, variance, out);
            }
            check_type_parameters(c, node.type_parameters, out);
            check_parameters(c, node.parameters, variance.inverse(), out);
        }
        NodeKind::RecordTypeAnnotation => {
            let node = &c.ast[Id::<RecordTypeAnnotation>::from_raw(node.raw())];
            for field in c.ast.list(node.positional_fields) {
                check_type(c, c.ast[*field].type_, variance, out);
            }
            if let Some(fields) = node.named_fields {
                for field in c.ast.list(c.ast[fields].fields) {
                    check_type(c, c.ast[*field].type_, variance, out);
                }
            }
        }
        _ => {}
    }
}

fn check_type_parameters(
    c: &LinterContext<'_>,
    parameters: Option<Id<TypeParameterList>>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(parameters) = parameters else { return };
    for parameter in c.ast.list(c.ast[parameters].type_parameters) {
        if let Some(bound) = c.ast[*parameter].bound {
            check_type(c, bound, Variance::InOut, out);
        }
    }
}

fn check_parameter(
    c: &LinterContext<'_>,
    parameter: Id<FormalParameter>,
    variance: Variance,
    out: &mut Vec<Diagnostic>,
) {
    let (type_, suffix) = match c.ast.kind(parameter) {
        NodeKind::RegularFormalParameter => {
            let parameter = &c.ast[Id::<RegularFormalParameter>::from_raw(parameter.raw())];
            (parameter.type_, parameter.function_typed_suffix)
        }
        NodeKind::FieldFormalParameter => {
            let parameter = &c.ast[Id::<FieldFormalParameter>::from_raw(parameter.raw())];
            (parameter.type_, parameter.function_typed_suffix)
        }
        NodeKind::SuperFormalParameter => {
            let parameter = &c.ast[Id::<SuperFormalParameter>::from_raw(parameter.raw())];
            (parameter.type_, parameter.function_typed_suffix)
        }
        _ => return,
    };
    if let Some(type_) = type_ {
        check_type(c, type_, variance, out);
    }
    if let Some(suffix) = suffix {
        let suffix = &c.ast[suffix];
        check_type_parameters(c, suffix.type_parameters, out);
        check_parameters(c, suffix.formal_parameters, variance.inverse(), out);
    }
}

fn check_parameters(
    c: &LinterContext<'_>,
    parameters: Id<FormalParameterList>,
    variance: Variance,
    out: &mut Vec<Diagnostic>,
) {
    for parameter in c.ast.list(c.ast[parameters].parameters) {
        check_parameter(c, *parameter, variance, out);
    }
}

fn check(c: &LinterContext<'_>, node: NodeId, out: &mut Vec<Diagnostic>) {
    match c.ast.kind(node) {
        NodeKind::AsExpression => check_type(
            c,
            c.ast[Id::<AsExpression>::from_raw(node)].type_,
            Variance::Out,
            out,
        ),
        NodeKind::CastPattern => check_type(
            c,
            c.ast[Id::<CastPattern>::from_raw(node)].type_,
            Variance::Out,
            out,
        ),
        NodeKind::ExtendsClause => check_type(
            c,
            c.ast[Id::<ExtendsClause>::from_raw(node)]
                .superclass
                .upcast(),
            Variance::Out,
            out,
        ),
        NodeKind::ExtensionOnClause => check_type(
            c,
            c.ast[Id::<ExtensionOnClause>::from_raw(node)].extended_type,
            Variance::Out,
            out,
        ),
        NodeKind::FunctionDeclaration => {
            let declaration = &c.ast[Id::<FunctionDeclaration>::from_raw(node)];
            if let Some(return_type) = declaration.return_type {
                check_type(c, return_type, Variance::Out, out);
            }
            let function = &c.ast[declaration.function_expression];
            check_type_parameters(c, function.type_parameters, out);
            if let Some(parameters) = function.parameters {
                check_parameters(c, parameters, Variance::In, out);
            }
        }
        NodeKind::ImplementsClause => {
            let clause = &c.ast[Id::<ImplementsClause>::from_raw(node)];
            for type_ in c.ast.list(clause.interfaces) {
                check_type(c, type_.upcast(), Variance::Out, out);
            }
        }
        NodeKind::IsExpression => check_type(
            c,
            c.ast[Id::<IsExpression>::from_raw(node)].type_,
            Variance::Out,
            out,
        ),
        NodeKind::MethodDeclaration => {
            let declaration = &c.ast[Id::<MethodDeclaration>::from_raw(node)];
            if let Some(return_type) = declaration.return_type {
                check_type(c, return_type, Variance::Out, out);
            }
            check_type_parameters(c, declaration.type_parameters, out);
            if let Some(parameters) = declaration.parameters {
                check_parameters(c, parameters, Variance::In, out);
            }
        }
        NodeKind::MixinOnClause => {
            let clause = &c.ast[Id::<MixinOnClause>::from_raw(node)];
            for type_ in c.ast.list(clause.superclass_constraints) {
                check_type(c, type_.upcast(), Variance::Out, out);
            }
        }
        NodeKind::ObjectPattern => check_type(
            c,
            c.ast[Id::<ObjectPattern>::from_raw(node)].type_.upcast(),
            Variance::Out,
            out,
        ),
        NodeKind::PrimaryConstructorDeclaration => {
            let declaration = &c.ast[Id::<PrimaryConstructorDeclaration>::from_raw(node)];
            for parameter in c.ast.list(c.ast[declaration.formal_parameters].parameters) {
                if c.ast.kind(*parameter) == NodeKind::RegularFormalParameter {
                    check_parameter(c, *parameter, Variance::Out, out);
                }
            }
        }
        NodeKind::TypeParameter => {
            if let Some(bound) = c.ast[Id::<TypeParameter>::from_raw(node)].bound {
                check_type(c, bound, Variance::InOut, out);
            }
        }
        NodeKind::VariableDeclarationList => {
            if let Some(type_) = c.ast[Id::<VariableDeclarationList>::from_raw(node)].type_ {
                check_type(c, type_, Variance::Out, out);
            }
        }
        NodeKind::WithClause => {
            let clause = &c.ast[Id::<WithClause>::from_raw(node)];
            for type_ in c.ast.list(clause.mixin_types) {
                check_type(c, type_.upcast(), Variance::Out, out);
            }
        }
        _ => {}
    }
}
