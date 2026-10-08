// Dart source: pkg/analyzer/lib/src/dart/element/element.dart (*FragmentImpl)

//! Fragment data structs: one struct per Dart `*FragmentImpl` class with
//! data, with the same composition and mutability rules as
//! [`crate::element`].

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use dartr_ast::NodeId;
use indexmap::IndexMap;
use parking_lot::Mutex;

use crate::data::{
    ConstExprId, JoinedPatternVariableInconsistency, LibraryExport, LibraryImport, Metadata,
    PartInclude, SourceRef,
};
use crate::element::{LibraryElement, PrefixElement};
use crate::ids::{
    EId, ElementId, FId, FragmentId, JoinPatternVariableFragment, PatternVariableFragment,
    PropertyInducingFragment,
};
use crate::name::Name;
use crate::slot::{BoolSlot, FragmentFlagCell, OnceSlot, VarSlot};
use crate::types::ParameterKind;

macro_rules! deref_base {
    ($t:ty => $field:ident: $base:ty) => {
        impl Deref for $t {
            type Target = $base;
            #[inline(always)]
            fn deref(&self) -> &$base {
                &self.$field
            }
        }
        impl DerefMut for $t {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut $base {
                &mut self.$field
            }
        }
    };
}

/// `FragmentImpl`: data of every fragment.
#[derive(Debug)]
pub struct FragmentData {
    /// The name. Most Dart subclasses declare their own `name` field.
    /// Dart: ConstructorFragmentImpl.name
    /// Dart: FormalParameterFragmentImpl.name
    /// Dart: FunctionFragmentImpl.name
    /// Dart: InstanceFragmentImpl.name
    /// Dart: LabelFragmentImpl.name
    /// Dart: LocalVariableFragmentImpl.name
    /// Dart: MethodFragmentImpl.name
    /// Dart: PrefixFragmentImpl.name
    /// Dart: PropertyAccessorFragmentImpl.name
    /// Dart: PropertyInducingFragmentImpl.name
    /// Dart: TypeAliasFragmentImpl.name
    /// Dart: TypeParameterFragmentImpl.name
    pub name: Option<Name>,
    /// Dart: ConstructorFragmentImpl.nameOffset
    /// Dart: FormalParameterFragmentImpl.nameOffset
    /// Dart: FunctionFragmentImpl.nameOffset
    /// Dart: InstanceFragmentImpl.nameOffset
    /// Dart: LocalVariableFragmentImpl.nameOffset
    /// Dart: MethodFragmentImpl.nameOffset
    /// Dart: PrefixFragmentImpl.nameOffset
    /// Dart: PropertyAccessorFragmentImpl.nameOffset
    /// Dart: PropertyInducingFragmentImpl.nameOffset
    /// Dart: TypeAliasFragmentImpl.nameOffset
    /// Dart: TypeParameterFragmentImpl.nameOffset
    pub name_offset: Option<u32>,
    /// The element of this fragment, set when the element is created (each
    /// Dart subclass has a typed `late final element`).
    /// Dart: ClassFragmentImpl.element
    /// Dart: ConstructorFragmentImpl.element
    /// Dart: EnumFragmentImpl.element
    /// Dart: ExtensionFragmentImpl.element
    /// Dart: ExtensionTypeFragmentImpl.element
    /// Dart: FieldFragmentImpl.element
    /// Dart: FormalParameterFragmentImpl._element
    /// Dart: GenericFunctionTypeFragmentImpl.element
    /// Dart: GetterFragmentImpl.element
    /// Dart: LabelFragmentImpl.element
    /// Dart: LibraryFragmentImpl.library
    /// Dart: LocalFunctionFragmentImpl.element
    /// Dart: LocalVariableFragmentImpl._element2
    /// Dart: MethodFragmentImpl.element
    /// Dart: MixinFragmentImpl.element
    /// Dart: MultiplyDefinedFragmentImpl.element
    /// Dart: PrefixFragmentImpl.element
    /// Dart: SetterFragmentImpl.element
    /// Dart: TopLevelFunctionFragmentImpl.element
    /// Dart: TopLevelVariableFragmentImpl.element
    /// Dart: TypeAliasFragmentImpl.element
    /// Dart: TypeParameterFragmentImpl.element
    pub element: OnceSlot<ElementId>,
    /// Dart: FragmentImpl.enclosingFragment
    pub enclosing_fragment: Option<FragmentId>,
    /// Dart: ConstructorFragmentImpl.previousFragment
    /// Dart: FormalParameterFragmentImpl.previousFragment
    /// Dart: GetterFragmentImpl.previousFragment
    /// Dart: InstanceFragmentImpl.previousFragment
    /// Dart: LocalFunctionFragmentImpl.previousFragment
    /// Dart: MethodFragmentImpl.previousFragment
    /// Dart: PrefixFragmentImpl.previousFragment
    /// Dart: PropertyInducingFragmentImpl.previousFragment
    /// Dart: SetterFragmentImpl.previousFragment
    /// Dart: TopLevelFunctionFragmentImpl.previousFragment
    /// Dart: TypeAliasFragmentImpl.previousFragment
    /// Dart: TypeParameterFragmentImpl.previousFragment
    pub previous_fragment: Option<FragmentId>,
    /// Dart: ConstructorFragmentImpl.nextFragment
    /// Dart: FormalParameterFragmentImpl.nextFragment
    /// Dart: GetterFragmentImpl.nextFragment
    /// Dart: InstanceFragmentImpl.nextFragment
    /// Dart: LocalFunctionFragmentImpl.nextFragment
    /// Dart: MethodFragmentImpl.nextFragment
    /// Dart: PrefixFragmentImpl.nextFragment
    /// Dart: PropertyInducingFragmentImpl.nextFragment
    /// Dart: SetterFragmentImpl.nextFragment
    /// Dart: TopLevelFunctionFragmentImpl.nextFragment
    /// Dart: TypeAliasFragmentImpl.nextFragment
    /// Dart: TypeParameterFragmentImpl.nextFragment
    pub next_fragment: Option<FragmentId>,
    /// Dart: FragmentImpl.firstTokenOffset
    pub first_token_offset: Option<u32>,
    /// Dart: FragmentImpl._flags
    pub flags: FragmentFlagCell,
    /// Dart: FragmentImpl.documentationComment
    pub documentation_comment: Option<Arc<str>>,
    /// Dart: FragmentImpl.metadata
    pub metadata: Metadata,
    /// Dart: FragmentImpl._codeOffset
    pub code_offset: Option<u32>,
    /// Dart: FragmentImpl._codeLength
    pub code_length: Option<u32>,
}

