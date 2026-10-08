// Dart source: pkg/analyzer/lib/src/dart/element/element.dart (*ElementImpl)

//! Element data structs: one struct per Dart `*ElementImpl` class that has
//! data. A Dart superclass becomes a field holding the base struct
//! (`ClassElement.interface: InterfaceElementData` →
//! `InterfaceElementData.instance: InstanceElementData` →
//! `ElementData`), and each struct derefs to its base, so
//! `class.name` and `class.supertype` both work.
//!
//! Every Dart field is ported (see `tests/schema_coverage.rs`): each Rust
//! field that holds a Dart field has a `Dart: Class.field` line in its doc.
//! Fields that hold data the Dart code computes from the first fragment are
//! marked "cached".
//!
//! Mutability (design §1.2):
//! - Plain fields: structural data, written through `&mut ElementStore`
//!   while a builder phase owns the store.
//! - [`OnceSlot`]: Dart `late final` fields and lazy caches (set once).
//! - [`VarSlot`] / [`BoolSlot`] / [`ElementFlagCell`]: Dart non-final fields
//!   that link phases (or the resolver, for local elements) assign through a
//!   shared reference, possibly more than once.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use indexmap::IndexMap;

use crate::FeatureSet;
use crate::data::{
    ConstantInitializer, FieldNameNonPromotabilityInfo, LibraryLanguageVersion, Metadata,
    Namespace, TopLevelInferenceError,
};
use crate::fragment::{
    ClassFragment, ConstructorFragment, EnumFragment, ExtensionFragment, ExtensionTypeFragment,
    FieldFragment, FormalParameterFragment, GenericFunctionTypeFragment, GetterFragment,
    LabelFragment, LibraryFragment, LocalFunctionFragment, LocalVariableFragment, MethodFragment,
    MixinFragment, MultiplyDefinedFragment, PrefixFragment, SetterFragment,
    TopLevelFunctionFragment, TopLevelVariableFragment, TypeAliasFragment, TypeParameterFragment,
};
use crate::ids::{EId, ElementId, FId, FragmentId, InterfaceElement, PropertyInducingElement};
use crate::name::Name;
use crate::slot::{BoolSlot, ElementFlagCell, OnceSlot, VarSlot};
use crate::types::{ElemRef, ParameterKind, TypeId, TypeList, Variance};

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

/// Typed access to the first fragment, which [`ElementData::first_fragment`]
/// stores untyped.
macro_rules! first_fragment {
    ($t:ty => $f:ty) => {
        impl $t {
            /// The first fragment (Dart `firstFragment`).
            #[inline(always)]
            pub fn first_fragment(&self) -> FId<$f> {
                FId::from_raw(self.element().first_fragment)
            }
        }
    };
}

/// `ElementImpl`: data of every element.
#[derive(Debug)]
pub struct ElementData {
    /// The element name (cached from the first fragment; constructors and
    /// methods store it in Dart: `ConstructorElementImpl.name`,
    /// `MethodElementImpl.name`; the library name is `LibraryElementImpl._name`).
    /// Dart: ConstructorElementImpl.name
    /// Dart: MethodElementImpl.name
    /// Dart: LibraryElementImpl._name
    /// Dart: MultiplyDefinedElementImpl.name
    pub name: Option<Name>,
    /// The library (cached; the library itself for a library element; `None`
    /// for `dynamic` and `Never`).
    pub library: Option<EId<crate::LibraryElement>>,
    /// The enclosing element (cached; Dart `enclosingElement`).
    pub enclosing: Option<ElementId>,
    /// The first fragment. Each Dart subclass has its own typed
    /// `_firstFragment` field; use the typed `first_fragment()` method of
    /// the concrete struct.
    /// Dart: ClassElementImpl._firstFragment
    /// Dart: ConstructorElementImpl._firstFragment
    /// Dart: EnumElementImpl._firstFragment
    /// Dart: ExtensionElementImpl._firstFragment
    /// Dart: ExtensionTypeElementImpl._firstFragment
    /// Dart: FieldElementImpl._firstFragment
    /// Dart: FormalParameterElementImpl._firstFragment
    /// Dart: GenericFunctionTypeElementImpl._firstFragment
    /// Dart: GetterElementImpl._firstFragment
    /// Dart: LabelElementImpl._firstFragment
    /// Dart: LibraryElementImpl._firstFragment
    /// Dart: LocalFunctionElementImpl._firstFragment
    /// Dart: LocalVariableElementImpl._firstFragment
    /// Dart: MethodElementImpl._firstFragment
    /// Dart: MixinElementImpl._firstFragment
    /// Dart: MultiplyDefinedElementImpl._firstFragment
    /// Dart: PrefixElementImpl._firstFragment
    /// Dart: SetterElementImpl._firstFragment
    /// Dart: TopLevelFunctionElementImpl._firstFragment
    /// Dart: TopLevelVariableElementImpl._firstFragment
    /// Dart: TypeAliasElementImpl._firstFragment
    /// Dart: TypeParameterElementImpl._firstFragment
    pub first_fragment: FragmentId,
    /// Dart: ElementImpl._flags
    pub flags: ElementFlagCell,
    /// Dart: ElementImpl.previousFragmentOfDifferentKind
    pub previous_fragment_of_different_kind: Option<FragmentId>,
}

