// Dart source: pkg/linter/lib/src/util/variance_checker.dart
//! Dart `VarianceChecker`: walks a type annotation with the variance of
//! each position and calls a hook on each named type.

use super::util::*;
use crate::LinterContext;
use dartr_ast::*;
use dartr_element::{EId, InterfaceElement, Tag, TypeAliasElement, TypeId, Variance};
use dartr_typesystem::TypeExt;

/// Dart `Variance.inverse`.
fn inverse(variance: Variance) -> Variance {
    match variance {
        Variance::Covariant => Variance::Contravariant,
        Variance::Contravariant => Variance::Covariant,
        other => other,
    }
}

/// Dart `VarianceChecker` with `checkNamedType` as [check_named_type].
pub struct VarianceChecker<'c, 'a, F: FnMut(Variance, TypeId, NodeId)> {
    pub c: &'c LinterContext<'a>,
    pub check_named_type: F,
}

impl<F: FnMut(Variance, TypeId, NodeId)> VarianceChecker<'_, '_, F> {
    /// Dart `check`.
    pub fn check(&mut self, variance: Variance, type_annotation: Option<NodeId>) {
        let c = self.c;
        let Some(type_annotation) = type_annotation else {
            return;
        };
        if c.is_synthetic(type_annotation) {
            return;
        }
        match kind(c, type_annotation) {
            NodeKind::NamedType => {
                let n = &c.ast[Id::<NamedType>::from_raw(type_annotation)];
                if let Some(type_arguments) = n.type_arguments {
                    let arguments = c.ast.list_raw(c.ast[type_arguments].arguments).to_vec();
                    let type_parameters =
                        c.element(type_annotation)
                            .map(|e| base(c, e))
                            .and_then(|element| {
                                let ctx = rctx(c)?;
                                match element.tag() {
                                    Tag::Class | Tag::Mixin | Tag::Enum => Some(
                                        ctx.interface_type_parameters(
                                            EId::<InterfaceElement>::from_raw(element),
                                        )
                                        .to_vec(),
                                    ),
                                    Tag::TypeAlias => Some(
                                        ctx.get(EId::<TypeAliasElement>::from_raw(element))
                                            .type_params
                                            .clone(),
                                    ),
                                    _ => None,
                                }
                            });
                    match type_parameters {
                        Some(parameters) if parameters.len() == arguments.len() => {
                            let ctx = rctx(c).unwrap();
                            for (&parameter, &argument) in parameters.iter().zip(&arguments) {
                                let parameter_variance = ctx.type_parameter_variance(parameter);
                                let variance = if ctx.type_parameter_is_legacy_covariant(parameter)
                                    || parameter_variance == Variance::Covariant
                                {
                                    variance
                                } else if parameter_variance == Variance::Contravariant {
                                    inverse(variance)
                                } else {
                                    Variance::Invariant
                                };
                                self.check(variance, Some(argument));
                            }
                        }
                        _ => {
                            for argument in arguments {
                                self.check(variance, Some(argument));
                            }
                        }
                    }
                }
                let Some(static_type) = annotation_type(c, type_annotation) else {
                    return;
                };
                (self.check_named_type)(variance, static_type, type_annotation);
            }
            NodeKind::GenericFunctionType => {
                let n = &c.ast[Id::<GenericFunctionType>::from_raw(type_annotation)];
                self.check(variance, n.return_type.map(|t| t.raw()));
                if let Some(list) = n.type_parameters {
                    for &parameter in c.ast.list(c.ast[list].type_parameters) {
                        self.check_bound(parameter);
                    }
                }
                for parameter in parameters(c, Some(n.parameters)) {
                    self.check_formal_parameter(inverse(variance), parameter);
                }
            }
            NodeKind::RecordTypeAnnotation => {
                let n = &c.ast[Id::<RecordTypeAnnotation>::from_raw(type_annotation)];
                for &field in c.ast.list(n.positional_fields) {
                    self.check(variance, Some(c.ast[field].type_.raw()));
                }
                if let Some(named) = n.named_fields {
                    for &field in c.ast.list(c.ast[named].fields) {
                        self.check(variance, Some(c.ast[field].type_.raw()));
                    }
                }
            }
            _ => {}
        }
    }

    /// Dart `checkBound`.
    pub fn check_bound(&mut self, type_parameter: Id<TypeParameter>) {
        let bound = self.c.ast[type_parameter].bound.map(|b| b.raw());
        self.check(Variance::Invariant, bound);
    }

    /// Dart `checkFormalParameter`.
    pub fn check_formal_parameter(&mut self, variance: Variance, formal_parameter: NodeId) {
        let c = self.c;
        let (type_, suffix) = match kind(c, formal_parameter) {
            NodeKind::RegularFormalParameter => {
                let p = &c.ast[Id::<RegularFormalParameter>::from_raw(formal_parameter)];
                (p.type_, p.function_typed_suffix)
            }
            NodeKind::FieldFormalParameter => {
                let p = &c.ast[Id::<FieldFormalParameter>::from_raw(formal_parameter)];
                (p.type_, p.function_typed_suffix)
            }
            NodeKind::SuperFormalParameter => {
                let p = &c.ast[Id::<SuperFormalParameter>::from_raw(formal_parameter)];
                (p.type_, p.function_typed_suffix)
            }
            _ => return,
        };
        // Dart `isExplicitlyTyped`.
        if type_.is_none() && suffix.is_none() {
            return;
        }
        self.check(variance, type_.map(|t| t.raw()));
        // Dart matches `SuperFormalParameter` and `FieldFormalParameter`
        // before the function-typed case.
        if kind(c, formal_parameter) != NodeKind::RegularFormalParameter {
            return;
        }
        if let Some(suffix) = suffix {
            let s = &c.ast[suffix];
            if let Some(list) = s.type_parameters {
                for &parameter in c.ast.list(c.ast[list].type_parameters) {
                    self.check_bound(parameter);
                }
            }
            for parameter in parameters(c, Some(s.formal_parameters)) {
                self.check_formal_parameter(inverse(variance), parameter);
            }
        }
    }

    /// Dart `checkOut`.
    pub fn check_out(&mut self, type_annotation: Option<NodeId>) {
        self.check(Variance::Covariant, type_annotation);
    }
}
