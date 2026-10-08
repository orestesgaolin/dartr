// Dart source: pkg/analyzer/lib/dart/analysis/declared_variables.dart

//! [`DeclaredVariables`]: the `-D` variables of the command line.

use indexmap::IndexMap;

/// Dart `DeclaredVariables`: an object used to provide access to the values
/// of variables that have been defined on the command line using the `-D`
/// option.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeclaredVariables {
    /// A table mapping the names of declared variables to their values.
    declared_variables: IndexMap<String, String>,
}

impl DeclaredVariables {
    /// Dart `DeclaredVariables()`: no variables.
    pub fn new() -> DeclaredVariables {
        DeclaredVariables::default()
    }

    /// Dart `DeclaredVariables.fromMap`.
    pub fn from_map(
        variable_map: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> DeclaredVariables {
        DeclaredVariables {
            declared_variables: variable_map
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }

    /// Dart `variableNames`.
    pub fn variable_names(&self) -> impl Iterator<Item = &str> {
        self.declared_variables.keys().map(String::as_str)
    }

    /// Dart `get`: the raw string value of the variable with the given
    /// [name], or `None` if the variable is not defined.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.declared_variables.get(name).map(String::as_str)
    }
}
