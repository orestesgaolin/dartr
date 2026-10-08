// Dart source: pkg/_fe_analyzer_shared/test/exhaustiveness/utils.dart

//! The helpers of `utils.dart`.
//!
//! Dart registers one `test(...)` per logical case from inside these helpers.
//! Rust cannot create test functions at run time, so the helpers run each
//! logical case at once and record it, with its Dart test name, in a
//! [Collector]. One Rust `#[test]` per Dart `group` (or top-level `test`)
//! owns a collector and fails if any recorded case failed. The case names can
//! be written to a directory (`DARTR_EXH_TEST_NAMES_DIR`) to compare them
//! with the names that `dart test` reports.

use std::io::Write;

use dartr_type_analyzer::exhaustiveness::{
    Key, ObjectPropertyLookup, Path, Space, StaticType, StaticTypeArena, is_exhaustive,
};
use indexmap::IndexMap;

/// Records the logical test cases of one Dart group.
pub struct Collector {
    file: String,
    group: String,
    results: Vec<(String, Result<(), String>)>,
}

impl Collector {
    /// [file] is the Dart test file name, [group] the Dart group name (empty
    /// for a top-level test).
    pub fn new(file: &str, group: &str) -> Collector {
        Collector {
            file: file.to_string(),
            group: group.to_string(),
            results: vec![],
        }
    }

    /// Dart `test(name, body)`.
    pub fn test(&mut self, name: &str, body: impl FnOnce() -> Result<(), String>) {
        let full_name = if self.group.is_empty() {
            name.to_string()
        } else {
            format!("{} {}", self.group, name)
        };
        let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
            Ok(result) => result,
            Err(payload) => Err(payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".to_string())),
        };
        self.results.push((full_name, result));
    }

    /// Fails if any case failed. Prints the number of cases.
    pub fn finish(self) {
        if let Ok(dir) = std::env::var("DARTR_EXH_TEST_NAMES_DIR") {
            let path = std::path::Path::new(&dir).join(format!("{}.txt", self.file));
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .expect("names file");
            let mut text = String::new();
            for (name, _) in &self.results {
                text.push_str(&format!("{}\t{}\n", self.file, name));
            }
            // One write per collector, so parallel tests do not interleave.
            file.write_all(text.as_bytes()).unwrap();
        }
        let failures: Vec<String> = self
            .results
            .iter()
            .filter_map(|(name, result)| result.as_ref().err().map(|e| format!("{name}: {e}")))
            .collect();
        println!(
            "[{}] group '{}': {} logical cases, {} failed",
            self.file,
            self.group,
            self.results.len(),
            failures.len()
        );
        assert!(
            failures.is_empty(),
            "{} of {} cases failed:\n{}",
            failures.len(),
            self.results.len(),
            failures.join("\n")
        );
    }
}

/// Dart `expect(actual, expected)` as a result.
pub fn expect_eq<T: PartialEq + std::fmt::Debug>(actual: T, expected: T) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("Expected: {expected:?}\n  Actual: {actual:?}"))
    }
}

/// The Dart `Object` arguments of the helpers: a [Space], a [StaticType], the
/// string `'∅'`, or a list of these.
#[derive(Clone)]
pub enum Obj {
    Space(Space),
    Type(StaticType),
    /// Dart `'∅'`.
    Empty,
    List(Vec<Obj>),
}

impl From<Space> for Obj {
    fn from(space: Space) -> Obj {
        Obj::Space(space)
    }
}

impl From<StaticType> for Obj {
    fn from(type_: StaticType) -> Obj {
        Obj::Type(type_)
    }
}

impl From<Vec<Obj>> for Obj {
    fn from(list: Vec<Obj>) -> Obj {
        Obj::List(list)
    }
}

/// Builds a `Vec<Obj>` (a Dart `List<Object>`).
macro_rules! objs {
    ($($e:expr),* $(,)?) => {
        vec![$($crate::exhaustiveness_support::utils::Obj::from($e)),*]
    };
}

/// Dart `ty(type, {field: value, ...})`.
macro_rules! ty {
    ($env:expr, $type:expr, { $($k:ident : $v:expr),* $(,)? }) => {
        $crate::exhaustiveness_support::utils::ty(
            $env,
            $type,
            &[$((stringify!($k), $crate::exhaustiveness_support::utils::Obj::from($v))),*],
        )
    };
}

/// Test that [spaces] is exhaustive over [value].
pub fn expect_exhaustive(
    c: &mut Collector,
    field_lookup: &dyn ObjectPropertyLookup,
    value: &Space,
    spaces: &[Space],
) {
    expect_exhaustive_internal(c, field_lookup, value, spaces, true);
}

/// Test that [cases] are exhaustive over [type_] if and only if all cases are
/// included and that all subsets of the cases are not exhaustive.
pub fn expect_exhaustive_only_all(
    c: &mut Collector,
    field_lookup: &dyn ObjectPropertyLookup,
    type_: StaticType,
    cases: Vec<Obj>,
) {
    test_cases(c, field_lookup, type_, cases, true);
}

