// Dart source: tools/oracle/bin/resolved_el.dart (dumpResolvedLibraries)

//! `dartr dump resolved-el` and `dartr dump resolved`: links the libraries
//! of the inputs (shared setup with `dump elements`, see
//! [`crate::elements::link_inputs`]), resolves each library with
//! `dartr_resolver::analyze_library` and writes one line per input in the
//! format of `tools/oracle/bin/resolved_el.dart` (see the file comment
//! there for the keys).
//!
//! The libraries are resolved in parallel (rayon); the output order is the
//! input order. A panic of the resolver is caught: a panic in one unit gives
//! a `"panic"` key in that unit object, a panic outside the units gives a
//! line `{"path":p,"panic":"<message>"}`.

use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};

use dartr_ast::{
    Ast, ConstructorName, Entity, Expression, InvocationExpression, NamedType, NodeId,
    SimpleIdentifier,
};
use dartr_driver::file_state::FileId;
use dartr_element::{
    Ctx, ElemRef, ElementId, FeatureSet, NoopSink, Tag, TypeId, TypeProvider, WorldSnapshot,
};
use dartr_link::dump::{
    compare_utf16, element_name, error_json, first_fragment, fragment_offset, ref_,
};
use dartr_resolver::library_analyzer::{
    LibraryAnalysisInput, ResolvedUnit, UnitInput, analyze_library,
};
use dartr_resolver::options::AnalysisOptions;
use dartr_syntax::severity_lower_name;
use rayon::prelude::*;

use crate::elements::{Input, link_inputs};
use crate::json::write_string;

/// What one line holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResolvedMode {
    /// `resolved-el`: the element of each `SimpleIdentifier`, `NamedType`
    /// and `ConstructorName`.
    Elements,
    /// `resolved`: diagnostics and the static types of expressions.
    Types,
}

/// One input after linking.
enum Job {
    Error(&'static str),
    Library {
        driver: usize,
        library: dartr_element::EId<dartr_element::LibraryElement>,
        uri: String,
        units: Vec<UnitInput>,
        options: AnalysisOptions,
    },
}

/// One line per input path, in input order.
pub fn dump_resolved_all(inputs: &[String], mode: ResolvedMode) -> Vec<String> {
    let linked = link_inputs(inputs);
    let jobs: Vec<Job> = linked
        .inputs
        .iter()
        .map(|input| match *input {
            Input::Error(e) => Job::Error(e),
            Input::Library { driver, file } => library_job(&linked, driver, file),
        })
        .collect();
    let worlds: Vec<&WorldSnapshot> = linked.drivers.iter().map(|d| &d.state.world).collect();
    let tps: Vec<TypeProvider> = worlds
        .iter()
        .map(|w| dartr_link::types_builder::world_type_provider(w))
        .collect();

    let start = std::time::Instant::now();
    let lines = inputs
        .par_iter()
        .zip(jobs.par_iter())
        .map(|(p, job)| match job {
            Job::Error(e) => error_json(p, e),
            Job::Library {
                driver,
                library,
                uri,
                units,
                options,
            } => {
                let input = LibraryAnalysisInput {
                    world: worlds[*driver],
                    type_provider: &tps[*driver],
                    library: *library,
                    units: units.clone(),
                    options: *options,
                };
                match catch_unwind(AssertUnwindSafe(|| analyze_library(&input))) {
                    Ok(result) => {
                        library_line(p, uri, worlds[*driver], &tps[*driver], &result.units, mode)
                    }
                    Err(e) => panic_json(p, &panic_message(&*e)),
                }
            }
        })
        .collect();
    if std::env::var_os("DARTR_TIMINGS").is_some() {
        eprintln!(
            "timings: {} inputs resolved and written in {:.1} ms (threads: {})",
            inputs.len(),
            start.elapsed().as_secs_f64() * 1e3,
            rayon::current_num_threads()
        );
    }
    lines
}

fn library_job(linked: &crate::elements::LinkedInputs, driver: usize, file: FileId) -> Job {
    let d = &linked.drivers[driver];
    let Some((library, units)) = d.library_units(file) else {
        return Job::Error("NotLinked");
    };
    let path = d.fs.file(file).path.to_string();
    let options = match linked.driver_context[driver] {
        Some(context) => {
            let o = linked
                .collection
                .options_for(&linked.collection.contexts[context], &path);
            AnalysisOptions {
                strict_casts: o.strict_casts,
                strict_inference: o.strict_inference,
                strict_raw_types: o.strict_raw_types,
            }
        }
        None => AnalysisOptions::default(),
    };
    Job::Library {
        driver,
        library,
        uri: d.fs.file(file).uri_str.to_string(),
        units,
        options,
    }
}

fn panic_message(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<String>()
        .cloned()
        .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "panic".to_string())
}

