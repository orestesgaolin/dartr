// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/constructor_reference_context.dart

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConstructorReferenceContext {
    /// The constructor reference is preceded by `new`.
    New,
    /// The constructor reference is preceded by `const`.
    Const,
    /// The constructor reference is the prefix of an implicit creation
    /// expression, like `Class.named` in `Class.named()`.
    Implicit,
    /// The constructor reference is the target of a redirecting factory, like
    /// `Class.named` in `factory Foo() = Class.named;`.
    RedirectingFactory,
}

impl ConstructorReferenceContext {
    /// Dart `Enum.name`.
    pub fn name(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Const => "Const",
            Self::Implicit => "Implicit",
            Self::RedirectingFactory => "RedirectingFactory",
        }
    }
}
