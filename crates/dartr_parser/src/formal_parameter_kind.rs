// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/formal_parameter_kind.dart

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FormalParameterKind {
    RequiredPositional,
    RequiredNamed,
    OptionalNamed,
    OptionalPositional,
}

impl FormalParameterKind {
    /// Dart `Enum.name`.
    pub fn name(self) -> &'static str {
        match self {
            Self::RequiredPositional => "requiredPositional",
            Self::RequiredNamed => "requiredNamed",
            Self::OptionalNamed => "optionalNamed",
            Self::OptionalPositional => "optionalPositional",
        }
    }

    pub fn is_required_positional(self) -> bool {
        self == Self::RequiredPositional
    }

    pub fn is_optional_named(self) -> bool {
        self == Self::OptionalNamed
    }

    pub fn is_optional_positional(self) -> bool {
        self == Self::OptionalPositional
    }

    pub fn is_required_named(self) -> bool {
        self == Self::RequiredNamed
    }

    pub fn is_required(self) -> bool {
        self.is_required_positional() || self.is_required_named()
    }

    pub fn is_optional(self) -> bool {
        self.is_optional_positional() || self.is_optional_named()
    }

    pub fn is_positional(self) -> bool {
        self.is_required_positional() || self.is_optional_positional()
    }

    pub fn is_named(self) -> bool {
        self.is_required_named() || self.is_optional_named()
    }
}
