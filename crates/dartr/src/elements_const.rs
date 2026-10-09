// Dart source: tools/oracle/bin/elements.dart (`--with-const`)

//! The `"const"` key of `dartr dump elements --with-const`: the constant
//! value of a const top-level variable or field (enum constants included),
//! as Dart `element.computeConstantValue()` written with
//! `DartObjectImpl.toString()`, or `null` for an invalid constant
//! (docs/design/semantics.md §5.1).

use dartr_driver::driver::Driver;
use dartr_driver::file_state::FileId;
use dartr_element::{ElementId, FragmentFlags, Tag};
use dartr_resolver::library_analyzer::{
    ExternalUnitCache, LibraryAnalysisInput, analyze_library, compute_constant_values,
};
use dartr_resolver::options::AnalysisOptions;
use indexmap::IndexMap;

use crate::json::write_string;

/// The JSON text of the `"const"` value of each const top-level variable
/// and field of the library [file] (a JSON string, or `null` when the
/// constant is invalid): analyzes the library and computes the values.
pub(crate) fn library_const_values(driver: &Driver, file: FileId) -> IndexMap<ElementId, String> {
    let mut result = IndexMap::new();
    let Some((library, units)) = driver.library_units(file) else {
        return result;
    };
    let world = &driver.state.world;
    let tp = dartr_link::types_builder::world_type_provider(world);
    let options = AnalysisOptions::default();
    let external = ExternalUnitCache::new(world, &tp, options, driver.unit_sources());
    let input = LibraryAnalysisInput {
        world,
        type_provider: &tp,
        library,
        units,
        options,
        external: Some(&external),
    };
    let analyzed =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| analyze_library(&input)));
    let Ok(analyzed) = analyzed else {
        return result;
    };
    let ctx = dartr_element::Ctx {
        world,
        current: None,
        local: None,
        tp: &tp,
        features: &dartr_element::FeatureSet::default(),
        req: &dartr_element::NoopSink,
    };
    // The const top-level variables and fields of the library.
    let mut elements = Vec::new();
    for unit in &analyzed.units {
        let fragment = ctx.fragment(unit.fragment);
        for &v in &fragment.variables {
            push_const(
                &ctx,
                &mut elements,
                ctx.fragment_data(v.raw())
                    .and_then(|d| d.element.try_get().copied()),
            );
        }
        for instance in instance_fragments(&ctx, unit.fragment) {
            let Some(element) = ctx
                .fragment_data(instance)
                .and_then(|d| d.element.try_get().copied())
            else {
                continue;
            };
            let Some(instance) = element.cast::<dartr_element::InstanceElement>() else {
                continue;
            };
            for f in &ctx.instance(instance).fields {
                push_const(&ctx, &mut elements, Some(f.raw()));
            }
        }
    }
    for (e, value) in compute_constant_values(&input, &analyzed, &elements) {
        let mut json = String::new();
        match value {
            Some(text) => write_string(&mut json, &text),
            None => json.push_str("null"),
        }
        result.insert(e, json);
    }
    result
}

fn push_const(ctx: &dartr_element::Ctx<'_>, out: &mut Vec<ElementId>, e: Option<ElementId>) {
    let Some(e) = e else {
        return;
    };
    if !matches!(e.tag(), Tag::Field | Tag::TopLevelVariable) || out.contains(&e) {
        return;
    }
    let Some(data) = ctx.element_data(e) else {
        return;
    };
    let flags = ctx
        .fragment_data(data.first_fragment)
        .map(|f| f.flags.get())
        .unwrap_or(FragmentFlags::EMPTY);
    if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST) {
        out.push(e);
    }
}

fn instance_fragments(
    ctx: &dartr_element::Ctx<'_>,
    unit: dartr_element::FId<dartr_element::LibraryFragment>,
) -> Vec<dartr_element::FragmentId> {
    let f = ctx.fragment(unit);
    let mut result: Vec<dartr_element::FragmentId> = Vec::new();
    result.extend(f.classes.iter().map(|c| c.raw()));
    result.extend(f.enums.iter().map(|c| c.raw()));
    result.extend(f.mixins.iter().map(|c| c.raw()));
    result.extend(f.extensions.iter().map(|c| c.raw()));
    result.extend(f.extension_types.iter().map(|c| c.raw()));
    result
}
