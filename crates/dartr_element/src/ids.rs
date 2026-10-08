// Dart source: pkg/analyzer/lib/src/dart/element/element.dart (element and
// fragment identity: `ElementImpl.id`, `FragmentImpl.id`, `is` / `as` checks)

//! Element and fragment ids.
//!
//! - [`StoreId`]: one [`crate::ElementStore`] (a library cycle, the global
//!   synthetic store, or the local store of one analysis task).
//! - [`ElementId`] / [`FragmentId`]: packed `store (24 bits) | tag (8 bits) |
//!   index (32 bits)`. The [`Tag`] says which vector of the store holds the
//!   data, so a run-time type check (Dart `is` / `as`) needs no store access.
//! - [`EId<T>`] / [`FId<T>`]: typed views, like `dartr_ast::Id<T>`. `T` is a
//!   data struct ([`crate::ClassElement`]) or a category marker (an empty enum
//!   per abstract Dart interface: [`InterfaceElement`], [`ExecutableElement`],
//!   ...). [`EId::upcast`] is checked at compile time (`T: SubtypeOf<U>`),
//!   [`ElementId::cast`] at run time.
//!
//! Ids have no `Ord`: the determinism rule (design §2.5) says output may never
//! depend on id values. Use [`ElementId::raw`] only for canonical forms that
//! are never printed (for example the sorted pairs of a substitution).

use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroU64;

/// The kind of a store, encoded in the [`StoreId`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum StoreKind {
    /// The elements of one library cycle, built by the linker, then frozen.
    Cycle,
    /// Global and append-only: elements created inside shared caches (fresh
    /// type parameters of `FreshTypeParameters`, synthetic members).
    Synthetic,
    /// The elements of one body analysis task (locals, local functions,
    /// labels, fresh type parameters). Dropped with the task.
    Local,
}

/// One element store. Unique within one [`crate::Generation`], never reused.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoreId(u32);

impl StoreId {
    /// The global synthetic store of a generation.
    pub const SYNTHETIC: StoreId = StoreId(0);
    const LOCAL_BIT: u32 = 1 << 23;
    /// The largest store number (24 bits, the top bit marks local stores).
    pub const MAX_INDEX: u32 = Self::LOCAL_BIT - 1;

    /// The store of a library cycle with the number [index] (1 ..= MAX_INDEX).
    pub fn cycle(index: u32) -> StoreId {
        assert!(
            index != 0 && index <= Self::MAX_INDEX,
            "cycle store index {index}"
        );
        StoreId(index)
    }

    /// The store of a body analysis task with the number [index].
    pub fn local(index: u32) -> StoreId {
        assert!(index <= Self::MAX_INDEX, "local store index {index}");
        StoreId(Self::LOCAL_BIT | index)
    }

    pub fn kind(self) -> StoreKind {
        if self.0 == 0 {
            StoreKind::Synthetic
        } else if self.0 & Self::LOCAL_BIT != 0 {
            StoreKind::Local
        } else {
            StoreKind::Cycle
        }
    }

    #[inline(always)]
    pub fn is_local(self) -> bool {
        self.0 & Self::LOCAL_BIT != 0
    }

    #[inline(always)]
    pub fn raw(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for StoreId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            StoreKind::Synthetic => write!(f, "S"),
            StoreKind::Cycle => write!(f, "C{}", self.0),
            StoreKind::Local => write!(f, "L{}", self.0 & !Self::LOCAL_BIT),
        }
    }
}

