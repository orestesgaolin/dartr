//! Differential check of `dartr_constant` against the pinned analyzer and the
//! Dart VM.
//!
//! The fixture `fixtures/const_values.json` is written by
//! `tools/oracle/bin/const_values.dart` (see its header for the format).
//! Regenerate it with:
//!
//! ```text
//! (cd tools/oracle && dart run bin/const_values.dart \
//!    > ../../crates/dartr_constant/tests/fixtures/const_values.json)
//! ```
//!
//! - `analyzer` column: the result of the ported `DartObjectImpl` operation,
//!   printed with `DartObjectImpl::display`, must be the same text as the
//!   analyzer's `DartObjectImpl.toString()` (or the same diagnostic code).
//! - `runtime` column: the `dart_num` helpers (Dart VM `int` / `double`
//!   semantics, `double.toString()`, `int.parse`) must give the same value as
//!   the VM for the arithmetic, bitwise and shift operations.

mod support;

use std::sync::Arc;

use dartr_constant::dart_num;
use dartr_constant::{
    BoolState, DartObjectImpl, DeclaredVariables, DoubleState, EvalResult,
    FromEnvironmentEvaluator, InstanceState, IntState, StringState, SymbolState, TypeState,
};
use dartr_element::TypeId;
use serde_json::Value;
use support::T;

fn fixture() -> Value {
    let text = include_str!("fixtures/const_values.json");
    serde_json::from_str(text).expect("valid fixture")
}

fn decode_double(hex: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(hex, 16).expect("hex bits"))
}

fn type_named(t: &T, name: &str) -> TypeId {
    let tp = t.tp();
    match name {
        "int" => tp.int_type(),
        "double" => tp.double_type(),
        "num" => tp.num_type(),
        "String" => tp.string_type(),
        "bool" => tp.bool_type(),
        "Object" => tp.object_type(),
        "Object?" => tp.object_question_type(),
        "Null" => tp.null_type(),
        _ => panic!("type {name}"),
    }
}

/// The analyzer value of an operand token (`analyzerValue` of the script).
fn value(t: &T, operand: &Value) -> DartObjectImpl {
    let ts = t.ts();
    let tp = t.tp();
    if let Value::Object(map) = operand {
        let elements = || -> Vec<DartObjectImpl> {
            map["e"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| value(t, e))
                .collect()
        };
        return match map["k"].as_str().unwrap() {
            "list" => t.list_value(type_named(t, map["t"].as_str().unwrap()), elements()),
            "set" => t.set_value(type_named(t, map["t"].as_str().unwrap()), Some(elements())),
            "map" => t.map_value(
                type_named(t, map["kt"].as_str().unwrap()),
                type_named(t, map["vt"].as_str().unwrap()),
                elements(),
            ),
            "record" => {
                let positional = map["p"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| value(t, e))
                    .collect();
                let named = map["n"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.as_str(), value(t, v)))
                    .collect();
                t.record_value(positional, named)
            }
            k => panic!("composite {k}"),
        };
    }
    let s = operand.as_str().unwrap();
    let object = |ty: TypeId, state: InstanceState| DartObjectImpl::new(&ts, ty, state);
    match s {
        "n" => return t.null_value(),
        "iu" => return object(tp.int_type(), InstanceState::Int(IntState::UNKNOWN_VALUE)),
        "du" => {
            return object(
                tp.double_type(),
                InstanceState::Double(DoubleState::UNKNOWN_VALUE),
            );
        }
        "su" => {
            return object(
                tp.string_type(),
                InstanceState::String(StringState::UNKNOWN_VALUE),
            );
        }
        "bu" => {
            return object(
                tp.bool_type(),
                InstanceState::Bool(BoolState::UNKNOWN_VALUE),
            );
        }
        _ => {}
    }
    let (kind, rest) = s.split_at(2);
    match kind {
        "i:" => t.int_value(Some(rest.parse().unwrap())),
        "d:" => t.double_value(Some(decode_double(rest))),
        "s:" => t.string_value(Some(rest)),
        "b:" => t.bool_value(Some(rest == "true")),
        "y:" => object(
            tp.symbol_type(),
            InstanceState::Symbol(SymbolState::new(Some(Arc::from(rest)))),
        ),
        "t:" => object(
            tp.type_type(),
            InstanceState::Type(TypeState::new(&ts, Some(type_named(t, rest)))),
        ),
        _ => panic!("operand {s}"),
    }
}

fn analyzer_result(t: &T, result: EvalResult<DartObjectImpl>) -> String {
    match result {
        Ok(object) => object.display(&t.ts()),
        Err(e) => format!(
            "E:{}{}",
            e.locatable_diagnostic.code.lower_case_unique_name(),
            if e.is_runtime_exception {
                ":runtime"
            } else {
                ""
            }
        ),
    }
}

