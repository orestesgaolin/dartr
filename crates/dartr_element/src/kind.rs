// Dart source: pkg/analyzer/lib/dart/element/element.dart (ElementKind) and
// the `kind` getters of the *ElementImpl classes in
// pkg/analyzer/lib/src/dart/element/element.dart

//! Dart `ElementKind` (the public kind of an element, used in messages and
//! by clients). The storage kind is [`crate::Tag`].

use crate::ids::{ElementId, Tag};

macro_rules! element_kinds {
    ($($name:ident = $ordinal:literal, $dart:literal, $display:literal;)*) => {
        /// Dart `ElementKind`.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum ElementKind {
            $($name,)*
        }

        impl ElementKind {
            /// All kinds, in ordinal order.
            pub const ALL: &'static [ElementKind] = &[$(ElementKind::$name,)*];

            /// Dart `name` (`CLASS`, `TOP_LEVEL_VARIABLE`, ...).
            pub fn name(self) -> &'static str {
                match self {
                    $(ElementKind::$name => $dart,)*
                }
            }

            /// Dart `ordinal`.
            pub fn ordinal(self) -> u32 {
                match self {
                    $(ElementKind::$name => $ordinal,)*
                }
            }

            /// Dart `displayName`.
            pub fn display_name(self) -> &'static str {
                match self {
                    $(ElementKind::$name => $display,)*
                }
            }
        }
    };
}

element_kinds! {
    AugmentationImport = 0, "AUGMENTATION_IMPORT", "augmentation import";
    Class = 1, "CLASS", "class";
    ClassAugmentation = 2, "CLASS_AUGMENTATION", "class augmentation";
    CompilationUnit = 3, "COMPILATION_UNIT", "compilation unit";
    Constructor = 4, "CONSTRUCTOR", "constructor";
    Dynamic = 5, "DYNAMIC", "<dynamic>";
    Enum = 6, "ENUM", "enum";
    Error = 7, "ERROR", "<error>";
    Export = 8, "EXPORT", "export directive";
    Extension = 9, "EXTENSION", "extension";
    ExtensionType = 10, "EXTENSION_TYPE", "extension type";
    Field = 11, "FIELD", "field";
    Function = 12, "FUNCTION", "function";
    GenericFunctionType = 13, "GENERIC_FUNCTION_TYPE", "generic function type";
    Getter = 14, "GETTER", "getter";
    Import = 15, "IMPORT", "import directive";
    Label = 16, "LABEL", "label";
    Library = 17, "LIBRARY", "library";
    LibraryAugmentation = 18, "LIBRARY_AUGMENTATION", "library augmentation";
    LocalVariable = 19, "LOCAL_VARIABLE", "local variable";
    Method = 20, "METHOD", "method";
    Mixin = 21, "MIXIN", "mixin";
    Name = 22, "NAME", "<name>";
    Never = 23, "NEVER", "<never>";
    Parameter = 24, "PARAMETER", "parameter";
    Part = 25, "PART", "part";
    Prefix = 26, "PREFIX", "import prefix";
    Record = 27, "RECORD", "record";
    Setter = 28, "SETTER", "setter";
    TopLevelVariable = 29, "TOP_LEVEL_VARIABLE", "top level variable";
    FunctionTypeAlias = 30, "FUNCTION_TYPE_ALIAS", "function type alias";
    TypeParameter = 31, "TYPE_PARAMETER", "type parameter";
    TypeAlias = 32, "TYPE_ALIAS", "type alias";
    Universe = 33, "UNIVERSE", "<universe>";
}

impl Tag {
    /// The `kind` getter of the Dart class of this tag.
    pub fn element_kind(self) -> ElementKind {
        match self {
            Tag::Class => ElementKind::Class,
            Tag::Enum => ElementKind::Enum,
            Tag::Mixin => ElementKind::Mixin,
            Tag::Extension => ElementKind::Extension,
            Tag::ExtensionType => ElementKind::ExtensionType,
            Tag::Field => ElementKind::Field,
            Tag::Getter => ElementKind::Getter,
            Tag::Setter => ElementKind::Setter,
            Tag::Method => ElementKind::Method,
            Tag::Constructor => ElementKind::Constructor,
            Tag::TopLevelFunction | Tag::LocalFunction => ElementKind::Function,
            Tag::TopLevelVariable => ElementKind::TopLevelVariable,
            Tag::TypeAlias => ElementKind::TypeAlias,
            Tag::TypeParameter => ElementKind::TypeParameter,
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                ElementKind::Parameter
            }
            Tag::Prefix => ElementKind::Prefix,
            Tag::Library => ElementKind::Library,
            Tag::GenericFunctionType => ElementKind::GenericFunctionType,
            Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable => ElementKind::LocalVariable,
            Tag::Label => ElementKind::Label,
            Tag::MultiplyDefined => ElementKind::Error,
            Tag::Dynamic => ElementKind::Dynamic,
            Tag::Never => ElementKind::Never,
        }
    }
}

impl ElementId {
    /// Dart `element.kind`.
    pub fn kind(self) -> ElementKind {
        self.tag().element_kind()
    }
}
