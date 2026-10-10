import 'package:analyzer/analysis_rule/analysis_rule.dart';
import 'package:analyzer/analysis_rule/rule_context.dart';
import 'package:analyzer/analysis_rule/rule_visitor_registry.dart';
import 'package:analyzer/dart/ast/ast.dart';
import 'package:analyzer/dart/ast/visitor.dart';
import 'package:analyzer/error/error.dart';

class NoPrintCalls extends AnalysisRule {
  static const LintCode code = LintCode(
    'no_print_calls',
    "Don't call 'print'.",
    correctionMessage: 'Try using a logger.',
  );

  NoPrintCalls()
    : super(name: 'no_print_calls', description: "Don't call 'print'.");

  @override
  LintCode get diagnosticCode => code;

  @override
  void registerNodeProcessors(
    RuleVisitorRegistry registry,
    RuleContext context,
  ) {
    registry.addMethodInvocation(this, _Visitor(this));
  }
}

class _Visitor extends SimpleAstVisitor<void> {
  final NoPrintCalls rule;

  _Visitor(this.rule);

  @override
  void visitMethodInvocation(MethodInvocation node) {
    if (node.target == null && node.methodName.name == 'print') {
      rule.reportAtNode(node.methodName);
    }
  }
}
