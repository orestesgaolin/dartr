//! Test support: builds an [`Ast`] from the `ast` dump of the oracle
//! (`tools/oracle`, mode `ast`), without a parser.
//!
//! The dump has the node class names and the child entities in
//! `childEntities` order, but not the property names. The loader assigns the
//! entities to the fields of the node ([`crate::NodeInfo::fields`], the same
//! order) with a backtracking match: a node goes to a node field whose type
//! accepts its kind, a token goes to a token field whose expected lexemes
//! (from the property name: `classKeyword` -> `class`, `leftParenthesis` ->
//! `(`) accept it. Tokens are mapped to the tokens of the scanner output of
//! the same file by offset and type; tokens that the parser made (synthetic
//! tokens, split `>>`) are added to the token arena.
//!
//! Then [`crate::dump`] must give the same text as the oracle, and
//! [`crate::to_source`] the same text as Dart `toSource()`.
//!
//! Limits (the dump does not have the information):
//! - a token that fits two fields: `factory new()` (`new` is the name, the
//!   loader takes it as `newKeyword`), `f(covariant)` (`covariant` is the
//!   name);
//! - the order of a node list that `childEntities` sorted by offset
//!   (recovery: switch members out of order).
//! - nodes that only the resolver makes (`ConstructorReference`,
//!   `ExtensionOverride`, `ImplicitCallReference`, `TypeLiteral`) and the
//!   experimental anonymous methods are not in parser output.

use std::collections::{HashMap, HashSet};

use dartr_syntax::token::flags;
use dartr_syntax::{Token, TokenId, TokenType, scan_for_analyzer};
use serde_json::Value;

use crate::arena::{Ast, NodeId, NodeList};
use crate::generated::children::{FieldValue, build_node};
use crate::generated::nodes::{FieldInfo, FieldKind, NodeKind};
use crate::node_impl::ParameterKind;

/// The result of [`load`].
pub struct Loaded {
    pub ast: Ast,
    pub unit: NodeId,
}

enum Child {
    Node(NodeId),
    Token(TokenId),
}

struct Loader {
    ast: Ast,
    by_offset: HashMap<(u32, u8), TokenId>,
    kinds: HashMap<&'static str, NodeKind>,
    types: HashMap<&'static str, TokenType>,
    keywords: HashSet<&'static str>,
    first: TokenId,
    eof: TokenId,
}

/// Builds the AST of [source] from the oracle `ast` dump [json] (the value
/// of the `"ast"` key).
pub fn load(json: &Value, source: &str) -> Result<Loaded, String> {
    let scan = scan_for_analyzer(source);
    let first = scan.first;
    let tokens = scan.scan.tokens;
    let mut by_offset = HashMap::new();
    let mut eof = first;
    let mut add = |id: TokenId, t: &Token| {
        by_offset.entry((t.offset, t.ty.0)).or_insert(id);
    };
    for id in tokens.iter_from(first) {
        add(id, tokens.get(id));
        for c in tokens.comments(id) {
            add(c, tokens.get(c));
        }
        eof = id;
    }
    let mut types = HashMap::new();
    let mut keywords = HashSet::new();
    for i in 0..=255u8 {
        let ty = TokenType(i);
        let name = ty.name();
        if name.is_empty() {
            continue;
        }
        types.entry(name).or_insert(ty);
        if ty.is_keyword() {
            keywords.insert(ty.lexeme());
        }
    }
    let kinds = NodeKind::ALL.iter().map(|k| (k.name(), *k)).collect();
    let mut loader = Loader {
        ast: Ast::new(tokens),
        by_offset,
        kinds,
        types,
        keywords,
        first,
        eof,
    };
    let unit = loader.node(json)?;
    Ok(Loaded {
        ast: loader.ast,
        unit,
    })
}

fn num(v: &Value, key: &str) -> Result<u32, String> {
    v.get(key)
        .and_then(Value::as_u64)
        .map(|n| n as u32)
        .ok_or_else(|| format!("missing {key}"))
}