impl ElementData {
    pub fn new(name: Option<Name>, first_fragment: FragmentId) -> ElementData {
        ElementData {
            name,
            library: None,
            enclosing: None,
            first_fragment,
            flags: ElementFlagCell::default(),
            previous_fragment_of_different_kind: None,
        }
    }

    #[inline(always)]
    pub fn element(&self) -> &ElementData {
        self
    }
}

/// `InstanceElementImpl`.
#[derive(Debug)]
pub struct InstanceElementData {
    pub element: ElementData,
    /// Cached from the type parameter fragments of the first fragment
    /// (Dart `typeParameters`).
    pub type_params: Vec<EId<TypeParameterElement>>,
    /// Dart: InstanceElementImpl._fields
    pub fields: Vec<EId<FieldElement>>,
    /// Dart: InstanceElementImpl._getters
    pub getters: Vec<EId<GetterElement>>,
    /// Dart: InstanceElementImpl._setters
    pub setters: Vec<EId<SetterElement>>,
    /// Dart: InstanceElementImpl._methods
    pub methods: Vec<EId<MethodElement>>,
}
deref_base!(InstanceElementData => element: ElementData);

impl InstanceElementData {
    pub fn new(element: ElementData) -> Self {
        InstanceElementData {
            element,
            type_params: Vec::new(),
            fields: Vec::new(),
            getters: Vec::new(),
            setters: Vec::new(),
            methods: Vec::new(),
        }
    }
}

/// `InterfaceElementImpl`.
#[derive(Debug)]
pub struct InterfaceElementData {
    pub instance: InstanceElementData,
    /// Lazy cache of `instantiate(nullabilitySuffix: none)` without
    /// arguments.
    /// Dart: InterfaceElementImpl._nonNullableInstance
    pub non_nullable_instance: OnceSlot<TypeId>,
    /// Dart: InterfaceElementImpl._nullableInstance
    pub nullable_instance: OnceSlot<TypeId>,
    /// Set by types_builder, changed by interface_cycles and
    /// `_setDefaultSupertypes`.
    /// Dart: InterfaceElementImpl._supertype
    pub supertype: VarSlot<TypeId>,
    /// Dart: InterfaceElementImpl._mixins
    pub mixins: VarSlot<TypeList>,
    /// Dart: InterfaceElementImpl._interfaces
    pub interfaces: VarSlot<TypeList>,
    /// Dart: InterfaceElementImpl._thisType
    pub this_type: OnceSlot<TypeId>,
    /// Dart: InterfaceElementImpl.interfaceCycle
    pub interface_cycle: OnceSlot<Vec<EId<InterfaceElement>>>,
    /// Dart: InterfaceElementImpl._allSupertypes
    pub all_supertypes: OnceSlot<TypeList>,
    /// Dart: InterfaceElementImpl._constructors
    pub constructors: Vec<EId<ConstructorElement>>,
    /// Dart: InterfaceElementImpl._hasNonFinalField
    pub has_non_final_field: BoolSlot,
}
deref_base!(InterfaceElementData => instance: InstanceElementData);

