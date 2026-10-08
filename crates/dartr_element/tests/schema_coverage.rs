//! Checks that every instance field of the Dart `*ElementImpl` /
//! `*FragmentImpl` classes (schema/element.json, extracted from the pinned
//! analyzer) is either held by a Rust field (a `Dart: Class.field` line in
//! the doc of that field) or listed in [`DROPPED`] with the reason.
//!
//! When the schema is regenerated for a new SDK version, this test lists
//! the new fields to port and the ported fields that no longer exist.

use std::collections::BTreeSet;
use std::path::Path;

/// Dart fields without a Rust field, with the reason.
const DROPPED: &[(&str, &str)] = &[
    ("ElementImpl.id", "identity is the ElementId"),
    ("FragmentImpl.id", "identity is the FragmentId"),
    ("ClassElementImpl.reference", REFERENCE),
    ("EnumElementImpl.reference", REFERENCE),
    ("MixinElementImpl.reference", REFERENCE),
    ("ExtensionElementImpl.reference", REFERENCE),
    ("ExtensionTypeElementImpl.reference", REFERENCE),
    ("ConstructorElementImpl.reference", REFERENCE),
    ("FieldElementImpl.reference", REFERENCE),
    ("GetterElementImpl.reference", REFERENCE),
    ("SetterElementImpl.reference", REFERENCE),
    ("MethodElementImpl.reference", REFERENCE),
    ("TopLevelFunctionElementImpl.reference", REFERENCE),
    ("TopLevelVariableElementImpl.reference", REFERENCE),
    ("TypeAliasElementImpl.reference", REFERENCE),
    ("LibraryElementImpl.reference", REFERENCE),
    ("ConstructorElementImpl.isCycleFree", CONSTANTS),
    ("ConstructorElementImpl.isConstantEvaluated", CONSTANTS),
    ("VariableElementImpl.evaluationResult", CONSTANTS),
    ("ElementAnnotationImpl.evaluationResult", CONSTANTS),
    ("ElementAnnotationImpl.additionalErrors", CONSTANTS),
    ("InstanceElementImpl.requirementState", FINE),
    ("LibraryElementImpl.requirementState", FINE),
    ("LibraryElementImpl.manifest", FINE),
    ("LibraryElementImpl.nameUnion", FINE),
    (
        "InterfaceElementImpl.mixinInferenceCallback",
        "linker state of types_builder (unit B2), not element data",
    ),
    (
        "PropertyInducingElementImpl.typeInference",
        "linker status vectors of top-level inference (unit C10, design §3)",
    ),
    (
        "PropertyInducingElementImpl.internal",
        "helper object without own data",
    ),
    (
        "LibraryElementImpl.internal",
        "helper object without own data",
    ),
    (
        "LibraryElementImpl._context",
        "no AnalysisContext objects; see Ctx",
    ),
    (
        "LibraryElementImpl._session",
        "no AnalysisSession objects; see Ctx",
    ),
    ("LibraryElementImpl.hasTypeProviderSystemSet", "Ctx.tp"),
    (
        "LibraryElementImpl.typeProvider",
        "Ctx.tp / WorldSnapshot.type_provider",
    ),
    (
        "LibraryElementImpl.typeSystem",
        "TypeSystem is built per library (unit A3)",
    ),
    (
        "LibraryElementImpl.exportEntries",
        "dartr_link LinkedCycle.export_scopes (design §2.2)",
    ),
    (
        "LibraryElementImpl._libraryDeclarations",
        "lazy cache of extension lookup (unit C6), kept in the cycle caches",
    ),
    ("LibraryFragmentImpl._scope", SCOPE),
    ("PrefixElementImpl._scope", SCOPE),
];

const REFERENCE: &str = "summary Reference tree: dartr_link ReferenceTable maps symbolic paths to ElementIds (design §2.4)";
const CONSTANTS: &str =
    "constant evaluation results live in the cycle constant cache (units D1-D2, design §2.3)";
const FINE: &str = "fine-grained dependencies: RequirementSink and manifests (design §4.3)";
const SCOPE: &str = "scopes are built and cached by unit A8";

fn schema_fields() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/element.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut out = BTreeSet::new();
    for class in schema["classes"].as_array().unwrap() {
        let name = class["name"].as_str().unwrap();
        for field in class["fields"].as_array().unwrap() {
            out.insert(format!("{name}.{}", field["name"].as_str().unwrap()));
        }
    }
    out
}

/// `Dart: Class.field` markers in doc comments of the crate sources.
fn ported_fields() -> BTreeSet<String> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = BTreeSet::new();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        for line in std::fs::read_to_string(&path).unwrap().lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("/// Dart: ") {
                let token = rest.split_whitespace().next().unwrap();
                if let Some((class, field)) = token.split_once('.')
                    && class.ends_with("Impl")
                {
                    out.insert(format!("{class}.{field}"));
                }
            }
        }
    }
    out
}

#[test]
fn every_dart_field_is_ported_or_dropped_with_a_reason() {
    let schema = schema_fields();
    let ported = ported_fields();
    let dropped: BTreeSet<String> = DROPPED.iter().map(|(f, _)| f.to_string()).collect();

    assert!(schema.len() > 150, "schema has {} fields", schema.len());

    let missing: Vec<&String> = schema
        .iter()
        .filter(|f| !ported.contains(*f) && !dropped.contains(*f))
        .collect();
    assert!(
        missing.is_empty(),
        "Dart fields without a Rust field: {missing:#?}"
    );

    let stale: Vec<&String> = ported
        .iter()
        .chain(dropped.iter())
        .filter(|f| !schema.contains(*f))
        .collect();
    assert!(
        stale.is_empty(),
        "markers for fields not in the schema: {stale:#?}"
    );

    let both: Vec<&String> = ported.intersection(&dropped).collect();
    assert!(both.is_empty(), "fields both ported and dropped: {both:#?}");
}

#[test]
fn flags_match_the_storage_enums_of_the_schema() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/element.json");
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let names = |e: &str| -> Vec<String> {
        schema["enums"][e]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(
        dartr_element::ElementFlags::DART_NAMES,
        names("_ElementStorageFlag")
    );
    assert_eq!(
        dartr_element::FragmentFlags::DART_NAMES,
        names("_FragmentStorageFlag")
    );
}
