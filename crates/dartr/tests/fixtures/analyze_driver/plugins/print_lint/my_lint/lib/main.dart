import 'package:analysis_server_plugin/plugin.dart';
import 'package:analysis_server_plugin/registry.dart';

import 'src/no_print_calls.dart';

final plugin = MyLintPlugin();

class MyLintPlugin extends Plugin {
  @override
  String get name => 'my_lint';

  @override
  void register(PluginRegistry registry) {
    registry.registerLintRule(NoPrintCalls());
  }
}
