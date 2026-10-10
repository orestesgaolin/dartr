// Dart source: pkg/analysis_server/lib/src/lsp/completion_utils.dart (createOverrideSuggestionData, createTypedSuggestionData)
// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (writeOverride)

//! The code of override and typed suggestions (Dart
//! `createOverrideSuggestionData`, `createTypedSuggestionData`).

#[allow(unused_imports)]
use dartr_typesystem::TypeExt;
use dartr_element::{Ctx, ElemRef, Tag, TypeId, TypeKind};
use dartr_typesystem::member;

use super::Request;
use super::candidate::{Candidate, Kind, TypeImportData, utf16_len};
use super::elem;
use super::lsp::type_display;

/// A writer that records the selection (Dart `DartEditBuilder`).
struct Writer {
    text: String,
    display: Option<String>,
    selection: Option<(usize, usize)>,
}

impl Writer {
    fn write(&mut self, s: &str) {
        self.text.push_str(s);
        if let Some(d) = &mut self.display {
            d.push_str(s);
        }
    }

    fn writeln(&mut self, s: &str) {
        self.text.push_str(s);
        self.text.push('\n');
    }

    fn select_all(&mut self, s: &str) {
        let start = utf16_len(&self.text);
        self.text.push_str(s);
        self.selection = Some((start, utf16_len(s)));
    }

    fn select_here(&mut self) {
        self.selection = Some((utf16_len(&self.text), 0));
    }
}

/// Dart `_writeTypeIfCan`: writes [ty] unless it is `dynamic` (and
/// [write_dynamic] is false).
fn write_type(w: &mut Writer, ctx: &Ctx<'_>, ty: TypeId, write_dynamic: bool) -> bool {
    if matches!(ctx.ty(ty), TypeKind::Dynamic) && !write_dynamic {
        return false;
    }
    if matches!(ctx.ty(ty), TypeKind::Invalid) {
        return false;
    }
    w.write(&type_display(ctx, ty));
    true
}