impl FragmentData {
    pub fn new(name: Option<Name>, name_offset: Option<u32>) -> FragmentData {
        FragmentData {
            name,
            name_offset,
            element: OnceSlot::new(),
            enclosing_fragment: None,
            previous_fragment: None,
            next_fragment: None,
            first_token_offset: None,
            flags: FragmentFlagCell::default(),
            documentation_comment: None,
            metadata: Metadata::default(),
            code_offset: None,
            code_length: None,
        }
    }

    #[inline(always)]
    pub fn fragment(&self) -> &FragmentData {
        self
    }
}

/// `InstanceFragmentImpl`.
#[derive(Debug)]
pub struct InstanceFragmentData {
    pub fragment: FragmentData,
    /// Dart: InstanceFragmentImpl._typeParameters
    pub type_params: Vec<FId<TypeParameterFragment>>,
    /// Dart: InstanceFragmentImpl._fields
    pub fields: Vec<FId<FieldFragment>>,
    /// Dart: InstanceFragmentImpl._getters
    pub getters: Vec<FId<GetterFragment>>,
    /// Dart: InstanceFragmentImpl._setters
    pub setters: Vec<FId<SetterFragment>>,
    /// Dart: InstanceFragmentImpl._methods
    pub methods: Vec<FId<MethodFragment>>,
}
deref_base!(InstanceFragmentData => fragment: FragmentData);