/// The lexemes a token field accepts (`None`: any).
fn expected_lexemes(field: &str) -> Option<&'static [&'static str]> {
    Some(match field {
        "leftParenthesis" => &["("],
        "rightParenthesis" => &[")"],
        "semicolon" => &[";"],
        "colon" => &[":"],
        "comma" => &[","],
        "arrow" => &["=>"],
        "atSign" => &["@"],
        "poundSign" => &["#"],
        "star" => &["*"],
        "period" => &[".", "?.", "..", "?.."],
        "question" | "keyQuestion" | "valueQuestion" => &["?"],
        "notOperator" => &["!"],
        "asOperator" | "asToken" => &["as"],
        "isOperator" => &["is"],
        "spreadOperator" => &["...", "...?"],
        "constFinalOrVarKeyword" => &["const", "final", "var"],
        "propertyKeyword" => &["get", "set"],
        "varianceKeyword" => &["in", "out", "inout"],
        "typeKeyword" => &["type"],
        // `ClassTypeAlias.typedefKeyword` is the `class` keyword.
        "typedefKeyword" => &["typedef", "class"],
        "functionDefinition" => &["=>"],
        _ => return None,
    })
}

impl Loader {
    fn token(&mut self, v: &Value) -> Result<TokenId, String> {
        let k = v
            .get("k")
            .and_then(Value::as_str)
            .ok_or("token without k")?;
        let o = num(v, "o")?;
        let l = num(v, "l")?;
        let x = v
            .get("x")
            .and_then(Value::as_str)
            .ok_or("token without x")?;
        let syn = v.get("syn").and_then(Value::as_bool).unwrap_or(false);
        let ty = *self
            .types
            .get(k)
            .ok_or_else(|| format!("unknown token type {k}"))?;
        if let Some(&id) = self.by_offset.get(&(o, ty.0)) {
            let t = self.ast.tokens.get(id);
            if t.length == l && t.is_synthetic() == syn && self.ast.tokens.lexeme(id) == x {
                return Ok(id);
            }
        }
        // A token that the parser made.
        let mut t = Token::fixed(ty, o, 0, syn);
        t.length = l;
        if syn {
            t.flags |= flags::SYNTHETIC;
        }
        let id = self.ast.tokens.push_with_lexeme(t, x);
        self.by_offset.insert((o, ty.0), id);
        Ok(id)
    }