/// Dart `writeOverride` (with `invokeSuper` and `setSelection`).
fn write_override(ctx: &Ctx<'_>, element: ElemRef, invoke_super: bool) -> Option<(String, String, Option<(usize, usize)>)> {
    let base = member::base_element(ctx, element);
    let member_name = ctx.element_name(base)?.to_string();
    if member_name.is_empty() {
        return None;
    }
    let is_getter = base.tag() == Tag::Getter;
    let is_setter = base.tag() == Tag::Setter;
    let is_operator = elem::is_operator(ctx, base);
    let prefix = "  ";
    let prefix2 = "    ";
    let mut w = Writer {
        text: String::new(),
        display: None,
        selection: None,
    };
    w.writeln("@override");
    w.write(prefix);
    if is_getter {
        w.writeln(&format!("// TODO: implement {member_name}"));
        w.write(prefix);
    }
    let return_type = member::return_type(ctx, element);
    if !is_setter && write_type(&mut w, ctx, return_type, false) {
        w.write(" ");
    }
    if is_getter {
        w.write("get ");
    } else if is_setter {
        w.write("set ");
    } else if is_operator {
        w.write("operator ");
    }
    let mut display = String::new();
    w.display = Some(String::new());
    w.write(&member_name);
    display.push_str(&w.display.take().unwrap());
    if is_getter {
        w.write(" => ");
        if invoke_super {
            w.select_all(&format!("super.{member_name}"));
            w.writeln(";");
        } else {
            w.select_all("throw UnimplementedError()");
            w.write(";");
        }
        display.push_str(" => …");
        return Some((w.text, display, w.selection));
    }
    let parameters = member::formal_parameters(ctx, element);
    w.display = Some(String::new());
    // Dart `writeTypeParameters(element.type.typeParameters)`.
    let type_params = member::type_parameters(ctx, element);
    if !type_params.is_empty() {
        w.write("<");
        for (i, p) in type_params.iter().enumerate() {
            if i > 0 {
                w.write(", ");
            }
            w.write(ctx.element_name(p.raw()).unwrap_or(""));
            if let Some(bound) = ctx.get(*p).bound.get() {
                w.write(" extends ");
                write_type(&mut w, ctx, bound, true);
            }
        }
        w.write(">");
    }
    // Dart `writeFormalParameters`.
    let mut names: Vec<String> = parameters
        .iter()
        .filter_map(|p| ctx.element_name(member::base_element(ctx, *p)).map(str::to_string))
        .collect();
    w.write("(");
    let mut saw_named = false;
    let mut saw_positional = false;
    let mut parameter_names = Vec::new();
    for (i, p) in parameters.iter().enumerate() {
        if i > 0 {
            w.write(", ");
        }
        let kind = elem::parameter_kind(ctx, *p);
        if kind.is_named() {
            if !saw_named {
                w.write("{");
                saw_named = true;
            }
        } else if kind.is_optional_positional() && !saw_positional {
            w.write("[");
            saw_positional = true;
        }
        let p_base = member::base_element(ctx, *p);
        let mut name = ctx.element_name(p_base).unwrap_or("").to_string();
        if name.is_empty() {
            let mut index = 1;
            name = format!("p{index}");
            while names.contains(&name) {
                index += 1;
                name = format!("p{index}");
            }
            names.push(name.clone());
        }
        parameter_names.push((name.clone(), kind));
        if member::is_covariant(ctx, *p) {
            w.write("covariant ");
        }
        if kind.is_required_named() {
            w.write("required ");
        }
        let has_type = write_type(&mut w, ctx, member::type_(ctx, *p), false);
        if !name.is_empty() {
            if has_type {
                w.write(" ");
            }
            w.write(&name);
        }
        if let Some(fp) = p_base.cast::<dartr_element::FormalParameterElement>() {
            if let Some(code) = dartr_element::display_string::default_value_code(ctx, fp) {
                w.write(" = ");
                w.write(&code);
            }
        }
    }
    if saw_named {
        w.write("}");
    } else if saw_positional {
        w.write("]");
    }
    w.write(")");
    display.push_str(&w.display.take().unwrap());
    w.writeln(" {");
    w.write(prefix2);
    w.write(&format!("// TODO: implement {member_name}"));
    let super_invocation = |w: &mut Writer| {
        let mut s = String::new();
        s.push_str(if is_operator { " " } else { "." });
        s.push_str(&member_name);
        s.push_str(if is_operator { " " } else { "(" });
        for (i, (name, kind)) in parameter_names.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            if kind.is_named() {
                s.push_str(name);
                s.push_str(": ");
            }
            s.push_str(name);
        }
        s.push_str(if is_operator { ";" } else { ");" });
        let _ = w;
        s
    };
    if is_setter {
        if invoke_super {
            w.writeln("");
            w.write(prefix2);
            let first = parameter_names.first().map(|p| p.0.clone()).unwrap_or_default();
            w.select_all(&format!("super.{member_name} = {first};"));
        } else {
            w.select_here();
        }
    } else if matches!(ctx.ty(return_type), TypeKind::Void) {
        if invoke_super {
            w.writeln("");
            w.write(prefix2);
            let s = super_invocation(&mut w);
            w.select_all(&format!("super{s}"));
        } else {
            w.select_here();
        }
    } else {
        w.writeln("");
        w.write(prefix2);
        if invoke_super {
            let s = super_invocation(&mut w);
            w.select_all(&format!("return super{s}"));
        } else {
            w.select_all("throw UnimplementedError();");
        }
    }
    w.writeln("");
    w.write(prefix);
    w.write("}");
    display.push_str(" { … }");
    Some((w.text, display, w.selection))
}