fn panic_json(path: &str, message: &str) -> String {
    let mut out = String::new();
    out.push_str("{\"path\":");
    write_string(&mut out, path);
    out.push_str(",\"panic\":");
    write_string(&mut out, message);
    out.push('}');
    out
}

/// `{"path":p,"uri":..,"units":[...]}`.
fn library_line(
    path: &str,
    uri: &str,
    world: &WorldSnapshot,
    tp: &TypeProvider,
    units: &[ResolvedUnit],
    mode: ResolvedMode,
) -> String {
    let mut out = String::new();
    out.push_str("{\"path\":");
    write_string(&mut out, path);
    out.push_str(",\"uri\":");
    write_string(&mut out, uri);
    out.push_str(",\"units\":[");
    let features = FeatureSet::default();
    let sink = NoopSink;
    for (i, unit) in units.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let ctx = Ctx {
            world,
            current: None,
            local: Some(&unit.local),
            tp,
            features: &features,
            req: &sink,
        };
        out.push_str("{\"path\":");
        write_string(&mut out, &unit.path);
        let mut body = String::new();
        let written = catch_unwind(AssertUnwindSafe(|| match mode {
            ResolvedMode::Elements => write_elements(&mut body, &ctx, unit),
            ResolvedMode::Types => write_types(&mut body, &ctx, unit),
        }));
        // A panic of the resolver comes first; a panic while the results
        // are written (for example a dangling element id) is reported the
        // same way.
        let written_ok = written.is_ok();
        let panic = unit.panic.clone().or_else(|| {
            written
                .err()
                .map(|e| format!("dump: {}", panic_message(&*e)))
        });
        if let Some(message) = panic {
            out.push_str(",\"panic\":");
            write_string(&mut out, &message);
        }
        if written_ok {
            out.push_str(&body);
        }
        out.push('}');
    }
    out.push_str("]}");
    out
}

/// The nodes of [unit] in pre-order over the child entities (the order of
/// the `ast` dump).
fn nodes_in_order(ast: &Ast, root: NodeId) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        result.push(id);
        let children = ast.child_entities(id);
        for e in children.into_iter().rev() {
            if let Entity::Node(n) = e {
                stack.push(n);
            }
        }
    }
    result
}

fn write_type(out: &mut String, ctx: &Ctx<'_>, ty: Option<TypeId>) {
    match ty {
        Some(ty) => write_string(
            out,
            &dartr_element::type_display_string_with(
                ctx,
                ty,
                dartr_element::DisplayOptions::default(),
            ),
        ),
        None => out.push_str("null"),
    }
}

/// `,"nodes":[...]` of mode `resolved-el`.
fn write_elements(out: &mut String, ctx: &Ctx<'_>, unit: &ResolvedUnit) {
    let ast = &unit.ast;
    out.push_str(",\"nodes\":[");
    let mut first = true;
    for id in nodes_in_order(ast, unit.unit.raw()) {
        if !(ast.is::<SimpleIdentifier>(id)
            || ast.is::<NamedType>(id)
            || ast.is::<ConstructorName>(id))
        {
            continue;
        }
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(
            out,
            "{{\"o\":{},\"e\":{},\"k\":\"{}\",\"el\":",
            ast.offset(id),
            ast.end(id),
            ast.kind(id).name()
        );
        match unit.tables.element.get(id) {
            None => out.push_str("null"),
            Some(r) => {
                let (el, member) = element_ref(ctx, *r);
                write_string(out, &el);
                if let Some(member) = member {
                    out.push_str(",\"member\":");
                    write_string(out, &member);
                }
            }
        }
        out.push('}');
    }
    out.push(']');
}