impl InstanceFragmentData {
    pub fn new(fragment: FragmentData) -> Self {
        InstanceFragmentData {
            fragment,
            type_params: Vec::new(),
            fields: Vec::new(),
            getters: Vec::new(),
            setters: Vec::new(),
            methods: Vec::new(),
        }
    }
}

/// `InterfaceFragmentImpl`.
#[derive(Debug)]
pub struct InterfaceFragmentData {
    pub instance: InstanceFragmentData,
    /// Dart: InterfaceFragmentImpl._constructors
    pub constructors: Vec<FId<ConstructorFragment>>,
}
deref_base!(InterfaceFragmentData => instance: InstanceFragmentData);

impl InterfaceFragmentData {
    pub fn new(fragment: FragmentData) -> Self {
        InterfaceFragmentData {
            instance: InstanceFragmentData::new(fragment),
            constructors: Vec::new(),
        }
    }
}

/// `ClassFragmentImpl`.
#[derive(Debug)]
pub struct ClassFragment {
    pub interface: InterfaceFragmentData,
}
deref_base!(ClassFragment => interface: InterfaceFragmentData);

/// `EnumFragmentImpl`.
#[derive(Debug)]
pub struct EnumFragment {
    pub interface: InterfaceFragmentData,
}
deref_base!(EnumFragment => interface: InterfaceFragmentData);

/// `MixinFragmentImpl`.
#[derive(Debug)]
pub struct MixinFragment {
    pub interface: InterfaceFragmentData,
    /// Set in link phase 7 (mixin super-invoked names).
    /// Dart: MixinFragmentImpl.superInvokedNames
    pub super_invoked_names: OnceSlot<Vec<Name>>,
}
deref_base!(MixinFragment => interface: InterfaceFragmentData);

/// `ExtensionTypeFragmentImpl`.
#[derive(Debug)]
pub struct ExtensionTypeFragment {
    pub interface: InterfaceFragmentData,
}
deref_base!(ExtensionTypeFragment => interface: InterfaceFragmentData);

/// `ExtensionFragmentImpl`.
#[derive(Debug)]
pub struct ExtensionFragment {
    pub instance: InstanceFragmentData,
}
deref_base!(ExtensionFragment => instance: InstanceFragmentData);

/// `ExecutableFragmentImpl` (with `FunctionTypedFragmentImpl`).
#[derive(Debug)]
pub struct ExecutableFragmentData {
    pub fragment: FragmentData,
    /// Dart: ExecutableFragmentImpl._typeParameters
    pub type_params: Vec<FId<TypeParameterFragment>>,
    /// Dart: ExecutableFragmentImpl._formalParameters
    pub formal_params: Vec<FId<FormalParameterFragment>>,
}
deref_base!(ExecutableFragmentData => fragment: FragmentData);

impl ExecutableFragmentData {
    pub fn new(fragment: FragmentData) -> Self {
        ExecutableFragmentData {
            fragment,
            type_params: Vec::new(),
            formal_params: Vec::new(),
        }
    }
}

/// `ConstructorFragmentImpl`.
#[derive(Debug)]
pub struct ConstructorFragment {
    pub executable: ExecutableFragmentData,
    /// The initializers of a const constructor, copied into the cycle's
    /// `ConstExprs` (link phase 6).
    /// Dart: ConstructorFragmentImpl._constantInitializers
    pub constant_initializers: OnceSlot<Vec<ConstExprId>>,
    /// Dart: ConstructorFragmentImpl.newKeywordOffset
    pub new_keyword_offset: Option<u32>,
    /// Dart: ConstructorFragmentImpl.factoryKeywordOffset
    pub factory_keyword_offset: Option<u32>,
    /// Dart: ConstructorFragmentImpl.typeName
    pub type_name: Option<Name>,
    /// Dart: ConstructorFragmentImpl.typeNameOffset
    pub type_name_offset: Option<u32>,
    /// Dart: ConstructorFragmentImpl.periodOffset
    pub period_offset: Option<u32>,
    /// Dart: ConstructorFragmentImpl.nameEnd
    pub name_end: Option<u32>,
    /// Dart: ConstructorFragmentImpl.thisKeywordOffset
    pub this_keyword_offset: Option<u32>,
}
deref_base!(ConstructorFragment => executable: ExecutableFragmentData);

