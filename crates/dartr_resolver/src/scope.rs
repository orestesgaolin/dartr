// Dart source: pkg/analyzer/lib/src/dart/element/scope.dart
// (Scope, ScopeLookupResult, LibraryFragmentScope, PrefixScope,
// EnclosedScope, TypeParameterScope, InstanceScope, ExtensionScope,
// FormalParameterScope, LocalScope, ConstructorInitializerScope,
// PrimaryParameterScope, LabelScope, DocImportScope)

//! The scopes of body analysis.
//!
//! STUB (unit C1, scopes): the public API below is fixed; the bodies are
//! `todo!()` until the scope port lands.
//!
//! - [`LibraryScopes`]: the scopes of one library that do not change during
//!   analysis (the library declarations, the prefix scopes and the scope of
//!   each library fragment). Built once per library from the linked element
//!   model ([`Ctx`]) and shared (read-only, `Sync`) by the unit resolvers.
//! - [`NameScope`]: Dart `Scope.lookup` of the current lexical scope (the
//!   scope context of the resolution visitor implements it).
//! - [`ScopeLookupResult`]: Dart `ScopeLookupResult`.

use dartr_element::{Ctx, EId, ElementId, FId, LibraryElement, LibraryFragment, PrefixElement};

/// Dart `ScopeLookupResult`: the getter and the setter found for a name.
///
/// A Dart `MultiplyDefinedElementImpl` is an element with
/// `Tag::MultiplyDefined` (created in the unit's local arena).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScopeLookupResult {
    pub getter: Option<ElementId>,
    pub setter: Option<ElementId>,
}

/// Dart `Scope.lookup(id)` of a lexical scope.
pub trait NameScope {
    fn lookup(&self, id: &str) -> ScopeLookupResult;
}

/// The library-level scopes of one library (Dart `LibraryFragmentImpl.scope`
/// of each fragment, `PrefixElementImpl.scope` of each prefix).
#[derive(Debug, Default)]
pub struct LibraryScopes {
    _private: (),
}

impl LibraryScopes {
    /// Builds the scopes of [library] from its linked elements.
    pub fn build(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> LibraryScopes {
        let _ = (ctx, library);
        LibraryScopes::default()
    }

    /// Dart `LibraryFragmentImpl.scope.lookup(id)`.
    pub fn fragment_lookup(
        &self,
        ctx: &Ctx<'_>,
        fragment: FId<LibraryFragment>,
        id: &str,
    ) -> ScopeLookupResult {
        let _ = (ctx, fragment, id);
        todo!("LibraryFragmentScope.lookup (unit C1)")
    }

    /// Dart `PrefixElementImpl.scope.lookup(id)`.
    pub fn prefix_lookup(
        &self,
        ctx: &Ctx<'_>,
        prefix: EId<PrefixElement>,
        id: &str,
    ) -> ScopeLookupResult {
        let _ = (ctx, prefix, id);
        todo!("PrefixScope.lookup (unit C1)")
    }
}
