//! dartr_typesystem: the type algorithms of the analyzer
//! (`pkg/analyzer/lib/src/dart/element/*`): substitution, visitors,
//! subtyping, the `TypeSystemImpl` predicates, normalization, top merge,
//! closures, least upper / greatest lower bounds.
//!
//! The type model and the interner are in `dartr_element`; this crate adds
//! the Dart `TypeImpl` methods as the extension trait [`TypeExt`] on
//! [`dartr_element::Ctx`], and [`TypeSystem`] (`TypeSystemImpl`).
//!
//! Display strings (`getDisplayString`, unit A1) are in
//! `dartr_element::display_string`, because diagnostics in `dartr_element`
//! need them and they need no type algorithm.
//!
//! Type inference (unit A6): [`type_constraint_gatherer`] (the analyzer's
//! `TypeConstraintGatherer` over the shared constraint generation of
//! `dartr_type_analyzer`), [`generic_inferrer`] (`GenericInferrer`) and
//! [`type_system_operations`] (`TypeSystemOperations`, the analyzer's
//! implementation of the shared operations traits of `dartr_flow`).
//!
//! Inheritance (unit A7): [`inheritance_manager3`] (`InheritanceManager3`,
//! interfaces cached on the elements), [`member`] (substituted members,
//! `ElemRef::Member`), [`lookup`] (the `lookUp*` methods of elements and
//! interface types) and [`interface_dump`] (the `interface` dump of the
//! oracle). This is a module of `dartr_typesystem`, not its own crate,
//! because `TypeSystem.isAssignableTo` needs `getCallMethodType`, which is
//! `InterfaceTypeImpl.lookUpMethod` and so the inheritance manager.
//!
//! [`test_support`] is a port of the analyzer test helpers
//! (`test_library_builder.dart`, `mock_sdk_elements.dart`,
//! `type_system_base.dart`): it builds the mock SDK and test libraries from
//! the same spec strings and parses types like `List<int>?`.

pub mod class_hierarchy;
pub mod element_type;
pub mod equality;
pub mod generic_inferrer;
pub mod greatest_lower_bound;
pub mod inheritance_manager3;
pub mod interface_dump;
pub mod least_greatest_closure;
pub mod least_upper_bound;
pub mod lookup;
pub mod member;
pub mod normalize;
pub mod recursion_guard;
pub mod replace_top_bottom_visitor;
pub mod replacement_visitor;
pub mod runtime_type_equality;
pub mod subtype;
pub mod test_support;
pub mod top_merge;
pub mod type_algebra;
pub mod type_constraint_gatherer;
pub mod type_demotion;
pub mod type_ext;
pub mod type_schema;
pub mod type_schema_elimination;
pub mod type_system;
pub mod type_system_operations;
pub mod type_visitor;

pub use type_algebra::{FreshTypeParameters, MapSubstitution, Substitution};
pub use type_ext::TypeExt;
pub use type_system::TypeSystem;
