// Dart source: pkg/analyzer/test/id_tests/assigned_variables_test.dart (the
// id test of `FlowAnalysisHelper.computeAssignedVariables` over the data
// files in pkg/_fe_analyzer_shared/test/flow_analysis/assigned_variables/
// data), pkg/analyzer/lib/src/util/ast_data_extractor.dart (node offsets).

//! Tests of the flow analysis glue of the resolver over parsed Dart code:
//!
//! - [`assigned_variables_data`] runs the Dart id test data of the assigned
//!   variables pre-pass: every `/*member: name:data*/` and `/*data*/`
//!   annotation in the data files is compared with the result of
//!   [`compute_assigned_variables`], and every node with assigned or
//!   captured variables must have an annotation.
//! - [`label_targets`]: `getLabelTarget` of `break` and `continue`.
//!
//! The resolution visitor (another unit) sets the elements of declarations
//! and identifiers. Here a small binder does that: it creates the elements
//! of the formal parameters, local variables, local functions, closures and
//! labels, and binds each `SimpleIdentifier` / `LabelReference` to the last
//! declaration with the same name in the member. The data files use unique
//! names in each member, so the binder needs no scopes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dartr_ast::{
    Ast, AstVisitor, BinaryExpression, CatchClauseParameter, ClassDeclaration, CompilationUnit,
    ConditionalExpression, ConstructorDeclaration, DeclaredIdentifier, DeclaredVariablePattern,
    Expression, FieldDeclaration, FieldFormalParameter, ForElement, FormalParameter,
    FunctionDeclaration, FunctionDeclarationStatement, FunctionExpression, Id, IfElement,
    IfStatement, Label, LabelReference, MapLiteralEntry, MethodDeclaration, NameWithTypeParameters,
    NodeId, NodeKind, NullAwareElement, RegularFormalParameter, SimpleIdentifier, SpreadElement,
    Statement, SuperFormalParameter, SwitchExpressionCase, SwitchMember, SwitchStatement,
    TopLevelVariableDeclaration, VariableDeclaration, VariableDeclarationList,
};
use dartr_element::{
    Ctx, EId, ElemRef, ElementData, ElementId, ElementStore, ExecutableElementData,
    ExecutableFragmentData, FId, FormalParameterElement, FormalParameterFragment, FragmentData,
    FragmentFlags, FragmentId, LabelElement, LabelFragment, LocalFunctionElement,
    LocalFunctionFragment, LocalVariableElement, LocalVariableFragment, ParameterKind,
    PatternVariableFragment, PromotableElement, ResolutionTables, Tag, TypeId, VarSlot,
    VariableElementData, VariableFragmentData,
};
use dartr_flow::flow_analysis::PromotionKey;
use dartr_resolver::flow_analysis_visitor::{
    ResolverAssignedVariables, compute_assigned_variables, get_label_target, pattern_variables,
};
use dartr_typesystem::test_support::TypeSystemTest;
use indexmap::IndexSet;

// ================================================================ binder

/// Creates the elements of the local declarations of one member and binds
/// the identifiers (a stand-in for the resolution visitor).
struct Binder<'b, 'c> {
    ctx: &'b Ctx<'c>,
    store: &'b ElementStore,
    tables: &'b mut ResolutionTables,
    /// The declared variables and labels, by name, in declaration order.
    names: Vec<(String, ElementId)>,
    /// The formal parameters of the enclosing executables; the first frame
    /// is the member.
    frames: Vec<Vec<EId<FormalParameterElement>>>,
    /// The label references, bound in [Binder::finish] (a `continue` can
    /// refer to the label of a later switch member).
    label_references: Vec<(NodeId, String)>,
}

impl Binder<'_, '_> {
    fn fragment_data(&self, name: &str, offset: u32) -> FragmentData {
        FragmentData::new(Some(self.ctx.name(name)), Some(offset))
    }

    fn element_data(&self, name: &str, fragment: FragmentId) -> ElementData {
        ElementData::new(Some(self.ctx.name(name)), fragment)
    }