    fn node(&mut self, v: &Value) -> Result<NodeId, String> {
        let t = v.get("t").and_then(Value::as_str).ok_or("node without t")?;
        let kind = *self
            .kinds
            .get(t)
            .ok_or_else(|| format!("unknown node class {t}"))?;
        let mut children = Vec::new();
        for c in v
            .get("c")
            .and_then(Value::as_array)
            .ok_or("node without c")?
        {
            if c.get("t").is_some() {
                children.push(Child::Node(self.node(c)?));
            } else {
                children.push(Child::Token(self.token(c)?));
            }
        }
        let fields = kind.info().fields;
        let values = match kind {
            NodeKind::Comment => {
                let refs: Vec<NodeId> = children
                    .iter()
                    .filter_map(|c| {
                        if let Child::Node(n) = c {
                            Some(*n)
                        } else {
                            None
                        }
                    })
                    .collect();
                let toks: Vec<TokenId> = children
                    .iter()
                    .filter_map(|c| {
                        if let Child::Token(t) = c {
                            Some(*t)
                        } else {
                            None
                        }
                    })
                    .collect();
                let refs = self.ast.new_list(refs.into_iter().map(crate::Id::from_raw));
                vec![
                    FieldValue::NodeList(refs),
                    FieldValue::TokenList(self.ast.new_token_list(toks)),
                    FieldValue::Default,
                ]
            }
            NodeKind::FormalParameterList => {
                // `_childEntities` puts the left delimiter before the first
                // parameter after it.
                let mut toks = Vec::new();
                let mut params = Vec::new();
                for c in &children {
                    match c {
                        Child::Node(n) => params.push(crate::Id::from_raw(*n)),
                        Child::Token(t) => toks.push(*t),
                    }
                }
                let lex = |t: &TokenId| self.ast.tokens.lexeme(*t).to_string();
                let left = toks
                    .iter()
                    .find(|t| matches!(lex(t).as_str(), "[" | "{"))
                    .copied();
                let right = toks
                    .iter()
                    .find(|t| matches!(lex(t).as_str(), "]" | "}"))
                    .copied();
                if toks.len() != 2 + left.is_some() as usize + right.is_some() as usize {
                    return Err(format!(
                        "FormalParameterList: unexpected tokens {}",
                        self.describe(&children)
                    ));
                }
                let params: NodeList<crate::AstNode> = self.ast.new_list(params);
                vec![
                    FieldValue::Token(Some(toks[0])),
                    FieldValue::NodeList(params),
                    FieldValue::Token(left),
                    FieldValue::Token(right),
                    FieldValue::Token(Some(*toks.last().unwrap())),
                ]
            }
            _ => {
                let assignment = self.assign(kind, fields, &children).ok_or_else(|| {
                    format!(
                        "{t}: cannot assign child entities {}",
                        self.describe(&children)
                    )
                })?;
                let mut values = Vec::new();
                for (fi, f) in fields.iter().enumerate() {
                    let taken: Vec<&Child> = assignment
                        .iter()
                        .zip(&children)
                        .filter(|(a, _)| **a == fi)
                        .map(|(_, c)| c)
                        .collect();
                    values.push(match f.kind {
                        FieldKind::Token => {
                            let tok = taken.first().map(|c| match c {
                                Child::Token(t) => *t,
                                Child::Node(_) => unreachable!(),
                            });
                            let tok = match (kind, f.name) {
                                (NodeKind::CompilationUnit, "beginToken") => Some(self.first),
                                (NodeKind::CompilationUnit, "endToken") => Some(self.eof),
                                _ => tok,
                            };
                            FieldValue::Token(tok)
                        }
                        FieldKind::TokenList => {
                            let toks: Vec<TokenId> = taken
                                .iter()
                                .map(|c| match c {
                                    Child::Token(t) => *t,
                                    Child::Node(_) => unreachable!(),
                                })
                                .collect();
                            FieldValue::TokenList(self.ast.new_token_list(toks))
                        }
                        FieldKind::Node => FieldValue::Node(taken.first().map(|c| match c {
                            Child::Node(n) => *n,
                            Child::Token(_) => unreachable!(),
                        })),
                        FieldKind::NodeList => {
                            let nodes: Vec<NodeId> = taken
                                .iter()
                                .map(|c| match c {
                                    Child::Node(n) => *n,
                                    Child::Token(_) => unreachable!(),
                                })
                                .collect();
                            let list: NodeList<crate::AstNode> = self
                                .ast
                                .new_list(nodes.into_iter().map(crate::Id::from_raw));
                            FieldValue::NodeList(list)
                        }
                        FieldKind::Other => FieldValue::Default,
                    });
                }
                values
            }
        };
        let id = build_node(&mut self.ast, kind, &values);
        self.fill_other(kind, id);
        Ok(id)
    }

    /// Sets the values that are not child entities and that the AST
    /// builder computes (literal values, parameter kinds).
    fn fill_other(&mut self, kind: NodeKind, id: NodeId) {
        let ast = &mut self.ast;
        match kind {
            NodeKind::BooleanLiteral => {
                let id = crate::Id::<crate::BooleanLiteral>::from_raw(id);
                let v = ast.tokens.lexeme(ast[id].literal) == "true";
                ast[id].value = v;
            }
            NodeKind::FormalParameterList => {
                // Dart: the kind of a parameter comes from the delimiter
                // group it is in.
                let id = crate::Id::<crate::FormalParameterList>::from_raw(id);
                let n = ast[id].clone();
                let Some(d) = n.left_delimiter else { return };
                let named = ast.tokens.lexeme(d) == "{";
                let d_offset = ast.tokens.offset(d);
                for p in ast.list(n.parameters).to_vec() {
                    if ast.offset(p) < d_offset {
                        continue;
                    }
                    let k = match (named, Self::is_required(ast, p.raw())) {
                        (true, true) => ParameterKind::NamedRequired,
                        (true, false) => ParameterKind::Named,
                        (false, _) => ParameterKind::Positional,
                    };
                    Self::set_parameter_kind(ast, p.raw(), k);
                }
            }
            _ => {}
        }
    }

    fn is_required(ast: &Ast, p: NodeId) -> bool {
        match ast.kind(p) {
            NodeKind::RegularFormalParameter => ast
                [crate::Id::<crate::RegularFormalParameter>::from_raw(p)]
            .required_keyword
            .is_some(),
            NodeKind::FieldFormalParameter => ast
                [crate::Id::<crate::FieldFormalParameter>::from_raw(p)]
            .required_keyword
            .is_some(),
            NodeKind::SuperFormalParameter => ast
                [crate::Id::<crate::SuperFormalParameter>::from_raw(p)]
            .required_keyword
            .is_some(),
            _ => false,
        }
    }

