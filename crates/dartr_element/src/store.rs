// Dart source: none (storage of the Rust element model, design §1.2)

//! [`ElementStore`] and [`FragmentStore`]: the elements and fragments of one
//! library cycle (or of the synthetic store, or of one analysis task), one
//! [`Arena`] per [`Tag`].

use crate::element::*;
use crate::fragment::*;
use crate::ids::*;
use crate::slot::Arena;

/// An element type with its own storage: a data struct, or a subclass
/// marker that shares the data struct of its superclass
/// ([`FieldFormalParameterElement`] → [`FormalParameterElement`]).
pub trait StoredElement: ElementType {
    /// The tag of new elements of this type.
    const TAG: Tag;
    type Data;
    fn arena(store: &ElementStore) -> &Arena<Self::Data>;
    fn arena_mut(store: &mut ElementStore) -> &mut Arena<Self::Data>;
}

/// A fragment type with its own storage.
pub trait StoredFragment: FragmentType {
    const TAG: Tag;
    type Data;
    fn arena(store: &FragmentStore) -> &Arena<Self::Data>;
    fn arena_mut(store: &mut FragmentStore) -> &mut Arena<Self::Data>;
}

macro_rules! stores {
    (
        elements { $($e_marker:ident [$($e_tag:ident)|+] => $e_data:ident in $e_field:ident;)* }
        element_subclasses { $($ea_marker:ident [$($ea_tag:ident)|+] => $ea_data:ident in $ea_field:ident;)* }
        fragments { $($f_marker:ident [$($f_tag:ident)|+] => $f_data:ident in $f_field:ident;)* }
        fragment_subclasses { $($fa_marker:ident [$($fa_tag:ident)|+] => $fa_data:ident in $fa_field:ident;)* }
    ) => {
        /// The elements of one store, per kind. Fields are the arenas; use
        /// [`ElementStore::add`] / [`ElementStore::get`] with typed ids.
        #[derive(Debug, Default)]
        pub struct ElementArenas {
            $(pub $e_field: Arena<$e_data>,)*
        }

        /// The fragments of one store, per kind.
        #[derive(Debug, Default)]
        pub struct FragmentStore {
            $(pub $f_field: Arena<$f_data>,)*
        }

        stores!(@impl_e $($e_marker [$($e_tag)|+] => $e_data in $e_field;)*);
        stores!(@impl_e $($ea_marker [$($ea_tag)|+] => $ea_data in $ea_field;)*);
        stores!(@impl_f $($f_marker [$($f_tag)|+] => $f_data in $f_field;)*);
        stores!(@impl_f $($fa_marker [$($fa_tag)|+] => $fa_data in $fa_field;)*);
    };
    (@impl_e $($marker:ident [$tag:ident $(| $more:ident)*] => $data:ident in $field:ident;)*) => {
        $(
            stores!(@type ElementType, $marker, [$tag $(| $more)*]);
            impl StoredElement for $marker {
                const TAG: Tag = Tag::$tag;
                type Data = $data;
                #[inline(always)]
                fn arena(store: &ElementStore) -> &Arena<$data> {
                    &store.elements.$field
                }
                #[inline(always)]
                fn arena_mut(store: &mut ElementStore) -> &mut Arena<$data> {
                    &mut store.elements.$field
                }
            }
        )*
    };
    (@impl_f $($marker:ident [$tag:ident $(| $more:ident)*] => $data:ident in $field:ident;)*) => {
        $(
            stores!(@type FragmentType, $marker, [$tag $(| $more)*]);
            impl StoredFragment for $marker {
                const TAG: Tag = Tag::$tag;
                type Data = $data;
                #[inline(always)]
                fn arena(store: &FragmentStore) -> &Arena<$data> {
                    &store.$field
                }
                #[inline(always)]
                fn arena_mut(store: &mut FragmentStore) -> &mut Arena<$data> {
                    &mut store.$field
                }
            }
        )*
    };
    // Data structs get their ElementType / FragmentType impl here; the
    // subclass markers already have one (ids.rs).
    (@type $trait:ident, $marker:ident, [$($tag:ident)|+]) => {
        stores!(@type_if $trait, $marker, [$($tag)|+]);
    };
    (@type_if $trait:ident, FieldFormalParameterElement, $tags:tt) => {};
    (@type_if $trait:ident, SuperFormalParameterElement, $tags:tt) => {};
    (@type_if $trait:ident, PatternVariableElement, $tags:tt) => {};
    (@type_if $trait:ident, BindPatternVariableElement, $tags:tt) => {};
    (@type_if $trait:ident, JoinPatternVariableElement, $tags:tt) => {};
    (@type_if $trait:ident, FieldFormalParameterFragment, $tags:tt) => {};
    (@type_if $trait:ident, SuperFormalParameterFragment, $tags:tt) => {};
    (@type_if $trait:ident, PatternVariableFragment, $tags:tt) => {};
    (@type_if $trait:ident, BindPatternVariableFragment, $tags:tt) => {};
    (@type_if $trait:ident, JoinPatternVariableFragment, $tags:tt) => {};
    (@type_if $trait:ident, $marker:ident, [$($tag:ident)|+]) => {
        impl $trait for $marker {
            const NAME: &'static str = stringify!($marker);
            #[inline(always)]
            fn test(tag: Tag) -> bool {
                matches!(tag, $(Tag::$tag)|+)
            }
        }
    };
}

