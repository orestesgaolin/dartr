// Dart source: pkg/analyzer/lib/src/dart/resolver/variable_declaration_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (setVariableType,
// variableTypeFromInitializerType)

//! `VariableDeclarationResolver`: the initializer of a variable declaration
//! and the inferred type of an implicitly typed local variable.

use dartr_ast::{Id, VariableDeclaration, VariableDeclarationList};
use dartr_element::{
    EId, ElementId, FieldElement, LocalVariableElement, PromotableElement, PropertyInducingElement,
    TypeId, ElementFlags,
};
use dartr_flow::flow_analysis::FlowAnalysis;
use dartr_typesystem::TypeExt;

use crate::element_ext;
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.setVariableType`.
pub fn set_variable_type(rv: &mut ResolverVisitor<'_>, variable: EId<PromotableElement>, ty: TypeId) {
    if let Some(local) = variable.raw().cast::<LocalVariableElement>() {
        element_ext::set_local_variable_type(&rv.ctx, local, ty);
    } else {
        unimplemented!("TODO(paulberry)");
    }
}

/// Dart `ResolverVisitor.variableTypeFromInitializerType`.
pub fn variable_type_from_initializer_type(rv: &mut ResolverVisitor<'_>, ty: TypeId) -> TypeId {
    if rv.ctx.is_dart_core_null(ty) {
        return TypeId::DYNAMIC;
    }
    rv.type_system.demote_type(ty)
}

/// The element of the declaration [node] (Dart
/// `node.declaredFragment!.element`).
pub fn declared_element(rv: &ResolverVisitor<'_>, node: Id<VariableDeclaration>) -> Option<ElementId> {
    let fragment = *rv.tables.declared_fragment.get(node)?;
    let data = rv.ctx.fragment_data(fragment)?;
    data.element.try_get().copied()
}

/// Dart `VariableDeclarationResolver.resolve(node)`.
pub fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<VariableDeclaration>) {
    let parent: Id<VariableDeclarationList> = rv
        .ast
        .cast(rv.ast.parent(node).expect("parent"))
        .expect("VariableDeclarationList");
    let parent_type = rv.ast[parent].type_;

    let Some(initializer) = rv.ast[node].initializer else {
        if rv.unit.options.strict_inference && parent_type.is_none() {
            let name = rv.ast[node].name;
            let lexeme = rv.lexeme(name).to_string();
            let d = dartr_diagnostics::diag::inference_failure_on_uninitialized_variable(&lexeme)
                .at_offset(rv.ast.offset(node) as usize, rv.ast.length(node) as usize);
            rv.report(d);
        }
        return;
    };

    let Some(element) = declared_element(rv, node) else {
        // Not bound (the binding pass is not ported for this declaration):
        // resolve the initializer without a variable.
        if rv.flow_analysis.is_active() {
            rv.resolve_expression(initializer, TypeId::UNKNOWN);
        }
        return;
    };
    let is_top_level = element.is::<FieldElement>() || element.tag() == dartr_element::Tag::TopLevelVariable;
    let is_late = element_ext::is_late(&rv.ctx, element);

    // Dart `inScopePrimaryConstructorParameters` (primary constructors, an
    // experiment): not ported.
    if is_top_level {
        let ast = &*rv.ast;
        rv.flow_analysis
            .body_or_initializer_enter(ast, rv.tables, node.raw(), None, None);
    } else if is_late {
        if let Some(flow) = rv.flow_analysis.flow.as_mut() {
            flow.late_initializer_begin(node.raw());
        }
    }

    let context_type = match element.cast::<PropertyInducingElement>() {
        Some(p)
            if rv
                .ctx
                .element_data(p.raw())
                .is_some_and(|d| d.flags.has(ElementFlags::PROPERTY_INDUCING_ELEMENT_IS_TYPE_INFERRED_FROM_INITIALIZER)) =>
        {
            TypeId::UNKNOWN
        }
        _ => element_ext::variable_type(&rv.ctx, element),
    };
    let initializer = rv.resolve_expression(initializer, context_type);

    let initializer_type = rv.type_or_throw(initializer);
    if parent_type.is_none()
        && let Some(local) = element.cast::<LocalVariableElement>()
    {
        let ty = variable_type_from_initializer_type(rv, initializer_type);
        element_ext::set_local_variable_type(&rv.ctx, local, ty);
    }

    if is_top_level {
        rv.flow_analysis.body_or_initializer_exit();
        crate::error::dead_code_verifier::flow_end(rv, node);
    } else if is_late {
        if let Some(flow) = rv.flow_analysis.flow.as_mut() {
            flow.late_initializer_end();
        }
    }

    // Initializers of top-level variables and fields are already included
    // into elements during linking. Dart `fragment.constantInitializer =
    // initializer` for local constants: the constant evaluation of local
    // constants reads the initializer node from the tables (wave D).

    // Dart `checkForAssignableExpressionAtType(initializer, initializerType,
    // element.type, ...)`: wave D.
}
