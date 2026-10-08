// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/assert.dart

/// Syntactic forms of `assert`.
///
/// An assertion can legally occur as a statement. However, assertions are also
/// experimentally allowed in initializers. For improved error recovery, we
/// also parse asserts as expressions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Assert {
    Expression,
    Initializer,
    Statement,
}

impl Assert {
    /// Dart `Enum.name`.
    pub fn name(self) -> &'static str {
        match self {
            Assert::Expression => "Expression",
            Assert::Initializer => "Initializer",
            Assert::Statement => "Statement",
        }
    }
}