impl InterfaceElementData {
    pub fn new(element: ElementData) -> Self {
        InterfaceElementData {
            instance: InstanceElementData::new(element),
            non_nullable_instance: OnceSlot::new(),
            nullable_instance: OnceSlot::new(),
            supertype: VarSlot::new(),
            mixins: VarSlot::with(TypeList::EMPTY),
            interfaces: VarSlot::with(TypeList::EMPTY),
            this_type: OnceSlot::new(),
            interface_cycle: OnceSlot::new(),
            all_supertypes: OnceSlot::new(),
            constructors: Vec::new(),
            has_non_final_field: BoolSlot::new(false),
        }
    }
}

/// `ClassElementImpl`.
#[derive(Debug)]
pub struct ClassElement {
    pub interface: InterfaceElementData,
}
deref_base!(ClassElement => interface: InterfaceElementData);
first_fragment!(ClassElement => ClassFragment);

/// `EnumElementImpl`.
#[derive(Debug)]
pub struct EnumElement {
    pub interface: InterfaceElementData,
}
deref_base!(EnumElement => interface: InterfaceElementData);
first_fragment!(EnumElement => EnumFragment);

/// `MixinElementImpl`.
#[derive(Debug)]
pub struct MixinElement {
    pub interface: InterfaceElementData,
    /// Dart: MixinElementImpl._superclassConstraints
    pub superclass_constraints: VarSlot<TypeList>,
}
deref_base!(MixinElement => interface: InterfaceElementData);
first_fragment!(MixinElement => MixinFragment);

/// `ExtensionTypeElementImpl`.
#[derive(Debug)]
pub struct ExtensionTypeElement {
    pub interface: InterfaceElementData,
    /// Dart: ExtensionTypeElementImpl.hasRepresentationSelfReference
    pub has_representation_self_reference: BoolSlot,
    /// Dart: ExtensionTypeElementImpl.hasImplementsSelfReference
    pub has_implements_self_reference: BoolSlot,
    /// Dart: ExtensionTypeElementImpl._typeErasure
    pub type_erasure: OnceSlot<TypeId>,
}
deref_base!(ExtensionTypeElement => interface: InterfaceElementData);
first_fragment!(ExtensionTypeElement => ExtensionTypeFragment);

/// `ExtensionElementImpl`.
#[derive(Debug)]
pub struct ExtensionElement {
    pub instance: InstanceElementData,
    /// Initially `InvalidType`.
    /// Dart: ExtensionElementImpl._extendedType
    pub extended_type: VarSlot<TypeId>,
}
deref_base!(ExtensionElement => instance: InstanceElementData);
first_fragment!(ExtensionElement => ExtensionFragment);

/// `ExecutableElementImpl` (with `FunctionTypedElementImpl`, which has no
/// fields).
#[derive(Debug)]
pub struct ExecutableElementData {
    pub element: ElementData,
    /// Cached (Dart `typeParameters`).
    pub type_params: Vec<EId<TypeParameterElement>>,
    /// Cached (Dart `formalParameters`).
    pub formal_params: Vec<EId<FormalParameterElement>>,
    /// Dart: ExecutableElementImpl._returnType
    pub return_type: VarSlot<TypeId>,
    /// The function type, built from the return type and the parameters on
    /// first use; reset when the return type changes.
    /// Dart: ExecutableElementImpl._type
    pub type_: VarSlot<TypeId>,
}
deref_base!(ExecutableElementData => element: ElementData);

impl ExecutableElementData {
    pub fn new(element: ElementData) -> Self {
        ExecutableElementData {
            element,
            type_params: Vec::new(),
            formal_params: Vec::new(),
            return_type: VarSlot::new(),
            type_: VarSlot::new(),
        }
    }
}