/// `MethodFragmentImpl`.
#[derive(Debug)]
pub struct MethodFragment {
    pub executable: ExecutableFragmentData,
}
deref_base!(MethodFragment => executable: ExecutableFragmentData);

/// `TopLevelFunctionFragmentImpl` (a `FunctionFragmentImpl`).
#[derive(Debug)]
pub struct TopLevelFunctionFragment {
    pub executable: ExecutableFragmentData,
}
deref_base!(TopLevelFunctionFragment => executable: ExecutableFragmentData);

/// `LocalFunctionFragmentImpl` (a `FunctionFragmentImpl`).
#[derive(Debug)]
pub struct LocalFunctionFragment {
    pub executable: ExecutableFragmentData,
}
deref_base!(LocalFunctionFragment => executable: ExecutableFragmentData);

/// `PropertyAccessorFragmentImpl`.
#[derive(Debug)]
pub struct PropertyAccessorFragmentData {
    pub executable: ExecutableFragmentData,
    /// Dart: PropertyAccessorFragmentImpl._inducingVariable
    pub inducing_variable: Option<FId<PropertyInducingFragment>>,
}
deref_base!(PropertyAccessorFragmentData => executable: ExecutableFragmentData);

/// `GetterFragmentImpl`.
#[derive(Debug)]
pub struct GetterFragment {
    pub accessor: PropertyAccessorFragmentData,
}
deref_base!(GetterFragment => accessor: PropertyAccessorFragmentData);

/// `SetterFragmentImpl`.
#[derive(Debug)]
pub struct SetterFragment {
    pub accessor: PropertyAccessorFragmentData,
}
deref_base!(SetterFragment => accessor: PropertyAccessorFragmentData);

/// `VariableFragmentImpl` (with `NonParameterVariableFragmentImpl`, which
/// has only flags).
#[derive(Debug)]
pub struct VariableFragmentData {
    pub fragment: FragmentData,
    /// Dart: VariableFragmentImpl.constantInitializer
    pub constant_initializer: Option<ConstExprId>,
}
deref_base!(VariableFragmentData => fragment: FragmentData);

impl VariableFragmentData {
    pub fn new(fragment: FragmentData) -> Self {
        VariableFragmentData {
            fragment,
            constant_initializer: None,
        }
    }
}

/// `PropertyInducingFragmentImpl`.
#[derive(Debug)]
pub struct PropertyInducingFragmentData {
    pub variable: VariableFragmentData,
    /// Dart: PropertyInducingFragmentImpl._inducedGetter
    pub induced_getter: Option<FId<GetterFragment>>,
    /// Dart: PropertyInducingFragmentImpl._inducedSetter
    pub induced_setter: Option<FId<SetterFragment>>,
}
deref_base!(PropertyInducingFragmentData => variable: VariableFragmentData);

/// `FieldFragmentImpl`.
#[derive(Debug)]
pub struct FieldFragment {
    pub property: PropertyInducingFragmentData,
    /// Dart: FieldFragmentImpl.inheritsCovariant
    pub inherits_covariant: BoolSlot,
}
deref_base!(FieldFragment => property: PropertyInducingFragmentData);

/// `TopLevelVariableFragmentImpl`.
#[derive(Debug)]
pub struct TopLevelVariableFragment {
    pub property: PropertyInducingFragmentData,
}
deref_base!(TopLevelVariableFragment => property: PropertyInducingFragmentData);

