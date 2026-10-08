// Dart source: pkg/_fe_analyzer_shared/lib/src/parser/member_kind.dart

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemberKind {
    /// A catch block, not a real member.
    Catch,
    /// A factory
    Factory,
    /// Old-style typedef.
    FunctionTypeAlias,
    /// Old-style function-typed parameter, not a real member.
    FunctionTypedParameter,
    /// A generalized function type, not a real member.
    GeneralizedFunctionType,
    /// A local function.
    Local,
    /// An anonymous method.
    AnonymousMethod,
    /// A non-static method in a class (including constructors).
    NonStaticMethod,
    /// A static method in a class.
    StaticMethod,
    /// A top-level method.
    TopLevelMethod,
    /// A non-static method in an extension.
    ExtensionNonStaticMethod,
    /// A static method in an extension.
    ExtensionStaticMethod,
    /// A non-static method in an extension type.
    ExtensionTypeNonStaticMethod,
    /// A static method in an extension type.
    ExtensionTypeStaticMethod,
    /// An instance field in a class.
    NonStaticField,
    /// A static field in a class.
    StaticField,
    /// A top-level field.
    TopLevelField,
    /// A primary constructor.
    PrimaryConstructor,
}

impl MemberKind {
    /// Dart `Enum.name`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Catch => "Catch",
            Self::Factory => "Factory",
            Self::FunctionTypeAlias => "FunctionTypeAlias",
            Self::FunctionTypedParameter => "FunctionTypedParameter",
            Self::GeneralizedFunctionType => "GeneralizedFunctionType",
            Self::Local => "Local",
            Self::AnonymousMethod => "AnonymousMethod",
            Self::NonStaticMethod => "NonStaticMethod",
            Self::StaticMethod => "StaticMethod",
            Self::TopLevelMethod => "TopLevelMethod",
            Self::ExtensionNonStaticMethod => "ExtensionNonStaticMethod",
            Self::ExtensionStaticMethod => "ExtensionStaticMethod",
            Self::ExtensionTypeNonStaticMethod => "ExtensionTypeNonStaticMethod",
            Self::ExtensionTypeStaticMethod => "ExtensionTypeStaticMethod",
            Self::NonStaticField => "NonStaticField",
            Self::StaticField => "StaticField",
            Self::TopLevelField => "TopLevelField",
            Self::PrimaryConstructor => "PrimaryConstructor",
        }
    }
}
