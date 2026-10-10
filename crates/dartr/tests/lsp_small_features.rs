//! Differential parity tests for the small LSP features against `dart
//! language-server` (SDK 3.13.3):
//! - `textDocument/documentColor` and `textDocument/colorPresentation`
//! - `textDocument/documentLink` (Dart files, `pubspec.yaml`, `analysis_options.yaml`)
//! - `textDocument/codeLens` (augmentation/augmented commands)
//! - `textDocument/inlineValue` (variables, parameters, properties)
//! - `dart/textDocument/super`, `dart/textDocument/augmentation`,
//!   `dart/textDocument/augmented`, and `dart/updateDiagnosticInformation`
//! - `dart/textDocument/publishFlutterOutline` and Flutter-enriched
//!   `dart/textDocument/publishOutline`

mod lsp_support;

use std::fs;
use std::path::{Path, PathBuf};

use lsp_support::*;
use serde_json::{Value, json};

struct TempFixture {
    root: PathBuf,
}

impl TempFixture {
    fn new() -> Self {
        let unique = format!(
            "dartr_lsp_small_features_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        fs::create_dir_all(&root).unwrap();
        let fixture = TempFixture { root };
        fixture.populate();
        fixture
    }

    fn write(&self, rel: &str, content: &str) -> PathBuf {
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, content).unwrap();
        p
    }

    fn populate(&self) {
        self.write(
            ".dart_tool/package_config.json",
            r#"{
  "configVersion": 2,
  "packages": [
    {
      "name": "sample",
      "rootUri": "..",
      "packageUri": "lib/",
      "languageVersion": "3.13"
    },
    {
      "name": "flutter",
      "rootUri": "../packages/flutter",
      "packageUri": "lib/",
      "languageVersion": "3.13"
    }
  ]
}
"#,
        );

        self.write(
            "pubspec.yaml",
            r#"name: sample
version: 1.0.0
environment:
  sdk: ^3.11.0
dependencies:
  flutter:
    path: packages/flutter
  http: ^1.0.0
  path:
    hosted: https://pub.dev
    version: ^1.8.0
dev_dependencies:
  lints: any
"#,
        );

        self.write(
            "analysis_options.yaml",
            r#"analyzer:
  enable-experiment:
    - augmentations
linter:
  rules:
    - prefer_single_quotes
    - avoid_print
    - cancel_subscriptions
"#,
        );

        self.write("examples/api/lib/material/app.0.dart", "void main() {}\n");

        // Mock Flutter package matching exact URIs checked by Dart's Flutter / Color utilities.
        self.write(
            "packages/flutter/pubspec.yaml",
            "name: flutter\nversion: 1.0.0\nenvironment:\n  sdk: ^3.11.0\n",
        );

        self.write(
            "packages/flutter/lib/ui.dart",
            r#"library dart.ui;

class Color {
  final double a;
  final double r;
  final double g;
  final double b;

  const Color(int value)
      : a = ((value >> 24) & 0xff) / 255.0,
        r = ((value >> 16) & 0xff) / 255.0,
        g = ((value >> 8) & 0xff) / 255.0,
        b = (value & 0xff) / 255.0;

  const Color.fromARGB(int a, int r, int g, int b)
      : a = (a & 0xff) / 255.0,
        r = (r & 0xff) / 255.0,
        g = (g & 0xff) / 255.0,
        b = (b & 0xff) / 255.0;

  const Color.fromRGBO(int r, int g, int b, double opacity)
      : a = opacity,
        r = (r & 0xff) / 255.0,
        g = (g & 0xff) / 255.0,
        b = (b & 0xff) / 255.0;

  const Color.from({
    required double alpha,
    required double red,
    required double green,
    required double blue,
  }) : a = alpha,
       r = red,
       g = green,
       b = blue;
}
"#,
        );

