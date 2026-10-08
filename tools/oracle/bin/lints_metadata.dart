// Dart source: pkg/linter/lib/src/rules.dart
import 'dart:convert';
import 'package:analyzer/src/lint/registry.dart';
import 'package:linter/src/rules.dart';
void main() {
  registerLintRules();
  print(jsonEncode([
    for (var rule in Registry.ruleRegistry.rules)
      {
        'name': rule.name,
        'description': rule.description,
        'state': rule.state.label,
        'since': rule.state.since?.toString(),
        'replacedBy': rule.state.replacedBy,
        'canUseParsedResult': rule.canUseParsedResult,
        'incompatibleRules': rule.incompatibleRules,
        'diagnosticCodes': [for (var code in rule.diagnosticCodes) code.lowerCaseUniqueName.split('.').last],
      },
  ]));
}
