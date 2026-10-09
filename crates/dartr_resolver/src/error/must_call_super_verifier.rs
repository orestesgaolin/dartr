// Dart source: pkg/analyzer/lib/src/error/must_call_super_verifier.dart

//! `MustCallSuperVerifier`: an override of a member annotated with
//! `@mustCallSuper` (directly or through an overridden member) must invoke
//! the overridden member (`must_call_super`).

use std::collections::VecDeque;

use dartr_ast::{EmptyFunctionBody, Id, MethodDeclaration};
use dartr_diagnostics::diag;
use dartr_element::{Ctx, EId, ElementId, FragmentFlags, InterfaceElement, Tag, TypeId};
use dartr_typesystem::inheritance_manager3::{GetMemberOptions, InheritanceManager3, Name};
use dartr_typesystem::{TypeExt, lookup};
use indexmap::IndexSet;

use super::support::{enclosing_of, library_of, token_range};
use super::{UnitVerifier, VerifierHost};
use crate::element_metadata::{UnitAst, element_has, flags};

/// Dart `MustCallSuperVerifier.checkMethodDeclaration(node)`.
pub fn check_method_declaration(v: &mut UnitVerifier<'_>, node: Id<MethodDeclaration>) {
    let ast = v.ast;
    let ctx = v.ctx;
    let is_static = ast[node]
        .modifier_keyword
        .is_some_and(|k| ast.tokens.lexeme(k) == "static");
    let is_complete =
        ast[node].external_keyword.is_some() || !ast.is::<EmptyFunctionBody>(ast[node].body);
    if is_static || !is_complete {
        return;
    }
    let Some(&fragment) = v.tables.declared_fragment.get(node.raw()) else {
        return;
    };
    let Some(element) = ctx
        .fragment_data(fragment)
        .and_then(|f| f.element.try_get().copied())
    else {
        return;
    };
    let unit = Some(UnitAst {
        ast: v.ast,
        tables: v.tables,
    });

    let Some(overridden) = find_overridden_member_with_must_call_super(&ctx, element, unit) else {
        return;
    };
    let Some(overridden_name) = ctx.element_name(overridden) else {
        return;
    };
    let overridden_enclosing_name = enclosing_of(&ctx, overridden)
        .and_then(|e| ctx.element_name(e))
        .map(str::to_string);

    if element.tag() == Tag::Method && has_concrete_super_method(&ctx, element) {
        verify_super_is_called(v, node, fragment, overridden_enclosing_name);
        return;
    }

    let Some(enclosing) = enclosing_of(&ctx, element) else {
        return;
    };
    if enclosing.tag() != Tag::Class {
        return;
    }
    let Some(enclosing) = enclosing.cast::<InterfaceElement>() else {
        return;
    };
    let _ = overridden_name;
    match element.tag() {
        Tag::Getter => {
            if lookup_inherited_concrete_member(&ctx, enclosing, element)
                .is_some_and(|e| e.tag() == Tag::Getter)
            {
                verify_super_is_called(v, node, fragment, overridden_enclosing_name);
            }
        }
        Tag::Setter => {
            // Dart passes the setter name without the trailing `=` to the
            // verifier; it is not part of the report.
            if lookup_inherited_concrete_member(&ctx, enclosing, element)
                .is_some_and(|e| e.tag() == Tag::Setter)
            {
                verify_super_is_called(v, node, fragment, overridden_enclosing_name);
            }
        }
        _ => {}
    }
}