/// `ConstructorElementImpl`.
#[derive(Debug)]
pub struct ConstructorElement {
    pub executable: ExecutableElementData,
    /// Dart: ConstructorElementImpl._redirectedConstructor
    pub redirected_constructor: VarSlot<ElemRef>,
    /// Dart: ConstructorElementImpl._superConstructor
    pub super_constructor: VarSlot<ElemRef>,
}
deref_base!(ConstructorElement => executable: ExecutableElementData);
first_fragment!(ConstructorElement => ConstructorFragment);

/// `MethodElementImpl`.
#[derive(Debug)]
pub struct MethodElement {
    pub executable: ExecutableElementData,
    /// Dart: MethodElementImpl.isOperatorEqualWithParameterTypeFromObject
    pub is_operator_equal_with_parameter_type_from_object: BoolSlot,
    /// Dart: MethodElementImpl.typeInferenceError
    pub type_inference_error: OnceSlot<TopLevelInferenceError>,
}
deref_base!(MethodElement => executable: ExecutableElementData);
first_fragment!(MethodElement => MethodFragment);

/// `PropertyAccessorElementImpl`.
#[derive(Debug)]
pub struct PropertyAccessorElementData {
    pub executable: ExecutableElementData,
    /// Dart: PropertyAccessorElementImpl._variable3
    pub variable: VarSlot<EId<PropertyInducingElement>>,
}
deref_base!(PropertyAccessorElementData => executable: ExecutableElementData);

/// `GetterElementImpl`.
#[derive(Debug)]
pub struct GetterElement {
    pub accessor: PropertyAccessorElementData,
}
deref_base!(GetterElement => accessor: PropertyAccessorElementData);
first_fragment!(GetterElement => GetterFragment);

/// `SetterElementImpl`.
#[derive(Debug)]
pub struct SetterElement {
    pub accessor: PropertyAccessorElementData,
}
deref_base!(SetterElement => accessor: PropertyAccessorElementData);
first_fragment!(SetterElement => SetterFragment);

/// `TopLevelFunctionElementImpl`.
#[derive(Debug)]
pub struct TopLevelFunctionElement {
    pub executable: ExecutableElementData,
}
deref_base!(TopLevelFunctionElement => executable: ExecutableElementData);
first_fragment!(TopLevelFunctionElement => TopLevelFunctionFragment);

/// `LocalFunctionElementImpl`.
#[derive(Debug)]
pub struct LocalFunctionElement {
    pub executable: ExecutableElementData,
}
deref_base!(LocalFunctionElement => executable: ExecutableElementData);
first_fragment!(LocalFunctionElement => LocalFunctionFragment);

/// `GenericFunctionTypeElementImpl`.
#[derive(Debug)]
pub struct GenericFunctionTypeElement {
    pub element: ElementData,
    /// Cached (Dart `typeParameters`).
    pub type_params: Vec<EId<TypeParameterElement>>,
    /// Cached (Dart `formalParameters`).
    pub formal_params: Vec<EId<FormalParameterElement>>,
    /// Dart: GenericFunctionTypeElementImpl.returnType
    pub return_type: VarSlot<TypeId>,
    /// Dart: GenericFunctionTypeElementImpl._type
    pub type_: VarSlot<TypeId>,
}
deref_base!(GenericFunctionTypeElement => element: ElementData);
first_fragment!(GenericFunctionTypeElement => GenericFunctionTypeFragment);

/// `VariableElementImpl`.
#[derive(Debug)]
pub struct VariableElementData {
    pub element: ElementData,
    /// Dart: VariableElementImpl._constantInitializer
    pub constant_initializer: OnceSlot<Option<ConstantInitializer>>,
}
deref_base!(VariableElementData => element: ElementData);

impl VariableElementData {
    pub fn new(element: ElementData) -> Self {
        VariableElementData {
            element,
            constant_initializer: OnceSlot::new(),
        }
    }
}

