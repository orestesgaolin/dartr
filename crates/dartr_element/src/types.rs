// Dart source: pkg/analyzer/lib/src/dart/element/type.dart (TypeImpl and
// subclasses), type_schema.dart (UnknownInferredType), member.dart
// (Substituted*ElementImpl), type_algebra.dart (MapSubstitution)

//! The type model: [`TypeKind`] values interned to [`TypeId`]s.
//!
//! - [`TypeId`] equality is exact structural identity, including the alias,
//!   parameter names and parameter element refs. It is not Dart `==` (that is
//!   `TypeSystem::dart_eq`, unit A3). Where the Dart code uses `identical`,
//!   use `TypeId ==`.
//! - Lists inside types (`typeArguments`, record fields, function parameters,
//!   type formals) are interned slices ([`ListId`]), so a [`TypeKind`] is
//!   `Copy` and small.
//! - Ids with bit 31 set live in the local overlay of one analysis task
//!   ([`crate::LocalArena`]). A value goes to the overlay if and only if it
//!   mentions a local id (a local type, list, element, substitution or
//!   member). So each structure has exactly one id: a global value never
//!   contains a local id (checked with `debug_assert!` in
//!   [`crate::Interner`]), and a local value always contains one.

use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroU32;

pub use dartr_ast::ParameterKind;

use crate::ids::{EId, ElementId, InterfaceElement};
use crate::name::Name;
use crate::{TypeAliasElement, TypeParameterElement};

pub(crate) const LOCAL_BIT: u32 = 1 << 31;

macro_rules! interned_id {
    ($(#[$m:meta])* $name:ident) => {
        $(#[$m])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(NonZeroU32);

        impl $name {
            #[inline(always)]
            pub(crate) fn new(index: u32, local: bool) -> $name {
                assert!(index < LOCAL_BIT - 1, "interner overflow");
                let v = (index + 1) | if local { LOCAL_BIT } else { 0 };
                $name(NonZeroU32::new(v).unwrap())
            }

            /// Whether this id is in the local overlay of an analysis task.
            #[inline(always)]
            pub fn is_local(self) -> bool {
                self.0.get() & LOCAL_BIT != 0
            }

            /// The index in its pool (global or local).
            #[inline(always)]
            pub fn index(self) -> u32 {
                (self.0.get() & !LOCAL_BIT) - 1
            }

            /// The raw value. Only for canonical forms; never for output.
            #[inline(always)]
            pub fn raw(self) -> u32 {
                self.0.get()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let l = if self.is_local() { "L" } else { "" };
                write!(f, "{}{}{}", stringify!($name), l, self.index())
            }
        }
    };
}

interned_id!(
    /// An interned type ([`TypeKind`]).
    TypeId
);
interned_id!(
    /// An interned substitution (Dart `MapSubstitution`): pairs of a type
    /// parameter and its replacement, sorted by the raw id of the type
    /// parameter (a canonical form, never output).
    SubstId
);
interned_id!(
    /// An interned substituted member ([`Member`]; Dart
    /// `Substituted*ElementImpl` of member.dart).
    MemberId
);
interned_id!(
    /// An interned alias reference ([`AliasRef`]; Dart
    /// `InstantiatedTypeAliasElementImpl`).
    AliasId
);

impl TypeId {
    /// `DynamicTypeImpl.instance`.
    pub const DYNAMIC: TypeId = TypeId(NonZeroU32::new(1).unwrap());
    /// `VoidTypeImpl.instance`.
    pub const VOID: TypeId = TypeId(NonZeroU32::new(2).unwrap());
    /// `InvalidTypeImpl.instance`.
    pub const INVALID: TypeId = TypeId(NonZeroU32::new(3).unwrap());
    /// `UnknownInferredType.instance` (the type schema `_`).
    pub const UNKNOWN: TypeId = TypeId(NonZeroU32::new(4).unwrap());
    /// `NeverTypeImpl.instance` (`Never`).
    pub const NEVER: TypeId = TypeId(NonZeroU32::new(5).unwrap());
    /// `NeverTypeImpl.instanceNullable` (`Never?`).
    pub const NEVER_QUESTION: TypeId = TypeId(NonZeroU32::new(6).unwrap());

    /// The kinds interned first, in this order, so that the constants above
    /// are valid in every interner.
    pub(crate) const FIXED: [TypeKind; 6] = [
        TypeKind::Dynamic,
        TypeKind::Void,
        TypeKind::Invalid,
        TypeKind::Unknown,
        TypeKind::Never(Nullability::None),
        TypeKind::Never(Nullability::Question),
    ];
}

/// An interned list of `T` (type arguments, record fields, function
/// parameters, type formals). [`ListId::EMPTY`] is the empty list in every
/// interner.
#[repr(transparent)]
pub struct ListId<T> {
    raw: u32,
    _t: PhantomData<fn() -> T>,
}

impl<T> ListId<T> {
    pub const EMPTY: ListId<T> = ListId {
        raw: 0,
        _t: PhantomData,
    };

    #[inline(always)]
    pub(crate) fn new(index: u32, local: bool) -> ListId<T> {
        assert!(index < LOCAL_BIT - 1, "interner overflow");
        ListId {
            raw: (index + 1) | if local { LOCAL_BIT } else { 0 },
            _t: PhantomData,
        }
    }

    #[inline(always)]
    pub fn is_empty(self) -> bool {
        self.raw == 0
    }

    #[inline(always)]
    pub fn is_local(self) -> bool {
        self.raw & LOCAL_BIT != 0
    }

    /// The index in its pool; only for a non-empty list.
    #[inline(always)]
    pub(crate) fn index(self) -> u32 {
        (self.raw & !LOCAL_BIT) - 1
    }

    #[inline(always)]
    pub fn raw(self) -> u32 {
        self.raw
    }
}

impl<T> Clone for ListId<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ListId<T> {}
impl<T> PartialEq for ListId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T> Eq for ListId<T> {}
impl<T> std::hash::Hash for ListId<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state)
    }
}
impl<T> Default for ListId<T> {
    fn default() -> Self {
        Self::EMPTY
    }
}
impl<T> fmt::Debug for ListId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            write!(f, "List[]")
        } else {
            let l = if self.is_local() { "L" } else { "" };
            write!(f, "List{}{}", l, self.index())
        }
    }
}