        self.write(
            "packages/flutter/lib/src/widgets/framework.dart",
            r#"class Key {
  final String value;
  const Key(this.value);
}

class BuildContext {}

abstract class Widget {
  final Key? key;
  const Widget({this.key});
}

abstract class StatelessWidget extends Widget {
  const StatelessWidget({super.key});
  Widget build(BuildContext context);
}

abstract class StatefulWidget extends Widget {
  const StatefulWidget({super.key});
  State createState();
}

abstract class State<T extends StatefulWidget> {
  Widget build(BuildContext context);
}
"#,
        );

        self.write(
            "packages/flutter/lib/src/widgets/basic.dart",
            r#"import 'framework.dart';

class Column extends Widget {
  final List<Widget> children;
  const Column({super.key, this.children = const <Widget>[]});
}

class Row extends Widget {
  final List<Widget> children;
  const Row({super.key, this.children = const <Widget>[]});
}

class Padding extends Widget {
  final Widget? child;
  const Padding({super.key, this.child});
}
"#,
        );

        self.write(
            "packages/flutter/lib/src/widgets/text.dart",
            r#"import 'framework.dart';

class Text extends Widget {
  final String data;
  const Text(this.data, {super.key});
}
"#,
        );

        self.write(
            "packages/flutter/lib/src/widgets/container.dart",
            r#"import '../../ui.dart';
import 'framework.dart';

class Container extends Widget {
  final Color? color;
  final Widget? child;
  const Container({super.key, this.color, this.child});
}
"#,
        );

        self.write(
            "packages/flutter/lib/widgets.dart",
            r#"export 'ui.dart' show Color;
export 'src/widgets/framework.dart';
export 'src/widgets/basic.dart';
export 'src/widgets/text.dart';
export 'src/widgets/container.dart';
"#,
        );

        self.write(
            "packages/flutter/lib/src/painting/colors.dart",
            r#"import '../../ui.dart';

class ColorSwatch<T> extends Color {
  final Map<T, Color> _swatch;
  const ColorSwatch(super.primary, this._swatch);
  Color? operator [](T index) => _swatch[index];
}
"#,
        );

        self.write(
            "packages/flutter/lib/src/material/colors.dart",
            r#"import '../../ui.dart';
import '../painting/colors.dart';

class MaterialColor extends ColorSwatch<int> {
  const MaterialColor(super.primary, super.swatch);
  Color get shade500 => this[500]!;
}

class Colors {
  static const Color red = Color(0xFFFF0000);
  static const Color green = Color.fromARGB(255, 0, 255, 0);
  static const MaterialColor primaryRed = MaterialColor(
    0xFFFF0000,
    <int, Color>{
      500: Color(0xFFFF5500),
    },
  );
}
"#,
        );

        self.write(
            "packages/flutter/lib/material.dart",
            r#"export 'widgets.dart';
export 'src/painting/colors.dart';
export 'src/material/colors.dart';
"#,
        );

        self.write(
            "lib/main.dart",
            r#"import 'package:flutter/material.dart';

part 'main_aug.dart';

/// Sample documentation with a link to https://dart.dev/guides and a tool block:
/// {@tool dartpad}
/// ** See code in examples/api/lib/material/app.0.dart **
/// {@end-tool}
class BaseClass {
  final int baseField;
  const BaseClass(this.baseField);

  String get label => 'base';

  int compute(int x) => x;
}

class SubClass extends BaseClass {
  const SubClass(super.baseField);

  @override
  String get label => 'sub';

  @override
  int compute(int x) => x + 1;

  int augmentedMethod(int a) => a;
}

const Color kConstRed = Color(0xFFFF0000);
const Color kArgbBlue = Color.fromARGB(255, 0, 128, 255);
const Color kRgboColor = Color.fromRGBO(10, 20, 30, 0.5);
const Color kMaterialRed = Colors.red;
final Color kShadeColor = Colors.primaryRed.shade500;
final Color? kIndexedColor = Colors.primaryRed[500];

int inlineValuesTarget(int paramA, String paramB, BaseClass holder) {
  var localSum = paramA + 1;
  for (var i = 0; i < 3; i++) {
    localSum += i;
  }
  if (localSum > 0) {
    var branchText = paramB;
    localSum += branchText.length + holder.baseField;
  }
  return localSum;
}

class MyCard extends StatelessWidget {
  final String title;
  const MyCard(this.title, {super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      color: Colors.red,
      child: Column(
        children: <Widget>[
          Text(title),
          const Padding(
            child: Text('Nested'),
          ),
        ],
      ),
    );
  }
}
"#,
        );

        self.write(
            "lib/main_aug.dart",
            r#"part of 'main.dart';

augment class SubClass {
  augment int augmentedMethod(int a) => a * 2;
}
"#,
        );
    }
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn spawn_initialized(program: &str, root: &Path) -> LspClient {
    let args = [
        "language-server",
        "--protocol=lsp",
        "--client-id=VS-Code",
        "--client-version=3.144.0",
    ];
    let mut c = LspClient::spawn(program, &args, &[]);
    c.configuration = json!({
        "codeLens": {
            "augmentation": true,
            "augmented": true,
        },
        "experimentalInlineValuesProperties": true,
    });
    let mut init_params = dart_code_initialize_params(root);
    init_params["initializationOptions"]["outline"] = json!(true);
    init_params["initializationOptions"]["flutterOutline"] = json!(true);
    let init = c.request("initialize", init_params);
    assert!(init["result"]["capabilities"].is_object(), "{init}");
    c.notify("initialized", json!({}));
    c.settle(true);
    c
}

