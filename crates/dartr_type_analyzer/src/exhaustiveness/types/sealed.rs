// Dart source: pkg/_fe_analyzer_shared/lib/src/exhaustiveness/types/sealed.dart

use std::cell::OnceCell;
use std::hash::Hash;
use std::rc::Rc;

use super::super::shared::{ExhaustivenessCache, TypeOperations};
use super::super::static_type::{StaticType, StaticTypeArena};
use super::{CacheTypeBased, EnumOperations, Restriction, TypeBasedKind, TypeBasedStaticType};

/// Interface implemented by analyzer/CFE to support [StaticType]s for sealed
/// classes.
pub trait SealedClassOperations {
    type Type;
    type Class: Clone + Eq + Hash;

    /// Returns the sealed class declaration for [type_] or `None` if [type_] is
    /// not a sealed class type.
    fn get_sealed_class(&self, type_: &Self::Type) -> Option<Self::Class>;

    /// Returns the direct subclasses of [sealed_class] that either extend,
    /// implement or mix it in.
    fn get_direct_subclasses(&self, sealed_class: &Self::Class) -> Vec<Self::Class>;

    /// Returns the instance of [sub_class] that implements
    /// [sealed_class_type].
    ///
    /// `None` might be returned if [sub_class] cannot implement
    /// [sealed_class_type]. For instance
    ///
    /// ```text
    ///     sealed class A<T> {}
    ///     class B<T> extends A<T> {}
    ///     class C extends A<int> {}
    /// ```
    ///
    /// here `C` has no implementation of `A<String>`.
    ///
    /// It is assumed that `TypeOperations.isSealedClass` is `true` for
    /// [sealed_class_type] and that [sub_class] is in `getDirectSubclasses` for
    /// `getSealedClass` of [sealed_class_type].
    fn get_subclass_as_instance_of(
        &self,
        sub_class: &Self::Class,
        sealed_class_type: &Self::Type,
    ) -> Option<Self::Type>;
}

/// [SealedClassInfo] stores information to compute the static type for a
/// sealed class.
pub struct SealedClassInfo<SO: SealedClassOperations> {
    sealed_class: SO::Class,
    sub_classes: OnceCell<Vec<SO::Class>>,
}

impl<SO: SealedClassOperations> SealedClassInfo<SO> {
    pub fn new(sealed_class: SO::Class) -> SealedClassInfo<SO> {
        SealedClassInfo {
            sealed_class,
            sub_classes: OnceCell::new(),
        }
    }

    /// Returns the classes that directly extends, implements or mix in
    /// the sealed class.
    pub fn sub_classes(&self, sealed_class_operations: &SO) -> &[SO::Class] {
        self.sub_classes
            .get_or_init(|| sealed_class_operations.get_direct_subclasses(&self.sealed_class))
    }
}

/// [StaticType] for a sealed class type.
///
/// The fields of the Dart class `SealedClassStaticType`; see
/// [TypeBasedKind::SealedClass]. The Dart fields `_cache` and
/// `_sealedClassOperations` are the cache that owns the type.
pub struct SealedClassStaticType<S> {
    sealed_info: Rc<S>,
    subtypes: OnceCell<Vec<StaticType>>,
}

impl<TO, EO, SO> ExhaustivenessCache<TO, EO, SO>
where
    TO: TypeOperations,
    EO: EnumOperations<Type = TO::Type>,
    SO: SealedClassOperations<Type = TO::Type>,
{
    /// Dart `new SealedClassStaticType(typeOperations, fieldLookup, type,
    /// cache, sealedClassOperations, sealedInfo)`.
    pub(crate) fn new_sealed_class_static_type(
        &self,
        type_: TO::Type,
        sealed_info: Rc<SealedClassInfo<SO>>,
    ) -> StaticType {
        self.new_type_based_static_type(TypeBasedStaticType::new(
            type_,
            false,
            Restriction::Unrestricted,
            None,
            TypeBasedKind::SealedClass(SealedClassStaticType {
                sealed_info,
                subtypes: OnceCell::new(),
            }),
        ))
    }

    /// `SealedClassStaticType.getSubtypes`.
    pub(crate) fn sealed_class_static_type_get_subtypes(
        &self,
        this_type: StaticType,
        this: &CacheTypeBased<TO, EO, SO>,
        sealed: &SealedClassStaticType<SealedClassInfo<SO>>,
    ) -> Vec<StaticType> {
        if let Some(subtypes) = sealed.subtypes.get() {
            return subtypes.clone();
        }
        let subtypes = self.sealed_class_static_type_create_subtypes(this_type, this, sealed);
        sealed.subtypes.get_or_init(|| subtypes).clone()
    }

    /// `SealedClassStaticType._createSubtypes`.
    fn sealed_class_static_type_create_subtypes(
        &self,
        this_type: StaticType,
        this: &CacheTypeBased<TO, EO, SO>,
        sealed: &SealedClassStaticType<SealedClassInfo<SO>>,
    ) -> Vec<StaticType> {
        let mut subtypes: Vec<StaticType> = vec![];
        let sub_classes = sealed
            .sealed_info
            .sub_classes(&self.sealed_class_operations)
            .to_vec();
        for sub_class in &sub_classes {
            let subtype = self
                .sealed_class_operations
                .get_subclass_as_instance_of(sub_class, &this.type_);
            if let Some(subtype) = subtype {
                if !self.type_operations.is_generic(&subtype) {
                    // If the subtype is not generic, we can test whether it can
                    // be an actual value of [_type] by testing whether it is a
                    // subtype of the overapproximation of [_type].
                    //
                    // For instance
                    //
                    //     sealed class A<T> {}
                    //     class B extends A<num> {}
                    //     class C<T extends num> A<T> {}
                    //
                    //     method<T extends String>(A<T> a) {
                    //       switch (a) {
                    //         case B: // Not needed, B cannot inhabit A<T>.
                    //         case C: // Needed, C<Never> inhabits A<T>.
                    //       }
                    //     }
                    if !self
                        .type_operations
                        .is_subtype_of(&subtype, &self.type_operations.overapproximate(&this.type_))
                    {
                        continue;
                    }
                }
                let static_type = self.get_static_type(&subtype);
                // Since the type of the [subtype] might not itself be a subtype
                // of [_type], for instance in the example above the type of
                // `case C:`, `C<num>`, is not a subtype of `A<T>`, we wrap the
                // static type to establish the subtype relation between the
                // [StaticType] for the enum element and this [StaticType].
                subtypes.push(self.new_wrapped_static_type(static_type, this_type));
            }
        }
        subtypes
    }
}