/// `PropertyInducingElementImpl`.
#[derive(Debug)]
pub struct PropertyInducingElementData {
    pub variable: VariableElementData,
    /// Dart: PropertyInducingElementImpl.getter
    pub getter: Option<EId<GetterElement>>,
    /// Dart: PropertyInducingElementImpl.setter
    pub setter: Option<EId<SetterElement>>,
    /// Dart: PropertyInducingElementImpl._type
    pub type_: VarSlot<TypeId>,
    /// Dart: PropertyInducingElementImpl.typeInferenceError
    pub type_inference_error: OnceSlot<TopLevelInferenceError>,
}
deref_base!(PropertyInducingElementData => variable: VariableElementData);

impl PropertyInducingElementData {
    pub fn new(element: ElementData) -> Self {
        PropertyInducingElementData {
            variable: VariableElementData::new(element),
            getter: None,
            setter: None,
            type_: VarSlot::new(),
            type_inference_error: OnceSlot::new(),
        }
    }
}

/// `FieldElementImpl`.
#[derive(Debug)]
pub struct FieldElement {
    pub property: PropertyInducingElementData,
}
deref_base!(FieldElement => property: PropertyInducingElementData);
first_fragment!(FieldElement => FieldFragment);

/// `TopLevelVariableElementImpl`.
#[derive(Debug)]
pub struct TopLevelVariableElement {
    pub property: PropertyInducingElementData,
}
deref_base!(TopLevelVariableElement => property: PropertyInducingElementData);
first_fragment!(TopLevelVariableElement => TopLevelVariableFragment);

/// `FormalParameterElementImpl` (also `FieldFormalParameterElementImpl` and
/// `SuperFormalParameterElementImpl`: the tag tells).
#[derive(Debug)]
pub struct FormalParameterElement {
    pub variable: VariableElementData,
    /// Cached from the fragment (Dart `parameterKind`).
    pub kind: ParameterKind,
    /// Initially `InvalidType`.
    /// Dart: FormalParameterElementImpl._type
    pub type_: VarSlot<TypeId>,
    /// Dart: FormalParameterElementImpl._baseFormalParameter
    pub base_formal_parameter: Option<EId<FormalParameterElement>>,
    /// Field formal parameters only.
    /// Dart: FieldFormalParameterElementImpl._field
    pub field: VarSlot<EId<FieldElement>>,
}
deref_base!(FormalParameterElement => variable: VariableElementData);
first_fragment!(FormalParameterElement => FormalParameterFragment);

/// `LocalVariableElementImpl` (also the pattern variable subclasses: the tag
/// tells).
#[derive(Debug)]
pub struct LocalVariableElement {
    pub variable: VariableElementData,
    /// Initially `InvalidType`.
    /// Dart: LocalVariableElementImpl.type
    pub type_: VarSlot<TypeId>,
}
deref_base!(LocalVariableElement => variable: VariableElementData);
first_fragment!(LocalVariableElement => LocalVariableFragment);

/// `LabelElementImpl`.
#[derive(Debug)]
pub struct LabelElement {
    pub element: ElementData,
}
deref_base!(LabelElement => element: ElementData);
first_fragment!(LabelElement => LabelFragment);

/// `PrefixElementImpl`.
#[derive(Debug)]
pub struct PrefixElement {
    pub element: ElementData,
    /// Dart: PrefixElementImpl.localId
    pub local_id: Name,
    /// Dart: PrefixElementImpl.lastFragment
    pub last_fragment: FId<PrefixFragment>,
}
deref_base!(PrefixElement => element: ElementData);
first_fragment!(PrefixElement => PrefixFragment);

/// `TypeAliasElementImpl`.
#[derive(Debug)]
pub struct TypeAliasElement {
    pub element: ElementData,
    /// Cached (Dart `typeParameters`).
    pub type_params: Vec<EId<TypeParameterElement>>,
    /// Dart: TypeAliasElementImpl._aliasedType
    pub aliased_type: VarSlot<TypeId>,
}
deref_base!(TypeAliasElement => element: ElementData);
first_fragment!(TypeAliasElement => TypeAliasFragment);