#[test]
fn small_lsp_features_match_dart_language_server() {
    if !dart_available() {
        eprintln!("skipped: `dart` is not on PATH");
        return;
    }

    let fixture = TempFixture::new();
    let main_path = fixture.root.join("lib/main.dart");
    let aug_path = fixture.root.join("lib/main_aug.dart");
    let pubspec_path = fixture.root.join("pubspec.yaml");
    let options_path = fixture.root.join("analysis_options.yaml");

    let main_uri = file_uri(&main_path);
    let aug_uri = file_uri(&aug_path);
    let pubspec_uri = file_uri(&pubspec_path);
    let options_uri = file_uri(&options_path);

    let main_text = fs::read_to_string(&main_path).unwrap();
    let aug_text = fs::read_to_string(&aug_path).unwrap();
    let pubspec_text = fs::read_to_string(&pubspec_path).unwrap();
    let options_text = fs::read_to_string(&options_path).unwrap();

    let run_session = |program: &str| {
        let mut c = spawn_initialized(program, &fixture.root);

        for (uri, lang, text) in [
            (&main_uri, "dart", &main_text),
            (&aug_uri, "dart", &aug_text),
            (&pubspec_uri, "yaml", &pubspec_text),
            (&options_uri, "yaml", &options_text),
        ] {
            c.notify(
                "textDocument/didOpen",
                json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": lang,
                        "version": 1,
                        "text": text,
                    }
                }),
            );
        }
        c.settle(true);

        // 1. textDocument/documentColor & textDocument/colorPresentation
        let colors = c.request(
            "textDocument/documentColor",
            json!({"textDocument": {"uri": main_uri}}),
        );
        let first_color_range = colors["result"][0]["range"].clone();
        let presentations = c.request(
            "textDocument/colorPresentation",
            json!({
                "textDocument": {"uri": main_uri},
                "color": {"alpha": 1.0, "red": 1.0, "green": 0.5, "blue": 0.0},
                "range": first_color_range,
            }),
        );
        let material_color_range = colors["result"][3]["range"].clone();
        let material_presentations = c.request(
            "textDocument/colorPresentation",
            json!({
                "textDocument": {"uri": main_uri},
                "color": {"alpha": 0.5, "red": 0.0, "green": 1.0, "blue": 0.25},
                "range": material_color_range,
            }),
        );

        // 2. textDocument/documentLink (Dart, pubspec.yaml, analysis_options.yaml)
        let dart_links = c.request(
            "textDocument/documentLink",
            json!({"textDocument": {"uri": main_uri}}),
        );
        let pubspec_links = c.request(
            "textDocument/documentLink",
            json!({"textDocument": {"uri": pubspec_uri}}),
        );
        let options_links = c.request(
            "textDocument/documentLink",
            json!({"textDocument": {"uri": options_uri}}),
        );

        // 3. textDocument/codeLens on main.dart and main_aug.dart
        let main_lenses = c.request(
            "textDocument/codeLens",
            json!({"textDocument": {"uri": main_uri}}),
        );
        let aug_lenses = c.request(
            "textDocument/codeLens",
            json!({"textDocument": {"uri": aug_uri}}),
        );

        // 4. textDocument/inlineValue inside `inlineValuesTarget`
        let inline_values = c.request(
            "textDocument/inlineValue",
            json!({
                "textDocument": {"uri": main_uri},
                "range": {
                    "start": {"line": 36, "character": 0},
                    "end": {"line": 47, "character": 0},
                },
                "context": {
                    "frameId": 1,
                    "stoppedLocation": {
                        "start": {"line": 45, "character": 2},
                        "end": {"line": 45, "character": 9},
                    },
                },
            }),
        );

        // 5. dart/textDocument/super, augmentation, augmented, updateDiagnosticInformation
        // Line 18: `const SubClass(super.baseField);` (character 9 is on `SubClass`)
        let super_ctor = c.request(
            "dart/textDocument/super",
            json!({
                "textDocument": {"uri": main_uri},
                "position": {"line": 18, "character": 9},
            }),
        );
        // Line 21: `String get label => 'sub';` (character 14 is on `label`)
        let super_getter = c.request(
            "dart/textDocument/super",
            json!({
                "textDocument": {"uri": main_uri},
                "position": {"line": 21, "character": 14},
            }),
        );
        // Line 24: `int compute(int x) => x + 1;` (character 8 is on `compute`)
        let super_method = c.request(
            "dart/textDocument/super",
            json!({
                "textDocument": {"uri": main_uri},
                "position": {"line": 24, "character": 8},
            }),
        );
        // Line 26: `int augmentedMethod(int a) => a;` (character 8 is on `augmentedMethod`)
        let augmentation_method = c.request(
            "dart/textDocument/augmentation",
            json!({
                "textDocument": {"uri": main_uri},
                "position": {"line": 26, "character": 8},
            }),
        );
        // In main_aug.dart line 3: `augment int augmentedMethod(int a) => a * 2;`
        let augmented_method = c.request(
            "dart/textDocument/augmented",
            json!({
                "textDocument": {"uri": aug_uri},
                "position": {"line": 3, "character": 16},
            }),
        );
        let update_diag = c.request(
            "dart/updateDiagnosticInformation",
            json!({"editors": [{"uri": main_uri}]}),
        );

        // 6. Outline and Flutter outline notifications
        let outline = c
            .state
            .outlines
            .get(&main_uri)
            .cloned()
            .unwrap_or(Value::Null);
        let flutter_outline = c
            .state
            .flutter_outlines
            .get(&main_uri)
            .cloned()
            .unwrap_or(Value::Null);

        let _ = c.shutdown_and_exit();
        json!({
            "documentColor": colors,
            "colorPresentation": presentations,
            "materialColorPresentation": material_presentations,
            "dartDocumentLink": dart_links,
            "pubspecDocumentLink": pubspec_links,
            "analysisOptionsDocumentLink": options_links,
            "mainCodeLens": main_lenses,
            "augCodeLens": aug_lenses,
            "inlineValue": inline_values,
            "superCtor": super_ctor,
            "superGetter": super_getter,
            "superMethod": super_method,
            "augmentation": augmentation_method,
            "augmented": augmented_method,
            "updateDiagnosticInformation": update_diag,
            "outline": outline,
            "flutterOutline": flutter_outline,
        })
    };

    let dart = run_session("dart");
    let dartr = run_session(env!("CARGO_BIN_EXE_dartr"));

    assert_eq!(dart["documentColor"]["result"].as_array().unwrap().len(), 7);
    assert_eq!(
        dart["colorPresentation"]["result"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        dart["materialColorPresentation"]["result"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert!(
        !dart["dartDocumentLink"]["result"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !dart["pubspecDocumentLink"]["result"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !dart["analysisOptionsDocumentLink"]["result"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !dart["mainCodeLens"]["result"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(!dart["augCodeLens"]["result"].as_array().unwrap().is_empty());
    assert!(!dart["inlineValue"]["result"].as_array().unwrap().is_empty());
    assert!(dart["superCtor"]["result"].is_object());
    assert!(dart["superGetter"]["result"].is_object());
    assert!(dart["superMethod"]["result"].is_object());
    assert!(dart["augmentation"]["result"].is_object());
    assert!(dart["augmented"]["result"].is_object());
    assert!(dart["outline"].is_object());
    assert!(dart["flutterOutline"].is_object());

    for key in [
        "documentColor",
        "colorPresentation",
        "materialColorPresentation",
        "dartDocumentLink",
        "pubspecDocumentLink",
        "analysisOptionsDocumentLink",
        "mainCodeLens",
        "augCodeLens",
        "inlineValue",
        "superCtor",
        "superGetter",
        "superMethod",
        "augmentation",
        "augmented",
        "updateDiagnosticInformation",
        "outline",
        "flutterOutline",
    ] {
        assert_eq!(
            dart[key],
            dartr[key],
            "mismatch for {key}:\n  dart:  {}\n  dartr: {}",
            serde_json::to_string_pretty(&dart[key]).unwrap(),
            serde_json::to_string_pretty(&dartr[key]).unwrap()
        );
    }
}