    fn set_parameter_kind(ast: &mut Ast, p: NodeId, k: ParameterKind) {
        match ast.kind(p) {
            NodeKind::RegularFormalParameter => {
                ast[crate::Id::<crate::RegularFormalParameter>::from_raw(p)].kind = k;
            }
            NodeKind::FieldFormalParameter => {
                ast[crate::Id::<crate::FieldFormalParameter>::from_raw(p)].kind = k;
            }
            NodeKind::SuperFormalParameter => {
                ast[crate::Id::<crate::SuperFormalParameter>::from_raw(p)].kind = k;
            }
            _ => {}
        }
    }

    fn describe(&self, children: &[Child]) -> String {
        let parts: Vec<String> = children
            .iter()
            .map(|c| match c {
                Child::Node(n) => self.ast.kind(*n).name().to_string(),
                Child::Token(t) => format!("'{}'", self.ast.tokens.lexeme(*t)),
            })
            .collect();
        format!("[{}]", parts.join(", "))
    }

    fn token_fits(&self, field: &FieldInfo, t: TokenId) -> bool {
        let lexeme = self.ast.tokens.lexeme(t);
        if let Some(expected) = expected_lexemes(field.name) {
            return expected.contains(&lexeme);
        }
        if let Some(word) = field.name.strip_suffix("Keyword") {
            if self.keywords.contains(word) {
                return lexeme == word;
            }
        }
        if field.name.ends_with("Keyword") || field.name == "keyword" {
            return self.ast.tokens.ty(t).is_keyword();
        }
        true
    }

    /// Assigns each child to a field index (backtracking, the first
    /// solution with the longest runs for lists).
    fn assign(
        &self,
        _kind: NodeKind,
        fields: &[FieldInfo],
        children: &[Child],
    ) -> Option<Vec<usize>> {
        let ordered = |relaxed: bool, order: &[usize]| -> Option<Vec<usize>> {
            let reordered: Vec<&Child> = order.iter().map(|&i| &children[i]).collect();
            let mut out = vec![usize::MAX; children.len()];
            let mut failed = HashSet::new();
            if self.assign_from(fields, &reordered, 0, 0, relaxed, &mut out, &mut failed) {
                let mut result = vec![usize::MAX; children.len()];
                for (pos, &ci) in order.iter().enumerate() {
                    result[ci] = out[pos];
                }
                Some(result)
            } else {
                None
            }
        };
        let identity: Vec<usize> = (0..children.len()).collect();
        // `childEntities` sorts the entities by offset when they are out of
        // property order (recovery). The common case: a comment after
        // annotations.
        let mut comment_first = identity.clone();
        if let Some(pos) = children.iter().position(|c| match c {
            Child::Node(n) => self.ast.kind(*n) == NodeKind::Comment,
            _ => false,
        }) {
            comment_first.remove(pos);
            comment_first.insert(0, pos);
        }
        if let Some(r) = ordered(false, &identity) {
            return Some(r);
        }
        if comment_first != identity {
            if let Some(r) = ordered(false, &comment_first) {
                return Some(r);
            }
        }
        if let Some(r) = self.assign_any_order(fields, children) {
            return Some(r);
        }
        // Recovery: a token in a field of another lexeme (`extends` as
        // `MixinOnClause.onKeyword`).
        ordered(true, &identity)
    }

    /// Any order: each entity goes to the first free field that fits.
    fn assign_any_order(&self, fields: &[FieldInfo], children: &[Child]) -> Option<Vec<usize>> {
        let mut out = vec![usize::MAX; children.len()];
        let mut used = vec![false; fields.len()];
        for (ci, c) in children.iter().enumerate() {
            let fi = fields.iter().enumerate().position(|(fi, f)| {
                f.child
                    && self.fits(f, c, false)
                    && (matches!(f.kind, FieldKind::NodeList | FieldKind::TokenList) || !used[fi])
            })?;
            used[fi] = true;
            out[ci] = fi;
        }
        for (fi, f) in fields.iter().enumerate() {
            if f.child
                && !f.nullable
                && matches!(f.kind, FieldKind::Node | FieldKind::Token)
                && !used[fi]
            {
                return None;
            }
        }
        Some(out)
    }