stores! {
    elements {
        ClassElement [Class] => ClassElement in classes;
        EnumElement [Enum] => EnumElement in enums;
        MixinElement [Mixin] => MixinElement in mixins;
        ExtensionElement [Extension] => ExtensionElement in extensions;
        ExtensionTypeElement [ExtensionType] => ExtensionTypeElement in extension_types;
        FieldElement [Field] => FieldElement in fields;
        GetterElement [Getter] => GetterElement in getters;
        SetterElement [Setter] => SetterElement in setters;
        MethodElement [Method] => MethodElement in methods;
        ConstructorElement [Constructor] => ConstructorElement in constructors;
        TopLevelFunctionElement [TopLevelFunction] => TopLevelFunctionElement in functions;
        TopLevelVariableElement [TopLevelVariable] => TopLevelVariableElement in variables;
        TypeAliasElement [TypeAlias] => TypeAliasElement in type_aliases;
        TypeParameterElement [TypeParameter] => TypeParameterElement in type_params;
        FormalParameterElement [FormalParameter | FieldFormalParameter | SuperFormalParameter]
            => FormalParameterElement in params;
        PrefixElement [Prefix] => PrefixElement in prefixes;
        LibraryElement [Library] => LibraryElement in libraries;
        GenericFunctionTypeElement [GenericFunctionType]
            => GenericFunctionTypeElement in generic_function_types;
        LocalVariableElement [LocalVariable | PatternVariable | BindPatternVariable
            | JoinPatternVariable] => LocalVariableElement in locals;
        LocalFunctionElement [LocalFunction] => LocalFunctionElement in local_functions;
        LabelElement [Label] => LabelElement in labels;
        MultiplyDefinedElement [MultiplyDefined] => MultiplyDefinedElement in multiply_defined;
    }
    element_subclasses {
        FieldFormalParameterElement [FieldFormalParameter] => FormalParameterElement in params;
        SuperFormalParameterElement [SuperFormalParameter] => FormalParameterElement in params;
        PatternVariableElement [PatternVariable | BindPatternVariable | JoinPatternVariable]
            => LocalVariableElement in locals;
        BindPatternVariableElement [BindPatternVariable] => LocalVariableElement in locals;
        JoinPatternVariableElement [JoinPatternVariable] => LocalVariableElement in locals;
    }
    fragments {
        ClassFragment [Class] => ClassFragment in classes;
        EnumFragment [Enum] => EnumFragment in enums;
        MixinFragment [Mixin] => MixinFragment in mixins;
        ExtensionFragment [Extension] => ExtensionFragment in extensions;
        ExtensionTypeFragment [ExtensionType] => ExtensionTypeFragment in extension_types;
        FieldFragment [Field] => FieldFragment in fields;
        GetterFragment [Getter] => GetterFragment in getters;
        SetterFragment [Setter] => SetterFragment in setters;
        MethodFragment [Method] => MethodFragment in methods;
        ConstructorFragment [Constructor] => ConstructorFragment in constructors;
        TopLevelFunctionFragment [TopLevelFunction] => TopLevelFunctionFragment in functions;
        TopLevelVariableFragment [TopLevelVariable] => TopLevelVariableFragment in variables;
        TypeAliasFragment [TypeAlias] => TypeAliasFragment in type_aliases;
        TypeParameterFragment [TypeParameter] => TypeParameterFragment in type_params;
        FormalParameterFragment [FormalParameter | FieldFormalParameter | SuperFormalParameter]
            => FormalParameterFragment in params;
        PrefixFragment [Prefix] => PrefixFragment in prefixes;
        LibraryFragment [Library] => LibraryFragment in units;
        GenericFunctionTypeFragment [GenericFunctionType]
            => GenericFunctionTypeFragment in generic_function_types;
        LocalVariableFragment [LocalVariable | PatternVariable | BindPatternVariable
            | JoinPatternVariable] => LocalVariableFragment in locals;
        LocalFunctionFragment [LocalFunction] => LocalFunctionFragment in local_functions;
        LabelFragment [Label] => LabelFragment in labels;
        MultiplyDefinedFragment [MultiplyDefined] => MultiplyDefinedFragment in multiply_defined;
    }
    fragment_subclasses {
        FieldFormalParameterFragment [FieldFormalParameter] => FormalParameterFragment in params;
        SuperFormalParameterFragment [SuperFormalParameter] => FormalParameterFragment in params;
        PatternVariableFragment [PatternVariable | BindPatternVariable | JoinPatternVariable]
            => LocalVariableFragment in locals;
        BindPatternVariableFragment [BindPatternVariable] => LocalVariableFragment in locals;
        JoinPatternVariableFragment [JoinPatternVariable] => LocalVariableFragment in locals;
    }
}

