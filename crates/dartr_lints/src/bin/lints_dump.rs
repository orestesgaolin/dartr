// Dart source: pkg/analyzer/lib/src/dart/analysis/library_analyzer.dart (_computeLints)
//! A JSON-lines differential runner, separate from the dartr CLI.
use dartr_ast::{PartDirective, PartOfDirective, SimpleStringLiteral};
use dartr_ast_builder::parse_file;
use dartr_lints::{
    ExperimentalFlag, Registry, RuleContextUnit, lint_library, rules::implemented_rules,
};
use indexmap::IndexMap;
use serde_json::{Value, json};
use std::{
    io::{self, BufRead},
    path::{Path, PathBuf},
};

fn diagnostic_json(d: dartr_diagnostics::Diagnostic) -> Value {
    json!({"code":d.code.name,"severity":d.severity.name(),"offset":d.offset,
           "length":d.length,"message":d.message})
}
fn normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
fn main() {
    if std::env::args().any(|arg| arg == "--metadata") {
        let rules: Vec<_> = dartr_lints::ALL_RULES.iter().map(|rule| {
            json!({
                "name": rule.name,
                "description": rule.description,
                "state": format!("{:?}", rule.state.kind).to_ascii_lowercase(),
                "since": rule.state.since.map(|(a,b,c)| format!("{a}.{b}.{c}")),
                "replacedBy": rule.state.replaced_by,
                "canUseParsedResult": rule.can_use_parsed_result(),
                "incompatibleRules": rule.incompatible_rules,
                "diagnosticCodes": rule.diagnostic_codes().map(|code| code.unique_name).collect::<Vec<_>>(),
            })
        }).collect();
        println!("{}", json!(rules));
        return;
    }
    if std::env::args().any(|arg| arg == "--list") {
        let registry = Registry::builtin();
        let rules: Vec<_> = implemented_rules().into_iter().map(|name| {
            let rule = registry.get_rule(name).expect("registered rule");
            json!({"name":name, "codes":rule.diagnostic_codes().map(|code| code.name).collect::<Vec<_>>()})
        }).collect();
        println!("{}", json!(rules));
        return;
    }
    let requests: Vec<Value> = io::stdin()
        .lock()
        .lines()
        .map(|line| serde_json::from_str(&line.expect("read request")).expect("JSON request"))
        .collect();
    let paths: Vec<_> = requests
        .iter()
        .map(|r| r["path"].as_str().expect("path"))
        .collect();
    let sources: Vec<_> = requests
        .iter()
        .zip(&paths)
        .map(|(r, path)| {
            let source = r["source"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| std::fs::read_to_string(path).expect("read Dart file"));
            source
                .strip_prefix('\u{feff}')
                .unwrap_or(&source)
                .to_owned()
        })
        .collect();
    let parsed: Vec<_> = sources
        .iter()
        .zip(&paths)
        .zip(&requests)
        .map(|((source, path), request)| {
            let version = request["languageVersion"].as_array().map_or((3, 13), |v| {
                (
                    v[0].as_u64().expect("language major") as u32,
                    v[1].as_u64().expect("language minor") as u32,
                )
            });
            let experiments: Vec<_> = request["experiments"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|name| {
                    let name = name.as_str().expect("experiment name");
                    ExperimentalFlag::VALUES
                        .iter()
                        .copied()
                        .find(|flag| flag.name() == name)
                        .expect("known experiment")
                })
                .collect();
            parse_file(source, path, version, &experiments)
        })
        .collect();
    let path_indices: IndexMap<_, _> = paths
        .iter()
        .enumerate()
        .map(|(i, path)| (normalized(Path::new(path)), i))
        .collect();
    let mut parts: Vec<Vec<usize>> = vec![vec![]; parsed.len()];
    let mut is_part = vec![false; parsed.len()];
    for (i, unit) in parsed.iter().enumerate() {
        for &directive in unit.ast.list(unit.ast[unit.unit].directives) {
            if unit.ast.is::<PartOfDirective>(directive) {
                is_part[i] = true;
            }
            if let Some(part) = unit.ast.cast::<PartDirective>(directive)
                && let Some(uri) = unit.ast.cast::<SimpleStringLiteral>(unit.ast[part].uri)
            {
                let path = Path::new(paths[i])
                    .parent()
                    .unwrap_or(Path::new(""))
                    .join(unit.ast[uri].value.as_ref());
                if let Some(&index) = path_indices.get(&normalized(&path)) {
                    parts[i].push(index);
                }
            }
        }
    }
    let mut diagnostics: Vec<Vec<Value>> = vec![vec![]; parsed.len()];
    let mut visited = vec![false; parsed.len()];
    // Defining units precede parts; unattached part files are still analyzed.
    let order = (0..parsed.len())
        .filter(|&i| !is_part[i])
        .chain((0..parsed.len()).filter(|&i| is_part[i]));
    for i in order {
        if visited[i] {
            continue;
        }
        let indices: Vec<_> = std::iter::once(i).chain(parts[i].iter().copied()).collect();
        let units: Vec<_> = indices
            .iter()
            .map(|&j| RuleContextUnit {
                parsed: &parsed[j],
                source: &sources[j],
                path: paths[j],
            })
            .collect();
        let enabled: Vec<_> = requests[i]["enabled"]
            .as_array()
            .expect("enabled rules")
            .iter()
            .map(|name| name.as_str().expect("rule name"))
            .collect();
        for (&index, result) in indices.iter().zip(lint_library(&units, &enabled)) {
            visited[index] = true;
            diagnostics[index] = result.into_iter().map(diagnostic_json).collect();
        }
    }
    for (path, diagnostics) in paths.iter().zip(diagnostics) {
        println!("{}", json!({"path":path,"diagnostics":diagnostics}));
    }
}
