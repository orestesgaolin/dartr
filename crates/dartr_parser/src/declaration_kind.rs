// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/declaration_kind.dart

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeclarationKind {
    /// A top level declaration.
    TopLevel,
    /// A class declaration. Not including a named mixin declaration.
    Class,
    /// A mixin declaration. Not including a named mixin declaration.
    Mixin,
    /// An extension declaration.
    Extension,
    /// An extension type declaration.
    ExtensionType,
    /// An enum.
    Enum,
}

impl DeclarationKind {
    /// Dart `Enum.name`.
    pub fn name(self) -> &'static str {
        match self {
            Self::TopLevel => "TopLevel",
            Self::Class => "Class",
            Self::Mixin => "Mixin",
            Self::Extension => "Extension",
            Self::ExtensionType => "ExtensionType",
            Self::Enum => "Enum",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeclarationHeaderKind {
    Class,
    ExtensionType,
}

impl DeclarationHeaderKind {
    /// Dart `Enum.name`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Class => "Class",
            Self::ExtensionType => "ExtensionType",
        }
    }
}