/// `FormalParameterFragmentImpl` (also the field formal and super formal
/// subclasses: the tag tells).
#[derive(Debug)]
pub struct FormalParameterFragment {
    pub variable: VariableFragmentData,
    /// Dart: FormalParameterFragmentImpl.parameterKind
    pub parameter_kind: ParameterKind,
    /// Field formal parameters of primary constructors only.
    /// Dart: FieldFormalParameterFragmentImpl.privateName
    pub private_name: Option<Name>,
}
deref_base!(FormalParameterFragment => variable: VariableFragmentData);

/// Data of the pattern variable subclasses of `LocalVariableFragmentImpl`.
/// Written by the resolver of one unit (local store).
#[derive(Debug, Default)]
pub struct PatternVariableFragmentData {
    /// Dart: PatternVariableFragmentImpl.join
    pub join: VarSlot<FId<JoinPatternVariableFragment>>,
    /// Dart: PatternVariableFragmentImpl.isVisitingWhenClause
    pub is_visiting_when_clause: BoolSlot,
    /// The `DeclaredVariablePattern` (bind variables only).
    /// Dart: BindPatternVariableFragmentImpl.node
    pub node: Option<NodeId>,
    /// Dart: BindPatternVariableFragmentImpl.isDuplicate
    pub is_duplicate: BoolSlot,
    /// Join variables only.
    /// Dart: JoinPatternVariableFragmentImpl.variables
    pub variables: Vec<FId<PatternVariableFragment>>,
    /// Dart: JoinPatternVariableFragmentImpl.inconsistency
    pub inconsistency: VarSlot<JoinedPatternVariableInconsistency>,
    /// The `SimpleIdentifier`s that reference the join variable.
    /// Dart: JoinPatternVariableFragmentImpl.references
    pub references: Mutex<Vec<NodeId>>,
}

/// `LocalVariableFragmentImpl` (also the pattern variable subclasses: the
/// tag tells).
#[derive(Debug)]
pub struct LocalVariableFragment {
    pub variable: VariableFragmentData,
    pub pattern: PatternVariableFragmentData,
}
deref_base!(LocalVariableFragment => variable: VariableFragmentData);

/// `LabelFragmentImpl`.
#[derive(Debug)]
pub struct LabelFragment {
    pub fragment: FragmentData,
    /// Dart: LabelFragmentImpl._onSwitchMember
    pub on_switch_member: bool,
}
deref_base!(LabelFragment => fragment: FragmentData);

/// `PrefixFragmentImpl`.
#[derive(Debug)]
pub struct PrefixFragment {
    pub fragment: FragmentData,
    /// Dart: PrefixFragmentImpl.offset
    pub offset: u32,
    /// Dart: PrefixFragmentImpl.isDeferred
    pub is_deferred: bool,
}
deref_base!(PrefixFragment => fragment: FragmentData);

/// `TypeAliasFragmentImpl`.
#[derive(Debug)]
pub struct TypeAliasFragment {
    pub fragment: FragmentData,
    /// Dart: TypeAliasFragmentImpl._typeParameters
    pub type_params: Vec<FId<TypeParameterFragment>>,
    /// Dart: TypeAliasFragmentImpl.hasSelfReference
    pub has_self_reference: BoolSlot,
}
deref_base!(TypeAliasFragment => fragment: FragmentData);

/// `TypeParameterFragmentImpl`.
#[derive(Debug)]
pub struct TypeParameterFragment {
    pub fragment: FragmentData,
}
deref_base!(TypeParameterFragment => fragment: FragmentData);