    fn formal_parameter(
        &mut self,
        ast: &Ast,
        node: NodeId,
        name: Option<dartr_syntax::TokenId>,
        tag: Tag,
    ) {
        let Some(name) = name else { return };
        let name = ast.tokens.lexeme(name).to_string();
        let fd = self.fragment_data(&name, ast.offset(node));
        let fragment: FId<FormalParameterFragment> =
            self.store.add_fragment(FormalParameterFragment {
                variable: VariableFragmentData::new(fd),
                parameter_kind: ParameterKind::Required,
                private_name: None,
            });
        // Field and super formal parameters differ by tag only.
        let fragment = FragmentId::new(fragment.store(), tag, fragment.index());
        let element: EId<FormalParameterElement> = self.store.add(FormalParameterElement {
            variable: VariableElementData::new(self.element_data(&name, fragment)),
            kind: ParameterKind::Required,
            type_: VarSlot::with(TypeId::DYNAMIC),
            base_formal_parameter: None,
            field: VarSlot::new(),
        });
        let element_id = ElementId::new(element.store(), tag, element.index());
        self.ctx
            .fragment_data(fragment)
            .unwrap()
            .element
            .set_once(element_id);
        self.tables.declared_fragment.insert(node, fragment);
        self.frames
            .last_mut()
            .unwrap()
            .push(EId::from_raw(element_id));
        self.names.push((name, element_id));
    }

