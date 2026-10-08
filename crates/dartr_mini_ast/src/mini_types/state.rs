// Dart source: pkg/_fe_analyzer_shared/test/mini_types.dart

//! The thread-local storage behind [`Type`], [`TypeParameter`],
//! [`InterfaceTypeName`] and [`TypeRegistry`](super::TypeRegistry) (no Dart
//! counterpart: Dart uses objects and a static field).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use indexmap::IndexMap;

use super::name::Name;
use super::registry::TypeNameInfo;
use super::types::{Type, TypeData};

/// The mutable data of a [`TypeParameter`](super::TypeParameter).
pub(crate) struct TypeParameterData {
    pub(crate) name: Name,
    pub(crate) explicit_bound: Option<Type>,
}

/// All the thread-local state of the mini types.
pub(crate) struct State {
    /// Interned types, indexed by `Type.0`.
    pub(crate) types: Vec<Rc<TypeData>>,
    /// The bucket hash of each interned type, indexed by `Type.0`.
    pub(crate) type_hashes: Vec<u64>,
    /// Interned type ids, by bucket hash.
    pub(crate) buckets: HashMap<u64, Vec<u32>>,
    /// Type parameters, indexed by `TypeParameter.0`.
    pub(crate) type_parameters: Vec<TypeParameterData>,
    /// Interface type names, indexed by `InterfaceTypeName.0`. The first
    /// entries are the static names of `TypeRegistry` (see
    /// [`STATIC_INTERFACE_TYPE_NAMES`]).
    pub(crate) interface_type_names: Vec<Name>,
    /// Dart `TypeRegistry._typeNameInfoMap`.
    pub(crate) type_name_info_map: Option<IndexMap<Name, TypeNameInfo>>,
}

/// The interface type names that are static fields of the Dart
/// `TypeRegistry` (`future`, `iterable`, `list`, `map`, `stream`), in the
/// order of their ids.
pub(crate) const STATIC_INTERFACE_TYPE_NAMES: [&str; 5] =
    ["Future", "Iterable", "List", "Map", "Stream"];

impl State {
    fn new() -> State {
        State {
            types: Vec::new(),
            type_hashes: Vec::new(),
            buckets: HashMap::new(),
            type_parameters: Vec::new(),
            interface_type_names: STATIC_INTERFACE_TYPE_NAMES
                .iter()
                .map(|n| Name::new(n))
                .collect(),
            type_name_info_map: None,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::new());
}

/// Runs [f] with the thread-local state. [f] must not call back into code
/// that accesses the state.
pub(crate) fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}
