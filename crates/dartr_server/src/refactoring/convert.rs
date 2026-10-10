// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/convert_getter_to_method.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/convert_method_to_getter.dart

//! The refactorings that convert a getter to a method and a method (or a
//! top-level function) to a getter.

use dartr_ast::*;
use dartr_element::{FragmentFlags, NoopSink, Tag};
use dartr_typesystem::{TypeExt, member};

use super::rename::{ChangeAcc, Engine, Target, element_path, source_references};
use super::status::RefactoringStatus;
use crate::correction::change::SourceChange;
use crate::index::RelationKind;
use crate::server::SElem;

/// Whether the getter [e] is explicit (Dart `isOriginDeclaration`).
fn is_explicit_accessor(e: &SElem) -> bool {
    e.with(|ctx| {
        !dartr_resolver::element_ext::first_fragment_flags(ctx, e.id)
            .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
    })
}

impl Engine<'_> {
    /// Dart `ConvertGetterToMethodRefactoringImpl._checkElement`.
    pub fn getter_to_method_initial(&self, getter: &SElem) -> RefactoringStatus {
        if !self.workspace_contains(&Target::Element(getter.clone())) {
            return RefactoringStatus::fatal("Only getters in your workspace can be converted.");
        }
        if !is_explicit_accessor(getter) {
            return RefactoringStatus::fatal("Only explicit getters can be converted to methods.");
        }
        RefactoringStatus::new()
    }

    /// Dart `ConvertGetterToMethodRefactoringImpl._getElements`.
    fn getter_elements(&mut self, getter: &SElem) -> Vec<SElem> {
        let mut out = Vec::new();
        let (top_level, field) = getter.with(|ctx| {
            let enclosing = ctx.element_data(getter.id).and_then(|d| d.enclosing);
            let field = dartr_resolver::element_metadata::accessor_variable_any(ctx, getter.id)
                .filter(|f| f.tag() == Tag::Field);
            (enclosing.is_some_and(|e| e.tag() == Tag::Library), field)
        });
        if top_level {
            out.push(getter.clone());
        }
        if let Some(field) = field {
            let (members, _) = self.hierarchy_members_and_parameters(&getter.same(field));
            for m in members {
                if m.id.tag() != Tag::Field {
                    continue;
                }
                let g = m.with(|ctx| match ctx.any(m.id) {
                    dartr_element::AnyElement::Field(f) => f.getter.map(|g| g.raw()),
                    _ => None,
                });
                if let Some(g) = g {
                    let g = m.same(g);
                    if is_explicit_accessor(&g) {
                        out.push(g);
                    }
                }
            }
        }
        out
    }

    /// Dart `ConvertGetterToMethodRefactoringImpl.checkFinalConditions`.
    pub fn getter_to_method_final(&mut self, getter: &SElem) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        for e in self.getter_elements(getter) {
            let matches = self.search_references(&e);
            for reference in source_references(&matches) {
                if reference.kind == RelationKind::IsReferencedByPatternField {
                    result.add_warning(
                        "Will match the method tear-off instead of the result.",
                        Some(super::status::Location {
                            file: reference.path.clone(),
                            offset: reference.offset,
                            length: reference.length,
                        }),
                    );
                }
            }
        }
        result
    }

    /// Dart `ConvertGetterToMethodRefactoringImpl.createChange`.
    pub fn getter_to_method_change(&mut self, getter: &SElem) -> SourceChange {
        let mut acc = ChangeAcc::default();
        for e in self.getter_elements(getter) {
            // `_updateElementDeclaration`.
            for (path, name_offset, name_length) in self.fragment_names(&e) {
                let Some(declaration) = self.declaration_node(&path, e.lib.context, name_offset)
                else {
                    break;
                };
                let resolved = self
                    .require_resolved_unit_in(&path, Some(e.lib.context))
                    .ok();
                let Some(resolved) = resolved else { break };
                let ast = &resolved.unit().ast;
                let keyword = if let Some(m) = ast.cast::<MethodDeclaration>(declaration) {
                    ast[m].property_keyword
                } else if let Some(f) = ast.cast::<FunctionDeclaration>(declaration) {
                    ast[f].property_keyword
                } else {
                    break;
                };
                if let Some(k) = keyword {
                    let start = ast.tokens.get(k).offset;
                    acc.add(&path, start, name_offset - start, "");
                }
                acc.add(&path, name_offset + name_length, 0, "()");
            }
            // `_updateElementReferences`.
            let matches = self.search_references(&e);
            for reference in source_references(&matches) {
                if reference.kind == RelationKind::IsReferencedByPatternField {
                    continue;
                }
                acc.add(
                    &reference.path,
                    reference.offset + reference.length,
                    0,
                    "()",
                );
            }
        }
        acc.into_change("Convert Getter To Method".to_string())
    }

    /// Dart `ConvertMethodToGetterRefactoringImpl._checkElement`.
    pub fn method_to_getter_initial(&self, element: &SElem) -> RefactoringStatus {
        if !self.workspace_contains(&Target::Element(element.clone())) {
            return RefactoringStatus::fatal("Only methods in your workspace can be converted.");
        }
        if !matches!(element.id.tag(), Tag::Method | Tag::TopLevelFunction) {
            return RefactoringStatus::fatal(
                "Only methods or top-level functions can be converted to getters.",
            );
        }
        let (void, has_parameters) = element.with(|ctx| {
            let x = element.id.cast::<dartr_element::ExecutableElement>();
            let returns = member::return_type(ctx, dartr_element::ElemRef::Base(element.id));
            (
                matches!(ctx.ty(returns), dartr_element::TypeKind::Void),
                x.is_some_and(|x| !ctx.executable(x).formal_params.is_empty()),
            )
        });
        if void {
            return RefactoringStatus::fatal(format!(
                "Cannot convert {} returning void.",
                element.id.kind().display_name()
            ));
        }
        if has_parameters {
            return RefactoringStatus::fatal(
                "Only methods without parameters can be converted to getters.",
            );
        }
        RefactoringStatus::new()
    }

    /// Dart `ConvertMethodToGetterRefactoringImpl.createChange`.
    pub fn method_to_getter_change(&mut self, element: &SElem) -> SourceChange {
        let mut acc = ChangeAcc::default();
        let elements: Vec<SElem> = match element.id.tag() {
            Tag::TopLevelFunction => vec![element.clone()],
            Tag::Method => {
                let (members, _) = self.hierarchy_members_and_parameters(element);
                members
            }
            _ => Vec::new(),
        };
        for e in elements {
            // `_updateElementDeclaration`.
            for (path, name_offset, _) in self.fragment_names(&e) {
                let Some(declaration) = self.declaration_node(&path, e.lib.context, name_offset)
                else {
                    break;
                };
                let Some(resolved) = self
                    .require_resolved_unit_in(&path, Some(e.lib.context))
                    .ok()
                else {
                    break;
                };
                let ast = &resolved.unit().ast;
                let parameters = if let Some(m) = ast.cast::<MethodDeclaration>(declaration) {
                    ast[m].parameters.map(|p| p.raw())
                } else if let Some(f) = ast.cast::<FunctionDeclaration>(declaration) {
                    ast[ast[f].function_expression].parameters.map(|p| p.raw())
                } else {
                    break;
                };
                let Some(parameters) = parameters else { break };
                acc.add(&path, name_offset, 0, "get ");
                acc.add(&path, ast.offset(parameters), ast.length(parameters), "");
            }
            // `_updateElementReferences`.
            let matches = self.search_references(&e);
            for reference in source_references(&matches) {
                let Some(ref_element) = self.match_element(&reference) else {
                    continue;
                };
                let Some(path) = ref_element.with(|ctx| element_path(ctx, ref_element.id)) else {
                    continue;
                };
                let Some(resolved) = self
                    .require_resolved_unit_in(&path, Some(reference.context))
                    .ok()
                else {
                    continue;
                };
                let unit = resolved.unit();
                let ast = &unit.ast;
                let mut node = ast.node_covering(unit.unit, reference.offset, 0);
                while let Some(n) = node {
                    if let Some(i) = ast.cast::<MethodInvocation>(n) {
                        let end = reference.offset + reference.length;
                        acc.add(&reference.path, end, ast.end(i) - end, "");
                        break;
                    }
                    node = ast.parent(n);
                }
            }
        }
        acc.into_change("Convert Method To Getter".to_string())
    }

    /// The name of each fragment of [e]: (path, offset, length).
    fn fragment_names(&self, e: &SElem) -> Vec<(String, u32, u32)> {
        e.with(|ctx| {
            let mut out = Vec::new();
            let mut f = ctx.element_data(e.id).map(|d| d.first_fragment);
            while let Some(id) = f {
                let Some(data) = ctx.fragment_data(id) else {
                    break;
                };
                let path = crate::navigation::fragment_path(ctx, id);
                let length = data
                    .name
                    .map(|n| crate::correction::change::utf16_len(ctx.name_str(n)));
                match (path, data.name_offset, length) {
                    (Some(p), Some(o), Some(l)) => out.push((p, o, l)),
                    _ => break,
                }
                f = data.next_fragment;
            }
            out
        })
    }

    /// The declaration (method, function, ...) whose name is at [offset].
    fn declaration_node(&mut self, path: &str, context: usize, offset: u32) -> Option<NodeId> {
        let resolved = self.require_resolved_unit_in(path, Some(context)).ok()?;
        let unit = resolved.unit();
        let ast = &unit.ast;
        let mut node = ast.node_covering(unit.unit, offset, 0);
        while let Some(n) = node {
            if ast.is::<MethodDeclaration>(n) || ast.is::<FunctionDeclaration>(n) {
                return Some(n);
            }
            node = ast.parent(n);
        }
        None
    }

    /// Dart `nodeCovering(offset)?.getElement()` at [offset] of [resolved].
    pub fn element_at(
        &mut self,
        resolved: &crate::server::ResolvedUnitRef,
        offset: u32,
    ) -> Option<SElem> {
        element_at_in_unit(resolved, offset)
    }
}