    fn local_variable(
        &mut self,
        ast: &Ast,
        node: NodeId,
        name: dartr_syntax::TokenId,
        flags: FragmentFlags,
    ) {
        let name = ast.tokens.lexeme(name).to_string();
        let fd = self.fragment_data(&name, ast.offset(node));
        fd.flags.set(flags, true);
        let fragment: FId<LocalVariableFragment> = self.store.add_fragment(LocalVariableFragment {
            variable: VariableFragmentData::new(fd),
            pattern: Default::default(),
        });
        let element: EId<LocalVariableElement> = self.store.add(LocalVariableElement {
            variable: VariableElementData::new(self.element_data(&name, fragment.raw())),
            type_: VarSlot::with(TypeId::DYNAMIC),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        self.tables.declared_fragment.insert(node, fragment.raw());
        self.names.push((name, element.raw()));
    }

    /// A bind pattern variable (Dart `BindPatternVariableElementImpl`).
    fn pattern_variable(&mut self, ast: &Ast, node: NodeId, name: dartr_syntax::TokenId) {
        let name = ast.tokens.lexeme(name).to_string();
        let element = add_pattern_variable(
            self.ctx,
            self.store,
            &name,
            ast.offset(node),
            Tag::BindPatternVariable,
        );
        let fragment = self.ctx.element_data(element).unwrap().first_fragment;
        self.tables.declared_fragment.insert(node, fragment);
        self.names.push((name, element));
    }

    /// A closure or a local function: the parameters go to a new frame.
    fn local_function(&mut self, ast: &Ast, node: NodeId) {
        self.frames.push(Vec::new());
        ast.visit_children(node, self);
        let parameters = self.frames.pop().unwrap();
        let fd = FragmentData::new(None, Some(ast.offset(node)));
        let fragment: FId<LocalFunctionFragment> = self.store.add_fragment(LocalFunctionFragment {
            executable: ExecutableFragmentData::new(fd),
        });
        let mut executable = ExecutableElementData::new(ElementData::new(None, fragment.raw()));
        executable.formal_params = parameters;
        let element: EId<LocalFunctionElement> =
            self.store.add(LocalFunctionElement { executable });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        self.tables.declared_fragment.insert(node, fragment.raw());
    }

    /// Binds the label references.
    fn finish(&mut self) {
        for (node, name) in std::mem::take(&mut self.label_references) {
            self.bind(node, &name);
        }
    }

    fn bind(&mut self, node: NodeId, name: &str) {
        if let Some((_, element)) = self.names.iter().rev().find(|(n, _)| n == name) {
            self.tables.element.insert(node, ElemRef::Base(*element));
        }
    }
}

impl AstVisitor for Binder<'_, '_> {
    fn visit_regular_formal_parameter(&mut self, ast: &Ast, node: Id<RegularFormalParameter>) {
        self.formal_parameter(ast, node.raw(), ast[node].name, Tag::FormalParameter);
        ast.visit_children(node, self);
    }

    fn visit_field_formal_parameter(&mut self, ast: &Ast, node: Id<FieldFormalParameter>) {
        self.formal_parameter(
            ast,
            node.raw(),
            Some(ast[node].name),
            Tag::FieldFormalParameter,
        );
        ast.visit_children(node, self);
    }

    fn visit_super_formal_parameter(&mut self, ast: &Ast, node: Id<SuperFormalParameter>) {
        self.formal_parameter(
            ast,
            node.raw(),
            Some(ast[node].name),
            Tag::SuperFormalParameter,
        );
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let list: Id<VariableDeclarationList> = ast.cast(ast.parent(node).unwrap()).unwrap();
        let grand_parent = ast.parent(list).unwrap();
        let is_member = ast.is::<TopLevelVariableDeclaration>(grand_parent)
            || ast.is::<FieldDeclaration>(grand_parent);
        if !is_member {
            let mut flags = FragmentFlags::EMPTY;
            if ast[list].late_keyword.is_some() {
                flags = flags | FragmentFlags::VARIABLE_FRAGMENT_IS_LATE;
            }
            match ast[list].keyword.map(|k| ast.tokens.lexeme(k)) {
                Some("final") => flags = flags | FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL,
                Some("const") => flags = flags | FragmentFlags::VARIABLE_FRAGMENT_IS_CONST,
                _ => {}
            }
            self.local_variable(ast, node.raw(), ast[node].name, flags);
        }
        ast.visit_children(node, self);
    }

    fn visit_declared_identifier(&mut self, ast: &Ast, node: Id<DeclaredIdentifier>) {
        self.local_variable(ast, node.raw(), ast[node].name, FragmentFlags::EMPTY);
        ast.visit_children(node, self);
    }

    fn visit_declared_variable_pattern(&mut self, ast: &Ast, node: Id<DeclaredVariablePattern>) {
        self.pattern_variable(ast, node.raw(), ast[node].name);
        ast.visit_children(node, self);
    }

    fn visit_catch_clause_parameter(&mut self, ast: &Ast, node: Id<CatchClauseParameter>) {
        self.local_variable(ast, node.raw(), ast[node].name, FragmentFlags::EMPTY);
    }

    fn visit_function_expression(&mut self, ast: &Ast, node: Id<FunctionExpression>) {
        if ast
            .parent(node)
            .is_some_and(|p| ast.is::<FunctionDeclaration>(p))
        {
            ast.visit_children(node, self);
        } else {
            self.local_function(ast, node.raw());
        }
    }

    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        if ast
            .parent(node)
            .is_some_and(|p| ast.is::<CompilationUnit>(p))
        {
            ast.visit_children(node, self);
        } else {
            self.local_function(ast, node.raw());
        }
    }

    fn visit_label(&mut self, ast: &Ast, node: Id<Label>) {
        let name = ast.tokens.lexeme(ast[node].name).to_string();
        let fd = self.fragment_data(&name, ast.offset(node));
        let fragment: FId<LabelFragment> = self.store.add_fragment(LabelFragment {
            fragment: fd,
            on_switch_member: ast.parent(node).is_some_and(|p| ast.is::<SwitchMember>(p)),
        });
        let element: EId<LabelElement> = self.store.add(LabelElement {
            element: self.element_data(&name, fragment.raw()),
        });
        self.store
            .fragment(fragment)
            .element
            .set_once(element.raw());
        self.tables.declared_fragment.insert(node, fragment.raw());
        self.names.push((format!("label {name}"), element.raw()));
    }

    fn visit_label_reference(&mut self, ast: &Ast, node: Id<LabelReference>) {
        let name = format!("label {}", ast.tokens.lexeme(ast[node].name));
        self.label_references.push((node.raw(), name));
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        let name = ast.tokens.lexeme(ast[node].token).to_string();
        self.bind(node.raw(), &name);
    }
}

/// Adds a pattern variable element (data `LocalVariableElement`, tag
/// [tag]: `BindPatternVariable` or `JoinPatternVariable`).
fn add_pattern_variable(
    ctx: &Ctx<'_>,
    store: &ElementStore,
    name: &str,
    offset: u32,
    tag: Tag,
) -> ElementId {
    let fd = FragmentData::new(Some(ctx.name(name)), Some(offset));
    let fragment: FId<LocalVariableFragment> = store.add_fragment(LocalVariableFragment {
        variable: VariableFragmentData::new(fd),
        pattern: Default::default(),
    });
    let fragment = FragmentId::new(fragment.store(), tag, fragment.index());
    let element: EId<LocalVariableElement> = store.add(LocalVariableElement {
        variable: VariableElementData::new(ElementData::new(Some(ctx.name(name)), fragment)),
        type_: VarSlot::with(TypeId::DYNAMIC),
    });
    let element = ElementId::new(element.store(), tag, element.index());
    ctx.fragment_data(fragment)
        .unwrap()
        .element
        .set_once(element);
    element
}

/// Dart `JoinPatternVariableFragmentImpl(variables: components)`: adds a
/// join variable named [name] and links [components] to it (`join`).
fn join(ctx: &Ctx<'_>, store: &ElementStore, name: &str, components: &[ElementId]) -> ElementId {
    let join = add_pattern_variable(ctx, store, name, 0, Tag::JoinPatternVariable);
    let join_fragment = ctx.element_data(join).unwrap().first_fragment;
    for &component in components {
        let fragment = ctx.element_data(component).unwrap().first_fragment;
        let fragment = fragment.cast::<PatternVariableFragment>().unwrap();
        ctx.fragment(fragment)
            .pattern
            .join
            .set(Some(join_fragment.cast().unwrap()));
    }
    join
}

/// A member of a unit: a top-level function, a method, a constructor, or
/// a top-level variable or field with an initializer.
struct Member {
    /// Dart `computeMemberId`: `name` or `Class.name`.
    id: String,
    node: NodeId,
    /// Whether the member has formal parameters (Dart
    /// `element.formalParameters`; `null` for variables).
    has_parameters: bool,
}

fn class_name(ast: &Ast, node: NodeId) -> Option<String> {
    let class = ast.this_or_ancestor_of_type::<ClassDeclaration>(node)?;
    let name_part = ast.cast::<NameWithTypeParameters>(ast[class].name_part)?;
    Some(ast.tokens.lexeme(ast[name_part].type_name).to_string())
}

fn members(ast: &Ast, root: NodeId) -> Vec<Member> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let qualified = |name: &str| match class_name(ast, node) {
            Some(class) => format!("{class}.{name}"),
            None => name.to_string(),
        };
        if let Some(f) = ast.cast::<FunctionDeclaration>(node) {
            if ast
                .parent(node)
                .is_some_and(|p| ast.is::<CompilationUnit>(p))
            {
                let id = qualified(ast.tokens.lexeme(ast[f].name));
                out.push(Member {
                    id,
                    node,
                    has_parameters: true,
                });
            }
            continue;
        }
        if let Some(m) = ast.cast::<MethodDeclaration>(node) {
            let id = qualified(ast.tokens.lexeme(ast[m].name));
            out.push(Member {
                id,
                node,
                has_parameters: true,
            });
            continue;
        }
        if let Some(c) = ast.cast::<ConstructorDeclaration>(node) {
            let name = ast[c].name.map_or("", |n| ast.tokens.lexeme(n));
            out.push(Member {
                id: qualified(name),
                node,
                has_parameters: true,
            });
            continue;
        }
        if let Some(v) = ast.cast::<VariableDeclaration>(node) {
            if ast[v].initializer.is_some() {
                let id = qualified(ast.tokens.lexeme(ast[v].name));
                out.push(Member {
                    id,
                    node,
                    has_parameters: false,
                });
            }
            continue;
        }
        let mut children = ast.children(node);
        children.reverse();
        stack.extend(children);
    }
    out
}