    fn fits(&self, f: &FieldInfo, c: &Child, relaxed: bool) -> bool {
        match (f.kind, c) {
            (FieldKind::Token | FieldKind::TokenList, Child::Token(t)) => {
                relaxed || self.token_fits(f, *t)
            }
            (FieldKind::Node | FieldKind::NodeList, Child::Node(n)) => {
                (f.accepts)(self.ast.kind(*n))
            }
            _ => false,
        }
    }

    fn assign_from(
        &self,
        fields: &[FieldInfo],
        children: &[&Child],
        fi: usize,
        ci: usize,
        relaxed: bool,
        out: &mut Vec<usize>,
        failed: &mut HashSet<(usize, usize)>,
    ) -> bool {
        if fi == fields.len() {
            return ci == children.len();
        }
        if failed.contains(&(fi, ci)) {
            return false;
        }
        let f = &fields[fi];
        let ok = match f.kind {
            FieldKind::Other => {
                self.assign_from(fields, children, fi + 1, ci, relaxed, out, failed)
            }
            _ if !f.child => self.assign_from(fields, children, fi + 1, ci, relaxed, out, failed),
            FieldKind::Token | FieldKind::Node => {
                let take = ci < children.len() && self.fits(f, children[ci], relaxed) && {
                    out[ci] = fi;
                    self.assign_from(fields, children, fi + 1, ci + 1, relaxed, out, failed)
                };
                take || (f.nullable
                    && self.assign_from(fields, children, fi + 1, ci, relaxed, out, failed))
            }
            FieldKind::TokenList | FieldKind::NodeList => {
                let mut n = 0;
                while ci + n < children.len() && self.fits(f, children[ci + n], relaxed) {
                    n += 1;
                }
                let mut found = false;
                for len in (0..=n).rev() {
                    for i in 0..len {
                        out[ci + i] = fi;
                    }
                    if self.assign_from(fields, children, fi + 1, ci + len, relaxed, out, failed) {
                        found = true;
                        break;
                    }
                }
                found
            }
        };
        if !ok {
            failed.insert((fi, ci));
        }
        ok
    }
}

/// The result of [`check`] for one file: `None` when the output is the
/// same as the oracle output, else the first difference.
#[derive(Debug, Default)]
pub struct FileCheck {
    pub dump_diff: Option<String>,
    pub source_diff: Option<String>,
}

impl FileCheck {
    pub fn is_ok(&self) -> bool {
        self.dump_diff.is_none() && self.source_diff.is_none()
    }
}

/// Loads [ast_json] (the raw JSON text of the oracle `ast` value) for
/// [source], then compares the `ast` dump with [ast_json] byte for byte and
/// `to_source` with [expected_source] (oracle `tosource`).
pub fn check(
    ast_json: &str,
    source: &str,
    expected_source: Option<&str>,
) -> Result<FileCheck, String> {
    let mut de = serde_json::Deserializer::from_str(ast_json);
    de.disable_recursion_limit();
    let value: Value = serde::Deserialize::deserialize(&mut de).map_err(|e| e.to_string())?;
    let loaded = load(&value, source)?;
    let dump = crate::dump::node_json(&loaded.ast, loaded.unit);
    let mut result = FileCheck::default();
    if dump != ast_json {
        result.dump_diff = Some(first_difference(ast_json, &dump));
    }
    if let Some(expected) = expected_source {
        let actual = crate::to_source::to_source(&loaded.ast, loaded.unit);
        if actual != expected {
            result.source_diff = Some(first_difference(expected, &actual));
        }
    }
    Ok(result)
}

/// Reads a file like Dart `File.readAsStringSync()` (a leading byte order
/// mark is removed).
pub fn read_source(path: &str) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    Some(dartr_syntax::strip_bom(&text).to_string())
}

/// The first difference of [expected] and [actual], with context.
pub fn first_difference(expected: &str, actual: &str) -> String {
    let i = expected
        .bytes()
        .zip(actual.bytes())
        .take_while(|(x, y)| x == y)
        .count();
    let ctx = |s: &str| {
        let mut lo = i.saturating_sub(150);
        while !s.is_char_boundary(lo) {
            lo -= 1;
        }
        let mut hi = (i + 150).min(s.len());
        while !s.is_char_boundary(hi) {
            hi += 1;
        }
        s[lo..hi].to_string()
    };
    format!(
        "at byte {i}\n  oracle: {}\n  dartr:  {}",
        ctx(expected),
        ctx(actual)
    )
}
