// Dart source: pkg/analyzer/lib/src/dart/element/type_provider.dart
// (TypeProviderImpl, TypeProviderBase)

//! [`TypeProvider`]: the fixed elements and types of `dart:core` and
//! `dart:async`.
//!
//! Each Dart getter has a slot. The linker fills the slots once the
//! `dart:core` / `dart:async` cycle has its elements (the Dart code looks
//! the classes up lazily, `_getClassElement`; while `dart:core` itself is
//! linked, slots are filled as soon as the class exists). A getter panics
//! when its slot is empty. The provider is then shared through
//! [`crate::WorldSnapshot::type_provider`].

use crate::ctx::Ctx;
use crate::flags::FragmentFlags;
use crate::ids::{EId, FId, InterfaceElement};
use crate::slot::OnceSlot;
use crate::store::StoredFragment;
use crate::types::{Nullability, TypeId, TypeKind};
use crate::{ClassElement, LibraryElement};

macro_rules! type_provider {
    (
        elements { $($el:ident,)* }
        types { $($ty:ident,)* }
    ) => {
        /// `TypeProviderImpl`. See the module documentation.
        #[derive(Debug, Default)]
        pub struct TypeProvider {
            /// Dart `_coreLibrary`.
            pub core_library: OnceSlot<EId<LibraryElement>>,
            /// Dart `_asyncLibrary`.
            pub async_library: OnceSlot<EId<LibraryElement>>,
            /// Dart `_enumElement` (`null` when `dart:core` has no `Enum`).
            pub enum_element: OnceSlot<Option<EId<ClassElement>>>,
            /// Dart `_enumType`.
            pub enum_type: OnceSlot<Option<TypeId>>,
            $(
                #[doc = concat!("Slot of the `", stringify!($el), "` getter.")]
                pub $el: OnceSlot<EId<ClassElement>>,
            )*
            $(
                #[doc = concat!("Slot of the `", stringify!($ty), "` getter.")]
                pub $ty: OnceSlot<TypeId>,
            )*
        }

        impl TypeProvider {
            $(
                #[doc = concat!("Dart `", stringify!($el), "`.")]
                #[track_caller]
                pub fn $el(&self) -> EId<ClassElement> {
                    *self.$el.get()
                }
            )*
            $(
                #[doc = concat!("Dart `", stringify!($ty), "`.")]
                #[track_caller]
                pub fn $ty(&self) -> TypeId {
                    *self.$ty.get()
                }
            )*

            /// The names of all slots, for checks that every slot is filled.
            pub const SLOT_NAMES: &'static [&'static str] =
                &[$(stringify!($el),)* $(stringify!($ty),)*];

            /// The names of the slots that are not filled yet.
            pub fn unset_slots(&self) -> Vec<&'static str> {
                let mut out = Vec::new();
                $(if !self.$el.is_set() { out.push(stringify!($el)); })*
                $(if !self.$ty.is_set() { out.push(stringify!($ty)); })*
                out
            }
        }
    };
}

type_provider! {
    elements {
        bool_element,
        deprecated_element,
        double_element,
        function_element,
        future_element,
        future_or_element,
        int_element,
        iterable_element,
        list_element,
        map_element,
        null_element,
        num_element,
        object_element,
        record_element,
        set_element,
        stack_trace_element,
        stream_element,
        string_element,
        symbol_element,
        type_element,
    }
    types {
        bool_type,
        deprecated_type,
        double_type,
        double_type_question,
        function_type,
        future_dynamic_type,
        future_null_type,
        future_or_null_type,
        int_type,
        int_type_question,
        iterable_dynamic_type,
        iterable_object_type,
        map_object_object_type,
        null_type,
        num_type,
        num_type_question,
        object_type,
        object_question_type,
        record_type,
        stack_trace_type,
        stream_dynamic_type,
        string_type,
        symbol_type,
        type_type,
    }
}

/// `_nonSubtypableClassMap`.
const NON_SUBTYPABLE: &[(&str, &[&str])] = &[
    ("dart:async", &["FutureOr"]),
    (
        "dart:core",
        &[
            "bool", "double", "Enum", "int", "Null", "num", "Record", "String",
        ],
    ),
    (
        "dart:typed_data",
        &[
            "ByteBuffer",
            "ByteData",
            "Endian",
            "Float32List",
            "Float32x4",
            "Float32x4List",
            "Float64List",
            "Float64x2",
            "Float64x2List",
            "Int16List",
            "Int32List",
            "Int32x4",
            "Int32x4List",
            "Int64List",
            "Int8List",
            "TypedData",
            "Uint16List",
            "Uint32List",
            "Uint64List",
            "Uint8ClampedList",
            "Uint8List",
            "UnmodifiableByteBufferView",
            "UnmodifiableByteDataView",
            "UnmodifiableFloat32ListView",
            "UnmodifiableFloat32x4ListView",
            "UnmodifiableFloat64ListView",
            "UnmodifiableFloat64x2ListView",
            "UnmodifiableInt16ListView",
            "UnmodifiableInt32ListView",
            "UnmodifiableInt32x4ListView",
            "UnmodifiableInt64ListView",
            "UnmodifiableInt8ListView",
            "UnmodifiableUint16ListView",
            "UnmodifiableUint32ListView",
            "UnmodifiableUint64ListView",
            "UnmodifiableUint8ClampedListView",
            "UnmodifiableUint8ListView",
        ],
    ),
];

