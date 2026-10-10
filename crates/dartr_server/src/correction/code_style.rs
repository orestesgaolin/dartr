// Dart source: pkg/analyzer/lib/src/analysis_options/code_style_options.dart

//! Dart `CodeStyleOptions`: the code style that the analysis options imply
//! (through enabled lint rules).

use dartr_ast::*;
use dartr_project::AnalysisOptions;

/// Dart `CodeStyleOptionsImpl`.
pub struct CodeStyleOptions<'a> {
    pub options: &'a AnalysisOptions,
}

impl CodeStyleOptions<'_> {
    /// Dart `isLintEnabled`.
    pub fn is_lint_enabled(&self, name: &str) -> bool {
        self.options.lint_rules.iter().any(|r| r == name)
    }

    pub fn add_trailing_commas(&self) -> bool {
        self.is_lint_enabled("require_trailing_commas")
    }

    pub fn make_locals_final(&self) -> bool {
        self.is_lint_enabled("prefer_final_locals")
    }

    pub fn prefer_const_declarations(&self) -> bool {
        self.is_lint_enabled("prefer_const_declarations")
    }

    pub fn preferred_quote_for_strings(&self) -> &'static str {
        self.lint_quote().unwrap_or("'")
    }

    pub fn sort_combinators(&self) -> bool {
        self.is_lint_enabled("combinators_ordering")
    }

    pub fn sort_constructors_first(&self) -> bool {
        self.is_lint_enabled("sort_constructors_first")
    }

    pub fn sort_unnamed_constructors_first(&self) -> bool {
        self.is_lint_enabled("sort_unnamed_constructors_first")
    }

    pub fn specify_types(&self) -> bool {
        self.is_lint_enabled("always_specify_types")
    }

    pub fn specify_return_types(&self) -> bool {
        self.is_lint_enabled("always_declare_return_types") || self.specify_types()
    }

    pub fn use_package_uris(&self) -> bool {
        self.is_lint_enabled("always_use_package_imports")
    }

    pub fn use_relative_uris(&self) -> bool {
        self.is_lint_enabled("prefer_relative_imports")
    }

    pub fn required_named_parameters_first(&self) -> bool {
        self.is_lint_enabled("always_put_required_named_parameters_first")
    }

    /// Dart `preferredQuoteForUris`.
    pub fn preferred_quote_for_uris(&self, ast: &Ast, directives: &[NodeId]) -> &'static str {
        if let Some(q) = self.lint_quote() {
            return q;
        }
        let mut single = 0;
        let mut double = 0;
        let mut add = |s: Id<SimpleStringLiteral>| {
            let lexeme = ast.tokens.lexeme(ast[s].literal);
            if lexeme.starts_with('"') {
                double += 1;
            } else {
                single += 1;
            }
        };
        for &d in directives {
            let uri = if let Some(i) = ast.cast::<ImportDirective>(d) {
                ast[i].uri
            } else if let Some(e) = ast.cast::<ExportDirective>(d) {
                ast[e].uri
            } else {
                continue;
            };
            if let Some(s) = ast.cast::<SimpleStringLiteral>(uri) {
                add(s);
            } else if let Some(a) = ast.cast::<AdjacentStrings>(uri) {
                for &s in ast.list(ast[a].strings) {
                    if let Some(s) = ast.cast::<SimpleStringLiteral>(s) {
                        add(s);
                    }
                }
            }
        }
        if double > single { "\"" } else { "'" }
    }

    fn lint_quote(&self) -> Option<&'static str> {
        if self.is_lint_enabled("prefer_single_quotes") {
            Some("'")
        } else if self.is_lint_enabled("prefer_double_quotes") {
            Some("\"")
        } else {
            None
        }
    }
}
