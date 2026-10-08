// Dart source: none (tests of dartr_ast::copy_subtree, the copy of
// expressions into the linker's ConstExprs arena; Dart detaches the nodes,
// see pkg/analyzer/lib/src/summary2/detach_nodes.dart)

//! Copies variable initializers and default values of parsed files into one
//! destination `Ast` and compares each copy with its original: `toSource`,
//! the `ast` dump (with offsets), and the token stream from the begin token
//! to the end token.

use std::path::PathBuf;
use std::sync::Arc;

use dartr_ast::dump::node_json;
use dartr_ast::to_source::to_source;
use dartr_ast::{Ast, AstCopier, AstVisitor, FormalParameterDefaultClause, Id, NodeId};
use dartr_ast::{VariableDeclaration, copy_subtree};
use dartr_syntax::Tokens;

/// Collects the variable initializers and the default values.
#[derive(Default)]
struct Collect {
    roots: Vec<NodeId>,
}

impl AstVisitor for Collect {
    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        if let Some(i) = ast[node].initializer {
            self.roots.push(i.raw());
        }
        self.visit_node(ast, node.raw());
    }

    fn visit_formal_parameter_default_clause(
        &mut self,
        ast: &Ast,
        node: Id<FormalParameterDefaultClause>,
    ) {
        self.roots.push(ast[node].value.raw());
        self.visit_node(ast, node.raw());
    }
}

fn roots(ast: &Ast, unit: NodeId) -> Vec<NodeId> {
    let mut c = Collect::default();
    ast.accept(unit, &mut c);
    c.roots
}

/// The lexemes from the begin token to the end token of [node].
fn token_lexemes(ast: &Ast, node: NodeId) -> Vec<String> {
    let end = ast.end_token(node);
    let mut out = Vec::new();
    for t in ast.tokens.iter_from(ast.begin_token(node)) {
        out.push(ast.tokens.lexeme(t).to_string());
        if t == end {
            return out;
        }
    }
    panic!("end token not reached");
}

/// The text that is compared for one node.
fn fingerprint(ast: &Ast, node: NodeId) -> (String, String, Vec<String>) {
    (
        to_source(ast, node),
        node_json(ast, node),
        token_lexemes(ast, node),
    )
}

/// An empty destination: its source text is empty, so every copied lexeme
/// must be stored with the token.
fn destination() -> Ast {
    Ast::new(Tokens::new(Arc::from("")))
}

const SOURCE: &str = r#"
class A {
  final int x;
  final String? s;
  const A(this.x, {this.s});
  const A.named({int x = 1, String s = 'a'}) : this(x, s: s);
}

enum E { one, two }

const a = A(1, s: 'b');
const b = A.named(x: 2, s: "c${1 + 2}d");
const c = <int>[1, 2, if (true) 3, ...[4]];
const d = <String, int>{'a': 1, 'b': 2};
const e = <double>{1.0, 2.5};
const f = 'x' 'y' "z$a" r'raw';
final g = (int x, {int y = 0}) => x + y;
const h = 1 as num;
const i = a is A;
const j = 1 > 2 ? 'yes' : 'no';
const k = (1, b: 'two', 3.0);
const E l = .one;
const m = E.two;
const n = -(1 + 2) * 3 ~/ 4;
const o = null ?? 'default';
const p = #sym;
const q = [for (var i in [1, 2]) i * 2];
var r = () async {
  await null;
  return 1;
};
const s = A.new;
const t = identical(1, 1);

void fn(int a, [int b = 2, List<int> c = const <int>[1]]) {}
void fn2({E e = .two, Object o = const A(3)}) {}
"#;

