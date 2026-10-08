//! The type interner: the global [`Interner`] (sharded, in the
//! [`crate::Generation`]) and the [`TypeOverlay`] of one analysis task (in the
//! [`crate::LocalArena`]).
//!
//! Most code interns through [`crate::Ctx`] (`ctx.intern`, `ctx.ty`,
//! `ctx.intern_list`, `ctx.list`), which picks the global interner or the
//! overlay by the rule of [`crate::types`]: a value goes to the overlay if
//! and only if it mentions a local id.

use std::hash::Hash;

use parking_lot::Mutex;

use crate::TypeParameterElement;
use crate::ids::EId;
use crate::lookup::LookupMap;
use crate::pool::{Pool, SHARDS};
use crate::types::{
    AliasId, AliasRef, FnParam, ListId, Member, MemberId, MentionsLocal, NamedType, SubstId,
    SubstPair, TypeId, TypeKind,
};

/// The pools of one interner (global or overlay).
pub struct TypePools {
    types: Pool<TypeKind>,
    type_lists: Pool<Box<[TypeId]>>,
    named_lists: Pool<Box<[NamedType]>>,
    param_lists: Pool<Box<[FnParam]>>,
    type_param_lists: Pool<Box<[EId<TypeParameterElement>]>>,
    substs: Pool<Box<[SubstPair]>>,
    members: Pool<Member>,
    aliases: Pool<AliasRef>,
    /// Lazy types of substituted members (Dart caches them in the member
    /// object: `SubstitutedExecutableElementImpl._type`,
    /// `SubstitutedVariableElementImpl._type`), keyed by (member index,
    /// slot). Only for lookups.
    member_types: Mutex<LookupMap<(u32, u8), TypeId>>,
}

impl TypePools {
    fn new(shards: usize) -> TypePools {
        TypePools {
            types: Pool::new(shards),
            type_lists: Pool::new(shards),
            named_lists: Pool::new(shards),
            param_lists: Pool::new(shards),
            type_param_lists: Pool::new(shards),
            substs: Pool::new(shards),
            members: Pool::new(shards),
            aliases: Pool::new(shards),
            member_types: Mutex::new(LookupMap::new()),
        }
    }
}

/// An element type of an interned list ([`ListId<T>`]).
pub trait ListItem: Copy + Hash + Eq + MentionsLocal + 'static {
    #[doc(hidden)]
    fn pool(pools: &TypePools) -> &Pool<Box<[Self]>>;
}

impl ListItem for TypeId {
    fn pool(p: &TypePools) -> &Pool<Box<[Self]>> {
        &p.type_lists
    }
}
impl ListItem for NamedType {
    fn pool(p: &TypePools) -> &Pool<Box<[Self]>> {
        &p.named_lists
    }
}
impl ListItem for FnParam {
    fn pool(p: &TypePools) -> &Pool<Box<[Self]>> {
        &p.param_lists
    }
}
impl ListItem for EId<TypeParameterElement> {
    fn pool(p: &TypePools) -> &Pool<Box<[Self]>> {
        &p.type_param_lists
    }
}

/// Shared implementation of the global interner and the overlay.
/// `LOCAL` is the local bit of the ids it returns.
struct Pools<const LOCAL: bool>(TypePools);

impl<const LOCAL: bool> Pools<LOCAL> {
    #[inline]
    fn check(&self, mentions_local: bool) {
        if LOCAL {
            debug_assert!(
                mentions_local,
                "a value without local ids must be interned globally"
            );
        } else {
            debug_assert!(!mentions_local, "a global type must not contain a local id");
        }
    }

    fn intern(&self, kind: TypeKind) -> TypeId {
        self.check(kind.mentions_local());
        TypeId::new(self.0.types.intern(kind), LOCAL)
    }

    fn lookup(&self, kind: &TypeKind) -> Option<TypeId> {
        self.0.types.lookup(kind).map(|i| TypeId::new(i, LOCAL))
    }

    fn get(&self, id: TypeId) -> &TypeKind {
        assert_eq!(id.is_local(), LOCAL, "{id:?} is not in this interner");
        self.0.types.get(id.index())
    }

    fn intern_list<T: ListItem>(&self, items: &[T]) -> ListId<T> {
        if items.is_empty() {
            return ListId::EMPTY;
        }
        self.check(items.iter().any(MentionsLocal::mentions_local));
        ListId::new(T::pool(&self.0).intern_ref::<[T]>(items), LOCAL)
    }

    fn list<T: ListItem>(&self, id: ListId<T>) -> &[T] {
        if id.is_empty() {
            return &[];
        }
        assert_eq!(id.is_local(), LOCAL, "{id:?} is not in this interner");
        T::pool(&self.0).get(id.index())
    }

    fn intern_subst(&self, pairs: &mut [SubstPair]) -> SubstId {
        // Canonical order by the raw id of the type parameter (never output).
        pairs.sort_unstable_by_key(|(p, _)| p.raw().raw());
        self.check(pairs.iter().any(MentionsLocal::mentions_local));
        SubstId::new(self.0.substs.intern_ref::<[SubstPair]>(pairs), LOCAL)
    }