/// Test that [cases] are not exhaustive over [type_]. Also test that omitting
/// each case is still not exhaustive.
pub fn expect_never_exhaustive(
    c: &mut Collector,
    field_lookup: &dyn ObjectPropertyLookup,
    type_: StaticType,
    cases: Vec<Obj>,
) {
    test_cases(c, field_lookup, type_, cases, false);
}

/// Test that [spaces] is not exhaustive over [value].
pub fn expect_not_exhaustive(
    c: &mut Collector,
    field_lookup: &dyn ObjectPropertyLookup,
    value: &Space,
    spaces: &[Space],
) {
    expect_exhaustive_internal(c, field_lookup, value, spaces, false);
}

pub fn fields_to_space(
    types: &dyn StaticTypeArena,
    fields: &[(&str, Obj)],
    path: &Path,
    as_record_names: bool,
) -> IndexMap<Key, Space> {
    fields
        .iter()
        .map(|(name, value)| {
            let key = if as_record_names {
                Key::record_name_key(*name)
            } else {
                Key::name_key(*name)
            };
            let space = parse_space(types, value, &path.add(key.clone()));
            (key, space)
        })
        .collect()
}

pub fn parse_space(types: &dyn StaticTypeArena, object: &Obj, path: &Path) -> Space {
    match object {
        Obj::Space(space) => space.clone(),
        Obj::Empty => Space::new(path.clone(), StaticType::NEVER_TYPE),
        Obj::Type(type_) => Space::new(path.clone(), *type_),
        Obj::List(list) => {
            let mut spaces: Option<Space> = None;
            for element in list {
                spaces = Some(match spaces {
                    None => parse_space(types, element, path),
                    Some(spaces) => spaces.union(types, &parse_space(types, element, path)),
                });
            }
            spaces.unwrap_or_else(|| Space::empty(path.clone()))
        }
    }
}

/// Parse a list of spaces using [parse_space].
pub fn parse_spaces(types: &dyn StaticTypeArena, objects: &[Obj]) -> Vec<Space> {
    objects
        .iter()
        .map(|object| parse_space(types, object, &Path::root()))
        .collect()
}

/// Make a [Space] with [type_] and [fields].
pub fn ty(env: &dyn ObjectPropertyLookup, type_: StaticType, fields: &[(&str, Obj)]) -> Space {
    let types = env.static_types();
    let path = Path::root();
    Space::with_properties(
        path.clone(),
        type_,
        fields_to_space(types, fields, &path, types.is_record(type_)),
        IndexMap::new(),
    )
}

/// Dart `List<Space>.toString()`.
pub fn spaces_to_list_text(types: &dyn StaticTypeArena, spaces: &[Space]) -> String {
    format!(
        "[{}]",
        spaces
            .iter()
            .map(|s| s.to_text(types))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn check_exhaustive(
    field_lookup: &dyn ObjectPropertyLookup,
    value: &Space,
    spaces: &[Space],
    expectation: bool,
) -> Result<(), String> {
    let types = field_lookup.static_types();
    let actual = is_exhaustive(field_lookup, value, spaces);
    if expectation != actual {
        if expectation {
            return Err(format!(
                "Expected {} to cover {} but did not.",
                spaces_to_list_text(types, spaces),
                value.to_text(types)
            ));
        } else {
            return Err(format!(
                "Expected {} to not cover {} but did.",
                spaces_to_list_text(types, spaces),
                value.to_text(types)
            ));
        }
    }
    Ok(())
}

fn expect_exhaustive_internal(
    c: &mut Collector,
    field_lookup: &dyn ObjectPropertyLookup,
    value: &Space,
    spaces: &[Space],
    expectation: bool,
) {
    let types = field_lookup.static_types();
    let name = format!(
        "{} - {} {} exhaustive",
        value.to_text(types),
        spaces
            .iter()
            .map(|s| s.to_text(types))
            .collect::<Vec<_>>()
            .join(" - "),
        if expectation { "is" } else { "is not" }
    );
    c.test(&name, || {
        check_exhaustive(field_lookup, value, spaces, expectation)
    });
}

/// Test that [cases] are not exhaustive over [type_].
fn test_cases(
    c: &mut Collector,
    field_lookup: &dyn ObjectPropertyLookup,
    type_: StaticType,
    cases: Vec<Obj>,
    expectation: bool,
) {
    let types = field_lookup.static_types();
    let value_space = Space::new(Path::root(), type_);
    let spaces = parse_spaces(types, &cases);

    c.test(&format!("{} with all cases", types.name(type_)), || {
        check_exhaustive(field_lookup, &value_space, &spaces, expectation)
    });

    // With any single case removed, should also not be exhaustive.
    for i in 0..spaces.len() {
        let mut filtered = spaces.clone();
        filtered.remove(i);

        c.test(
            &format!(
                "{} without case {}",
                types.name(type_),
                spaces[i].to_text(types)
            ),
            || check_exhaustive(field_lookup, &value_space, &filtered, false),
        );
    }
}