/// `typeArguments`, `positionalTypes`, ...
pub type TypeList = ListId<TypeId>;
/// Named record fields, sorted by name (code unit order of the text).
pub type NamedFields = ListId<NamedType>;
/// Type formals of a function type.
pub type TypeParamList = ListId<EId<TypeParameterElement>>;
/// Parameters of a function type.
pub type ParamList = ListId<FnParam>;

/// Dart `NullabilitySuffix` (`_fe_analyzer_shared`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Nullability {
    /// No suffix: `int`.
    #[default]
    None,
    /// `int?`.
    Question,
    /// `int*` (legacy; kept for completeness of the shared enum).
    Star,
}

/// Dart `Variance` (`_fe_analyzer_shared/lib/src/types/shared_type.dart`).
/// The lattice operations are in `dartr_flow::Variance`; this copy is the
/// stored value of [`crate::TypeParameterElement::variance`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Variance {
    Unrelated,
    Covariant,
    Contravariant,
    Invariant,
}

impl Variance {
    /// The `in`/`out`/`inout` keyword.
    pub fn keyword(self) -> &'static str {
        match self {
            Variance::Unrelated => "",
            Variance::Covariant => "out",
            Variance::Contravariant => "in",
            Variance::Invariant => "inout",
        }
    }
}

/// A reference to an element as the Dart code passes it around
/// (`InternalExecutableElement`, `InternalFormalParameterElement`, ...): the
/// base element or a substituted member.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ElemRef {
    Base(ElementId),
    Member(MemberId),
}

impl ElemRef {
    /// Whether this reference mentions a local id.
    pub fn is_local(self) -> bool {
        match self {
            ElemRef::Base(e) => e.store().is_local(),
            ElemRef::Member(m) => m.is_local(),
        }
    }
}

impl From<ElementId> for ElemRef {
    fn from(e: ElementId) -> ElemRef {
        ElemRef::Base(e)
    }
}

/// A substituted member (`SubstitutedElementImpl` of member.dart): the
/// declaration and the substitution of the type parameters of its enclosing
/// elements. Getters such as `type` and `returnType` substitute lazily
/// (methods on `Ctx`, unit A7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Member {
    pub base: ElementId,
    pub subst: SubstId,
}

/// One pair of a substitution.
pub type SubstPair = (EId<TypeParameterElement>, TypeId);

/// An instantiated type alias (`InstantiatedTypeAliasElementImpl`, the
/// `alias` of a type).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AliasRef {
    pub element: EId<TypeAliasElement>,
    pub args: TypeList,
}

/// A named record field (`RecordTypeNamedFieldImpl`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NamedType {
    pub name: Name,
    pub ty: TypeId,
}