/// `GenericFunctionTypeFragmentImpl`.
#[derive(Debug)]
pub struct GenericFunctionTypeFragment {
    pub fragment: FragmentData,
    /// Dart: GenericFunctionTypeFragmentImpl._typeParameters
    pub type_params: Vec<FId<TypeParameterFragment>>,
    /// Dart: GenericFunctionTypeFragmentImpl._formalParameters
    pub formal_params: Vec<FId<FormalParameterFragment>>,
    /// Dart: GenericFunctionTypeFragmentImpl.isNullable
    pub is_nullable: bool,
}
deref_base!(GenericFunctionTypeFragment => fragment: FragmentData);

/// `MultiplyDefinedFragmentImpl`.
#[derive(Debug)]
pub struct MultiplyDefinedFragment {
    pub fragment: FragmentData,
}
deref_base!(MultiplyDefinedFragment => fragment: FragmentData);

/// `LibraryFragmentImpl`: one compilation unit of a library.
#[derive(Debug)]
pub struct LibraryFragment {
    pub fragment: FragmentData,
    /// Dart: LibraryFragmentImpl.source
    pub source: SourceRef,
    /// Line starts (Dart `LineInfo.lineStarts`).
    /// Dart: LibraryFragmentImpl.lineInfo
    pub line_starts: Arc<[u32]>,
    /// The library (same as `fragment.element`, typed).
    pub library: EId<LibraryElement>,
    /// Dart: LibraryFragmentImpl._libraryExports
    pub library_exports: Vec<LibraryExport>,
    /// Dart: LibraryFragmentImpl._libraryImports
    pub library_imports: Vec<LibraryImport>,
    /// Dart: LibraryFragmentImpl._libraryImportPrefixes
    pub library_import_prefixes: Vec<EId<PrefixElement>>,
    /// Dart: LibraryFragmentImpl._libraryImportPrefixesById
    pub library_import_prefixes_by_id: IndexMap<Name, EId<PrefixElement>>,
    /// Dart: LibraryFragmentImpl._parts
    pub parts: Vec<PartInclude>,
    /// Dart: LibraryFragmentImpl._classes
    pub classes: Vec<FId<ClassFragment>>,
    /// Dart: LibraryFragmentImpl._enums
    pub enums: Vec<FId<EnumFragment>>,
    /// Dart: LibraryFragmentImpl._extensions
    pub extensions: Vec<FId<ExtensionFragment>>,
    /// Dart: LibraryFragmentImpl._extensionTypes
    pub extension_types: Vec<FId<ExtensionTypeFragment>>,
    /// Dart: LibraryFragmentImpl._functions
    pub functions: Vec<FId<TopLevelFunctionFragment>>,
    /// Dart: LibraryFragmentImpl._getters
    pub getters: Vec<FId<GetterFragment>>,
    /// Dart: LibraryFragmentImpl._setters
    pub setters: Vec<FId<SetterFragment>>,
    /// Dart: LibraryFragmentImpl._mixins
    pub mixins: Vec<FId<MixinFragment>>,
    /// Dart: LibraryFragmentImpl._typeAliases
    pub type_aliases: Vec<FId<TypeAliasFragment>>,
    /// Dart: LibraryFragmentImpl._variables
    pub variables: Vec<FId<TopLevelVariableFragment>>,
}
deref_base!(LibraryFragment => fragment: FragmentData);

impl LibraryFragment {
    pub fn new(fragment: FragmentData, source: SourceRef, library: EId<LibraryElement>) -> Self {
        LibraryFragment {
            fragment,
            source,
            line_starts: Arc::from([0u32].as_slice()),
            library,
            library_exports: Vec::new(),
            library_imports: Vec::new(),
            library_import_prefixes: Vec::new(),
            library_import_prefixes_by_id: IndexMap::new(),
            parts: Vec::new(),
            classes: Vec::new(),
            enums: Vec::new(),
            extensions: Vec::new(),
            extension_types: Vec::new(),
            functions: Vec::new(),
            getters: Vec::new(),
            setters: Vec::new(),
            mixins: Vec::new(),
            type_aliases: Vec::new(),
            variables: Vec::new(),
        }
    }
}