macro_rules! tags {
    ($($(#[$m:meta])* $name:ident = $v:literal,)*) => {
        /// The storage kind of an element or fragment: which vector of the
        /// store holds it. One tag per Dart `*ElementImpl` / `*FragmentImpl`
        /// class that can be instantiated. Subclasses without own element
        /// data (`FieldFormalParameterElementImpl`,
        /// `BindPatternVariableElementImpl`, ...) have their own tag but share
        /// the vector of their superclass.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[repr(u8)]
        pub enum Tag {
            $($(#[$m])* $name = $v,)*
        }

        impl Tag {
            pub const ALL: &'static [Tag] = &[$(Tag::$name,)*];

            #[inline(always)]
            pub fn from_u8(v: u8) -> Option<Tag> {
                match v {
                    $($v => Some(Tag::$name),)*
                    _ => None,
                }
            }

            pub fn name(self) -> &'static str {
                match self {
                    $(Tag::$name => stringify!($name),)*
                }
            }
        }
    };
}

tags! {
    Class = 1,
    Enum = 2,
    Mixin = 3,
    Extension = 4,
    ExtensionType = 5,
    Field = 6,
    Getter = 7,
    Setter = 8,
    Method = 9,
    Constructor = 10,
    TopLevelFunction = 11,
    TopLevelVariable = 12,
    TypeAlias = 13,
    TypeParameter = 14,
    FormalParameter = 15,
    FieldFormalParameter = 16,
    SuperFormalParameter = 17,
    Prefix = 18,
    /// `LibraryElementImpl` / `LibraryFragmentImpl` (one per unit).
    Library = 19,
    GenericFunctionType = 20,
    LocalVariable = 21,
    PatternVariable = 22,
    BindPatternVariable = 23,
    JoinPatternVariable = 24,
    LocalFunction = 25,
    Label = 26,
    MultiplyDefined = 27,
    /// `DynamicElementImpl.instance`: one fixed id, no data.
    Dynamic = 28,
    /// `NeverElementImpl.instance`: one fixed id, no data.
    Never = 29,
}

const INDEX_BITS: u32 = 32;
const TAG_BITS: u32 = 8;

#[inline(always)]
const fn pack(store: StoreId, tag: Tag, index: u32) -> NonZeroU64 {
    let raw =
        ((store.0 as u64) << (INDEX_BITS + TAG_BITS)) | ((tag as u64) << INDEX_BITS) | index as u64;
    // The tag is never 0, so the value is never 0.
    match NonZeroU64::new(raw) {
        Some(v) => v,
        None => panic!("zero id"),
    }
}

macro_rules! raw_id {
    ($(#[$m:meta])* $name:ident, $prefix:literal) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(NonZeroU64);

        impl $name {
            #[inline(always)]
            pub const fn new(store: StoreId, tag: Tag, index: u32) -> $name {
                $name(pack(store, tag, index))
            }

            #[inline(always)]
            pub fn store(self) -> StoreId {
                StoreId((self.0.get() >> (INDEX_BITS + TAG_BITS)) as u32)
            }

            #[inline(always)]
            pub fn tag(self) -> Tag {
                let v = (self.0.get() >> INDEX_BITS) as u8;
                // Only `new` creates ids, so the tag is valid.
                Tag::from_u8(v).expect("valid tag")
            }

            /// The index in the vector of [`Self::tag`] in the store.
            #[inline(always)]
            pub fn index(self) -> u32 {
                self.0.get() as u32
            }

            /// The packed value. Only for canonical forms (sorting before
            /// interning, hashing); never for output.
            #[inline(always)]
            pub fn raw(self) -> u64 {
                self.0.get()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}:{:?}#{}", $prefix, self.tag().name(), self.store(), self.index())
            }
        }
    };
}

raw_id!(
    /// An element (untyped). See the module documentation.
    ElementId,
    "E"
);
raw_id!(
    /// A fragment (untyped). See the module documentation.
    FragmentId,
    "F"
);

impl ElementId {
    /// `DynamicElementImpl.instance`.
    pub const DYNAMIC: ElementId = ElementId::new(StoreId::SYNTHETIC, Tag::Dynamic, 0);
    /// `NeverElementImpl.instance`.
    pub const NEVER: ElementId = ElementId::new(StoreId::SYNTHETIC, Tag::Never, 0);

    /// Dart `element is T` / `element as T?`: the typed id when the tag of
    /// this element is a `T`.
    #[inline(always)]
    pub fn cast<T: ElementType + ?Sized>(self) -> Option<EId<T>> {
        if T::test(self.tag()) {
            Some(EId::from_raw(self))
        } else {
            None
        }
    }

    /// Dart `element is T`.
    #[inline(always)]
    pub fn is<T: ElementType + ?Sized>(self) -> bool {
        T::test(self.tag())
    }
}

impl FragmentId {
    /// `DynamicFragmentImpl.instance`.
    pub const DYNAMIC: FragmentId = FragmentId::new(StoreId::SYNTHETIC, Tag::Dynamic, 0);
    /// `NeverFragmentImpl.instance`.
    pub const NEVER: FragmentId = FragmentId::new(StoreId::SYNTHETIC, Tag::Never, 0);

    /// Dart `fragment is T` / `fragment as T?`.
    #[inline(always)]
    pub fn cast<T: FragmentType + ?Sized>(self) -> Option<FId<T>> {
        if T::test(self.tag()) {
            Some(FId::from_raw(self))
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn is<T: FragmentType + ?Sized>(self) -> bool {
        T::test(self.tag())
    }
}

/// An element class or category: a data struct ([`crate::ClassElement`]) or
/// an abstract Dart interface ([`InterfaceElement`], an empty enum).
pub trait ElementType {
    /// The Dart name (the public interface name, without `Impl`).
    const NAME: &'static str;
    /// Dart `element is T`, by the tag of the element.
    fn test(tag: Tag) -> bool;
}

/// A fragment class or category.
pub trait FragmentType {
    const NAME: &'static str;
    fn test(tag: Tag) -> bool;
}

/// `T: SubtypeOf<U>`: every `T` is a `U` (Dart `T implements U`). Used by
/// [`EId::upcast`] and [`FId::upcast`].
pub trait SubtypeOf<U: ?Sized> {}

impl<T: ?Sized> SubtypeOf<T> for T {}

macro_rules! typed_id {
    ($(#[$m:meta])* $name:ident, $raw:ident, $bound:ident) => {
        $(#[$m])*
        #[repr(transparent)]
        pub struct $name<T: ?Sized> {
            raw: $raw,
            _t: PhantomData<fn() -> T>,
        }

        impl<T: ?Sized> $name<T> {
            /// A typed id without a check. The tag of [raw] must be a `T`.
            #[inline(always)]
            pub const fn from_raw(raw: $raw) -> Self {
                $name { raw, _t: PhantomData }
            }

            #[inline(always)]
            pub fn raw(self) -> $raw {
                self.raw
            }

            #[inline(always)]
            pub fn store(self) -> StoreId {
                self.raw.store()
            }

            #[inline(always)]
            pub fn tag(self) -> Tag {
                self.raw.tag()
            }

            #[inline(always)]
            pub fn index(self) -> u32 {
                self.raw.index()
            }

            /// Converts to a supertype (no check at run time).
            #[inline(always)]
            pub fn upcast<U: ?Sized>(self) -> $name<U>
            where
                T: SubtypeOf<U>,
            {
                $name::from_raw(self.raw)
            }

            /// Converts to another type when the tag matches (Dart `as T?`).
            #[inline(always)]
            pub fn cast<U: $bound + ?Sized>(self) -> Option<$name<U>> {
                self.raw.cast::<U>()
            }
        }

        impl<T: ?Sized> Clone for $name<T> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<T: ?Sized> Copy for $name<T> {}

        impl<T: ?Sized> PartialEq for $name<T> {
            fn eq(&self, other: &Self) -> bool {
                self.raw == other.raw
            }
        }

        impl<T: ?Sized> Eq for $name<T> {}

        impl<T: ?Sized> std::hash::Hash for $name<T> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.raw.hash(state)
            }
        }

        impl<T: ?Sized + $bound> fmt::Debug for $name<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({:?})", T::NAME, self.raw)
            }
        }

        impl<T: ?Sized> From<$name<T>> for $raw {
            #[inline(always)]
            fn from(id: $name<T>) -> $raw {
                id.raw
            }
        }
    };
}

typed_id!(
    /// A typed element id: `EId<ClassElement>`, `EId<InterfaceElement>`.
    /// Same layout as [`ElementId`].
    EId,
    ElementId,
    ElementType
);
typed_id!(
    /// A typed fragment id: `FId<ClassFragment>`, `FId<InstanceFragment>`.
    /// Same layout as [`FragmentId`].
    FId,
    FragmentId,
    FragmentType
);

macro_rules! categories {
    ($trait:ident: $($(#[$m:meta])* $name:ident => $dart:literal [$($tag:ident)|*],)*) => {
        $(
            $(#[$m])*
            pub enum $name {}

            impl $trait for $name {
                const NAME: &'static str = $dart;
                #[inline(always)]
                fn test(tag: Tag) -> bool {
                    matches!(tag, $(Tag::$tag)|*)
                }
            }
        )*
    };
}

categories! { ElementType:
    /// Any element (Dart `Element`).
    Element => "Element" [Class | Enum | Mixin | Extension | ExtensionType | Field | Getter
        | Setter | Method | Constructor | TopLevelFunction | TopLevelVariable | TypeAlias
        | TypeParameter | FormalParameter | FieldFormalParameter | SuperFormalParameter | Prefix
        | Library | GenericFunctionType | LocalVariable | PatternVariable | BindPatternVariable
        | JoinPatternVariable | LocalFunction | Label | MultiplyDefined | Dynamic | Never],
    /// Dart `InstanceElement`.
    InstanceElement => "InstanceElement" [Class | Enum | Mixin | Extension | ExtensionType],
    /// Dart `InterfaceElement`.
    InterfaceElement => "InterfaceElement" [Class | Enum | Mixin | ExtensionType],
    /// Dart `TypeDefiningElement`.
    TypeDefiningElement => "TypeDefiningElement" [Class | Enum | Mixin | ExtensionType
        | TypeAlias | TypeParameter | Dynamic | Never],
    /// Dart `TypeParameterizedElement`.
    TypeParameterizedElement => "TypeParameterizedElement" [Class | Enum | Mixin | Extension
        | ExtensionType | TypeAlias | Method | Constructor | Getter | Setter | TopLevelFunction
        | LocalFunction | GenericFunctionType],
    /// Dart `FunctionTypedElement`.
    FunctionTypedElement => "FunctionTypedElement" [Method | Constructor | Getter | Setter
        | TopLevelFunction | LocalFunction | GenericFunctionType],
    /// Dart `ExecutableElement`.
    ExecutableElement => "ExecutableElement" [Method | Constructor | Getter | Setter
        | TopLevelFunction | LocalFunction],
    /// Dart `PropertyAccessorElement`.
    PropertyAccessorElement => "PropertyAccessorElement" [Getter | Setter],
    /// Dart `VariableElement`.
    VariableElement => "VariableElement" [Field | TopLevelVariable | FormalParameter
        | FieldFormalParameter | SuperFormalParameter | LocalVariable | PatternVariable
        | BindPatternVariable | JoinPatternVariable],
    /// Dart `PropertyInducingElement`.
    PropertyInducingElement => "PropertyInducingElement" [Field | TopLevelVariable],
    /// Dart `PromotableElementImpl` (formal parameters and local variables).
    PromotableElement => "PromotableElement" [FormalParameter | FieldFormalParameter
        | SuperFormalParameter | LocalVariable | PatternVariable | BindPatternVariable
        | JoinPatternVariable],
    /// Dart `LocalElement`.
    LocalElement => "LocalElement" [FormalParameter | FieldFormalParameter
        | SuperFormalParameter | LocalVariable | PatternVariable | BindPatternVariable
        | JoinPatternVariable | LocalFunction],
    /// Dart `FieldFormalParameterElement` (data: [`crate::FormalParameterElement`]).
    FieldFormalParameterElement => "FieldFormalParameterElement" [FieldFormalParameter],
    /// Dart `SuperFormalParameterElement` (data: [`crate::FormalParameterElement`]).
    SuperFormalParameterElement => "SuperFormalParameterElement" [SuperFormalParameter],
    /// Dart `PatternVariableElement` (data: [`crate::LocalVariableElement`]).
    PatternVariableElement => "PatternVariableElement" [PatternVariable
        | BindPatternVariable | JoinPatternVariable],
    /// Dart `BindPatternVariableElement` (data: [`crate::LocalVariableElement`]).
    BindPatternVariableElement => "BindPatternVariableElement" [BindPatternVariable],
    /// Dart `JoinPatternVariableElement` (data: [`crate::LocalVariableElement`]).
    JoinPatternVariableElement => "JoinPatternVariableElement" [JoinPatternVariable],
    /// `DynamicElementImpl` (no data; [`ElementId::DYNAMIC`]).
    DynamicElement => "DynamicElement" [Dynamic],
    /// `NeverElementImpl` (no data; [`ElementId::NEVER`]).
    NeverElement => "NeverElement" [Never],
}

categories! { FragmentType:
    /// Any fragment (Dart `Fragment`).
    Fragment => "Fragment" [Class | Enum | Mixin | Extension | ExtensionType | Field | Getter
        | Setter | Method | Constructor | TopLevelFunction | TopLevelVariable | TypeAlias
        | TypeParameter | FormalParameter | FieldFormalParameter | SuperFormalParameter | Prefix
        | Library | GenericFunctionType | LocalVariable | PatternVariable | BindPatternVariable
        | JoinPatternVariable | LocalFunction | Label | MultiplyDefined | Dynamic | Never],
    /// Dart `InstanceFragment`.
    InstanceFragment => "InstanceFragment" [Class | Enum | Mixin | Extension | ExtensionType],
    /// Dart `InterfaceFragment`.
    InterfaceFragment => "InterfaceFragment" [Class | Enum | Mixin | ExtensionType],
    /// Dart `ExecutableFragment`.
    ExecutableFragment => "ExecutableFragment" [Method | Constructor | Getter | Setter
        | TopLevelFunction | LocalFunction],
    /// Dart `FunctionFragmentImpl` (top-level and local functions).
    FunctionFragment => "FunctionFragment" [TopLevelFunction | LocalFunction],
    /// Dart `PropertyAccessorFragment`.
    PropertyAccessorFragment => "PropertyAccessorFragment" [Getter | Setter],
    /// Dart `VariableFragment`.
    VariableFragment => "VariableFragment" [Field | TopLevelVariable | FormalParameter
        | FieldFormalParameter | SuperFormalParameter | LocalVariable | PatternVariable
        | BindPatternVariable | JoinPatternVariable],
    /// Dart `NonParameterVariableFragmentImpl`.
    NonParameterVariableFragment => "NonParameterVariableFragment" [Field | TopLevelVariable
        | LocalVariable | PatternVariable | BindPatternVariable | JoinPatternVariable],
    /// Dart `PropertyInducingFragment`.
    PropertyInducingFragment => "PropertyInducingFragment" [Field | TopLevelVariable],
    /// Dart `FieldFormalParameterFragment` (data: [`crate::FormalParameterFragment`]).
    FieldFormalParameterFragment => "FieldFormalParameterFragment" [FieldFormalParameter],
    /// Dart `SuperFormalParameterFragment` (data: [`crate::FormalParameterFragment`]).
    SuperFormalParameterFragment => "SuperFormalParameterFragment" [SuperFormalParameter],
    /// Dart `PatternVariableFragment` (data: [`crate::LocalVariableFragment`]).
    PatternVariableFragment => "PatternVariableFragment" [PatternVariable
        | BindPatternVariable | JoinPatternVariable],
    /// Dart `BindPatternVariableFragment` (data: [`crate::LocalVariableFragment`]).
    BindPatternVariableFragment => "BindPatternVariableFragment" [BindPatternVariable],
    /// Dart `JoinPatternVariableFragment` (data: [`crate::LocalVariableFragment`]).
    JoinPatternVariableFragment => "JoinPatternVariableFragment" [JoinPatternVariable],
}

/// Declares `T: SubtypeOf<U>` for each listed pair.
macro_rules! subtypes {
    ($($t:ty: $($u:ty),+;)*) => {
        $($(impl SubtypeOf<$u> for $t {})+)*
    };
}

subtypes! {
    InstanceElement: Element, TypeParameterizedElement;
    InterfaceElement: InstanceElement, TypeDefiningElement, TypeParameterizedElement, Element;
    TypeDefiningElement: Element;
    TypeParameterizedElement: Element;
    FunctionTypedElement: TypeParameterizedElement, Element;
    ExecutableElement: FunctionTypedElement, TypeParameterizedElement, Element;
    PropertyAccessorElement: ExecutableElement, FunctionTypedElement, Element;
    VariableElement: Element;
    PropertyInducingElement: VariableElement, Element;
    PromotableElement: VariableElement, Element;
    LocalElement: Element;
    FieldFormalParameterElement: crate::FormalParameterElement, PromotableElement, LocalElement,
        VariableElement, Element;
    SuperFormalParameterElement: crate::FormalParameterElement, PromotableElement, LocalElement,
        VariableElement, Element;
    PatternVariableElement: crate::LocalVariableElement, PromotableElement, LocalElement,
        VariableElement, Element;
    BindPatternVariableElement: PatternVariableElement, crate::LocalVariableElement,
        PromotableElement, LocalElement, VariableElement, Element;
    JoinPatternVariableElement: PatternVariableElement, crate::LocalVariableElement,
        PromotableElement, LocalElement, VariableElement, Element;
    DynamicElement: TypeDefiningElement, Element;
    NeverElement: TypeDefiningElement, Element;

    InstanceFragment: Fragment;
    InterfaceFragment: InstanceFragment, Fragment;
    ExecutableFragment: Fragment;
    FunctionFragment: ExecutableFragment, Fragment;
    PropertyAccessorFragment: ExecutableFragment, Fragment;
    VariableFragment: Fragment;
    NonParameterVariableFragment: VariableFragment, Fragment;
    PropertyInducingFragment: NonParameterVariableFragment, VariableFragment, Fragment;
    FieldFormalParameterFragment: crate::FormalParameterFragment, VariableFragment, Fragment;
    SuperFormalParameterFragment: crate::FormalParameterFragment, VariableFragment, Fragment;
    PatternVariableFragment: crate::LocalVariableFragment, NonParameterVariableFragment,
        VariableFragment, Fragment;
    BindPatternVariableFragment: PatternVariableFragment, crate::LocalVariableFragment,
        NonParameterVariableFragment, VariableFragment, Fragment;
    JoinPatternVariableFragment: PatternVariableFragment, crate::LocalVariableFragment,
        NonParameterVariableFragment, VariableFragment, Fragment;
}

// The data structs (concrete classes) and their Dart supertypes.
subtypes! {
    crate::ClassElement: InterfaceElement, InstanceElement, TypeDefiningElement,
        TypeParameterizedElement, Element;
    crate::EnumElement: InterfaceElement, InstanceElement, TypeDefiningElement,
        TypeParameterizedElement, Element;
    crate::MixinElement: InterfaceElement, InstanceElement, TypeDefiningElement,
        TypeParameterizedElement, Element;
    crate::ExtensionTypeElement: InterfaceElement, InstanceElement, TypeDefiningElement,
        TypeParameterizedElement, Element;
    crate::ExtensionElement: InstanceElement, TypeParameterizedElement, Element;
    crate::FieldElement: PropertyInducingElement, VariableElement, Element;
    crate::TopLevelVariableElement: PropertyInducingElement, VariableElement, Element;
    crate::GetterElement: PropertyAccessorElement, ExecutableElement, FunctionTypedElement,
        TypeParameterizedElement, Element;
    crate::SetterElement: PropertyAccessorElement, ExecutableElement, FunctionTypedElement,
        TypeParameterizedElement, Element;
    crate::MethodElement: ExecutableElement, FunctionTypedElement, TypeParameterizedElement,
        Element;
    crate::ConstructorElement: ExecutableElement, FunctionTypedElement,
        TypeParameterizedElement, Element;
    crate::TopLevelFunctionElement: ExecutableElement, FunctionTypedElement,
        TypeParameterizedElement, Element;
    crate::LocalFunctionElement: ExecutableElement, FunctionTypedElement,
        TypeParameterizedElement, LocalElement, Element;
    crate::GenericFunctionTypeElement: FunctionTypedElement, TypeParameterizedElement, Element;
    crate::TypeAliasElement: TypeDefiningElement, TypeParameterizedElement, Element;
    crate::TypeParameterElement: TypeDefiningElement, Element;
    crate::FormalParameterElement: VariableElement, PromotableElement, LocalElement, Element;
    crate::LocalVariableElement: VariableElement, PromotableElement, LocalElement, Element;
    crate::PrefixElement: Element;
    crate::LibraryElement: Element;
    crate::LabelElement: Element;
    crate::MultiplyDefinedElement: Element;

    crate::ClassFragment: InterfaceFragment, InstanceFragment, Fragment;
    crate::EnumFragment: InterfaceFragment, InstanceFragment, Fragment;
    crate::MixinFragment: InterfaceFragment, InstanceFragment, Fragment;
    crate::ExtensionTypeFragment: InterfaceFragment, InstanceFragment, Fragment;
    crate::ExtensionFragment: InstanceFragment, Fragment;
    crate::FieldFragment: PropertyInducingFragment, NonParameterVariableFragment,
        VariableFragment, Fragment;
    crate::TopLevelVariableFragment: PropertyInducingFragment, NonParameterVariableFragment,
        VariableFragment, Fragment;
    crate::GetterFragment: PropertyAccessorFragment, ExecutableFragment, Fragment;
    crate::SetterFragment: PropertyAccessorFragment, ExecutableFragment, Fragment;
    crate::MethodFragment: ExecutableFragment, Fragment;
    crate::ConstructorFragment: ExecutableFragment, Fragment;
    crate::TopLevelFunctionFragment: FunctionFragment, ExecutableFragment, Fragment;
    crate::LocalFunctionFragment: FunctionFragment, ExecutableFragment, Fragment;
    crate::FormalParameterFragment: VariableFragment, Fragment;
    crate::LocalVariableFragment: NonParameterVariableFragment, VariableFragment, Fragment;
    crate::TypeAliasFragment: Fragment;
    crate::TypeParameterFragment: Fragment;
    crate::GenericFunctionTypeFragment: Fragment;
    crate::PrefixFragment: Fragment;
    crate::LibraryFragment: Fragment;
    crate::LabelFragment: Fragment;
    crate::MultiplyDefinedFragment: Fragment;
}