/// One parameter of a function type (`FunctionTypeImpl.parameters`, which
/// are `InternalFormalParameterElement`s in Dart).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FnParam {
    /// `null` for positional parameters of synthesized types.
    pub name: Option<Name>,
    pub kind: ParameterKind,
    pub ty: TypeId,
    pub covariant: bool,
    /// The declaring parameter element (for `NamedExpression.element`,
    /// `@Deprecated`, `required`); `None` for synthesized parameters.
    pub element: Option<ElemRef>,
}

/// `FunctionTypeImpl` (type.dart:100).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FunctionTypeData {
    pub type_params: TypeParamList,
    /// Positional parameters in order, then named parameters sorted by name,
    /// as in the `FunctionTypeImpl` factory (type.dart:141).
    pub params: ParamList,
    pub required_positional: u16,
    pub ret: TypeId,
    pub nullability: Nullability,
    pub alias: Option<AliasId>,
}

/// A type (`TypeImpl` subclasses). Interned to a [`TypeId`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TypeKind {
    /// `DynamicTypeImpl`.
    Dynamic,
    /// `VoidTypeImpl`.
    Void,
    /// `InvalidTypeImpl`.
    Invalid,
    /// `UnknownInferredType`: the type schema `_` (type_schema.dart:21).
    Unknown,
    /// `NeverTypeImpl`.
    Never(Nullability),
    /// `InterfaceTypeImpl` (also `FutureOrTypeImpl`, `NullTypeImpl`: the
    /// element tells).
    Interface {
        element: EId<InterfaceElement>,
        args: TypeList,
        nullability: Nullability,
        alias: Option<AliasId>,
    },
    /// `FunctionTypeImpl`.
    Function(FunctionTypeData),
    /// `RecordTypeImpl`.
    Record {
        positional: TypeList,
        named: NamedFields,
        nullability: Nullability,
        alias: Option<AliasId>,
    },
    /// `TypeParameterTypeImpl`.
    TypeParameter {
        param: EId<TypeParameterElement>,
        nullability: Nullability,
        promoted_bound: Option<TypeId>,
        alias: Option<AliasId>,
    },
}

/// Whether a value mentions an id of the local overlay or a local store.
pub trait MentionsLocal {
    fn mentions_local(&self) -> bool;
}

impl MentionsLocal for TypeId {
    fn mentions_local(&self) -> bool {
        self.is_local()
    }
}

impl<T> MentionsLocal for ListId<T> {
    fn mentions_local(&self) -> bool {
        self.is_local()
    }
}

impl<T: ?Sized> MentionsLocal for EId<T> {
    fn mentions_local(&self) -> bool {
        self.store().is_local()
    }
}

impl MentionsLocal for ElemRef {
    fn mentions_local(&self) -> bool {
        self.is_local()
    }
}

impl MentionsLocal for Option<AliasId> {
    fn mentions_local(&self) -> bool {
        self.is_some_and(|a| a.is_local())
    }
}

impl MentionsLocal for NamedType {
    fn mentions_local(&self) -> bool {
        self.ty.is_local()
    }
}

impl MentionsLocal for FnParam {
    fn mentions_local(&self) -> bool {
        self.ty.is_local() || self.element.is_some_and(|e| e.is_local())
    }
}

impl MentionsLocal for SubstPair {
    fn mentions_local(&self) -> bool {
        self.0.mentions_local() || self.1.is_local()
    }
}

impl MentionsLocal for Member {
    fn mentions_local(&self) -> bool {
        self.base.store().is_local() || self.subst.is_local()
    }
}

impl MentionsLocal for AliasRef {
    fn mentions_local(&self) -> bool {
        self.element.mentions_local() || self.args.is_local()
    }
}

impl MentionsLocal for TypeKind {
    fn mentions_local(&self) -> bool {
        match *self {
            TypeKind::Dynamic
            | TypeKind::Void
            | TypeKind::Invalid
            | TypeKind::Unknown
            | TypeKind::Never(_) => false,
            TypeKind::Interface {
                element,
                args,
                alias,
                ..
            } => element.mentions_local() || args.is_local() || alias.mentions_local(),
            TypeKind::Function(f) => {
                f.type_params.is_local()
                    || f.params.is_local()
                    || f.ret.is_local()
                    || f.alias.mentions_local()
            }
            TypeKind::Record {
                positional,
                named,
                alias,
                ..
            } => positional.is_local() || named.is_local() || alias.mentions_local(),
            TypeKind::TypeParameter {
                param,
                promoted_bound,
                alias,
                ..
            } => {
                param.mentions_local()
                    || promoted_bound.is_some_and(|b| b.is_local())
                    || alias.mentions_local()
            }
        }
    }
}