/// Dart `createOverrideSuggestionData`.
pub fn override_data(q: &Request<'_, '_>, c: &Candidate) -> Option<TypeImportData> {
    let Kind::Override {
        element,
        should_invoke_super,
        skip_at,
        ..
    } = &c.kind
    else {
        return None;
    };
    let ctx = q.ctx;
    let (replacement, mut display, selection) = write_override(ctx, *element, *should_invoke_super)?;
    let mut completion = replacement.trim().to_string();
    let annotation = "@override";
    if has_override(q, q.target.containing_node) && completion.starts_with(annotation) {
        completion = completion[annotation.len()..].trim().to_string();
    }
    if *skip_at && completion.starts_with(annotation) {
        completion = completion[1..].to_string();
    }
    if completion.is_empty() {
        return None;
    }
    let (selection_offset, selection_length) = selection?;
    let delta = replacement
        .find(&completion)
        .map(|i| utf16_len(&replacement[..i]))
        .unwrap_or(0);
    if display.is_empty() {
        return None;
    }
    if *skip_at {
        display = format!("override {display}");
    }
    Some(TypeImportData {
        completion,
        display_text: display,
        imports: Vec::new(),
        selection_offset: Some(selection_offset.saturating_sub(delta)),
        selection_length: Some(selection_length),
    })
}

/// Dart `AstNode.hasOverride`.
fn has_override(q: &Request<'_, '_>, node: dartr_ast::NodeId) -> bool {
    let ast = q.ast;
    ast.children(node).into_iter().any(|c| {
        ast.cast::<dartr_ast::Annotation>(c).is_some_and(|a| {
            let name = ast[a].name.raw();
            ast.cast::<dartr_ast::SimpleIdentifier>(name)
                .is_some_and(|s| ast.tokens.lexeme(ast[s].token) == "override")
        })
    })
}

/// Dart `createTypedSuggestionData`.
pub fn typed_data(q: &Request<'_, '_>, c: &Candidate) -> Option<TypeImportData> {
    let typed = c.typed()?;
    if !typed.add_type_annotation && typed.keyword.is_none() && !typed.add_type_name {
        return None;
    }
    let ctx = q.ctx;
    let mut text = String::new();
    if let Some(k) = typed.keyword {
        text.push_str(k);
        text.push(' ');
    }
    if typed.add_type_annotation {
        if let Some(t) = typed_type(ctx, c) {
            text.push_str(&type_display(ctx, t));
            text.push(' ');
        }
    }
    let completion = c.completion(ctx);
    if typed.add_type_name {
        if let Some(t) = containing_type(ctx, c) {
            text.push_str(&type_display(ctx, t));
            if !completion.is_empty() {
                text.push('.');
            }
        }
    }
    if matches!(c.kind, Kind::SetState { .. }) && (typed.add_type_annotation || typed.keyword.is_some()) {
        text.push_str("setState");
    } else {
        text.push_str(&completion);
    }
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    Some(TypeImportData {
        completion: trimmed,
        display_text: completion,
        imports: Vec::new(),
        selection_offset: None,
        selection_length: None,
    })
}

/// Dart `TypedSuggestion.type`.
fn typed_type(ctx: &Ctx<'_>, c: &Candidate) -> Option<TypeId> {
    Some(match &c.kind {
        Kind::Field { element, .. } => member::type_(ctx, *element),
        Kind::Getter { element, .. } => member::return_type(ctx, *element),
        Kind::Method { element, .. } | Kind::SetState { element, .. } => member::type_(ctx, *element),
        Kind::RecordField { field_type, .. } => *field_type,
        Kind::FunctionCall { ty, .. } => *ty,
        _ => return None,
    })
}

/// Dart `TypedSuggestion.containingType`.
fn containing_type(ctx: &Ctx<'_>, c: &Candidate) -> Option<TypeId> {
    let element = c.element()?;
    let base = member::base_element(ctx, element);
    let enclosing = match &c.kind {
        Kind::Constructor { .. } => elem::enclosing(ctx, base),
        Kind::Field { .. } | Kind::Getter { .. } | Kind::Method { .. } => elem::enclosing(ctx, base),
        _ => None,
    }?;
    super::declaration::instance_this_type(ctx, enclosing)
}
