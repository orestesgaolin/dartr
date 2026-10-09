//! Port of `pkg/analyzer/test/src/dart/constant/potentially_constant_test.dart`
//! (`IsConstantTypeExpressionTest`, `PotentiallyConstantTest`): each test
//! resolves Dart code through `Driver::analyze_library` and checks which
//! nodes `get_not_potentially_constants` reports, or what
//! `is_constant_type_expression` / `is_potentially_constant_type_expression`
//! return. Nodes are found like Dart `FindNode`: the innermost node of the
//! kind that covers the only occurrence of the search string.
//!
//! The tests that need a resolver unit that is not merged in this branch
//! are `#[ignore]`d with the name of the unit.

mod support;

use dartr_ast::{
    ConstructorFieldInitializer, Id, NodeId, NodeKind, NodeType, TypeAnnotation,
    VariableDeclaration, VariableDeclarationList,
};
use dartr_resolver::constant::potentially_constant::{
    ConstCheckInput, get_not_potentially_constants, is_constant_type_expression,
    is_potentially_constant_type_expression,
};
use support::{Analyzed, analyze, is_attached};

/// Dart `AstNode Function(TestResolvedUnitResult result)` of the tests.
enum Get {
    /// Dart `_xInitializer(result)`:
    /// `findNode.variableDeclaration('x = ').initializer!`.
    XInit,
    /// Dart `findNode.<kind>(search)`.
    Find(&'static str, &'static str),
    /// Dart `findNode.variableDeclaration(search).initializer!`.
    VarInit(&'static str),
    /// Dart `findNode.constructorFieldInitializer(search).expression`.
    CfiExpr(&'static str),
}

/// Whether a node of [kind] matches the Dart `FindNode` method [find].
fn kind_matches(find: &str, kind: NodeKind) -> bool {
    match find {
        "simple" => kind == NodeKind::SimpleIdentifier,
        "prefixed" => kind == NodeKind::PrefixedIdentifier,
        "propertyAccess" => kind == NodeKind::PropertyAccess,
        "methodInvocation" => kind == NodeKind::MethodInvocation,
        "instanceCreation" => kind == NodeKind::InstanceCreationExpression,
        "listLiteral" => kind == NodeKind::ListLiteral,
        "setOrMapLiteral" => kind == NodeKind::SetOrMapLiteral,
        "namedType" => kind == NodeKind::NamedType,
        "typeAnnotation" => TypeAnnotation::test(kind),
        "typeLiteral" => kind == NodeKind::TypeLiteral,
        "postfix" => kind == NodeKind::PostfixExpression,
        "prefix" => kind == NodeKind::PrefixExpression,
        "dotShorthandConstructorInvocation" => kind == NodeKind::DotShorthandConstructorInvocation,
        "dotShorthandPropertyAccess" => kind == NodeKind::DotShorthandPropertyAccess,
        "constructorReference" => kind == NodeKind::ConstructorReference,
        "functionReference" => kind == NodeKind::FunctionReference,
        "namedArgument" => kind == NodeKind::NamedArgument,
        "recordLiteral" => kind == NodeKind::RecordLiteral,
        "variableDeclaration" => kind == NodeKind::VariableDeclaration,
        "variableDeclarationList" => kind == NodeKind::VariableDeclarationList,
        "constructorFieldInitializer" => kind == NodeKind::ConstructorFieldInitializer,
        _ => panic!("unknown FindNode method {find}"),
    }
}

/// Dart `FindNode._node(search, predicate)`: the innermost node of the
/// defining unit that covers the only occurrence of [search] and matches
/// [find].
fn find(a: &Analyzed, find: &str, search: &str) -> NodeId {
    let source = a.source();
    let mut matches = source.match_indices(search);
    let offset = matches
        .next()
        .unwrap_or_else(|| panic!("{search:?} not found"))
        .0 as u32;
    assert!(matches.next().is_none(), "{search:?} found more than once");
    let unit = a.unit();
    let ast = &unit.ast;
    let root = unit.unit.raw();
    let depth = |mut n: NodeId| {
        let mut d = 0;
        while let Some(p) = ast.parent(n) {
            d += 1;
            n = p;
        }
        d
    };
    (0..ast.node_count())
        .map(NodeId::from_index)
        .filter(|&n| {
            kind_matches(find, ast.kind(n))
                && ast.offset(n) <= offset
                && offset < ast.end(n)
                && is_attached(ast, root, n)
        })
        .min_by_key(|&n| (ast.length(n), std::cmp::Reverse(depth(n))))
        .unwrap_or_else(|| panic!("no {find} at {offset} ({search:?})"))
}

fn get_node(a: &Analyzed, get: &Get) -> NodeId {
    let ast = &a.unit().ast;
    let var_init = |search: &str| {
        let d = ast
            .cast::<VariableDeclaration>(find(a, "variableDeclaration", search))
            .unwrap();
        ast[d].initializer.expect("initializer").raw()
    };
    match *get {
        Get::XInit => var_init("x = "),
        Get::VarInit(search) => var_init(search),
        Get::Find(kind, search) => find(a, kind, search),
        Get::CfiExpr(search) => {
            let n = find(a, "constructorFieldInitializer", search);
            let i = ast.cast::<ConstructorFieldInitializer>(n).unwrap();
            ast[i].expression.raw()
        }
    }
}

/// Analyzes `main.dart` with [code] and the other [files] (Dart
/// `newFile`), then runs [f] with the check input of the defining unit.
fn with_input(files: &[(&str, &str)], code: &str, f: impl FnOnce(&Analyzed, &ConstCheckInput<'_>)) {
    let mut all = vec![("main.dart", code)];
    all.extend_from_slice(files);
    let Some(a) = analyze(&all) else {
        eprintln!("skipped: no Dart SDK on PATH");
        return;
    };
    let unit = a.unit();
    assert!(unit.panic.is_none(), "{:?}", unit.panic);
    let ctx = a.ctx(unit);
    let features = &ctx.get(a.library.library).feature_set;
    let input = ConstCheckInput {
        ctx: &ctx,
        ast: &unit.ast,
        tables: &unit.tables,
        rt: &unit.rt,
        features,
    };
    f(&a, &input);
}

/// The kind, offset and source text of [node], for failure messages.
fn describe(a: &Analyzed, node: NodeId) -> String {
    let ast = &a.unit().ast;
    let (start, end) = (ast.offset(node) as usize, ast.end(node) as usize);
    format!("{:?}@{start} {:?}", ast.kind(node), &a.source()[start..end])
}

/// Dart `_assertConst` (empty [expected]) and `_assertNotConst` of
/// `PotentiallyConstantTest`: the nodes that `getNotPotentiallyConstants`
/// returns for the node of [get] are the nodes of [expected] (in any
/// order).
fn check(files: &[(&str, &str)], code: &str, get: Get, expected: &[(&str, &str)]) {
    with_input(files, code, |a, input| {
        let node = get_node(a, &get);
        let mut actual = get_not_potentially_constants(input, node);
        let mut expected: Vec<NodeId> = expected
            .iter()
            .map(|&(kind, search)| find(a, kind, search))
            .collect();
        actual.sort();
        expected.sort();
        assert_eq!(
            actual.iter().map(|&n| describe(a, n)).collect::<Vec<_>>(),
            expected.iter().map(|&n| describe(a, n)).collect::<Vec<_>>(),
            "not potentially constant nodes of {}",
            describe(a, node)
        );
        assert_eq!(actual, expected);
    });
}

/// Dart `_assertConst`, `_assertNeverConst`, `_assertPotentiallyConst` of
/// `IsConstantTypeExpressionTest`: the results for the type of
/// `findNode.variableDeclarationList('x;')`.
fn check_type(files: &[(&str, &str)], code: &str, potentially: bool, constant: bool) {
    with_input(files, code, |a, input| {
        let ast = input.ast;
        let list = find(a, "variableDeclarationList", "x;");
        let list = ast.cast::<VariableDeclarationList>(list).unwrap();
        let type_: Id<TypeAnnotation> = ast[list].type_.expect("type");
        assert_eq!(
            (
                is_potentially_constant_type_expression(input, type_),
                is_constant_type_expression(input, type_)
            ),
            (potentially, constant),
            "(isPotentiallyConstantTypeExpression, isConstantTypeExpression) of {}",
            describe(a, type_.raw())
        );
    });
}

mod is_constant_type_expression {
    use super::*;
    /// Dart `test_class`.
    #[test]
    fn class() {
        check_type(
            &[],
            "\
int x;
",
            true,
            true,
        );
    }
    /// Dart `test_class_prefix`.
    #[test]
    fn class_prefix() {
        check_type(
            &[(
                "a.dart",
                "\
class A {}
",
            )],
            "\
import 'a.dart' as p;
p.A x;
",
            true,
            true,
        );
    }
    /// Dart `test_class_prefix_deferred`.
    #[test]
    fn class_prefix_deferred() {
        check_type(
            &[(
                "a.dart",
                "\
class A {}
",
            )],
            "\
import 'a.dart' deferred as p;
p.A x;
",
            false,
            false,
        );
    }
    /// Dart `test_class_typeArguments`.
    #[test]
    fn class_type_arguments() {
        check_type(
            &[],
            "\
List<int> x;
",
            true,
            true,
        );
    }
    /// Dart `test_class_typeArguments_notConst`.
    #[test]
    fn class_type_arguments_not_const() {
        check_type(
            &[],
            "\
class A<T> {
  m() {
    List<T> x;
  }
}
",
            true,
            false,
        );
    }
    /// Dart `test_dynamic`.
    #[test]
    fn dynamic() {
        check_type(
            &[],
            "\
dynamic x;
",
            true,
            true,
        );
    }
    /// Dart `test_genericFunctionType`.
    #[test]
    fn generic_function_type() {
        check_type(
            &[],
            "\
int Function<T extends num, U>(int, bool) x;
",
            true,
            true,
        );
    }
    /// Dart `test_genericFunctionType_formalParameterType`.
    #[test]
    fn generic_function_type_formal_parameter_type() {
        check_type(
            &[],
            "\
class A<T> {
  m() {
    Function(T) x;
  }
}
",
            true,
            false,
        );
    }
    /// Dart `test_genericFunctionType_returnType`.
    #[test]
    fn generic_function_type_return_type() {
        check_type(
            &[],
            "\
class A<T> {
  m() {
    T Function() x;
  }
}
",
            true,
            false,
        );
    }
    /// Dart `test_genericFunctionType_typeParameterBound`.
    #[test]
    fn generic_function_type_type_parameter_bound() {
        check_type(
            &[],
            "\
class A<T> {
  m() {
    Function<U extends T>() x;
  }
}
",
            true,
            false,
        );
    }
    /// Dart `test_recordType`.
    #[test]
    fn record_type() {
        check_type(
            &[],
            "\
(int, ) x;
",
            true,
            true,
        );
    }
    /// Dart `test_typeParameter_ofClass`.
    #[test]
    fn type_parameter_of_class() {
        check_type(
            &[],
            "\
class A<T> {
  T x;
}
",
            true,
            false,
        );
    }
    /// Dart `test_typeParameter_ofClass_nested`.
    #[test]
    fn type_parameter_of_class_nested() {
        check_type(
            &[],
            "\
class A<T> {
  List<T> x;
}
",
            true,
            false,
        );
    }
    /// Dart `test_typeParameter_ofFunction`.
    #[test]
    fn type_parameter_of_function() {
        check_type(
            &[],
            "\
void foo<T>() {
  T x;
}
",
            true,
            false,
        );
    }
    /// Dart `test_typeParameter_ofFunctionType`.
    #[test]
    fn type_parameter_of_function_type() {
        check_type(
            &[],
            "\
void foo() {
  void Function<X>(X) x;
}
",
            true,
            false,
        );
    }
    /// Dart `test_void`.
    #[test]
    fn void() {
        check_type(
            &[],
            "\
void x;
",
            true,
            true,
        );
    }
}