/// Dart `_findOverriddenMemberWithMustCallSuper(element)`: a member that
/// [element] overrides (walking up the superclasses, mixins and superclass
/// constraints, not the interfaces) and that is annotated with
/// `@mustCallSuper`.
fn find_overridden_member_with_must_call_super(
    ctx: &Ctx<'_>,
    element: ElementId,
    unit: Option<UnitAst<'_>>,
) -> Option<ElementId> {
    let class_element = enclosing_of(ctx, element)?.cast::<InterfaceElement>()?;
    let name = ctx.element_name(element)?;

    let mut superclasses: VecDeque<EId<InterfaceElement>> = VecDeque::new();
    let add_to_queue = |superclasses: &mut VecDeque<EId<InterfaceElement>>,
                        element: EId<InterfaceElement>| {
        for &mixin in ctx.element_mixins(element) {
            if let Some(m) = ctx.interface_element(mixin) {
                superclasses.push_back(m);
            }
        }
        if let Some(supertype) = ctx.element_supertype(element)
            && let Some(s) = ctx.interface_element(supertype)
        {
            superclasses.push_back(s);
        }
        for &constraint in ctx.element_superclass_constraints(element) {
            if let Some(c) = ctx.interface_element(constraint) {
                superclasses.push_back(c);
            }
        }
    };

    let mut visited: IndexSet<EId<InterfaceElement>> = IndexSet::new();
    add_to_queue(&mut superclasses, class_element);
    while let Some(ancestor) = superclasses.pop_front() {
        if !visited.insert(ancestor) {
            continue;
        }
        let instance = ancestor.upcast();
        let member: Option<ElementId> = match element.tag() {
            Tag::Method => lookup::get_method(ctx, instance, name).map(|m| m.raw()),
            Tag::Getter => lookup::get_method(ctx, instance, name)
                .map(|m| m.raw())
                .or_else(|| lookup::get_getter(ctx, instance, name).map(|g| g.raw())),
            Tag::Setter => lookup::get_setter(ctx, instance, name).map(|s| s.raw()),
            _ => None,
        };
        if let Some(member) = member
            && matches!(member.tag(), Tag::Method | Tag::Getter | Tag::Setter)
            && element_has(ctx, member, flags::MUST_CALL_SUPER, unit)
        {
            return Some(member);
        }
        add_to_queue(&mut superclasses, ancestor);
    }
    None
}

/// Dart `_hasConcreteSuperMethod(element)`.
fn has_concrete_super_method(ctx: &Ctx<'_>, element: ElementId) -> bool {
    let Some(class_element) = enclosing_of(ctx, element).and_then(|e| e.cast::<InterfaceElement>())
    else {
        return true;
    };
    let Some(name) = ctx.element_name(element) else {
        return true;
    };
    if is_concrete(ctx, ctx.element_supertype(class_element), name) {
        return true;
    }
    if ctx
        .element_mixins(class_element)
        .iter()
        .any(|&m| is_concrete(ctx, Some(m), name))
    {
        return true;
    }
    if ctx
        .element_superclass_constraints(class_element)
        .iter()
        .any(|&c| is_concrete(ctx, Some(c), name))
    {
        return true;
    }
    false
}

/// Dart `InterfaceType?.isConcrete(name)`.
fn is_concrete(ctx: &Ctx<'_>, ty: Option<TypeId>, name: &str) -> bool {
    let Some(element) = ty.and_then(|t| ctx.interface_element(t)) else {
        return false;
    };
    let library = library_of(ctx, element.raw());
    let im = InheritanceManager3::new(*ctx);
    im.get_member_with(
        element,
        Name::for_library(ctx, library, name),
        GetMemberOptions {
            concrete: true,
            ..GetMemberOptions::default()
        },
    )
    .is_some()
}

/// Dart `InterfaceElement.lookupInheritedConcreteMember(element)`.
fn lookup_inherited_concrete_member(
    ctx: &Ctx<'_>,
    interface: EId<InterfaceElement>,
    element: ElementId,
) -> Option<ElementId> {
    let name = Name::for_element(ctx, element.into())?;
    let im = InheritanceManager3::new(*ctx);
    im.get_member_with(
        interface,
        name,
        GetMemberOptions {
            for_super: true,
            ..GetMemberOptions::default()
        },
    )
    .map(|e| dartr_typesystem::member::base_element(ctx, e))
}

/// Dart `_verifySuperIsCalled(node, methodName, overriddenEnclosingName)`.
fn verify_super_is_called(
    v: &mut UnitVerifier<'_>,
    node: Id<MethodDeclaration>,
    fragment: dartr_element::FragmentId,
    overridden_enclosing_name: Option<String>,
) {
    let invokes_super_self = v
        .ctx
        .fragment_data(fragment)
        .is_some_and(|f| f.flags.has(FragmentFlags::EXECUTABLE_FRAGMENT_INVOKES_SUPER_SELF));
    if !invokes_super_self {
        let range = token_range(v.ast, v.ast[node].name);
        let class_name = overridden_enclosing_name.unwrap_or_default();
        v.report(diag::must_call_super(&class_name).at_offset(range.0 as usize, range.1 as usize));
    }
}