// ================================================================ data

/// Dart `_Data` of the id test.
struct Data {
    declared: Vec<String>,
    read: Vec<String>,
    read_captured: Vec<String>,
    assigned: Vec<String>,
    captured: Vec<String>,
}

impl Data {
    fn new(
        av: &ResolverAssignedVariables,
        ctx: &Ctx<'_>,
        sets: [&IndexSet<PromotionKey>; 5],
    ) -> Data {
        let names = |set: &IndexSet<PromotionKey>| {
            let mut names: Vec<String> = set
                .iter()
                .map(|&key| {
                    let variable: EId<PromotableElement> = av.variable_for_key(key);
                    let name = ctx.element_data(variable.raw()).unwrap().name.unwrap();
                    ctx.name_str(name).to_string()
                })
                .collect();
            names.sort();
            names
        };
        let [declared, read, read_captured, assigned, captured] = sets;
        Data {
            declared: names(declared),
            read: names(read),
            read_captured: names(read_captured),
            assigned: names(assigned),
            captured: names(captured),
        }
    }

    /// Dart `_AssignedVariablesDataInterpreter.isEmpty`.
    fn is_empty(&self) -> bool {
        self.assigned.is_empty() && self.captured.is_empty()
    }

    /// Dart `_AssignedVariablesDataInterpreter.getText`.
    fn text(&self) -> String {
        let set = |values: &[String]| format!("{{{}}}", values.join(", "));
        let mut parts = Vec::new();
        if !self.declared.is_empty() {
            parts.push(format!("declared={}", set(&self.declared)));
        }
        if !self.read.is_empty() {
            parts.push(format!("read={}", set(&self.read)));
        }
        if !self.read_captured.is_empty() {
            // Dart prints this set as `read=` too.
            parts.push(format!("read={}", set(&self.read_captured)));
        }
        if !self.assigned.is_empty() {
            parts.push(format!("assigned={}", set(&self.assigned)));
        }
        if !self.captured.is_empty() {
            parts.push(format!("captured={}", set(&self.captured)));
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Dart `AstDataExtractor._nodeOffset`.
fn node_offset(ast: &Ast, node: NodeId) -> u32 {
    if let Some(n) = ast.cast::<ConditionalExpression>(node) {
        ast.tokens.offset(ast[n].question)
    } else if let Some(n) = ast.cast::<BinaryExpression>(node) {
        ast.tokens.offset(ast[n].operator)
    } else if let Some(n) = ast.cast::<SwitchExpressionCase>(node) {
        ast.tokens.offset(ast[n].arrow)
    } else {
        ast.offset(node)
    }
}

/// Whether the id test registers a value for [node] by its offset (Dart
/// `AstDataExtractor`: expressions, statements other than expression
/// statements, collection elements, ...).
fn is_registered(ast: &Ast, node: NodeId) -> bool {
    ast.is::<Expression>(node)
        || (ast.is::<Statement>(node) && ast.kind(node) != NodeKind::ExpressionStatement)
        || ast.is::<ForElement>(node)
        || ast.is::<IfElement>(node)
        || ast.is::<FormalParameter>(node)
        || ast.is::<MapLiteralEntry>(node)
        || ast.is::<NullAwareElement>(node)
        || ast.is::<SpreadElement>(node)
        || ast.is::<SwitchExpressionCase>(node)
        || ast.is::<SwitchMember>(node)
        || ast.is::<VariableDeclaration>(node)
}

/// The annotations of a data file: member annotations by member id, node
/// annotations by the offset of the token after the comment.
#[derive(Default)]
struct Annotations {
    members: BTreeMap<String, String>,
    nodes: BTreeMap<u32, String>,
}

fn parse_annotations(source: &str) -> Annotations {
    let mut annotations = Annotations::default();
    let mut rest = 0;
    while let Some(start) = source[rest..].find("/*").map(|i| i + rest) {
        // Skip `/*` inside a line comment.
        let line_start = source[..start].rfind('\n').map_or(0, |i| i + 1);
        let end = source[start..]
            .find("*/")
            .map(|i| i + start)
            .expect("unterminated comment");
        rest = end + 2;
        if source[line_start..start].contains("//") {
            continue;
        }
        let text = source[start + 2..end].trim();
        if let Some(member) = text.strip_prefix("member: ") {
            let (id, data) = member.split_once(':').expect("member annotation");
            annotations
                .members
                .insert(id.trim().to_string(), data.trim().to_string());
        } else {
            let next = source[rest..]
                .find(|c: char| !c.is_whitespace())
                .map(|i| i + rest)
                .expect("annotation at the end of the file");
            annotations.nodes.insert(next as u32, text.to_string());
        }
    }
    annotations
}

/// Runs the id test of one data file; returns the mismatches.
fn check_file(path: &PathBuf) -> Vec<String> {
    let source = std::fs::read_to_string(path).unwrap();
    let file = path.file_name().unwrap().to_string_lossy().to_string();
    let parsed = dartr_ast_builder::parse_string(&source, &file);
    assert!(parsed.diagnostics.is_empty(), "{file}: parse errors");
    let ast = &parsed.ast;
    let test = TypeSystemTest::new();
    let ctx = test.ctx();
    let annotations = parse_annotations(&source);

    let mut actual_members = BTreeMap::new();
    let mut actual_nodes: BTreeMap<u32, (String, bool)> = BTreeMap::new();
    for member in members(ast, parsed.unit.raw()) {
        let mut tables = ResolutionTables::new();
        let mut binder = Binder {
            ctx: &ctx,
            store: &test.store,
            tables: &mut tables,
            names: Vec::new(),
            frames: vec![Vec::new()],
            label_references: Vec::new(),
        };
        ast.visit_children(member.node, &mut binder);
        binder.finish();
        let parameters = binder.frames.pop().unwrap();
        let parameters = member.has_parameters.then_some(parameters.as_slice());
        let av = compute_assigned_variables(ast, &tables, &ctx, member.node, parameters, None);

        let data = Data::new(
            &av,
            &ctx,
            [
                av.declared_at_top_level(),
                av.read_anywhere(),
                av.read_captured_anywhere(),
                av.written_anywhere(),
                av.captured_anywhere(),
            ],
        );
        actual_members.insert(member.id.clone(), (data.text(), data.is_empty()));

        let mut stack = ast.children(member.node);
        while let Some(node) = stack.pop() {
            stack.extend(ast.children(node));
            if !is_registered(ast, node) {
                continue;
            }
            // Dart `computeNodeValue`: a local function statement answers
            // for its function declaration.
            let key = match ast.cast::<FunctionDeclarationStatement>(node) {
                Some(s) => ast[s].function_declaration.raw(),
                None => node,
            };
            if !av.is_tracked(key) {
                continue;
            }
            let data = Data::new(
                &av,
                &ctx,
                [
                    av.declared_in_node(key),
                    av.read_in_node(key),
                    av.read_captured_in_node(key),
                    av.written_in_node(key),
                    av.captured_in_node(key),
                ],
            );
            let offset = node_offset(ast, node);
            let previous = actual_nodes.insert(offset, (data.text(), data.is_empty()));
            assert!(
                previous.is_none(),
                "{file}: two tracked nodes at offset {offset}"
            );
        }
    }

    let mut failures = Vec::new();
    for (id, (text, is_empty)) in &actual_members {
        match annotations.members.get(id) {
            Some(expected) if expected != text => {
                failures.push(format!(
                    "{file}: member {id}: expected \"{expected}\", got \"{text}\""
                ));
            }
            None if !is_empty => {
                failures.push(format!("{file}: member {id}: no annotation for \"{text}\""));
            }
            _ => {}
        }
    }
    for id in annotations.members.keys() {
        if !actual_members.contains_key(id) {
            failures.push(format!("{file}: member {id}: annotation without a member"));
        }
    }
    for (offset, (text, is_empty)) in &actual_nodes {
        match annotations.nodes.get(offset) {
            Some(expected) if expected != text => {
                failures.push(format!(
                    "{file}@{offset}: expected \"{expected}\", got \"{text}\""
                ));
            }
            None if !is_empty => {
                failures.push(format!("{file}@{offset}: no annotation for \"{text}\""));
            }
            _ => {}
        }
    }
    for (offset, expected) in &annotations.nodes {
        if !actual_nodes.contains_key(offset) {
            failures.push(format!(
                "{file}@{offset}: no tracked node for \"{expected}\""
            ));
        }
    }
    failures
}

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/dart-sdk/pkg/_fe_analyzer_shared/test/flow_analysis/assigned_variables/data")
}

#[test]
fn assigned_variables_data() {
    let mut files: Vec<PathBuf> = std::fs::read_dir(data_dir())
        .expect("the Dart SDK sources in third_party")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "dart"))
        .collect();
    files.sort();
    assert!(files.len() >= 20, "data files: {files:?}");
    let failures: Vec<String> = files.iter().flat_map(check_file).collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ================================================================ labels

/// The `break` / `continue` statements of [source] with the kind and offset
/// of the target statement of each (Dart `getLabelTarget`).
fn label_target_lines(source: &str) -> Vec<String> {
    let parsed = dartr_ast_builder::parse_string(source, "test.dart");
    assert!(parsed.diagnostics.is_empty(), "parse errors");
    let ast = &parsed.ast;
    let test = TypeSystemTest::new();
    let ctx = test.ctx();
    let mut tables = ResolutionTables::new();
    let mut binder = Binder {
        ctx: &ctx,
        store: &test.store,
        tables: &mut tables,
        names: Vec::new(),
        frames: vec![Vec::new()],
        label_references: Vec::new(),
    };
    ast.accept(parsed.unit, &mut binder);
    binder.finish();

    let mut statements = Vec::new();
    let mut stack = vec![parsed.unit.raw()];
    while let Some(node) = stack.pop() {
        let mut children = ast.children(node);
        children.reverse();
        stack.extend(children);
        let (is_break, label) = match ast.kind(node) {
            NodeKind::BreakStatement => (
                true,
                ast[ast.cast::<dartr_ast::BreakStatement>(node).unwrap()].label,
            ),
            NodeKind::ContinueStatement => (
                false,
                ast[ast.cast::<dartr_ast::ContinueStatement>(node).unwrap()].label,
            ),
            _ => continue,
        };
        statements.push((node, is_break, label));
    }
    statements
        .into_iter()
        .map(|(node, is_break, label)| {
            let element = label.and_then(|l| match tables.element.get(l) {
                Some(ElemRef::Base(e)) => Some(*e),
                _ => None,
            });
            let target = get_label_target(ast, &tables, &ctx, node, element, is_break);
            let statement = ast.tokens.lexeme(ast.begin_token(node)).to_string();
            let label = label.map_or(String::new(), |l| {
                format!(" {}", ast.tokens.lexeme(ast[l].name))
            });
            let target = match target {
                Some(t) => format!("{:?}@{}", ast.kind(t), ast.offset(t)),
                None => "null".to_string(),
            };
            format!("{statement}{label}@{} -> {target}", ast.offset(node))
        })
        .collect()
}

#[test]
fn label_targets() {
    let source = r#"
f(bool c) {
  while (c) {
    break;
  }
  do {
    continue;
  } while (c);
  outer: for (;;) {
    for (var i in []) {
      break outer;
    }
    switch (c) {
      case true:
        continue outer;
      case false:
        break;
    }
  }
  block: {
    break block;
  }
  switch (c) {
    case true:
      continue next;
    next:
    case false:
      break;
  }
  for (;;) {
    switch (c) {
      default:
        continue;
    }
  }
}
"#;
    let offset = |needle: &str| source.find(needle).unwrap();
    let while_ = offset("while (c) {");
    let do_ = offset("do {");
    let for_outer = offset("for (;;) {\n    for");
    let switch_inner = offset("switch (c) {\n      case true:\n        continue outer");
    let block = offset("block: {");
    let switch_next = offset("switch (c) {\n    case true:\n      continue next");
    let for_last = offset("for (;;) {\n    switch");
    let at = |needle: &str, from: usize| source[from..].find(needle).unwrap() + from;

    let expected = vec![
        format!("break@{} -> WhileStatement@{while_}", at("break;", while_)),
        format!("continue@{} -> DoStatement@{do_}", at("continue;", do_)),
        // A labeled loop: the loop itself.
        format!(
            "break outer@{} -> ForStatement@{for_outer}",
            at("break outer", 0)
        ),
        // `continue outer` from a switch inside the labeled loop.
        format!(
            "continue outer@{} -> ForStatement@{for_outer}",
            at("continue outer", 0)
        ),
        // An unlabeled `break` in a switch: the switch.
        format!(
            "break@{} -> SwitchStatement@{switch_inner}",
            at("break;", switch_inner)
        ),
        // A labeled block: the labeled statement.
        format!(
            "break block@{} -> LabeledStatement@{block}",
            at("break block", 0)
        ),
        // A label of a switch member: the switch statement.
        format!(
            "continue next@{} -> SwitchStatement@{switch_next}",
            at("continue next", 0)
        ),
        format!(
            "break@{} -> SwitchStatement@{switch_next}",
            at("break;", switch_next)
        ),
        // An unlabeled `continue` skips the switch.
        format!(
            "continue@{} -> ForStatement@{for_last}",
            at("continue;", for_last)
        ),
    ];
    assert_eq!(label_target_lines(source), expected);
}

// ================================================================ patterns

/// The names of the variables of [set], sorted (Dart `_setToString`).
fn variable_names(
    av: &ResolverAssignedVariables,
    ctx: &Ctx<'_>,
    set: &IndexSet<PromotionKey>,
) -> Vec<String> {
    let mut names: Vec<String> = set
        .iter()
        .map(|&key| {
            let variable: EId<PromotableElement> = av.variable_for_key(key);
            let name = ctx.element_data(variable.raw()).unwrap().name.unwrap();
            ctx.name_str(name).to_string()
        })
        .collect();
    names.sort();
    names
}

/// The pattern variables of an if-case statement and of switch case
/// groups, with the join variables that the resolution visitor creates
/// (`VariableBinder.logicalOrPatternFinish`,
/// `switchStatementSharedCaseScopeFinish`). The join variables are named
/// `x@or` and `x@group` here, so that the result shows which variable the
/// pass declares.
#[test]
fn pattern_variables_and_joins() {
    let source = r#"
f(Object o) {
  if (o case (int a || int a) && int b) {
    a = 0;
  }
  switch (o) {
    case int c:
    case String c:
      c = 0;
    case double d:
      d = 0;
  }
}
"#;
    let parsed = dartr_ast_builder::parse_string(source, "test.dart");
    assert!(parsed.diagnostics.is_empty(), "parse errors");
    let ast = &parsed.ast;
    let test = TypeSystemTest::new();
    let ctx = test.ctx();
    let function = members(ast, parsed.unit.raw()).remove(0);

    let mut tables = ResolutionTables::new();
    let mut binder = Binder {
        ctx: &ctx,
        store: &test.store,
        tables: &mut tables,
        names: Vec::new(),
        frames: vec![Vec::new()],
        label_references: Vec::new(),
    };
    ast.visit_children(function.node, &mut binder);
    let variables = |name: &str| -> Vec<ElementId> {
        binder
            .names
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, e)| *e)
            .collect()
    };
    let (a, c) = (variables("a"), variables("c"));
    let a_or = join(&ctx, &test.store, "a@or", &a);
    let c_group = join(&ctx, &test.store, "c@group", &c);
    let parameters = binder.frames.pop().unwrap();
    // The identifiers in the bodies refer to the joins (the resolution
    // visitor adds `variables.values` to the scope of the body).
    let mut stack = vec![function.node];
    while let Some(node) = stack.pop() {
        stack.extend(ast.children(node));
        if let Some(identifier) = ast.cast::<SimpleIdentifier>(node) {
            match ast.tokens.lexeme(ast[identifier].token) {
                "a" => tables.element.insert(node, ElemRef::Base(a_or)),
                "c" => tables.element.insert(node, ElemRef::Base(c_group)),
                _ => None,
            };
        }
    }

    let mut if_statement = None;
    let mut switch_statement = None;
    let mut stack = vec![function.node];
    while let Some(node) = stack.pop() {
        stack.extend(ast.children(node));
        if let Some(n) = ast.cast::<IfStatement>(node) {
            if_statement = Some(n);
        }
        if let Some(n) = ast.cast::<SwitchStatement>(node) {
            switch_statement = Some(n);
        }
    }
    let (if_statement, switch_statement) = (if_statement.unwrap(), switch_statement.unwrap());

    // `(int a || int a) && int b`: the join of `a`, then `b`.
    let case_clause = ast[if_statement].case_clause.unwrap();
    let pattern = ast[ast[case_clause].guarded_pattern].pattern;
    let names: Vec<(String, String)> = pattern_variables(ast, &tables, &ctx, pattern)
        .into_iter()
        .map(|(name, e)| {
            (
                name,
                ctx.name_str(ctx.element_data(e).unwrap().name.unwrap())
                    .to_string(),
            )
        })
        .collect();
    assert_eq!(
        names,
        vec![
            ("a".to_string(), "a@or".to_string()),
            ("b".to_string(), "b".to_string())
        ]
    );

    let av = compute_assigned_variables(ast, &tables, &ctx, function.node, Some(&parameters), None);
    assert_eq!(variable_names(&av, &ctx, av.declared_at_top_level()), ["o"]);
    let if_node = if_statement.raw();
    assert_eq!(
        variable_names(&av, &ctx, av.declared_in_node(if_node)),
        ["a@or", "b"]
    );
    // The case variables, and the variables of the groups: the join of `c`
    // for the group of two cases, `d` itself for the group of one case.
    let switch_node = switch_statement.raw();
    assert_eq!(
        variable_names(&av, &ctx, av.declared_in_node(switch_node)),
        ["c", "c", "c@group", "d"]
    );
    // The writes in the bodies are writes of the joins. A node does not
    // list the writes of the variables that it declares.
    assert_eq!(
        variable_names(&av, &ctx, av.written_anywhere()),
        ["a@or", "c@group", "d"]
    );
    assert!(av.written_in_node(if_node).is_empty());
    assert!(av.written_in_node(switch_node).is_empty());
}