/// `,"diagnostics":[...],"types":[...]` of mode `resolved`.
fn write_types(out: &mut String, ctx: &Ctx<'_>, unit: &ResolvedUnit) {
    let ast = &unit.ast;
    out.push_str(",\"diagnostics\":[");
    for (i, d) in unit.diagnostics.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"code\":");
        write_string(out, d.code.lower_case_name());
        out.push_str(",\"severity\":");
        write_string(out, severity_lower_name(d.severity));
        let _ = write!(out, ",\"o\":{},\"l\":{},\"msg\":", d.offset, d.length);
        write_string(out, &d.message);
        out.push('}');
    }
    out.push_str("],\"types\":[");
    let mut first = true;
    for id in nodes_in_order(ast, unit.unit.raw()) {
        if !ast.is::<Expression>(id) {
            continue;
        }
        let Some(ty) = unit.tables.static_type.get(id) else {
            continue;
        };
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(
            out,
            "{{\"o\":{},\"e\":{},\"k\":\"{}\",\"type\":",
            ast.offset(id),
            ast.end(id),
            ast.kind(id).name()
        );
        write_type(out, ctx, Some(*ty));
        if ast.is::<InvocationExpression>(id) {
            out.push_str(",\"inv\":");
            write_type(out, ctx, unit.tables.invoke_type.get(id).copied());
            out.push_str(",\"targs\":");
            match unit.tables.type_arg_types.get(id) {
                None => out.push_str("null"),
                Some(list) => {
                    out.push('[');
                    for (i, t) in ctx.list(*list).iter().enumerate() {
                        if i > 0 {
                            out.push(',');
                        }
                        write_type(out, ctx, Some(*t));
                    }
                    out.push(']');
                }
            }
        }
        out.push('}');
    }
    out.push(']');
}

/// The element reference of [r] (`"el"`) and, for a substituted member,
/// the substitution (`"member"`). See `elRef` and `memberString` in
/// `tools/oracle/bin/resolved_el.dart`.
fn element_ref(ctx: &Ctx<'_>, r: ElemRef) -> (String, Option<String>) {
    let (base, subst) = match r {
        ElemRef::Base(e) => (e, None),
        ElemRef::Member(m) => {
            let m = ctx.member(m);
            (m.base, Some(m.subst))
        }
    };
    let el = match local_ref(ctx, base) {
        Some(local) => local,
        None => ref_(ctx, Some(base)).unwrap_or_default(),
    };
    let member = subst.and_then(|subst| {
        let mut entries: Vec<String> = ctx
            .subst(subst)
            .iter()
            .filter(|(tp, _)| ctx.element_data(tp.raw()).and_then(|d| d.enclosing) != Some(base))
            .map(|(tp, ty)| {
                let name = element_name(ctx, tp.raw()).unwrap_or("");
                let ty = dartr_element::type_display_string_with(
                    ctx,
                    *ty,
                    dartr_element::DisplayOptions::default(),
                );
                format!("{name}: {ty}")
            })
            .collect();
        entries.sort_by(|a, b| compare_utf16(a, b));
        (!entries.is_empty()).then(|| entries.join(", "))
    });
    (el, member)
}

/// The kind name of a local element, `None` for other elements.
fn local_kind(tag: Tag) -> Option<&'static str> {
    match tag {
        Tag::LocalVariable => Some("variable"),
        Tag::PatternVariable | Tag::BindPatternVariable => Some("patternVariable"),
        Tag::JoinPatternVariable => Some("joinPatternVariable"),
        Tag::LocalFunction => Some("function"),
        Tag::Label => Some("label"),
        _ => None,
    }
}

/// `"local:<kind>:<name>@<offset>"` for a local element: a local variable,
/// pattern variable, local function or label, or a formal parameter or type
/// parameter with a local element in its enclosing chain.
fn local_ref(ctx: &Ctx<'_>, e: ElementId) -> Option<String> {
    let kind = match local_kind(e.tag()) {
        Some(kind) => kind,
        None => {
            let kind = match e.tag() {
                Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                    "parameter"
                }
                Tag::TypeParameter => "typeParameter",
                _ => return None,
            };
            let mut current = ctx.element_data(e).and_then(|d| d.enclosing);
            loop {
                let c = current?;
                if local_kind(c.tag()).is_some() {
                    break kind;
                }
                current = ctx.element_data(c).and_then(|d| d.enclosing);
            }
        }
    };
    let name = element_name(ctx, e).unwrap_or("");
    let fragment = first_fragment(ctx, e);
    let offset = ctx
        .fragment_data(fragment)
        .and_then(|d| d.name_offset)
        .map(i64::from)
        .unwrap_or_else(|| fragment_offset(ctx, fragment));
    Some(format!("local:{kind}:{name}@{offset}"))
}