mod potentially_constant {
    use super::*;
    /// Dart `test_adjacentStrings`.
    #[test]
    fn adjacent_strings() {
        check(
            &[],
            "\
var x = 'a' 'b';
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_asExpression`.
    #[test]
    fn as_expression() {
        check(
            &[],
            "\
const a = 0;
var x = a as int;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_asExpression_final`.
    #[test]
    fn as_expression_final() {
        check(
            &[],
            "\
final a = 0;
var x = a as int;
",
            Get::XInit,
            &[("simple", "a as")],
        );
    }
    /// Dart `test_asExpression_typeParameter`.
    #[test]
    fn as_expression_type_parameter() {
        check(
            &[],
            "\
const a = 0;
class A<T> {
  m() {
    var x = a as T;
  }
}
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_asExpression_typeParameter_nested`.
    #[test]
    fn as_expression_type_parameter_nested() {
        check(
            &[],
            "\
const a = 0;
class A<T> {
  m() {
    var x = a as List<T>;
  }
}
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_binaryExpression_andEager_const_const`.
    #[test]
    fn binary_expression_and_eager_const_const() {
        check(
            &[],
            "\
const a = false;
const b = false;
var x = a & b;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_binaryExpression_andEager_const_notConst`.
    #[test]
    fn binary_expression_and_eager_const_not_const() {
        check(
            &[],
            "\
const a = false;
final b = false;
var x = a & b;
",
            Get::XInit,
            &[("simple", "b;")],
        );
    }
    /// Dart `test_binaryExpression_andEager_notConst_const`.
    #[test]
    fn binary_expression_and_eager_not_const_const() {
        check(
            &[],
            "\
final a = false;
const b = false;
var x = a & b;
",
            Get::XInit,
            &[("simple", "a &")],
        );
    }
    /// Dart `test_binaryExpression_andEager_notConst_notConst`.
    #[test]
    fn binary_expression_and_eager_not_const_not_const() {
        check(
            &[],
            "\
final a = false;
final b = false;
var x = a & b;
",
            Get::XInit,
            &[("simple", "a &"), ("simple", "b;")],
        );
    }
    /// Dart `test_binaryExpression_andLazy_const_const`.
    #[test]
    fn binary_expression_and_lazy_const_const() {
        check(
            &[],
            "\
const a = false;
const b = false;
var x = a && b;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_binaryExpression_andLazy_const_notConst`.
    #[test]
    fn binary_expression_and_lazy_const_not_const() {
        check(
            &[],
            "\
const a = false;
final b = false;
var x = a && b;
",
            Get::XInit,
            &[("simple", "b;")],
        );
    }
    /// Dart `test_binaryExpression_andLazy_notConst_const`.
    #[test]
    fn binary_expression_and_lazy_not_const_const() {
        check(
            &[],
            "\
final a = false;
const b = false;
var x = a && b;
",
            Get::XInit,
            &[("simple", "a &")],
        );
    }
    /// Dart `test_binaryExpression_andLazy_notConst_notConst`.
    #[test]
    fn binary_expression_and_lazy_not_const_not_const() {
        check(
            &[],
            "\
final a = false;
final b = false;
var x = a && b;
",
            Get::XInit,
            &[("simple", "a &"), ("simple", "b;")],
        );
    }
    /// Dart `test_binaryExpression_ifNull_const_const`.
    #[test]
    fn binary_expression_if_null_const_const() {
        check(
            &[],
            "\
const a = 0;
const b = 1;
var x = a ?? b;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_binaryExpression_ifNull_const_notConst`.
    #[test]
    fn binary_expression_if_null_const_not_const() {
        check(
            &[],
            "\
const a = 0;
final b = 1;
var x = a ?? b;
",
            Get::XInit,
            &[("simple", "b;")],
        );
    }
    /// Dart `test_binaryExpression_ifNull_notConst_const`.
    #[test]
    fn binary_expression_if_null_not_const_const() {
        check(
            &[],
            "\
final a = 0;
const b = 1;
var x = a ?? b;
",
            Get::XInit,
            &[("simple", "a ??")],
        );
    }
    /// Dart `test_binaryExpression_ifNull_notConst_notConst`.
    #[test]
    fn binary_expression_if_null_not_const_not_const() {
        check(
            &[],
            "\
final a = 0;
final b = 1;
var x = a ?? b;
",
            Get::XInit,
            &[("simple", "a ??"), ("simple", "b;")],
        );
    }
    /// Dart `test_binaryExpression_orEager_const_const`.
    #[test]
    fn binary_expression_or_eager_const_const() {
        check(
            &[],
            "\
const a = false;
const b = false;
var x = a | b;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_binaryExpression_orEager_const_notConst`.
    #[test]
    fn binary_expression_or_eager_const_not_const() {
        check(
            &[],
            "\
const a = false;
final b = false;
var x = a | b;
",
            Get::XInit,
            &[("simple", "b;")],
        );
    }
    /// Dart `test_binaryExpression_orEager_notConst_const`.
    #[test]
    fn binary_expression_or_eager_not_const_const() {
        check(
            &[],
            "\
final a = false;
const b = false;
var x = a | b;
",
            Get::XInit,
            &[("simple", "a |")],
        );
    }
    /// Dart `test_binaryExpression_orEager_notConst_notConst`.
    #[test]
    fn binary_expression_or_eager_not_const_not_const() {
        check(
            &[],
            "\
final a = false;
final b = false;
var x = a | b;
",
            Get::XInit,
            &[("simple", "a |"), ("simple", "b;")],
        );
    }
    /// Dart `test_binaryExpression_orLazy_const_const`.
    #[test]
    fn binary_expression_or_lazy_const_const() {
        check(
            &[],
            "\
const a = false;
const b = false;
var x = a || b;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_binaryExpression_orLazy_const_notConst`.
    #[test]
    fn binary_expression_or_lazy_const_not_const() {
        check(
            &[],
            "\
const a = false;
final b = false;
var x = a || b;
",
            Get::XInit,
            &[("simple", "b;")],
        );
    }
    /// Dart `test_binaryExpression_orLazy_notConst_const`.
    #[test]
    fn binary_expression_or_lazy_not_const_const() {
        check(
            &[],
            "\
final a = false;
const b = false;
var x = a || b;
",
            Get::XInit,
            &[("simple", "a |")],
        );
    }
    /// Dart `test_binaryExpression_orLazy_notConst_notConst`.
    #[test]
    fn binary_expression_or_lazy_not_const_not_const() {
        check(
            &[],
            "\
final a = false;
final b = false;
var x = a || b;
",
            Get::XInit,
            &[("simple", "a |"), ("simple", "b;")],
        );
    }
    /// Dart `test_conditional`.
    #[test]
    fn conditional() {
        check(
            &[],
            "\
const a = 0;
const b = 0;
const c = 0;
var x = a ? b : c;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_conditional_final`.
    #[test]
    fn conditional_final() {
        check(
            &[],
            "\
final a = 0;
final b = 0;
final c = 0;
var x = a ? b : c;
",
            Get::XInit,
            &[("simple", "a ?"), ("simple", "b :"), ("simple", "c;")],
        );
    }
    /// Dart `test_constructorReference_explicitTypeArguments`.
    #[test]
    fn constructor_reference_explicit_type_arguments() {
        check(
            &[],
            "\
class A {
  final B Function() x;
  const A(): x = B<int>.new;
}

class B<T> {}
",
            Get::Find("constructorReference", "B<int>.new"),
            &[],
        );
    }
    /// Dart `test_constructorReference_explicitTypeArguments_nonConst`.
    #[test]
    fn constructor_reference_explicit_type_arguments_non_const() {
        check(
            &[],
            "\
import '' deferred as self;
class A {
  Object x;
  const A(): x = B<self.A>.new;
}

class B<T> {}
",
            Get::Find("constructorReference", "B<self.A>.new"),
            &[("typeAnnotation", "self.A")],
        );
    }
    /// Dart `test_constructorReference_noTypeArguments`.
    #[test]
    fn constructor_reference_no_type_arguments() {
        check(
            &[],
            "\
class A {
  final B Function() x;
  const A(): x = B.new;
}

class B {}
",
            Get::Find("constructorReference", "B.new"),
            &[],
        );
    }
    /// Dart `test_dotShorthandConstructorInvocation_const`.
    #[test]
    fn dot_shorthand_constructor_invocation_const() {
        check(
            &[],
            "\
class A {
  const A();
}

const A x = .new();
",
            Get::Find("dotShorthandConstructorInvocation", ".new()"),
            &[],
        );
    }
    /// Dart `test_dotShorthandConstructorInvocation_nonConst`.
    #[test]
    fn dot_shorthand_constructor_invocation_non_const() {
        check(
            &[],
            "\
class A {
  const A();
}

A x = .new();
",
            Get::XInit,
            &[("dotShorthandConstructorInvocation", ".new()")],
        );
    }
    /// Dart `test_dotShorthandPropertyAccess_const`.
    #[test]
    fn dot_shorthand_property_access_const() {
        check(
            &[],
            "\
class A {
  static const A a = A();
  const A();
}

const A x = .a;
",
            Get::Find("dotShorthandPropertyAccess", ".a"),
            &[],
        );
    }
    /// Dart `test_dotShorthandPropertyAccess_nonConst`.
    #[test]
    fn dot_shorthand_property_access_non_const() {
        check(
            &[],
            "\
class A {
  static A a = A();
}

A x = .a;
",
            Get::XInit,
            &[("simple", "a;")],
        );
    }
    /// Dart `test_functionReference_explicitTypeArguments`.
    #[test]
    fn function_reference_explicit_type_arguments() {
        check(
            &[],
            "\
class A {
  final int Function(int) x;
  const A(): x = id<int>;
}

X id<X>(X x) => x;
",
            Get::Find("functionReference", "id<int>"),
            &[],
        );
    }
    /// Dart `test_functionReference_explicitTypeArguments_nonConst`.
    #[test]
    fn function_reference_explicit_type_arguments_non_const() {
        check(
            &[],
            "\
import '' deferred as self;
class A {
  final int Function(int) x;
  const A(): x = id<self.A>;
}

X id<X>(X x) => x;
",
            Get::Find("functionReference", "id<self.A>"),
            &[("typeAnnotation", "self.A")],
        );
    }
    /// Dart `test_functionReference_noTypeArguments`.
    #[test]
    fn function_reference_no_type_arguments() {
        check(
            &[],
            "\
class A {
  final int Function(int) x;
  const A(): x = id;
}

X id<X>(X x) => x;
",
            Get::Find("simple", "id;"),
            &[],
        );
    }
    /// Dart `test_ifElement_then`.
    #[test]
    fn if_element_then() {
        check(
            &[],
            "\
const a = 0;
const b = 0;
var x = const [if (a) b];
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_ifElement_then_final`.
    #[test]
    fn if_element_then_final() {
        check(
            &[],
            "\
final a = 0;
final b = 0;
var x = const [if (a) b];
",
            Get::XInit,
            &[("simple", "a)"), ("simple", "b]")],
        );
    }
    /// Dart `test_ifElement_thenElse`.
    #[test]
    fn if_element_then_else() {
        check(
            &[],
            "\
const a = 0;
const b = 0;
const c = 0;
var x = const [if (a) b else c];
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_instanceCreation`.
    #[test]
    fn instance_creation() {
        check(
            &[],
            "\
class A {
  const A();
}

var x = new A(); // x
",
            Get::XInit,
            &[("instanceCreation", "A(); // x")],
        );
    }
    /// Dart `test_instanceCreation_const`.
    #[test]
    fn instance_creation_const() {
        check(
            &[],
            "\
class A {
  const A();
}

var x = const A();
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_isExpression`.
    #[test]
    fn is_expression() {
        check(
            &[],
            "\
const a = 0;
var x = a is int;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_isExpression_final`.
    #[test]
    fn is_expression_final() {
        check(
            &[],
            "\
final a = 0;
var x = a is int;
",
            Get::XInit,
            &[("simple", "a is")],
        );
    }
    /// Dart `test_isExpression_typeParameter`.
    #[test]
    fn is_expression_type_parameter() {
        check(
            &[],
            "\
const a = 0;
class A<T> {
  m() {
    var x = a is T;
  }
}
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_isExpression_typeParameter_nested`.
    #[test]
    fn is_expression_type_parameter_nested() {
        check(
            &[],
            "\
const a = 0;
class A<T> {
  m() {
    var x = a is List<T>;
  }
}
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_listLiteral`.
    #[test]
    fn list_literal() {
        check(
            &[],
            "\
var x = const [0, 1, 2];
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_listLiteral_notConst`.
    #[test]
    fn list_literal_not_const() {
        check(
            &[],
            "\
var x = [0, 1, 2];
",
            Get::XInit,
            &[("listLiteral", "0,")],
        );
    }
    /// Dart `test_listLiteral_notConst_element`.
    #[test]
    fn list_literal_not_const_element() {
        check(
            &[],
            "\
final a = 0;
final b = 1;
var x = const [a, b, 2];
",
            Get::XInit,
            &[("simple", "a,"), ("simple", "b,")],
        );
    }
    /// Dart `test_listLiteral_ofDynamic`.
    #[test]
    fn list_literal_of_dynamic() {
        check(
            &[],
            "\
var x = const <dynamic>[];
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_listLiteral_ofNever`.
    #[test]
    fn list_literal_of_never() {
        check(
            &[],
            "\
var x = const <Never>[];
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_listLiteral_ofVoid`.
    #[test]
    fn list_literal_of_void() {
        check(
            &[],
            "\
var x = const <void>[];
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_listLiteral_typeArgument`.
    #[test]
    fn list_literal_type_argument() {
        check(
            &[],
            "\
var x = const <int>[0, 1, 2];
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_listLiteral_typeArgument_notConstType`.
    #[test]
    fn list_literal_type_argument_not_const_type() {
        check(
            &[],
            "\
import '' deferred as self;
class A {
  m() {
    var x = const <self.A>[];
  }
}
",
            Get::XInit,
            &[("namedType", "A>[")],
        );
    }
    /// Dart `test_literal_bool`.
    #[test]
    fn literal_bool() {
        check(
            &[],
            "\
var x = true;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_literal_double`.
    #[test]
    fn literal_double() {
        check(
            &[],
            "\
var x = 1.2;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_literal_int`.
    #[test]
    fn literal_int() {
        check(
            &[],
            "\
var x = 0;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_literal_null`.
    #[test]
    fn literal_null() {
        check(
            &[],
            "\
var x = null;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_literal_simpleString`.
    #[test]
    fn literal_simple_string() {
        check(
            &[],
            "\
var x = '123';
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_literal_symbol`.
    #[test]
    fn literal_symbol() {
        check(
            &[],
            "\
var x = #a.b.c;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_mapLiteral`.
    #[test]
    fn map_literal() {
        check(
            &[],
            "\
var x = const {0: 1};
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_mapLiteral_notConst`.
    #[test]
    fn map_literal_not_const() {
        check(
            &[],
            "\
var x = {0: 1};
",
            Get::XInit,
            &[("setOrMapLiteral", "0: 1")],
        );
    }
    /// Dart `test_mapLiteral_notConst_key`.
    #[test]
    fn map_literal_not_const_key() {
        check(
            &[],
            "\
final a = 1;
final b = 2;
var x = const {0: 0, a: 1, b: 2};
",
            Get::XInit,
            &[("simple", "a:"), ("simple", "b:")],
        );
    }
    /// Dart `test_mapLiteral_notConst_value`.
    #[test]
    fn map_literal_not_const_value() {
        check(
            &[],
            "\
final a = 1;
final b = 2;
var x = const {0: 0, 1: a, 2: b};
",
            Get::XInit,
            &[("simple", "a,"), ("simple", "b}")],
        );
    }
    /// Dart `test_mapLiteral_typeArgument`.
    #[test]
    fn map_literal_type_argument() {
        check(
            &[],
            "\
var x = const <int, int>{0: 0};
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_mapLiteral_typeArgument_notConstType`.
    #[test]
    fn map_literal_type_argument_not_const_type() {
        check(
            &[],
            "\
class A<T> {
  m() {
    var x = const <T, T>{};
  }
}
",
            Get::XInit,
            &[("namedType", "T,"), ("namedType", "T>{")],
        );
    }
    /// Dart `test_methodInvocation_identical`.
    #[test]
    fn method_invocation_identical() {
        check(
            &[],
            "\
const a = 0;
const b = 0;
var x = identical(a, b);
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_methodInvocation_identical_final`.
    #[test]
    fn method_invocation_identical_final() {
        check(
            &[],
            "\
final a = 0;
final b = 0;
var x = identical(a, b);
",
            Get::XInit,
            &[("simple", "a,"), ("simple", "b)")],
        );
    }
    /// Dart `test_methodInvocation_name`.
    #[test]
    fn method_invocation_name() {
        check(
            &[],
            "\
const a = 0;
const b = 0;
var x = foo(a, b);
",
            Get::XInit,
            &[("methodInvocation", "foo")],
        );
    }
    /// Dart `test_methodInvocation_target`.
    #[test]
    fn method_invocation_target() {
        check(
            &[],
            "\
var x = a.foo();
",
            Get::XInit,
            &[("methodInvocation", "a.foo()")],
        );
    }
    /// Dart `test_namedExpression`.
    #[test]
    fn named_expression() {
        check(
            &[],
            "\
void f({a}) {}

var x = f(a: 0);
",
            Get::Find("namedArgument", "a: 0"),
            &[],
        );
    }
    /// Dart `test_parenthesizedExpression_const`.
    #[test]
    fn parenthesized_expression_const() {
        check(
            &[],
            "\
const a = 0;
var x = (a);
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_parenthesizedExpression_final`.
    #[test]
    fn parenthesized_expression_final() {
        check(
            &[],
            "\
final a = 0;
var x = (a);
",
            Get::XInit,
            &[("simple", "a);")],
        );
    }
    /// Dart `test_postfixExpression`.
    #[test]
    fn postfix_expression() {
        check(
            &[],
            "\
const a = 0;
var x = a++;
",
            Get::XInit,
            &[("postfix", "a++")],
        );
    }
    /// Dart `test_prefixedIdentifier_importPrefix_deferred`.
    #[test]
    fn prefixed_identifier_import_prefix_deferred() {
        check(
            &[(
                "a.dart",
                "\
const a = 0;
",
            )],
            "\
import 'a.dart' deferred as p;
var x = p.a + 1;
",
            Get::XInit,
            &[("prefixed", "p.a")],
        );
    }
    /// Dart `test_prefixedIdentifier_importPrefix_function`.
    #[test]
    fn prefixed_identifier_import_prefix_function() {
        check(
            &[(
                "a.dart",
                "\
void f() {}
",
            )],
            "\
import 'a.dart' as p;
var x = p.f;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_prefixedIdentifier_importPrefix_topVar`.
    #[test]
    fn prefixed_identifier_import_prefix_top_var() {
        check(
            &[(
                "a.dart",
                "\
const a = 0;
",
            )],
            "\
import 'a.dart' as p;
var x = p.a + 1;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_prefixedIdentifier_length_const`.
    #[test]
    fn prefixed_identifier_length_const() {
        check(
            &[],
            "\
const a = 'abc';
var x = a.length;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_prefixedIdentifier_length_final`.
    #[test]
    fn prefixed_identifier_length_final() {
        check(
            &[],
            "\
final a = 'abc';
var x = a.length;
",
            Get::XInit,
            &[("simple", "a.")],
        );
    }
    /// Dart `test_prefixedIdentifier_method_instance`.
    #[test]
    fn prefixed_identifier_method_instance() {
        check(
            &[],
            "\
class A {
  const A();
  m() {};
}

const a = const A();

var x = a.m;
",
            Get::XInit,
            &[("prefixed", "a.m")],
        );
    }
    /// Dart `test_prefixedIdentifier_method_static`.
    #[test]
    fn prefixed_identifier_method_static() {
        check(
            &[],
            "\
class A {
  static m() {};
}

var x = A.m;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_prefixedIdentifier_method_static_viaInstance`.
    #[test]
    fn prefixed_identifier_method_static_via_instance() {
        check(
            &[],
            "\
class A {
  const A();
  static m() {};
}

const a = const A();

var x = a.m;
",
            Get::XInit,
            &[("prefixed", "a.m")],
        );
    }
    /// Dart `test_prefixedIdentifier_prefix_variable`.
    #[test]
    fn prefixed_identifier_prefix_variable() {
        check(
            &[],
            "\
class A {
  final a = 0;
  const A();
}

const a = const A();

var x = a.b + 1;
",
            Get::XInit,
            &[("prefixed", "a.b + 1")],
        );
    }
    /// Dart `test_prefixedIdentifier_staticField_const`.
    #[test]
    fn prefixed_identifier_static_field_const() {
        check(
            &[],
            "\
class A {
  static const a = 0;
}
var x = A.a + 1;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_prefixedIdentifier_staticField_final`.
    #[test]
    fn prefixed_identifier_static_field_final() {
        check(
            &[],
            "\
class A {
  static final a = 0;
}
var x = A.a + 1;
",
            Get::XInit,
            &[("prefixed", "A.a")],
        );
    }
    /// Dart `test_prefixedIdentifier_typedef_interfaceType`.
    #[test]
    fn prefixed_identifier_typedef_interface_type() {
        check(
            &[(
                "a.dart",
                "\
typedef A = List<int>;
",
            )],
            "\
import 'a.dart' as p;
var x = p.A;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_prefixExpression_bang`.
    #[test]
    fn prefix_expression_bang() {
        check(
            &[],
            "\
const a = 0;
var x = !a;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_prefixExpression_minus`.
    #[test]
    fn prefix_expression_minus() {
        check(
            &[],
            "\
const a = 0;
var x = -a;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_prefixExpression_minus_final`.
    #[test]
    fn prefix_expression_minus_final() {
        check(
            &[],
            "\
final a = 0;
var x = -a;
",
            Get::XInit,
            &[("simple", "a;")],
        );
    }
    /// Dart `test_prefixExpression_plusPlus`.
    #[test]
    fn prefix_expression_plus_plus() {
        check(
            &[],
            "\
const a = 0;
var x = ++a;
",
            Get::XInit,
            &[("prefix", "++a")],
        );
    }
    /// Dart `test_prefixExpression_tilde`.
    #[test]
    fn prefix_expression_tilde() {
        check(
            &[],
            "\
const a = 0;
var x = ~a;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_propertyAccess_instanceMethod_withPrefix`.
    #[test]
    fn property_access_instance_method_with_prefix() {
        check(
            &[(
                "a.dart",
                "\
class A {
  void m() {}
}
",
            )],
            "\
import 'a.dart' as p;
var x = p.A.m;
",
            Get::XInit,
            &[("simple", "m;")],
        );
    }
    /// Dart `test_propertyAccess_length_final`.
    #[test]
    fn property_access_length_final() {
        check(
            &[],
            "\
final a = 'abc';
var x = (a).length;
",
            Get::XInit,
            &[("simple", "a).")],
        );
    }
    /// Dart `test_propertyAccess_length_stringLiteral`.
    #[test]
    fn property_access_length_string_literal() {
        check(
            &[],
            "\
var x = 'abc'.length;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_propertyAccess_staticField_withPrefix_const`.
    #[test]
    fn property_access_static_field_with_prefix_const() {
        check(
            &[(
                "a.dart",
                "\
class A {
  static const a = 0;
}
",
            )],
            "\
import 'a.dart' as p;
var x = p.A.a + 1;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_propertyAccess_staticField_withPrefix_deferred`.
    #[test]
    fn property_access_static_field_with_prefix_deferred() {
        check(
            &[(
                "a.dart",
                "\
class A {
  static const a = 0;
}
",
            )],
            "\
import 'a.dart' deferred as p;
var x = p.A.a + 1;
",
            Get::XInit,
            &[("propertyAccess", "p.A.a")],
        );
    }
    /// Dart `test_propertyAccess_staticField_withPrefix_final`.
    #[test]
    fn property_access_static_field_with_prefix_final() {
        check(
            &[(
                "a.dart",
                "\
class A {
  static final a = 0;
}
",
            )],
            "\
import 'a.dart' as p;
var x = p.A.a + 1;
",
            Get::XInit,
            &[("simple", "a + 1")],
        );
    }
    /// Dart `test_propertyAccess_staticMethod_withPrefix`.
    #[test]
    fn property_access_static_method_with_prefix() {
        check(
            &[(
                "a.dart",
                "\
class A {
  static void m() {}
}
",
            )],
            "\
import 'a.dart' as p;
var x = p.A.m;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_propertyAccess_staticMethod_withPrefix_deferred`.
    #[test]
    fn property_access_static_method_with_prefix_deferred() {
        check(
            &[(
                "a.dart",
                "\
class A {
  static void m() {}
}
",
            )],
            "\
import 'a.dart' deferred as p;
var x = p.A.m;
",
            Get::XInit,
            &[("propertyAccess", "p.A.m")],
        );
    }
    /// Dart `test_propertyAccess_target_instanceCreation`.
    #[test]
    fn property_access_target_instance_creation() {
        check(
            &[],
            "\
class A {
  final a = 0;
}

var x = A().a + 1;
",
            Get::XInit,
            &[("propertyAccess", "A().a")],
        );
    }
    /// Dart `test_propertyAccess_target_variable`.
    #[test]
    fn property_access_target_variable() {
        check(
            &[(
                "a.dart",
                "\
class A {
  final a = 0;
  const A();
}

const a = const A();
",
            )],
            "\
import 'a.dart' as p;

var x = p.a.b + 1;
",
            Get::XInit,
            &[("propertyAccess", "p.a.b + 1")],
        );
    }
    /// Dart `test_recordLiteral`.
    #[test]
    fn record_literal() {
        check(
            &[],
            "\
var x = const (0, 1, 2);
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_recordLiteral_constructorParameter`.
    #[test]
    fn record_literal_constructor_parameter() {
        check(
            &[],
            "\
class C {
  final Object f;
  const C(int a) : f = (0, a);
}
",
            Get::Find("recordLiteral", "(0"),
            &[],
        );
    }
    /// Dart `test_recordLiteral_namedField_const`.
    #[test]
    fn record_literal_named_field_const() {
        check(
            &[],
            "\
var x = const (f1: 0);
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_recordLiteral_namedField_notConst_element`.
    #[test]
    fn record_literal_named_field_not_const_element() {
        check(
            &[],
            "\
final a = 0;
var x = const (f1: a);
",
            Get::XInit,
            &[("simple", "a)")],
        );
    }
    /// Dart `test_recordLiteral_notConst`.
    #[test]
    fn record_literal_not_const() {
        check(
            &[],
            "\
var x = (0, 1, 2);
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_recordLiteral_notConst_element`.
    #[test]
    fn record_literal_not_const_element() {
        check(
            &[],
            "\
final a = 0;
final b = 1;
var x = const (a, b, 2);
",
            Get::XInit,
            &[("simple", "a,"), ("simple", "b,")],
        );
    }
    /// Dart `test_setLiteral`.
    #[test]
    fn set_literal() {
        check(
            &[],
            "\
var x = const {0, 1, 2};
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_setLiteral_notConst`.
    #[test]
    fn set_literal_not_const() {
        check(
            &[],
            "\
var x = {0, 1, 2};
",
            Get::XInit,
            &[("setOrMapLiteral", "0,")],
        );
    }
    /// Dart `test_setLiteral_notConst_element`.
    #[test]
    fn set_literal_not_const_element() {
        check(
            &[],
            "\
final a = 0;
final b = 1;
var x = const {a, b, 2};
",
            Get::XInit,
            &[("simple", "a,"), ("simple", "b,")],
        );
    }
    /// Dart `test_setLiteral_typeArgument`.
    #[test]
    fn set_literal_type_argument() {
        check(
            &[],
            "\
var x = const <int>{0, 1, 2};
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_setLiteral_typeArgument_notConstType`.
    #[test]
    fn set_literal_type_argument_not_const_type() {
        check(
            &[],
            "\
import '' deferred as self;
class A {
  m() {
    var x = const <self.A>{};
  }
}
",
            Get::XInit,
            &[("namedType", "A>{")],
        );
    }
    /// Dart `test_simpleIdentifier_class`.
    #[test]
    fn simple_identifier_class() {
        check(
            &[],
            "\
var x = int;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_simpleIdentifier_function`.
    #[test]
    fn simple_identifier_function() {
        check(
            &[],
            "\
var x = f;

void f() {}
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_simpleIdentifier_localVar_const`.
    #[test]
    fn simple_identifier_local_var_const() {
        check(
            &[],
            "\
main() {
  const a = 0;
  var x = a + 1;
}
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_simpleIdentifier_localVar_final`.
    #[test]
    fn simple_identifier_local_var_final() {
        check(
            &[],
            "\
main() {
  final a = 0;
  var x = a + 1;
}
",
            Get::XInit,
            &[("simple", "a +")],
        );
    }
    /// Dart `test_simpleIdentifier_method_static`.
    #[test]
    fn simple_identifier_method_static() {
        check(
            &[],
            "\
class A {
  static m() {};

  final Object f;

  const A() : f = m; // ref
}
",
            Get::Find("simple", "m; // ref"),
            &[],
        );
    }
    /// Dart `test_simpleIdentifier_parameterOfConstPrimaryConstructor_inFieldInitializer_instance_late`.
    #[test]
    fn simple_identifier_parameter_of_const_primary_constructor_in_field_initializer_instance_late()
    {
        check(
            &[],
            "\
class const C(int a) {
  late final int f = a + 1;
}
",
            Get::VarInit("f ="),
            &[("simple", "a +")],
        );
    }
    /// Dart `test_simpleIdentifier_parameterOfConstPrimaryConstructor_inFieldInitializer_instance_notLate`.
    #[test]
    fn simple_identifier_parameter_of_const_primary_constructor_in_field_initializer_instance_not_late()
     {
        check(
            &[],
            "\
class const C(int a) {
  final int f = a + 1;
}
",
            Get::VarInit("f ="),
            &[],
        );
    }
    /// Dart `test_simpleIdentifier_parameterOfConstPrimaryConstructor_inFieldInitializer_static`.
    #[test]
    fn simple_identifier_parameter_of_const_primary_constructor_in_field_initializer_static() {
        check(
            &[],
            "\
class const C(int a) {
  static final int f = a + 1;
}
",
            Get::VarInit("f ="),
            &[("simple", "a +")],
        );
    }
    /// Dart `test_simpleIdentifier_parameterOfConstPrimaryConstructor_inInitializer`.
    #[test]
    fn simple_identifier_parameter_of_const_primary_constructor_in_initializer() {
        check(
            &[],
            "\
class const C(int a) {
  final int f;
  this : f = a + 1;
}
",
            Get::CfiExpr("f ="),
            &[],
        );
    }
    /// Dart `test_simpleIdentifier_parameterOfConstSecondaryConstructor_inBody`.
    #[test]
    fn simple_identifier_parameter_of_const_secondary_constructor_in_body() {
        check(
            &[],
            "\
class C {
  const C(int a) {
    var x = a + 1;
  }
}
",
            Get::XInit,
            &[("simple", "a +")],
        );
    }
    /// Dart `test_simpleIdentifier_parameterOfConstSecondaryConstructor_inInitializer`.
    #[test]
    fn simple_identifier_parameter_of_const_secondary_constructor_in_initializer() {
        check(
            &[],
            "\
class C {
  final int f;
  const C(int a) : f = a + 1;
}
",
            Get::CfiExpr("f ="),
            &[],
        );
    }
    /// Dart `test_simpleIdentifier_parameterOfNotConstPrimaryConstructor_inConstructorFieldInitializer`.
    #[test]
    fn simple_identifier_parameter_of_not_const_primary_constructor_in_constructor_field_initializer()
     {
        check(
            &[],
            "\
class C(int a) {
  final int f;
  this : f = a + 1;
}
",
            Get::CfiExpr("f ="),
            &[("simple", "a +")],
        );
    }
    /// Dart `test_simpleIdentifier_parameterOfNotConstPrimaryConstructor_inFieldInitializer`.
    #[test]
    fn simple_identifier_parameter_of_not_const_primary_constructor_in_field_initializer() {
        check(
            &[],
            "\
class C(int a) {
  final int f = a + 1;
}
",
            Get::VarInit("f ="),
            &[("simple", "a +")],
        );
    }
    /// Dart `test_simpleIdentifier_parameterOfNotConstSecondaryConstructor`.
    #[test]
    fn simple_identifier_parameter_of_not_const_secondary_constructor() {
        check(
            &[],
            "\
class C {
  final int f;
  C(int a) : f = a + 1;
}
",
            Get::CfiExpr("f ="),
            &[("simple", "a +")],
        );
    }
    /// Dart `test_simpleIdentifier_topVar_const`.
    #[test]
    fn simple_identifier_top_var_const() {
        check(
            &[],
            "\
const a = 0;
var x = a + 1;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_simpleIdentifier_topVar_final`.
    #[test]
    fn simple_identifier_top_var_final() {
        check(
            &[],
            "\
final a = 0;
var x = a + 1;
",
            Get::XInit,
            &[("simple", "a +")],
        );
    }
    /// Dart `test_simpleIdentifier_typedef_functionType`.
    #[test]
    fn simple_identifier_typedef_function_type() {
        check(
            &[],
            "\
typedef A = void Function();
var x = A;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_simpleIdentifier_typedef_interfaceType`.
    #[test]
    fn simple_identifier_typedef_interface_type() {
        check(
            &[],
            "\
typedef A = List<int>;
var x = A;
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_spreadElement`.
    #[test]
    fn spread_element() {
        check(
            &[],
            "\
const a = [0, 1, 2];
var x = const [...a];
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_spreadElement_final`.
    #[test]
    fn spread_element_final() {
        check(
            &[],
            "\
final a = [0, 1, 2];
var x = const [...a];
",
            Get::XInit,
            &[("simple", "a];")],
        );
    }
    /// Dart `test_stringInterpolation_topVar_const`.
    #[test]
    fn string_interpolation_top_var_const() {
        check(
            &[],
            "\
const a = 0;
var x = 'a $a b';
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_stringInterpolation_topVar_final`.
    #[test]
    fn string_interpolation_top_var_final() {
        check(
            &[],
            "\
final a = 0;
var x = 'a $a b';
",
            Get::XInit,
            &[("simple", "a b")],
        );
    }
    /// Dart `test_stringLiteral`.
    #[test]
    fn string_literal() {
        check(
            &[],
            "\
var x = 'a';
",
            Get::XInit,
            &[],
        );
    }
    /// Dart `test_typeLiteral`.
    #[test]
    fn type_literal() {
        check(
            &[],
            "\
class A {
  Type x;
  const A(): x = List<int>;
}
",
            Get::Find("typeLiteral", "List<int>"),
            &[],
        );
    }
    /// Dart `test_typeLiteral_nonConst`.
    #[test]
    fn type_literal_non_const() {
        check(
            &[],
            "\
import '' deferred as self;
class A {
  Type x;
  const A(): x = List<self.A>;
}
",
            Get::Find("typeLiteral", "List<self.A>"),
            &[("typeAnnotation", "self.A")],
        );
    }
    /// Dart `test_typeLiteral_typeParameter_class`.
    #[test]
    fn type_literal_type_parameter_class() {
        check(
            &[],
            "\
class A<T> {
  final Object f;
  A() : f = T;
}
",
            Get::Find("typeLiteral", "T;"),
            &[],
        );
    }
    /// Dart `test_typeLiteral_typeParameter_class_214`.
    #[test]
    fn type_literal_type_parameter_class_214() {
        check(
            &[],
            "\
// @dart = 2.14
class A<T> {
  final Object f;
  A() : f = T;
}
",
            Get::Find("typeLiteral", "T;"),
            &[("typeLiteral", "T;")],
        );
    }
}
