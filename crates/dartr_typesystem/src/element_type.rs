// Dart source: pkg/analyzer/lib/src/dart/element/element.dart
// (ExecutableElementImpl.type, GenericFunctionTypeElementImpl.type,
// FormalParameterElementImpl as a function type parameter)

//! The function type of an executable element (`ExecutableElementImpl.type`,
//! cached in its `type_` slot as the Dart `_type` field).

use dartr_element::{
    Ctx, EId, ElemRef, ElementFlags, ExecutableElement, FnParam, FormalParameterElement,
    Nullability, TypeId,
};

use crate::type_ext::TypeExt;

/// The function type parameter for a formal parameter element.
pub fn formal_parameter_as_fn_param(ctx: &Ctx<'_>, param: EId<FormalParameterElement>) -> FnParam {
    let data = ctx.get(param);
    FnParam {
        name: data.name,
        kind: data.kind,
        ty: data.type_.get().unwrap_or(TypeId::INVALID),
        covariant: data
            .flags
            .has(ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT),
        element: Some(ElemRef::Base(param.raw())),
    }
}

/// `ExecutableElementImpl.returnType`: the return type; for a constructor
/// whose return type is not set, `enclosingElement.thisType`
/// (`ConstructorElementImpl.returnType`). Else a type that is not set is
/// `InvalidType`.
pub fn executable_return_type(ctx: &Ctx<'_>, element: EId<ExecutableElement>) -> TypeId {
    if let Some(t) = ctx.executable(element).return_type.get() {
        return t;
    }
    if element.raw().tag() == dartr_element::Tag::Constructor {
        if let Some(interface) = ctx
            .element_data(element.raw())
            .and_then(|d| d.enclosing)
            .and_then(|e| e.cast::<dartr_element::InterfaceElement>())
        {
            return ctx.interface_this_type(interface);
        }
    }
    TypeId::INVALID
}

/// `ExecutableElementImpl.type`.
pub fn executable_type(ctx: &Ctx<'_>, element: EId<ExecutableElement>) -> TypeId {
    let data = ctx.executable(element);
    if let Some(t) = data.type_.get() {
        return t;
    }
    let params: Vec<FnParam> = data
        .formal_params
        .iter()
        .map(|&p| formal_parameter_as_fn_param(ctx, p))
        .collect();
    let ret = executable_return_type(ctx, element);
    let t = ctx.function_type(&data.type_params, &params, ret, Nullability::None, None);
    data.type_.set(Some(t));
    t
}