    fn subst(&self, id: SubstId) -> &[SubstPair] {
        assert_eq!(id.is_local(), LOCAL, "{id:?} is not in this interner");
        self.0.substs.get(id.index())
    }

    fn intern_member(&self, member: Member) -> MemberId {
        self.check(member.mentions_local());
        MemberId::new(self.0.members.intern(member), LOCAL)
    }

    fn member(&self, id: MemberId) -> &Member {
        assert_eq!(id.is_local(), LOCAL, "{id:?} is not in this interner");
        self.0.members.get(id.index())
    }

    fn member_type(&self, id: MemberId, slot: u8) -> Option<TypeId> {
        assert_eq!(id.is_local(), LOCAL, "{id:?} is not in this interner");
        self.0.member_types.lock().get(&(id.index(), slot)).copied()
    }

    fn set_member_type(&self, id: MemberId, slot: u8, t: TypeId) -> TypeId {
        assert_eq!(id.is_local(), LOCAL, "{id:?} is not in this interner");
        let mut map = self.0.member_types.lock();
        if let Some(&old) = map.get(&(id.index(), slot)) {
            return old;
        }
        map.insert((id.index(), slot), t);
        t
    }

    fn intern_alias(&self, alias: AliasRef) -> AliasId {
        self.check(alias.mentions_local());
        AliasId::new(self.0.aliases.intern(alias), LOCAL)
    }

    fn alias(&self, id: AliasId) -> &AliasRef {
        assert_eq!(id.is_local(), LOCAL, "{id:?} is not in this interner");
        self.0.aliases.get(id.index())
    }
}

macro_rules! interner_api {
    ($name:ident) => {
        impl $name {
            /// Interns a type.
            pub fn intern(&self, kind: TypeKind) -> TypeId {
                self.0.intern(kind)
            }

            /// The id of [kind] when it was interned here (no insert).
            pub fn lookup(&self, kind: &TypeKind) -> Option<TypeId> {
                self.0.lookup(kind)
            }

            /// The type with [id]. Panics for an id of another interner.
            #[inline]
            pub fn get(&self, id: TypeId) -> &TypeKind {
                self.0.get(id)
            }

            /// Interns a list ([`ListId::EMPTY`] for an empty slice).
            pub fn intern_list<T: ListItem>(&self, items: &[T]) -> ListId<T> {
                self.0.intern_list(items)
            }

            /// The items of [id].
            #[inline]
            pub fn list<T: ListItem>(&self, id: ListId<T>) -> &[T] {
                self.0.list(id)
            }

            /// Interns a substitution. [pairs] is sorted into the canonical
            /// order first.
            pub fn intern_subst(&self, pairs: &mut [SubstPair]) -> SubstId {
                self.0.intern_subst(pairs)
            }

            /// The sorted pairs of [id].
            pub fn subst(&self, id: SubstId) -> &[SubstPair] {
                self.0.subst(id)
            }

            pub fn intern_member(&self, member: Member) -> MemberId {
                self.0.intern_member(member)
            }

            pub fn member(&self, id: MemberId) -> &Member {
                self.0.member(id)
            }

            /// The cached type in [slot] of the member [id] (see
            /// `Ctx::member_type_cached`).
            pub fn member_type(&self, id: MemberId, slot: u8) -> Option<TypeId> {
                self.0.member_type(id, slot)
            }

            /// Caches [t] in [slot] of the member [id], unless a type is
            /// already there; returns the cached type.
            pub fn set_member_type(&self, id: MemberId, slot: u8, t: TypeId) -> TypeId {
                self.0.set_member_type(id, slot, t)
            }

            pub fn intern_alias(&self, alias: AliasRef) -> AliasId {
                self.0.intern_alias(alias)
            }

            pub fn alias(&self, id: AliasId) -> &AliasRef {
                self.0.alias(id)
            }

            /// The number of interned types.
            pub fn type_count(&self) -> usize {
                self.0.0.types.len()
            }
        }
    };
}

/// The global interner of a [`crate::Generation`]: `SHARDS` shards, lock-free
/// reads. Values must not mention local ids (`debug_assert!`).
pub struct Interner(Pools<false>);

interner_api!(Interner);

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

impl Interner {
    pub fn new() -> Interner {
        let interner = Interner(Pools(TypePools::new(SHARDS)));
        for (i, kind) in TypeId::FIXED.into_iter().enumerate() {
            let id = interner.intern(kind);
            debug_assert_eq!(id.index() as usize, i);
        }
        interner
    }
}

/// The local overlay of one analysis task. Values must mention a local id
/// (`debug_assert!`); [`crate::Ctx`] routes the others to the [`Interner`].
pub struct TypeOverlay(Pools<true>);

interner_api!(TypeOverlay);

impl Default for TypeOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeOverlay {
    pub fn new() -> TypeOverlay {
        TypeOverlay(Pools(TypePools::new(1)))
    }
}