#[test]
fn copy_initializers_and_default_values() {
    let parsed = dartr_ast_builder::parse_string(SOURCE, "/test.dart");
    assert!(
        parsed.diagnostics.is_empty(),
        "diagnostics: {:?}",
        parsed.diagnostics
    );
    let src = &parsed.ast;
    let originals = roots(src, parsed.unit.raw());
    // 20 top-level initializers; default values: 2 in `A.named`, 2 in
    // `fn`, 2 in `fn2`, 1 in `g`.
    assert_eq!(
        originals.len(),
        20 + 2 + 4 + 1,
        "roots: {}",
        originals.len()
    );

    let mut dst = destination();
    let mut copies = Vec::new();
    for &node in &originals {
        let copy = copy_subtree(src, node, &mut dst);
        assert_eq!(dst.parent(copy), None);
        assert_eq!(dst.kind(copy), src.kind(node));
        copies.push(copy);
    }
    // Check after all copies, so that a later copy cannot change an earlier
    // one.
    for (&node, &copy) in originals.iter().zip(&copies) {
        let expected = fingerprint(src, node);
        let actual = fingerprint(&dst, copy);
        assert_eq!(actual, expected);
        // The copy is detached: its tokens start and end in the copy.
        assert!(dst.tokens.previous(dst.begin_token(copy)).is_none());
        assert!(dst.tokens.next(dst.end_token(copy)).is_none());
    }
    // A sample of the text, so that the test shows what is compared.
    let text: Vec<String> = copies.iter().map(|&c| to_source(&dst, c)).collect();
    assert!(
        text.contains(&"A.named(x: 2, s: \"c${1 + 2}d\")".to_string()),
        "{text:#?}"
    );
    assert!(
        text.contains(&"'x' 'y' \"z$a\" r'raw'".to_string()),
        "{text:#?}"
    );
    assert!(text.contains(&".one".to_string()), "{text:#?}");
    assert!(text.contains(&"const A(3)".to_string()), "{text:#?}");
}

#[test]
fn copier_parents_and_reuse() {
    let parsed = dartr_ast_builder::parse_string("const x = [1, (2, 3)];\n", "/t.dart");
    let src = &parsed.ast;
    let root = roots(src, parsed.unit.raw())[0];
    let mut dst = destination();
    let mut copier = AstCopier::new(src, &mut dst);
    let first = copier.copy(root);
    let second = copier.copy(root);
    assert_ne!(first, second);
    // Every node of the copy has its parent in the copy.
    fn check(ast: &Ast, node: NodeId) -> usize {
        let mut n = 1;
        for c in ast.children(node) {
            assert_eq!(ast.parent(c), Some(node));
            n += check(ast, c);
        }
        n
    }
    assert_eq!(check(&dst, first), check(&dst, second));
    assert_eq!(check(&dst, first), check(src, root));
    // The two copies do not share tokens.
    assert_ne!(dst.begin_token(first), dst.begin_token(second));
    assert_eq!(to_source(&dst, first), "[1, (2, 3)]");
}

/// Copies every variable initializer and default value of the SDK library
/// sources into one destination per file and compares them. Run with
/// `cargo test -p dartr_ast_builder --test copy -- --ignored --nocapture`.
/// `DARTR_SDK_LIB` overrides the directory.
#[test]
#[ignore]
fn copy_sdk_lib() {
    let dir = std::env::var_os("DARTR_SDK_LIB")
        .map(PathBuf::from)
        .unwrap_or_else(|| dartr_difftest::repo_root().join("third_party/dart-sdk/sdk/lib"));
    let files = dartr_difftest::collect_dart_files(&[dir]).expect("sdk files");
    assert!(!files.is_empty());
    let mut count = 0usize;
    let mut tokens = 0usize;
    let mut failures = Vec::new();
    for path in &files {
        let content = std::fs::read_to_string(path).unwrap();
        let parsed = dartr_ast_builder::parse_string(&content, path);
        let src = &parsed.ast;
        let mut dst = destination();
        for node in roots(src, parsed.unit.raw()) {
            let copy = copy_subtree(src, node, &mut dst);
            count += 1;
            if fingerprint(src, node) != fingerprint(&dst, copy) {
                failures.push(format!("{path}: {}", to_source(src, node)));
            }
        }
        tokens += dst.tokens.len();
    }
    println!(
        "files: {}, copied roots: {count}, copied tokens: {tokens}, failures: {}",
        files.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:#?}");
}