/// The ported analyzer operation `op` (`binaryAnalyzer` / `unaryAnalyzer`
/// of the script); `None` for an operation that the analyzer does not have.
fn analyzer(t: &T, op: &str, a: &Value, b: &Value) -> Option<String> {
    let ts = t.ts();
    let fs = &t.feature_set;
    let x = value(t, a);
    if b.is_null() {
        let result = match op {
            "neg" => x.negated(&ts),
            "~" => x.bit_not(&ts),
            "!" => x.logical_not(&ts),
            "toString" => x.perform_to_string(&ts),
            "length" => x.string_length(&ts),
            "display" => return Some(x.display(&ts)),
            _ => panic!("unary {op}"),
        };
        return Some(analyzer_result(t, result));
    }
    let y = value(t, b);
    let result = match op {
        "+" => x.add(&ts, &y),
        "-" => x.minus(&ts, &y),
        "*" => x.times(&ts, &y),
        "/" => x.divide(&ts, &y),
        "~/" => x.integer_divide(&ts, &y),
        "%" => x.remainder(&ts, &y),
        "<" => x.less_than(&ts, &y),
        "<=" => x.less_than_or_equal(&ts, &y),
        ">" => x.greater_than(&ts, &y),
        ">=" => x.greater_than_or_equal(&ts, &y),
        "==" => x.equal_equal(&ts, fs, &y),
        "!=" => x.not_equal(&ts, fs, &y),
        "identical" => Ok(x.is_identical2(&ts, &y)),
        "&" => x.eager_and(&ts, &y),
        "|" => x.eager_or(&ts, &y),
        "^" => x.eager_xor(&ts, &y),
        "<<" => x.shift_left(&ts, &y),
        ">>" => x.shift_right(&ts, &y),
        ">>>" => x.logical_shift_right(&ts, &y),
        "&&" => x.lazy_and(&ts, || Ok(y.clone())),
        "||" => x.lazy_or(&ts, || Ok(y.clone())),
        "concat" => x.concatenate(&ts, &y),
        "remainder" => return None,
        _ => panic!("binary {op}"),
    };
    Some(analyzer_result(t, result))
}

#[derive(Clone, Copy)]
enum Num {
    Int(i64),
    Double(f64),
}

fn num(token: &Value) -> Option<Num> {
    let s = token.as_str()?;
    if let Some(rest) = s.strip_prefix("i:") {
        return Some(Num::Int(rest.parse().unwrap()));
    }
    s.strip_prefix("d:")
        .map(|rest| Num::Double(decode_double(rest)))
}

fn encode(n: Num) -> String {
    match n {
        Num::Int(i) => format!("i:{i}"),
        Num::Double(d) => format!("d:{:016x}", d.to_bits()),
    }
}

fn to_double(n: Num) -> f64 {
    match n {
        Num::Int(i) => dart_num::int_to_double(i),
        Num::Double(d) => d,
    }
}

const IDBZE: &str = "T:IntegerDivisionByZeroException";
const UNSUPPORTED: &str = "T:UnsupportedError";
const ARGUMENT: &str = "T:ArgumentError";

