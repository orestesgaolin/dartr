// Dart source: pkg/analysis_server/lib/src/plugin2/generator.dart
// Dart source: pkg/analyzer/lib/src/analysis_options/analysis_options.dart
// (PluginSource.toYaml, PluginConfiguration.sourceYaml)

//! The synthetic plugin package (Dart `PluginPackageGenerator`): a pubspec
//! that depends on `analysis_server_plugin` and on every configured plugin,
//! and the entrypoint `bin/plugin.dart` that the bridge spawns.

use dartr_project::analysis_options::{PluginConfiguration, PluginSource};

/// Dart `PluginSource.toYaml(name:)`.
pub fn source_yaml(name: &str, source: &PluginSource) -> String {
    match source {
        PluginSource::Versioned {
            constraint,
            hosted_url: None,
        } => format!("  {name}: {constraint}\n"),
        PluginSource::Versioned {
            constraint,
            hosted_url: Some(hosted),
        } => format!("  {name}:\n    version: {constraint}\n    hosted: {hosted}\n"),
        PluginSource::Git {
            url,
            path,
            git_ref,
            tag_pattern,
        } => {
            let mut buffer = format!("  {name}:\n    git:\n      url: {url}\n");
            if let Some(r) = git_ref {
                buffer.push_str(&format!("      ref: {r}\n"));
            }
            if let Some(p) = path {
                buffer.push_str(&format!("      path: {p}\n"));
            }
            if let Some(t) = tag_pattern {
                buffer.push_str(&format!("      tag_pattern: {t}\n"));
            }
            buffer
        }
        PluginSource::Path { path } => format!("  {name}:\n    path: {path}\n"),
    }
}

/// Dart `PluginPackageGenerator.generateEntrypoint`.
pub fn generate_entrypoint(configurations: &[PluginConfiguration]) -> String {
    let mut imports: Vec<String> = vec![
        "'package:analysis_server_plugin/src/plugin_server.dart'".to_string(),
        "'package:analyzer/file_system/physical_file_system.dart'".to_string(),
        "'package:analyzer_plugin/src/channel/isolate_channel.dart'".to_string(),
    ];
    for configuration in configurations {
        imports.push(format!(
            "'package:{0}/main.dart' as {0}",
            configuration.name
        ));
    }
    // Dart `List.sort` of strings: by UTF-16 code units.
    imports.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let mut buffer = String::from("import 'dart:isolate';\n");
    for import in &imports {
        buffer.push_str(&format!("import {import};\n"));
    }
    buffer.push_str(
        "Future<void> main(List<String> args, SendPort sendPort) async {
  var pluginServer = PluginServer.new2(
    resourceProvider: PhysicalResourceProvider.INSTANCE,
    plugins: {
",
    );
    for configuration in configurations {
        buffer.push_str(&format!("      '{0}': {0}.plugin,\n", configuration.name));
    }
    buffer.push_str(
        "    },
  );
  await pluginServer.initialize();
  var channel = PluginIsolateChannel(sendPort);
  pluginServer.start(channel);
}
",
    );
    buffer
}

/// Dart `PluginPackageGenerator.generatePubspec`.
pub fn generate_pubspec(
    configurations: &[PluginConfiguration],
    dependency_overrides: Option<&[(String, PluginSource)]>,
) -> String {
    let mut buffer = String::from(
        "name: plugin_entrypoint
version: 0.0.1
environment:
  sdk: ^3.6.0
dependencies:
  # The version of the analysis_server_plugin package that matches the protocol
  # used by the active analysis_server.
  analysis_server_plugin: ^0.3.8
",
    );
    for configuration in configurations {
        buffer.push_str(&source_yaml(&configuration.name, &configuration.source));
    }
    if let Some(overrides) = dependency_overrides {
        buffer.push_str("dependency_overrides:\n");
        for (name, source) in overrides {
            buffer.push_str(&source_yaml(name, source));
        }
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configuration(name: &str, source: PluginSource) -> PluginConfiguration {
        PluginConfiguration {
            name: name.to_string(),
            source,
            diagnostic_configs: Vec::new(),
        }
    }

    #[test]
    fn pubspec_and_entrypoint_like_the_server() {
        let configurations = [
            configuration(
                "jaspr_lints",
                PluginSource::Versioned {
                    constraint: "^0.7.2".into(),
                    hosted_url: None,
                },
            ),
            configuration(
                "my_plugin",
                PluginSource::Path {
                    path: "/p/my".into(),
                },
            ),
        ];
        assert_eq!(
            generate_pubspec(&configurations, None),
            "name: plugin_entrypoint
version: 0.0.1
environment:
  sdk: ^3.6.0
dependencies:
  # The version of the analysis_server_plugin package that matches the protocol
  # used by the active analysis_server.
  analysis_server_plugin: ^0.3.8
  jaspr_lints: ^0.7.2
  my_plugin:
    path: /p/my
"
        );
        assert_eq!(
            generate_entrypoint(&configurations),
            "import 'dart:isolate';
import 'package:analysis_server_plugin/src/plugin_server.dart';
import 'package:analyzer/file_system/physical_file_system.dart';
import 'package:analyzer_plugin/src/channel/isolate_channel.dart';
import 'package:jaspr_lints/main.dart' as jaspr_lints;
import 'package:my_plugin/main.dart' as my_plugin;
Future<void> main(List<String> args, SendPort sendPort) async {
  var pluginServer = PluginServer.new2(
    resourceProvider: PhysicalResourceProvider.INSTANCE,
    plugins: {
      'jaspr_lints': jaspr_lints.plugin,
      'my_plugin': my_plugin.plugin,
    },
  );
  await pluginServer.initialize();
  var channel = PluginIsolateChannel(sendPort);
  pluginServer.start(channel);
}
"
        );
    }
}