/// `TypeParameterElementImpl`.
#[derive(Debug)]
pub struct TypeParameterElement {
    pub element: ElementData,
    /// `None` = legacy covariant (no declared variance).
    /// Dart: TypeParameterElementImpl._variance
    pub variance: Option<Variance>,
    /// Dart: TypeParameterElementImpl.bound
    pub bound: VarSlot<TypeId>,
    /// Dart: TypeParameterElementImpl.defaultType
    pub default_type: VarSlot<TypeId>,
}
deref_base!(TypeParameterElement => element: ElementData);
first_fragment!(TypeParameterElement => TypeParameterFragment);

impl TypeParameterElement {
    pub fn new(element: ElementData) -> Self {
        TypeParameterElement {
            element,
            variance: None,
            bound: VarSlot::new(),
            default_type: VarSlot::new(),
        }
    }
}

/// `MultiplyDefinedElementImpl`.
#[derive(Debug)]
pub struct MultiplyDefinedElement {
    pub element: ElementData,
    /// Dart: MultiplyDefinedElementImpl.libraryFragment
    pub library_fragment: FId<LibraryFragment>,
    /// Dart: MultiplyDefinedElementImpl.conflictingElements
    pub conflicting_elements: Vec<ElementId>,
}
deref_base!(MultiplyDefinedElement => element: ElementData);
first_fragment!(MultiplyDefinedElement => MultiplyDefinedFragment);

/// `LibraryElementImpl`.
#[derive(Debug)]
pub struct LibraryElement {
    pub element: ElementData,
    /// Dart: LibraryElementImpl._metadata
    pub metadata: Metadata,
    /// Dart: LibraryElementImpl._documentationComment
    pub documentation_comment: Option<Arc<str>>,
    /// Dart: LibraryElementImpl._languageVersion
    pub language_version: LibraryLanguageVersion,
    /// Dart: LibraryElementImpl._featureSet
    pub feature_set: FeatureSet,
    /// Dart: LibraryElementImpl._entryPoint
    pub entry_point: OnceSlot<Option<EId<TopLevelFunctionElement>>>,
    /// The synthetic `loadLibrary` function (Dart `LoadLibraryFunctionProvider`).
    /// Dart: LibraryElementImpl.loadLibraryProvider
    pub load_library_function: OnceSlot<EId<TopLevelFunctionElement>>,
    /// Dart: LibraryElementImpl._nameOffset
    pub name_offset: i32,
    /// Dart: LibraryElementImpl._nameLength
    pub name_length: u32,
    /// Dart: LibraryElementImpl._classes
    pub classes: Vec<EId<ClassElement>>,
    /// Dart: LibraryElementImpl._enums
    pub enums: Vec<EId<EnumElement>>,
    /// Dart: LibraryElementImpl._extensions
    pub extensions: Vec<EId<ExtensionElement>>,
    /// Dart: LibraryElementImpl._extensionTypes
    pub extension_types: Vec<EId<ExtensionTypeElement>>,
    /// Dart: LibraryElementImpl._getters
    pub getters: Vec<EId<GetterElement>>,
    /// Dart: LibraryElementImpl._setters
    pub setters: Vec<EId<SetterElement>>,
    /// Dart: LibraryElementImpl._mixins
    pub mixins: Vec<EId<MixinElement>>,
    /// Dart: LibraryElementImpl._topLevelFunctions
    pub top_level_functions: Vec<EId<TopLevelFunctionElement>>,
    /// Dart: LibraryElementImpl._topLevelVariables
    pub top_level_variables: Vec<EId<TopLevelVariableElement>>,
    /// Dart: LibraryElementImpl._typeAliases
    pub type_aliases: Vec<EId<TypeAliasElement>>,
    /// Dart: LibraryElementImpl._exportNamespace
    pub export_namespace: OnceSlot<Arc<Namespace>>,
    /// Dart: LibraryElementImpl._publicNamespace
    pub public_namespace: OnceSlot<Arc<Namespace>>,
    /// Dart: LibraryElementImpl._fieldNameNonPromotabilityInfo
    pub field_name_non_promotability_info: OnceSlot<IndexMap<Name, FieldNameNonPromotabilityInfo>>,
}
deref_base!(LibraryElement => element: ElementData);
first_fragment!(LibraryElement => LibraryFragment);