pub fn element_at_in_unit(resolved: &crate::server::ResolvedUnitRef, offset: u32) -> Option<SElem> {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let u = crate::element_locator::Unit {
        ctx: &ctx,
        ast: &unit.ast,
        tables: &unit.tables,
    };
    let node = unit.ast.node_covering(unit.unit, offset, 0)?;
    let e = crate::element_locator::get_element(&u, node)?;
    Some(SElem {
        lib: resolved.library.clone(),
        unit: resolved.index,
        id: e,
    })
}

pub fn is_method_to_getter_available_in_unit(
    resolved: &crate::server::ResolvedUnitRef,
    offset: u32,
) -> bool {
    let Some(element) = element_at_in_unit(resolved, offset) else {
        return false;
    };
    if !matches!(element.id.tag(), Tag::Method | Tag::TopLevelFunction) {
        return false;
    }
    element.with(|ctx| {
        if dartr_resolver::error::support::library_of(ctx, element.id)
            .is_some_and(|l| ctx.library_uri(l).starts_with("dart:"))
        {
            return false;
        }
        let x = element.id.cast::<dartr_element::ExecutableElement>();
        let returns = member::return_type(ctx, dartr_element::ElemRef::Base(element.id));
        !matches!(ctx.ty(returns), dartr_element::TypeKind::Void)
            && x.is_some_and(|x| ctx.executable(x).formal_params.is_empty())
    })
}
