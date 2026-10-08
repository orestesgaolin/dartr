//! Robustness check of unit C1 over a corpus (for example
//! `third_party/dart-sdk/tests/language`, which has many files with
//! errors): links every library of the directory and runs the element
//! binding visitor and the resolution visitor on each unit. Fails when one
//! of the two passes panics.
//!
//! ```text
//! DARTR_C1_CORPUS=third_party/dart-sdk/tests/language \
//!   cargo test --release -p dartr_resolver --test c1_corpus -- --ignored --nocapture
//! ```

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use dartr_driver::driver::Driver;
use dartr_driver::file_state::{FileConfig, FileSystemState, SourceFactory};
use dartr_element::{Ctx, FeatureSet, Generation, NoopSink, ResolutionTables};
use dartr_project::{DartSdk, Packages, Workspace};
use dartr_resolver::resolver::UnitContext;
use dartr_resolver::scope::LibraryScopes;
use dartr_resolver::tables::ResolverTables;

fn dart_files(dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            dart_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "dart") {
            out.push(path.to_string_lossy().to_string());
        }
    }
}

#[test]
#[ignore]
fn c1_passes_do_not_panic_on_corpus() {
    let Ok(corpus) = std::env::var("DARTR_C1_CORPUS") else {
        eprintln!("set DARTR_C1_CORPUS to a directory of Dart files");
        return;
    };
    let corpus = std::fs::canonicalize(&corpus).expect("corpus dir");
    let Some(sdk_path) = dartr_project::sdk::find_sdk_path() else {
        eprintln!("skipped: no Dart SDK on PATH");
        return;
    };
    let mut files = Vec::new();
    dart_files(&corpus, &mut files);

    let sdk = DartSdk::new(&sdk_path);
    let version = sdk
        .language_version()
        .map(|v| (v.major, v.minor))
        .unwrap_or((3, 13));
    let source_factory = SourceFactory {
        workspace: Workspace::basic(Packages::empty(), &corpus.to_string_lossy()),
        sdk: Some(sdk),
    };
    let config_for = Box::new(move |_: &str, _: &str| FileConfig {
        package_language_version: version,
        experiments: Vec::new(),
    });
    let mut driver = Driver::new(
        FileSystemState::new(source_factory, config_for),
        Arc::new(Generation::new(0)),
    );
    let ids: Vec<_> = files
        .iter()
        .map(|f| driver.fs.get_file_for_path(f))
        .collect();
    driver.fs.discover();
    let libraries: Vec<_> = ids
        .into_iter()
        .filter(|&f| !driver.fs.file(f).kind().is_part())
        .collect();
    driver.link_libraries(&libraries);

    let locations: Arc<Mutex<Vec<String>>> = Arc::default();
    {
        let locations = locations.clone();
        std::panic::set_hook(Box::new(move |info| {
            let location = info
                .location()
                .map(|l| format!("{}:{}", l.file(), l.line()))
                .unwrap_or_default();
            locations.lock().unwrap().push(location);
        }));
    }

    let world = &driver.state.world;
    let tp = dartr_link::types_builder::world_type_provider(world);
    let features = FeatureSet::default();
    let sink = NoopSink;
    let mut failures = Vec::new();
    let mut units_done = 0usize;
    let mut lookups = 0usize;
    let mut declared = 0usize;
    let mut annotated = 0usize;
    let mut reported = 0usize;
    let mut unbound: Vec<String> = Vec::new();
    let mut mismatched: Vec<String> = Vec::new();
    let mut dump: Vec<String> = Vec::new();
    for &file in &libraries {
        let uri = driver.fs.file(file).uri_str.clone();
        let Some(&library) = world.libraries.get(&uri) else {
            continue;
        };
        let global = Ctx {
            world,
            current: None,
            local: None,
            tp: &tp,
            features: &features,
            req: &sink,
        };
        let library_features = global.get(library).feature_set.clone();
        let global = Ctx {
            features: &library_features,
            ..global
        };
        let scopes = match catch_unwind(AssertUnwindSafe(|| LibraryScopes::build(&global, library)))
        {
            Ok(s) => s,
            Err(_) => {
                let at = locations.lock().unwrap().pop().unwrap_or_default();
                failures.push(format!("{uri}: LibraryScopes::build panicked at {at}"));
                continue;
            }
        };
        for fragment in dartr_driver::analysis::library_fragments(&global, library) {
            let path = global.fragment(fragment).source.path.clone();
            let Some(unit_file) = driver.fs.get_existing_from_path(&path) else {
                continue;
            };
            let parsed = driver.fs.file(unit_file).c().parsed.clone();
            let local = world.generation.new_local_arena();
            let ctx = Ctx {
                local: Some(&local),
                ..global
            };
            let unit_ctx = UnitContext {
                library,
                fragment,
                scopes: &scopes,
                options: Default::default(),
                features: parsed.feature_set,
            };
            let mut ast = parsed.ast.clone();
            let mut tables = ResolutionTables::new();
            let mut rt = ResolverTables::new();
            let mut diagnostics = Vec::new();
            let result = catch_unwind(AssertUnwindSafe(|| {
                dartr_resolver::element_binding_visitor::bind_unit(
                    &ctx,
                    &ast,
                    parsed.unit,
                    fragment,
                    &mut tables,
                    &mut rt,
                );
                dartr_resolver::resolution_visitor::resolve_unit(
                    &ctx,
                    unit_ctx,
                    &mut ast,
                    parsed.unit,
                    &mut tables,
                    &mut rt,
                    &mut diagnostics,
                );
            }));
            units_done += 1;
            lookups += (0..ast.node_count())
                .filter(|&i| {
                    rt.scope_lookup_result
                        .get(dartr_ast::NodeId::from_index(i))
                        .is_some()
                })
                .count();
            declared += (0..ast.node_count())
                .filter(|&i| {
                    tables
                        .declared_fragment
                        .get(dartr_ast::NodeId::from_index(i))
                        .is_some()
                })
                .count();
            annotated += (0..ast.node_count())
                .filter(|&i| {
                    tables
                        .annotation_type
                        .get(dartr_ast::NodeId::from_index(i))
                        .is_some()
                })
                .count();
            reported += diagnostics.len();
            if std::env::var_os("DARTR_C1_DUMP").is_some() {
                let text = std::fs::read_to_string(path.as_ref()).unwrap_or_default();
                let line_starts: Vec<usize> = std::iter::once(0)
                    .chain(text.match_indices('\n').map(|(i, _)| i + 1))
                    .collect();
                for d in &diagnostics {
                    // Offsets are UTF-16; the corpus is ASCII almost
                    // everywhere, which is enough for this comparison.
                    let line = line_starts
                        .iter()
                        .rposition(|&s| s <= d.offset)
                        .unwrap_or(0);
                    let column = d.offset - line_starts[line] + 1;
                    dump.push(format!(
                        "{}|{}|{}|{}",
                        d.code.name.to_uppercase(),
                        path,
                        line + 1,
                        column
                    ));
                }
            }
            // Every declaration node gets a fragment (the walker matched
            // the linked fragments with the nodes).
            use dartr_ast::NodeKind as K;
            for i in 0..ast.node_count() {
                let n = dartr_ast::NodeId::from_index(i);
                let kind = ast.kind(n);
                let is_declaration = matches!(
                    kind,
                    K::ClassDeclaration
                        | K::ClassTypeAlias
                        | K::MixinDeclaration
                        | K::EnumDeclaration
                        | K::ExtensionDeclaration
                        | K::ExtensionTypeDeclaration
                        | K::FunctionTypeAlias
                        | K::GenericTypeAlias
                        | K::ConstructorDeclaration
                        | K::MethodDeclaration
                        | K::FunctionDeclaration
                        | K::VariableDeclaration
                        | K::EnumConstantDeclaration
                        | K::TypeParameter
                        | K::RegularFormalParameter
                        | K::FieldFormalParameter
                        | K::SuperFormalParameter
                        | K::GenericFunctionType
                        | K::FunctionExpression
                        | K::DeclaredIdentifier
                        | K::DeclaredVariablePattern
                        | K::Label
                );
                // The name of the fragment is at the name of the node.
                let name_token = match kind {
                    K::MethodDeclaration => {
                        Some(ast[dartr_ast::Id::<dartr_ast::MethodDeclaration>::from_raw(n)].name)
                    }
                    K::FunctionDeclaration => {
                        Some(ast[dartr_ast::Id::<dartr_ast::FunctionDeclaration>::from_raw(n)].name)
                    }
                    K::VariableDeclaration => {
                        Some(ast[dartr_ast::Id::<dartr_ast::VariableDeclaration>::from_raw(n)].name)
                    }
                    K::TypeParameter => {
                        Some(ast[dartr_ast::Id::<dartr_ast::TypeParameter>::from_raw(n)].name)
                    }
                    K::EnumConstantDeclaration => Some(
                        ast[dartr_ast::Id::<dartr_ast::EnumConstantDeclaration>::from_raw(n)].name,
                    ),
                    K::RegularFormalParameter => {
                        ast[dartr_ast::Id::<dartr_ast::RegularFormalParameter>::from_raw(n)].name
                    }
                    _ => None,
                };
                if let (Some(token), Some(&f)) = (name_token, tables.declared_fragment.get(n)) {
                    let fragment_offset = ctx.fragment_data(f).and_then(|d| d.name_offset);
                    let lexeme = ast.tokens.lexeme(token);
                    if !lexeme.is_empty()
                        && !ast.tokens.get(token).is_synthetic()
                        && fragment_offset != Some(ast.tokens.offset(token))
                    {
                        mismatched.push(format!(
                            "{path}@{}: {kind:?} {lexeme} fragment name offset {fragment_offset:?}",
                            ast.tokens.offset(token)
                        ));
                    }
                }
                if is_declaration
                    && tables.declared_fragment.get(n).is_none()
                    && ast.root(n) == parsed.unit.raw()
                {
                    unbound.push(format!("{path}@{}: {kind:?}", ast.offset(n)));
                }
            }
            if let Err(e) = result {
                let message = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                let at = locations.lock().unwrap().pop().unwrap_or_default();
                failures.push(format!("{path}: {message} at {at}"));
            }
        }
    }
    let _ = std::panic::take_hook();
    eprintln!(
        "c1 corpus: {} libraries, {units_done} units, {} failures; \
         {declared} declared fragments, {annotated} annotation types, \
         {lookups} scope lookups, {reported} diagnostics",
        libraries.len(),
        failures.len()
    );
    if let Some(out) = std::env::var_os("DARTR_C1_DUMP") {
        dump.sort();
        std::fs::write(out, dump.join("\n") + "\n").expect("write dump");
    }
    eprintln!("mismatched name offsets: {}", mismatched.len());
    for m in mismatched.iter().take(30) {
        eprintln!("  {m}");
    }
    eprintln!("unbound declarations: {}", unbound.len());
    for u in unbound.iter().take(30) {
        eprintln!("  {u}");
    }
    for f in failures.iter().take(50) {
        eprintln!("  {f}");
    }
    assert!(failures.is_empty(), "{} units panicked", failures.len());
}