/// The Dart VM operation `op` on numbers, built from the `dart_num`
/// helpers; `None` for operations that this check does not cover.
fn runtime(op: &str, a: &Value, b: &Value) -> Option<String> {
    let x = num(a)?;
    if b.is_null() {
        return Some(match (op, x) {
            ("neg", Num::Int(i)) => encode(Num::Int(i.wrapping_neg())),
            ("neg", Num::Double(d)) => encode(Num::Double(-d)),
            ("~", Num::Int(i)) => encode(Num::Int(!i)),
            ("toString", Num::Int(i)) => format!("s:{i}"),
            ("toString", Num::Double(d)) => format!("s:{}", dart_num::double_to_string(d)),
            _ => return None,
        });
    }
    let y = num(b)?;
    let int_or = |r: Option<i64>, error: &str| r.map_or(error.to_string(), |v| encode(Num::Int(v)));
    let double = |v: f64| encode(Num::Double(v));
    Some(match (op, x, y) {
        ("+", Num::Int(p), Num::Int(q)) => encode(Num::Int(p.wrapping_add(q))),
        ("-", Num::Int(p), Num::Int(q)) => encode(Num::Int(p.wrapping_sub(q))),
        ("*", Num::Int(p), Num::Int(q)) => encode(Num::Int(p.wrapping_mul(q))),
        ("+", _, _) => double(to_double(x) + to_double(y)),
        ("-", _, _) => double(to_double(x) - to_double(y)),
        ("*", _, _) => double(to_double(x) * to_double(y)),
        ("/", _, _) => double(to_double(x) / to_double(y)),
        ("~/", Num::Int(p), Num::Int(q)) => int_or(dart_num::int_truncating_div(p, q), IDBZE),
        ("~/", _, _) => int_or(
            dart_num::double_to_int(to_double(x) / to_double(y)),
            UNSUPPORTED,
        ),
        ("%", Num::Int(p), Num::Int(q)) => int_or(dart_num::int_modulo(p, q), IDBZE),
        ("%", _, _) => double(dart_num::double_modulo(to_double(x), to_double(y))),
        ("remainder", Num::Int(p), Num::Int(q)) => int_or(dart_num::int_remainder(p, q), IDBZE),
        ("remainder", _, _) => double(dart_num::double_remainder(to_double(x), to_double(y))),
        ("&", Num::Int(p), Num::Int(q)) => encode(Num::Int(p & q)),
        ("|", Num::Int(p), Num::Int(q)) => encode(Num::Int(p | q)),
        ("^", Num::Int(p), Num::Int(q)) => encode(Num::Int(p ^ q)),
        ("<<" | ">>" | ">>>", Num::Int(_), Num::Int(q)) if q < 0 => ARGUMENT.to_string(),
        ("<<", Num::Int(p), Num::Int(q)) => encode(Num::Int(dart_num::int_shl(p, q))),
        (">>", Num::Int(p), Num::Int(q)) => encode(Num::Int(dart_num::int_shr(p, q))),
        (">>>", Num::Int(p), Num::Int(q)) => encode(Num::Int(dart_num::int_ushr(p, q))),
        _ => return None,
    })
}

fn report(kind: &str, checked: usize, failures: &[String]) {
    eprintln!("{kind}: {checked} checked, {} different", failures.len());
    for f in failures.iter().take(40) {
        eprintln!("  {f}");
    }
    assert!(
        failures.is_empty(),
        "{kind}: {} of {checked} cases differ",
        failures.len()
    );
}

#[test]
fn analyzer_column_matches_dart_object_impl() {
    let t = T::new();
    let fixture = fixture();
    let mut checked = 0;
    let mut failures = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let [op, a, b, expected, _] = case.as_array().unwrap().as_slice() else {
            panic!("case {case}");
        };
        let op = op.as_str().unwrap();
        let expected = expected.as_str().unwrap();
        if expected == "-" {
            continue;
        }
        let actual = analyzer(&t, op, a, b).expect("an analyzer operation");
        checked += 1;
        if actual != expected {
            failures.push(format!(
                "{op} {a} {b}: expected {expected}, actual {actual}"
            ));
        }
    }
    report("analyzer", checked, &failures);
    assert!(checked > 10_000);
}

#[test]
fn runtime_column_matches_dart_num() {
    let fixture = fixture();
    let mut checked = 0;
    let mut failures = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let [op, a, b, _, expected] = case.as_array().unwrap().as_slice() else {
            panic!("case {case}");
        };
        let expected = expected.as_str().unwrap();
        if expected == "-" {
            continue;
        }
        let Some(actual) = runtime(op.as_str().unwrap(), a, b) else {
            continue;
        };
        checked += 1;
        if actual != expected {
            failures.push(format!(
                "{op} {a} {b}: expected {expected}, actual {actual}"
            ));
        }
    }
    report("runtime", checked, &failures);
    assert!(checked > 5_000);
}

#[test]
fn from_environment_matches_int_parse() {
    let t = T::new();
    let ts = t.ts();
    let fixture = fixture();
    let mut checked = 0;
    let mut failures = Vec::new();
    for case in fixture["parse"].as_array().unwrap() {
        let [input, int_expected, bool_expected, runtime_expected] =
            case.as_array().unwrap().as_slice()
        else {
            panic!("case {case}");
        };
        let input = input.as_str().unwrap();
        let variables = DeclaredVariables::from_map([("x", input)]);
        let evaluator = FromEnvironmentEvaluator::new(&ts, &variables);
        let int_actual = evaluator.get_int(Some("x"), None).display(&ts);
        let bool_actual = evaluator.get_bool(Some("x"), None).display(&ts);
        let runtime_actual = match dart_num::int_parse(input) {
            Some(v) => format!("i:{v}"),
            None => "T:FormatException".to_string(),
        };
        for (expected, actual) in [
            (int_expected, int_actual),
            (bool_expected, bool_actual),
            (runtime_expected, runtime_actual),
        ] {
            checked += 1;
            if expected.as_str().unwrap() != actual {
                failures.push(format!("{input:?}: expected {expected}, actual {actual}"));
            }
        }
    }
    report("parse", checked, &failures);
}
