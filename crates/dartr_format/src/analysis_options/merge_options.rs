// Dart source: dart_style lib/src/analysis_options/merge_options.dart

//! Merges a defaults options set with an overrides options set.

use crate::analysis_options::analysis_options_file::Value;

/// Merges a [defaults] options set with an [overrides] options set using
/// simple override semantics, suitable for merging two configurations where
/// one defines default values that are added to (and possibly overridden) by an
/// overriding one.
///
/// The merge rules are:
///
/// *   Lists are concatenated without duplicates.
/// *   A list of strings is promoted to a map of strings to `true` when merged
///     with another map of strings to booleans. For example `['opt1', 'opt2']`
///     is promoted to `{'opt1': true, 'opt2': true}`.
/// *   Maps unioned. When both have the same key, the corresponding values are
///     merged, recursively.
/// *   Otherwise, a non-`null` override replaces a default value.
pub fn merge(defaults: &Value, overrides: &Value) -> Value {
    match (defaults, overrides) {
        (Value::List(list), Value::Map(_)) if is_all_strings(list) && is_to_bools(overrides) => {
            merge(&promote_list(list), overrides)
        }
        (Value::Map(_), Value::List(list)) if is_to_bools(defaults) && is_all_strings(list) => {
            merge(defaults, &promote_list(list))
        }
        (Value::Map(defaults_map), Value::Map(overrides_map)) => {
            Value::Map(merge_map(defaults_map, overrides_map))
        }
        (Value::List(defaults_list), Value::List(overrides_list)) => {
            Value::List(merge_list(defaults_list, overrides_list))
        }
        // Default to override, unless the overriding value is `null`.
        (_, Value::Null) => defaults.clone(),
        _ => overrides.clone(),
    }
}

fn is_all_strings(list: &[Value]) -> bool {
    list.iter().all(|e| matches!(e, Value::String(_)))
}

fn is_to_bools(map: &Value) -> bool {
    match map {
        Value::Map(entries) => entries.iter().all(|(_, v)| matches!(v, Value::Bool(_))),
        _ => false,
    }
}

/// Promote a list of strings to a map of those strings to `true`.
fn promote_list(list: &[Value]) -> Value {
    let mut entries: Vec<(Value, Value)> = Vec::new();
    for element in list {
        // Dart map literal: a repeated key keeps its first position.
        if let Some(entry) = entries.iter_mut().find(|(k, _)| k.key_equals(element)) {
            entry.1 = Value::Bool(true);
        } else {
            entries.push((element.clone(), Value::Bool(true)));
        }
    }
    Value::Map(entries)
}

/// Merge lists, avoiding duplicates.
fn merge_list(defaults: &[Value], overrides: &[Value]) -> Vec<Value> {
    // Add them both to a set so that the overrides replace the defaults.
    let mut result: Vec<Value> = Vec::new();
    for element in defaults.iter().chain(overrides) {
        if !result.iter().any(|e| e.key_equals(element)) {
            result.push(element.clone());
        }
    }
    result
}

/// Merge maps (recursively).
fn merge_map(defaults: &[(Value, Value)], overrides: &[(Value, Value)]) -> Vec<(Value, Value)> {
    let mut merged: Vec<(Value, Value)> = defaults.to_vec();

    for (key, value) in overrides {
        match merged.iter_mut().find(|(k, _)| k.key_equals(key)) {
            Some(entry) => entry.1 = merge(&entry.1, value),
            None => merged.push((key.clone(), value.clone())),
        }
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Value {
        Value::String(text.into())
    }

    #[test]
    fn merges_like_dart_style() {
        let defaults = Value::Map(vec![
            (
                s("formatter"),
                Value::Map(vec![(s("page_width"), Value::Int(100))]),
            ),
            (s("rules"), Value::List(vec![s("a"), s("b")])),
        ]);
        let overrides = Value::Map(vec![
            (
                s("formatter"),
                Value::Map(vec![(s("trailing_commas"), s("preserve"))]),
            ),
            (s("rules"), Value::Map(vec![(s("a"), Value::Bool(false))])),
        ]);
        let merged = merge(&defaults, &overrides);
        assert_eq!(
            merged,
            Value::Map(vec![
                (
                    s("formatter"),
                    Value::Map(vec![
                        (s("page_width"), Value::Int(100)),
                        (s("trailing_commas"), s("preserve")),
                    ])
                ),
                (
                    s("rules"),
                    Value::Map(vec![
                        (s("a"), Value::Bool(false)),
                        (s("b"), Value::Bool(true))
                    ])
                ),
            ])
        );
        assert_eq!(merge(&Value::Int(1), &Value::Null), Value::Int(1));
    }
}