/// The elements and fragments of one library cycle (built by `dartr_link`,
/// then frozen behind `Arc`), of the synthetic store of a generation, or of
/// one analysis task ([`crate::LocalArena`]).
#[derive(Debug)]
pub struct ElementStore {
    pub id: StoreId,
    pub elements: ElementArenas,
    pub fragments: FragmentStore,
}

/// A borrowed element of any kind (for `match`, Dart `switch (element)`).
#[derive(Clone, Copy, Debug)]
pub enum AnyElement<'a> {
    Class(&'a ClassElement),
    Enum(&'a EnumElement),
    Mixin(&'a MixinElement),
    Extension(&'a ExtensionElement),
    ExtensionType(&'a ExtensionTypeElement),
    Field(&'a FieldElement),
    Getter(&'a GetterElement),
    Setter(&'a SetterElement),
    Method(&'a MethodElement),
    Constructor(&'a ConstructorElement),
    TopLevelFunction(&'a TopLevelFunctionElement),
    TopLevelVariable(&'a TopLevelVariableElement),
    TypeAlias(&'a TypeAliasElement),
    TypeParameter(&'a TypeParameterElement),
    /// Also field formal and super formal parameters (see the id tag).
    FormalParameter(&'a FormalParameterElement),
    Prefix(&'a PrefixElement),
    Library(&'a LibraryElement),
    GenericFunctionType(&'a GenericFunctionTypeElement),
    /// Also pattern variables (see the id tag).
    LocalVariable(&'a LocalVariableElement),
    LocalFunction(&'a LocalFunctionElement),
    Label(&'a LabelElement),
    MultiplyDefined(&'a MultiplyDefinedElement),
    Dynamic,
    Never,
}

impl ElementStore {
    pub fn new(id: StoreId) -> ElementStore {
        ElementStore {
            id,
            elements: ElementArenas::default(),
            fragments: FragmentStore::default(),
        }
    }

    /// Adds an element. Works through `&self`: the synthetic store and local
    /// stores add while others read; a builder phase usually owns the store.
    pub fn add<T: StoredElement>(&self, data: T::Data) -> EId<T> {
        let index = T::arena(self).push(data);
        EId::from_raw(ElementId::new(self.id, T::TAG, index))
    }

    /// Adds a fragment.
    pub fn add_fragment<T: StoredFragment>(&self, data: T::Data) -> FId<T> {
        let index = T::arena(&self.fragments).push(data);
        FId::from_raw(FragmentId::new(self.id, T::TAG, index))
    }

    /// The data of an element of this store.
    #[inline]
    pub fn get<T: StoredElement + ?Sized>(&self, id: EId<T>) -> &T::Data {
        debug_assert_eq!(
            id.store(),
            self.id,
            "{:?} is not in store {:?}",
            id.raw(),
            self.id
        );
        T::arena(self).get(id.index())
    }

    /// Mutable data, while a builder phase owns the store.
    #[inline]
    pub fn get_mut<T: StoredElement + ?Sized>(&mut self, id: EId<T>) -> &mut T::Data {
        debug_assert_eq!(id.store(), self.id);
        T::arena_mut(self).get_mut(id.index())
    }

    #[inline]
    pub fn fragment<T: StoredFragment + ?Sized>(&self, id: FId<T>) -> &T::Data {
        debug_assert_eq!(
            id.store(),
            self.id,
            "{:?} is not in store {:?}",
            id.raw(),
            self.id
        );
        T::arena(&self.fragments).get(id.index())
    }

    #[inline]
    pub fn fragment_mut<T: StoredFragment + ?Sized>(&mut self, id: FId<T>) -> &mut T::Data {
        debug_assert_eq!(id.store(), self.id);
        T::arena_mut(&mut self.fragments).get_mut(id.index())
    }

    /// The element with [id], by its tag.
    pub fn any(&self, id: ElementId) -> AnyElement<'_> {
        let i = id.index();
        let e = &self.elements;
        match id.tag() {
            Tag::Class => AnyElement::Class(e.classes.get(i)),
            Tag::Enum => AnyElement::Enum(e.enums.get(i)),
            Tag::Mixin => AnyElement::Mixin(e.mixins.get(i)),
            Tag::Extension => AnyElement::Extension(e.extensions.get(i)),
            Tag::ExtensionType => AnyElement::ExtensionType(e.extension_types.get(i)),
            Tag::Field => AnyElement::Field(e.fields.get(i)),
            Tag::Getter => AnyElement::Getter(e.getters.get(i)),
            Tag::Setter => AnyElement::Setter(e.setters.get(i)),
            Tag::Method => AnyElement::Method(e.methods.get(i)),
            Tag::Constructor => AnyElement::Constructor(e.constructors.get(i)),
            Tag::TopLevelFunction => AnyElement::TopLevelFunction(e.functions.get(i)),
            Tag::TopLevelVariable => AnyElement::TopLevelVariable(e.variables.get(i)),
            Tag::TypeAlias => AnyElement::TypeAlias(e.type_aliases.get(i)),
            Tag::TypeParameter => AnyElement::TypeParameter(e.type_params.get(i)),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                AnyElement::FormalParameter(e.params.get(i))
            }
            Tag::Prefix => AnyElement::Prefix(e.prefixes.get(i)),
            Tag::Library => AnyElement::Library(e.libraries.get(i)),
            Tag::GenericFunctionType => {
                AnyElement::GenericFunctionType(e.generic_function_types.get(i))
            }
            Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable => AnyElement::LocalVariable(e.locals.get(i)),
            Tag::LocalFunction => AnyElement::LocalFunction(e.local_functions.get(i)),
            Tag::Label => AnyElement::Label(e.labels.get(i)),
            Tag::MultiplyDefined => AnyElement::MultiplyDefined(e.multiply_defined.get(i)),
            Tag::Dynamic => AnyElement::Dynamic,
            Tag::Never => AnyElement::Never,
        }
    }

    /// `ElementImpl` data of any element; `None` for `dynamic` and `Never`.
    pub fn element_data(&self, id: ElementId) -> Option<&ElementData> {
        Some(match self.any(id) {
            AnyElement::Class(x) => x.element(),
            AnyElement::Enum(x) => x.element(),
            AnyElement::Mixin(x) => x.element(),
            AnyElement::Extension(x) => x.element(),
            AnyElement::ExtensionType(x) => x.element(),
            AnyElement::Field(x) => x.element(),
            AnyElement::Getter(x) => x.element(),
            AnyElement::Setter(x) => x.element(),
            AnyElement::Method(x) => x.element(),
            AnyElement::Constructor(x) => x.element(),
            AnyElement::TopLevelFunction(x) => x.element(),
            AnyElement::TopLevelVariable(x) => x.element(),
            AnyElement::TypeAlias(x) => x.element(),
            AnyElement::TypeParameter(x) => x.element(),
            AnyElement::FormalParameter(x) => x.element(),
            AnyElement::Prefix(x) => x.element(),
            AnyElement::Library(x) => x.element(),
            AnyElement::GenericFunctionType(x) => x.element(),
            AnyElement::LocalVariable(x) => x.element(),
            AnyElement::LocalFunction(x) => x.element(),
            AnyElement::Label(x) => x.element(),
            AnyElement::MultiplyDefined(x) => x.element(),
            AnyElement::Dynamic | AnyElement::Never => return None,
        })
    }

    /// `InstanceElementImpl` data.
    pub fn instance(&self, id: EId<InstanceElement>) -> &InstanceElementData {
        match self.any(id.raw()) {
            AnyElement::Class(x) => x,
            AnyElement::Enum(x) => x,
            AnyElement::Mixin(x) => x,
            AnyElement::ExtensionType(x) => x,
            AnyElement::Extension(x) => &x.instance,
            _ => unreachable!("{id:?}"),
        }
    }

    /// `InterfaceElementImpl` data.
    pub fn interface(&self, id: EId<InterfaceElement>) -> &InterfaceElementData {
        match self.any(id.raw()) {
            AnyElement::Class(x) => &x.interface,
            AnyElement::Enum(x) => &x.interface,
            AnyElement::Mixin(x) => &x.interface,
            AnyElement::ExtensionType(x) => &x.interface,
            _ => unreachable!("{id:?}"),
        }
    }

    /// `ExecutableElementImpl` data.
    pub fn executable(&self, id: EId<ExecutableElement>) -> &ExecutableElementData {
        match self.any(id.raw()) {
            AnyElement::Method(x) => &x.executable,
            AnyElement::Constructor(x) => &x.executable,
            AnyElement::Getter(x) => x,
            AnyElement::Setter(x) => x,
            AnyElement::TopLevelFunction(x) => &x.executable,
            AnyElement::LocalFunction(x) => &x.executable,
            _ => unreachable!("{id:?}"),
        }
    }

    /// `PropertyAccessorElementImpl` data.
    pub fn property_accessor(
        &self,
        id: EId<PropertyAccessorElement>,
    ) -> &PropertyAccessorElementData {
        match self.any(id.raw()) {
            AnyElement::Getter(x) => &x.accessor,
            AnyElement::Setter(x) => &x.accessor,
            _ => unreachable!("{id:?}"),
        }
    }

    /// `VariableElementImpl` data.
    pub fn variable(&self, id: EId<VariableElement>) -> &VariableElementData {
        match self.any(id.raw()) {
            AnyElement::Field(x) => x,
            AnyElement::TopLevelVariable(x) => x,
            AnyElement::FormalParameter(x) => &x.variable,
            AnyElement::LocalVariable(x) => &x.variable,
            _ => unreachable!("{id:?}"),
        }
    }

    /// `PropertyInducingElementImpl` data.
    pub fn property_inducing(
        &self,
        id: EId<PropertyInducingElement>,
    ) -> &PropertyInducingElementData {
        match self.any(id.raw()) {
            AnyElement::Field(x) => &x.property,
            AnyElement::TopLevelVariable(x) => &x.property,
            _ => unreachable!("{id:?}"),
        }
    }

    /// `FragmentImpl` data of any fragment; `None` for `dynamic` and `Never`.
    pub fn fragment_data(&self, id: FragmentId) -> Option<&FragmentData> {
        let i = id.index();
        let f = &self.fragments;
        Some(match id.tag() {
            Tag::Class => f.classes.get(i).fragment(),
            Tag::Enum => f.enums.get(i).fragment(),
            Tag::Mixin => f.mixins.get(i).fragment(),
            Tag::Extension => f.extensions.get(i).fragment(),
            Tag::ExtensionType => f.extension_types.get(i).fragment(),
            Tag::Field => f.fields.get(i).fragment(),
            Tag::Getter => f.getters.get(i).fragment(),
            Tag::Setter => f.setters.get(i).fragment(),
            Tag::Method => f.methods.get(i).fragment(),
            Tag::Constructor => f.constructors.get(i).fragment(),
            Tag::TopLevelFunction => f.functions.get(i).fragment(),
            Tag::TopLevelVariable => f.variables.get(i).fragment(),
            Tag::TypeAlias => f.type_aliases.get(i).fragment(),
            Tag::TypeParameter => f.type_params.get(i).fragment(),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                f.params.get(i).fragment()
            }
            Tag::Prefix => f.prefixes.get(i).fragment(),
            Tag::Library => f.units.get(i).fragment(),
            Tag::GenericFunctionType => f.generic_function_types.get(i).fragment(),
            Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable => f.locals.get(i).fragment(),
            Tag::LocalFunction => f.local_functions.get(i).fragment(),
            Tag::Label => f.labels.get(i).fragment(),
            Tag::MultiplyDefined => f.multiply_defined.get(i).fragment(),
            Tag::Dynamic | Tag::Never => return None,
        })
    }

    /// `InstanceFragmentImpl` data.
    pub fn instance_fragment(&self, id: FId<InstanceFragment>) -> &InstanceFragmentData {
        let i = id.index();
        let f = &self.fragments;
        match id.tag() {
            Tag::Class => f.classes.get(i),
            Tag::Enum => f.enums.get(i),
            Tag::Mixin => f.mixins.get(i),
            Tag::ExtensionType => f.extension_types.get(i),
            Tag::Extension => &f.extensions.get(i).instance,
            _ => unreachable!("{id:?}"),
        }
    }

    /// `ExecutableFragmentImpl` data.
    pub fn executable_fragment(&self, id: FId<ExecutableFragment>) -> &ExecutableFragmentData {
        let i = id.index();
        let f = &self.fragments;
        match id.tag() {
            Tag::Method => &f.methods.get(i).executable,
            Tag::Constructor => &f.constructors.get(i).executable,
            Tag::Getter => f.getters.get(i),
            Tag::Setter => f.setters.get(i),
            Tag::TopLevelFunction => &f.functions.get(i).executable,
            Tag::LocalFunction => &f.local_functions.get(i).executable,
            _ => unreachable!("{id:?}"),
        }
    }

    /// `VariableFragmentImpl` data.
    pub fn variable_fragment(&self, id: FId<VariableFragment>) -> &VariableFragmentData {
        let i = id.index();
        let f = &self.fragments;
        match id.tag() {
            Tag::Field => f.fields.get(i),
            Tag::TopLevelVariable => f.variables.get(i),
            Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
                &f.params.get(i).variable
            }
            Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable => &f.locals.get(i).variable,
            _ => unreachable!("{id:?}"),
        }
    }
}