impl TypeProvider {
    /// Dart `dynamicType`.
    pub fn dynamic_type(&self) -> TypeId {
        TypeId::DYNAMIC
    }

    /// Dart `neverType`.
    pub fn never_type(&self) -> TypeId {
        TypeId::NEVER
    }

    /// Dart `bottomType`.
    pub fn bottom_type(&self) -> TypeId {
        TypeId::NEVER
    }

    /// Dart `voidType`.
    pub fn void_type(&self) -> TypeId {
        TypeId::VOID
    }

    /// Dart `enumElement`.
    pub fn enum_element(&self) -> Option<EId<ClassElement>> {
        *self.enum_element.get()
    }

    /// Dart `enumType`.
    pub fn enum_type(&self) -> Option<TypeId> {
        *self.enum_type.get()
    }

    fn instantiate(ctx: &Ctx<'_>, element: EId<ClassElement>, args: &[TypeId]) -> TypeId {
        let args = ctx.intern_list(args);
        ctx.intern(TypeKind::Interface {
            element: element.upcast::<InterfaceElement>(),
            args,
            nullability: Nullability::None,
            alias: None,
        })
    }

    /// Dart `futureOrType(valueType)`.
    pub fn future_or_type(&self, ctx: &Ctx<'_>, value_type: TypeId) -> TypeId {
        Self::instantiate(ctx, self.future_or_element(), &[value_type])
    }

    /// Dart `futureType(valueType)`.
    pub fn future_type(&self, ctx: &Ctx<'_>, value_type: TypeId) -> TypeId {
        Self::instantiate(ctx, self.future_element(), &[value_type])
    }

    /// Dart `iterableType(elementType)`.
    pub fn iterable_type(&self, ctx: &Ctx<'_>, element_type: TypeId) -> TypeId {
        Self::instantiate(ctx, self.iterable_element(), &[element_type])
    }

    /// Dart `listType(elementType)`.
    pub fn list_type(&self, ctx: &Ctx<'_>, element_type: TypeId) -> TypeId {
        Self::instantiate(ctx, self.list_element(), &[element_type])
    }

    /// Dart `mapType(keyType, valueType)`.
    pub fn map_type(&self, ctx: &Ctx<'_>, key_type: TypeId, value_type: TypeId) -> TypeId {
        Self::instantiate(ctx, self.map_element(), &[key_type, value_type])
    }

    /// Dart `setType(elementType)`.
    pub fn set_type(&self, ctx: &Ctx<'_>, element_type: TypeId) -> TypeId {
        Self::instantiate(ctx, self.set_element(), &[element_type])
    }

    /// Dart `streamType(elementType)`.
    pub fn stream_type(&self, ctx: &Ctx<'_>, element_type: TypeId) -> TypeId {
        Self::instantiate(ctx, self.stream_element(), &[element_type])
    }

    /// Dart `isNonSubtypableClass(element)`: [name] is the class name,
    /// [library_uri] the URI of its library.
    pub fn is_non_subtypable_class(name: &str, library_uri: &str) -> bool {
        NON_SUBTYPABLE
            .iter()
            .any(|(uri, names)| *uri == library_uri && names.contains(&name))
    }

    /// Dart `isObjectGetter(id)`: `objectType.element.getGetter(id)` is
    /// an instance getter.
    pub fn is_object_getter(&self, ctx: &Ctx<'_>, id: &str) -> bool {
        let object = ctx.get(self.object_element());
        object.getters.iter().any(|&g| {
            let data = ctx.get(g);
            data.name.is_some_and(|n| ctx.name_str(n) == id)
                && !is_static(ctx, data.first_fragment())
        })
    }

    /// Dart `isObjectMember(id)`.
    pub fn is_object_member(&self, ctx: &Ctx<'_>, id: &str) -> bool {
        self.is_object_getter(ctx, id) || self.is_object_method(ctx, id)
    }

    /// Dart `isObjectMethod(id)`: `objectType.element.getMethod(id)` is
    /// an instance method. (`getMethod` compares `lookupName`; `Object` has
    /// no unary minus, so the name is the lookup name.)
    pub fn is_object_method(&self, ctx: &Ctx<'_>, id: &str) -> bool {
        let object = ctx.get(self.object_element());
        object.methods.iter().any(|&m| {
            let data = ctx.get(m);
            data.name.is_some_and(|n| ctx.name_str(n) == id)
                && !is_static(ctx, data.first_fragment())
        })
    }
}

/// `ExecutableElementImpl.isStatic` (`_firstFragment.isStatic`).
fn is_static<T: StoredFragment>(ctx: &Ctx<'_>, fragment: FId<T>) -> bool {
    ctx.fragment_data(fragment.raw())
        .is_some_and(|f| f.flags.has(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC))
}
