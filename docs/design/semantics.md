# dartr design: semantics (phases 5–7) and the driver

Status: proposal. Dart sources: `third_party/dart-sdk` (tag 3.13.3).
Paths below are relative to `pkg/analyzer/lib/src` unless they start with `_fe_analyzer_shared/`.

## 0. What we port (sizes at 3.13.3)

| Dart area | lines | dartr crate |
|---|---|---|
| `dart/element/*` (element.dart 12.4k, type.dart, type_system.dart, subtype, LUB/GLB, normalize, type_algebra, inheritance_manager3, generic_inferrer, constraint gatherer, display_string_builder, scope.dart) | 27.8k | `dartr_element`, `dartr_typesystem` |
| `summary2/*` (link.dart, library_builder, element_builder, reference_resolver, types_builder, top_level_inference, type_alias, default_types_builder, …) | 20.7k | `dartr_link` |
| `dart/resolver/*`, `generated/resolver.dart` (5.4k) | 25.2k | `dartr_resolver` |
| `_fe_analyzer_shared/lib/src/flow_analysis/*`, `type_inference/*`, `types/shared_type.dart` | 18.4k | `dartr_flow` (generic) |
| `dart/constant/*` (evaluation 3.8k, value 3.3k, constant_verifier 1.7k) | ~10k | `dartr_constant` |
| `error/*`, `generated/error_verifier.dart` (8.5k) | ~25k | `dartr_verify` |
| `dart/analysis/{driver,file_state,library_graph,library_context,library_analyzer,unlinked_api_signature}.dart`, `fine/*` (11k) | ~20k | `dartr_driver` |

PLAN.md lists one `dartr_semantics` crate. We split it into the crates above, for compile times and so that
agents can work in parallel. `dartr_semantics` can stay as a re-export facade.

Dependency order: `dartr_ast` → `dartr_element` → `dartr_typesystem` → (`dartr_flow` is independent) →
`dartr_resolver` → `dartr_link` → `dartr_constant` → `dartr_verify` → `dartr_driver`.
`dartr_link` depends on the resolver because linking resolves expressions: top-level inference,
const initializers, default values and metadata (`summary2/ast_resolver.dart`).

## 1. Element and type representation

### 1.1 Model in 3.13.3

3.13.3 uses the Element/Fragment model. For each declaration kind there is an `XElementImpl` (the semantic
entity) and an `XFragmentImpl` (one syntactic piece of it; there are several only with augmentations,
an experiment, or with parts). See `element.dart`:
`ClassElementImpl`:137 / `ClassFragmentImpl`:628, `ElementImpl`:2060, `FragmentImpl`:4388,
`LibraryElementImpl`:6688 / `LibraryFragmentImpl`:7652 (one per unit; parts can nest),
`TypeParameterElementImpl`:11382.
Substituted members are `Substituted*ElementImpl` in `member.dart` (the old `Member`).
Mixins `Internal*Element` (element.dart:6230–6453) are interfaces that both base elements and substituted members implement.

### 1.2 Stores and ids

```rust
/// Packed: store (24 bits) | kind (8 bits) | index (32 bits). Copy, Hash, Ord.
#[derive(Copy, Clone, PartialEq, Eq, Hash)] pub struct ElementId(u64);
#[derive(Copy, Clone, PartialEq, Eq, Hash)] pub struct FragmentId(u64);
/// Typed view, like dartr_ast::Id<T>: EId<ClassElement>, EId<InterfaceElement> (category).
pub struct EId<T: ?Sized>(ElementId, PhantomData<fn() -> T>);

pub struct StoreId(u32);          // unique within one Generation, never reused
pub enum StoreKind { Cycle, Synthetic /* global, append-only */, Local /* one analysis task */ }

/// Elements of one library cycle. Built by dartr_link, then frozen behind Arc.
pub struct ElementStore {
    pub id: StoreId,
    pub classes: Vec<ClassElement>, pub enums: Vec<EnumElement>, pub mixins: Vec<MixinElement>,
    pub extensions: Vec<ExtensionElement>, pub extension_types: Vec<ExtensionTypeElement>,
    pub fields: Vec<FieldElement>, pub getters: Vec<GetterElement>, pub setters: Vec<SetterElement>,
    pub methods: Vec<MethodElement>, pub constructors: Vec<ConstructorElement>,
    pub functions: Vec<TopLevelFunctionElement>, pub variables: Vec<TopLevelVariableElement>,
    pub type_aliases: Vec<TypeAliasElement>, pub type_params: Vec<TypeParameterElement>,
    pub params: Vec<FormalParameterElement>, pub prefixes: Vec<PrefixElement>,
    pub libraries: Vec<LibraryElement>, pub fragments: FragmentStore, // same layout, per kind
    pub generic_function_types: Vec<GenericFunctionTypeElement>,
    // local kinds are used only in Local stores:
    pub locals: Vec<LocalVariableElement>, pub local_functions: Vec<LocalFunctionElement>,
    pub labels: Vec<LabelElement>,
}
```

Element structs hold the fields of the Dart `*ElementImpl`, with ids in place of object references:

```rust
pub struct ClassElement {                      // element.dart:137 + InterfaceElementImpl:5688
    pub name: Option<Name>,                    // interned string (Name = u32 symbol)
    pub library: EId<LibraryElement>,
    pub fragments: SmallVec<[FragmentId; 1]>,
    pub flags: ElementFlags,                   // bitflags: abstract, base, final, sealed, mixin_class, simply_bounded, has_non_final_field, …
    pub type_params: Box<[EId<TypeParameterElement>]>,
    pub supertype: OnceLock<Option<TypeId>>,   // set in link phase "resolveTypes"
    pub mixins: OnceLock<Box<[TypeId]>>, pub interfaces: OnceLock<Box<[TypeId]>>,
    pub fields: Vec<EId<FieldElement>>, pub getters: Vec<EId<GetterElement>>, /* setters, methods, constructors */
    pub this_type: OnceLock<TypeId>,
}
pub struct TypeParameterElement {               // element.dart:11382
    pub name: Option<Name>, pub fragment: FragmentId, pub enclosing: Option<ElementId>,
    pub variance: Option<Variance>,             // None = legacy covariant
    pub bound: OnceLock<Option<TypeId>>, pub default_type: OnceLock<Option<TypeId>>,
}
pub struct FormalParameterElement {             // element.dart:3892
    pub name: Option<Name>, pub kind: ParameterKind, pub flags: ElementFlags /* covariant, initializing_formal, super_formal, has_default */,
    pub ty: OnceLock<TypeId>, pub type_params: Box<[EId<TypeParameterElement>]>,
    pub default_value: Option<ConstExprId>,     // resolved expression in the cycle's ConstExprs arena
    pub field: Option<EId<FieldElement>>,       // FieldFormalParameterElementImpl
}
```

Write rules:
- *Structural* data (children lists, names, flags known from syntax) is written through `&mut ElementStore` while a
  builder phase owns the store (element_builder, synthetic constructors, enum children, `_replaceConstFieldsIfNoConstConstructor`).
- Data computed later in linking, some of it by recursive lazy computation (types, supertypes, inferred types, bounds,
  defaults), is in `OnceLock` slots. Linking reads the cycle through `&ElementStore`, so recursive
  inference (top_level_inference.dart:185 `_PropertyInducingElementTypeInference` with its status/cycle
  detection) works with shared borrows. `OnceLock` keeps the frozen store `Sync`.
  Helper: `slot.set_once(v)` panics on a second set (catches porting bugs).
- `late` fields in Dart become `OnceLock` (lazy) or `Option` + `expect` (must be set by a phase).

Lookup context, passed everywhere (the replacement for "an object knows its data"):

```rust
pub struct Ctx<'a> {
    pub world: &'a WorldSnapshot,             // frozen stores, global interner, synthetic store
    pub current: Option<&'a ElementStore>,    // the cycle being linked (shared borrow)
    pub local: Option<&'a LocalArena>,        // body analysis task: local elements + type overlay
    pub tp: &'a TypeProvider, pub features: &'a FeatureSet,
    pub req: &'a dyn RequirementSink,         // fine-grained deps; NoopSink in v1 (see §4)
}
impl Ctx<'_> {
    pub fn class(&self, id: EId<ClassElement>) -> &ClassElement;  // dispatches on id.store()
    pub fn global(&self) -> Ctx<'_>;          // same but local = None (used inside shared caches)
}
```

`LocalArena` uses interior mutability (a `RefCell<ElementStore>` plus an append-only typed arena), so the
resolver can add local elements while holding `&Ctx`. It is `!Sync`, which is fine because it belongs to one thread.

### 1.3 References and members

Analyzer: `InternalExecutableElement` is either a base element or a `SubstitutedExecutableElementImpl`
(member.dart:246), created on the fly by the inheritance manager and by resolution of `a.foo` on `List<int>`.
Rust: a small interned value.

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum ElemRef { Base(ElementId), Member(MemberId) }   // MemberId -> (base: ElementId, subst: SubstId)
```

`SubstId` interns a `MapSubstitution` (`type_algebra.dart:130`): sorted `(EId<TypeParameterElement>, TypeId)` pairs.
Getters such as `member.type` and `member.returnType` are methods on `Ctx` that substitute lazily, the same as member.dart.

### 1.4 Types

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash)] pub struct TypeId(NonZeroU32); // bit 31 = local overlay
pub enum Nullability { None, Question, Star }       // NullabilitySuffix (shared)
pub enum TypeKind {                                  // type.dart
    Dynamic, Void, Invalid, Unknown,                 // UnknownInferredType = type schema `_` (type_schema.dart:21)
    Never(Nullability),
    Interface { element: EId<InterfaceElement>, args: TypeList, nullability: Nullability, alias: Option<AliasRef> },
    Function(FunctionTypeData),
    Record { positional: TypeList, named: NamedFields /* sorted by name */, nullability: Nullability, alias: Option<AliasRef> },
    TypeParameter { param: EId<TypeParameterElement>, nullability: Nullability, promoted_bound: Option<TypeId>, alias: Option<AliasRef> },
}
pub struct FunctionTypeData {                        // FunctionTypeImpl (type.dart:100)
    pub type_params: Box<[EId<TypeParameterElement>]>,
    pub params: Box<[FnParam]>,                      // named params sorted, as in the factory (type.dart:141)
    pub required_positional: u16, pub ret: TypeId, pub nullability: Nullability, pub alias: Option<AliasRef>,
}
pub struct FnParam { pub name: Option<Name>, pub kind: ParameterKind, pub ty: TypeId, pub covariant: bool,
                     pub element: Option<ElemRef> }  // declaring parameter, for NamedExpression.element, @Deprecated, required
pub struct AliasRef { pub element: EId<TypeAliasElement>, pub args: TypeList } // InstantiatedTypeAliasElementImpl
```

Decisions:
- **Interning.** `TypeId` equality is *exact structural identity*, including alias, parameter names and parameter element refs.
  It is not Dart `==`. Dart `==` is ported as `TypeSystem::dart_eq(a, b)`: it ignores `alias` and, for generic
  function types, compares up to renaming of type formals (`relateTypeFormals` + `instantiate`, type.dart:255;
  `_equalParameters` type.dart:488). Where Dart uses identity (`identical`), use `TypeId ==`. The difference
  matters for display strings: two types that are Dart-equal but have different aliases print differently.
  If the interner merged them, the printed name would depend on thread timing.
- **Substitution, not de Bruijn.** Function types bind named `TypeParameterElement`s, as in Dart.
  `FreshTypeParameters` (type_algebra.dart:108) allocates fresh elements: in the `LocalArena` during body
  analysis, in the cycle store during linking, and in the global `Synthetic` store inside shared caches
  (inheritance, member types). Why not de Bruijn: `getDisplayString`, inference error messages
  ("couldn't infer type parameter 'T'") and `GenericInferrer` (keyed by `TypeParameterElement`) all use
  element identity and names. Porting with substitution keeps `type_algebra.dart`, `subtype.dart` (generic
  function case) and `least_upper_bound.dart` line by line.
- **Cycles.** `T extends Comparable<T>` is an id cycle (bound slot → TypeId → param id), not an ownership cycle.
- **Interner layout.** Global: N=64 shards, each `parking_lot::Mutex<hashbrown::HashTable<u32>>` plus an append-only
  `boxcar::Vec<TypeKind>` (lock-free reads). Local overlay: a `LocalArena` interner. A type goes there only if
  it is not already global and it mentions a local element or is created during body analysis.
  `debug_assert!` that a global type never contains a local id. Lazy shared caches always run under
  `ctx.global()`.
- `TypeList`/`NamedFields` are interned slices (`ListId`), so `Interface` stays small (16 bytes).
- Type visitors (`type_visitor.dart`, `replacement_visitor.dart`) become `match` on `TypeKind` with a
  `ReplacementVisitor` trait whose default methods are the Dart methods; they rebuild through the interner.

### 1.5 TypeProvider, TypeSystem

`TypeProvider` (`type_provider.dart`) holds fixed `TypeId`s (`int`, `Object?`, `Future<dynamic>`, …).
It is built after the `dart:core`/`dart:async` cycle is frozen and stored in the `WorldSnapshot`.
Linking `dart:core` itself uses a provider whose slots are filled lazily (`_createTypeSystemIfNotLinkingDartCore`, link.dart).
`TypeSystemImpl` → `struct TypeSystem<'a> { ctx: Ctx<'a>, strict_casts, strict_inference, ... }`, built per library.

## 2. Library dependencies, cycles, linking, the SDK

### 2.1 Cycles

Port `library_graph.dart`: a Tarjan SCC walk over import and export edges (`_LibraryNode.computeDependencies`). Each `LibraryCycle` has
`api_signature` (transitive), `non_transitive_api_signature` and `direct_dependencies`.
Unlinked API signatures come from `unlinked_api_signature.dart` (tokens of declarations without function bodies).

### 2.2 Linking one cycle (port of `summary2/link.dart` `Linker._buildOutlines`)

Phases, all run over every library of the cycle, in this order (keep it exactly):

1. `LibraryBuilder.build` → `ElementBuilder` per unit (fragments and elements, `Reference`s) — parallel per library (rayon).
2. `buildElements` + `_buildExportScopes` (export fixpoint) → `LibraryFragmentScope`/`PrefixScope` (dart/element/scope.dart).
3. Create the type system; `_resolveTypes` (reference_resolver → NamedTypeBuilder/TypesBuilder, type_alias, simply_bounded, default_types_builder, variance_builder, interface_cycles).
4. `_computeHasNonFinalField`, `_setDefaultSupertypes`, synthetic class/enum constructors, const fields, field formals, enum children, field promotability.
5. `SuperConstructorResolver`, `_performTopLevelInference` (top_level_inference.dart, instance_member_inferrer.dart).
6. extension types, `_resolveConstructors` (initializers of const constructors), `_resolveConstantInitializers`, default values, metadata.
7. mixin super-invoked names, `EnclosingTypeParameterReferenceFlag`, `ElementNameUnion`, `_detachNodes`.

Phases 3–7 are sequential inside a cycle (v1). Expressions that linking resolves (const initializers, default
values, metadata, constructor initializers, and the initializers of variables whose types are inferred) are
**copied** into a cycle-owned `ConstExprs` arena: a `dartr_ast::Ast` holding detached subtrees, as in `detach_nodes.dart`.
They are resolved there; resolution may rewrite nodes with `replace_with`. The file ASTs stay read-only and
shareable. Resolution side tables for `ConstExprs` belong to the frozen cycle; constant evaluation needs them.

Freezing produces:
```rust
pub struct LinkedCycle {
    pub store: ElementStore, pub const_exprs: ConstExprs,
    pub libraries: Vec<EId<LibraryElement>>, pub export_scopes: Vec<ExportScope>,
    pub caches: CycleCaches,                 // lazily filled, Sync
    pub api_signature: Hash128, pub manifest: Option<CycleManifest>,   // §4 v2
}
pub struct CycleCaches {
    pub interfaces: Box<[OnceLock<Arc<Interface>>]>,      // InheritanceManager3._interfaces, per interface element
    pub consts: OnceLock<ConstTable>,                     // values of all const vars/ctors/annotations of the cycle
    pub metadata_flags: Box<[OnceLock<MetaFlags>]>,       // deprecated, required, … from annotations
}
```

### 2.3 Lazy shared state after freezing

`InheritanceManager3` (inheritance_manager3.dart:86) keeps identity maps keyed by element. In Rust it is a
stateless API over `CycleCaches.interfaces` with `get_or_init`. Values are deterministic, so if two threads
race they compute the same value and one result is dropped. Lower dependency cycles may be computed while
computing a higher one; there are no lock cycles because a `OnceLock` init only reaches *lower or same* cycles.
Within the same cycle, recursion only goes to superclasses, which interface_cycles already made acyclic.

Constants: `computeConstants` (dart/constant/compute.dart) walks a dependency graph across libraries.
dartr: `CycleCaches.consts.get_or_init` evaluates *all* constants of the cycle at once, using
`ConstExprs`, in the SCC order of `_ConstantWalker`. Dependencies are in the same cycle or in lower frozen cycles.
Diagnostics from constant evaluation of the library under analysis are produced by that library's
analysis (ConstantVerifier and `_computeConstantErrors`, library_analyzer.dart:301), not by the cache.
Checkpoint C7 checks that both give equal values.

### 2.4 SDK

`dart analyze` links the SDK from source. The SDK core libraries form one big cycle (core, async,
collection, _internal, …), linked eagerly; it is cached in the byte store under `cycle.linkedKey` (library_context.dart:246).
Libraries such as `dart:html` and `dart:io` form other cycles that are linked only when imported.
`LinkedElementFactory` then reads members lazily (`DeferredMembersReadingMixin`).

dartr:
- v1: link the SDK cycles from source, on demand (the dart:core cycle is always needed). Measure first; a
  parallel Rust parse plus link of the core cycle should take ~100–300 ms.
- v2: `LinkedCycle` serialization (`dartr_link::summary`) into a cache dir:
  `$XDG_CACHE_HOME/dartr/<dartr-build-hash>/linked/<api_signature>.bin`. The SDK cycle comes first because it is
  on the critical path (one big sequential cycle). The format is our own (bincode-like and versioned, or
  `rkyv` for mmap). Do not use the analyzer's format.
  Cross-cycle references are serialized symbolically (port of `summary2/reference.dart`, e.g. `dart:core::@class::List::@method::map`)
  and resolved to `ElementId`s on load.
  The serialized form is designed together with `ElementStore` from day 1 (no `ElementId` from a
  foreign store inside the bytes; everything goes through a `ReferenceTable`).
- Lazy loading per member is not needed: the cost to fix is linking, not reading.

### 2.5 Parallelism (rayon)

1. Read, scan and parse every file in parallel; compute unlinked data and API signatures in parallel.
2. Library graph and SCCs: sequential (cheap).
3. Link: a DAG scheduler. A cycle is linked when all its direct dependencies are frozen. Each frozen cycle is
   published into the `WorldSnapshot` (§4.1) and wakes its dependents.
4. Analyze bodies: library L can start as soon as L's cycle is frozen (a pipeline, no barrier). In L:
   bind elements to declarations (element_binding_visitor.dart), then resolve each unit in parallel
   (ResolutionVisitor + ResolverVisitor, each with a unit-local `LocalArena`), then sequentially the
   library-wide steps of `LibraryAnalyzer` (library_analyzer.dart:682 `_parseAndResolve`,
   `_computeConstants`, `_computeDiagnostics`: imports verifier, unused elements and duplicate definitions span all units).

Why this is sound: body resolution reads frozen elements, `ConstExprs` and the deterministic `OnceLock` caches.
It writes only to its own `LocalArena`, to the side tables of its units (`ResolutionTables`), and to its
own diagnostics. It never changes a linked element. In the analyzer, top-level inferred types are fixed by
linking, and the resolver re-resolves initializers only for diagnostics.
**Determinism rule:** output may never depend on id values or on hash iteration order.
Every Dart `Map`/`Set` that is iterated is ported as `IndexMap`/`IndexSet` (Dart's defaults keep insertion order).
`FxHashMap` is allowed only for pure lookups. A clippy `disallowed_methods` lint bans `HashMap::iter` in semantic crates.

## 3. Mutation during resolution

```rust
/// Per unit, indexed by NodeId (dartr_ast::NodeMap). Mirrors fields the Dart resolver sets on AST nodes.
pub struct ResolutionTables {
    pub static_type: NodeMap<TypeId>,             // Expression.staticType
    pub element: NodeMap<ElemRef>,                // SimpleIdentifier/NamedType/… .element
    pub declared_fragment: NodeMap<FragmentId>,   // Declaration.declaredFragment
    pub invoke_type: NodeMap<TypeId>,             // InvocationExpression.staticInvokeType
    pub type_arg_types: NodeMap<ListId>,          // inferred type arguments
    pub param_element: NodeMap<ElemRef>,          // argument → correspondingParameter
    pub pattern_info: NodeMap<PatternInfo>,       // matched value types, required types
    pub flags: NodeMap<NodeFlags>,                // e.g. isNullAware short-circuit, implicit tear-off
}
```

- The resolver gets `&mut Ast` (a clone of the parsed `Arc<Ast>`; cloning the arena Vecs is a memcpy) because
  `ast_rewrite.dart` and `resolver.dart` rewrite nodes (MethodInvocation → InstanceCreationExpression, …).
- `ResolverVisitor` (resolver.dart:119) mixes in `TypeAnalyzer` (`_fe_analyzer_shared/.../type_analyzer.dart:273`)
  and `ErrorDetectionHelpers`. A Dart mixin becomes a Rust trait whose provided methods are the mixin body and
  whose required methods are the abstract members; `ResolverVisitor` implements it. Helper resolvers
  (`MethodInvocationResolver`, …) become structs with a `&mut ResolverVisitor`, or free functions taking
  `&mut ResolverVisitor`, to avoid aliasing borrows.
- Element-level state computed lazily at analysis time goes in the `CycleCaches` `OnceLock`s (§2.3). There are
  no locks around elements.
- Linking: single-threaded per cycle, `OnceLock` slots plus explicit status vectors for recursion
  (top-level inference status, type alias cycle status, `simply_bounded`), as the Dart does.

## 4. Incremental model (LSP)

### 4.1 Snapshots

```rust
pub struct WorldSnapshot {                  // immutable; Arc-shared with running tasks
    pub generation: Arc<Generation>,        // interner + synthetic store; reset rarely
    pub cycles: im::HashMap<CycleKey, Arc<LinkedCycle>>,
    pub library_to_cycle: im::HashMap<FileId, CycleKey>,
    pub type_provider: Arc<TypeProvider>,
}
```

An edit creates a new snapshot and cancels tasks running on older ones (an atomic cancel flag checked between
units and in long loops, as in rust-analyzer). Results are tagged with their snapshot.

### 4.2 v1 (same as the analyzer without fine dependencies)

1. A file changes: re-read it, rescan and reparse it, recompute its unlinked API signature (file_state.dart).
2. If the API signature is unchanged (only bodies changed), keep every cycle and reanalyze only the library that
   contains the file. Its analysis cache key is the library signature: the cycle's transitive api_signature
   plus the content hashes of its units, as in driver.dart.
3. If the API changed or directives changed, rebuild the library graph for the affected files
   (`LibraryCycle.dispose` propagates to `directUsers`). Relink the changed cycle and every transitive user
   (new `CycleKey`s); unaffected cycles are reused (`Arc` clones). Then reanalyze the libraries of the
   relinked cycles: priority files (open, visible) first, the rest in the background.
4. Garbage: old cycles and their types stay in the generation's interner and synthetic store.
   When retired data is more than 50% of live data, start a new `Generation`: re-link everything from the v2
   disk/memory summaries (or from source in v1), then swap the snapshot.

### 4.3 Path to fine-grained (`fine/`)

The analyzer's fine mode: a `LibraryManifest` (fine/library_manifest.dart) gives each top-level item and member a
`ManifestItemId` and a hash of its API. Linking and analysis record `RequirementsManifest` (fine/requirements.dart:864):
for each dependency library, the names looked up, the export entries, and the interface members used. A
cycle is relinked, or a library reanalyzed, only if a recorded requirement fails.

dartr plan:
- Day 1: `Ctx.req: &dyn RequirementSink` with the calls the analyzer makes (`globalResultRequirements?.record…`) at
  the same places: scope lookups (dart/element/scope.dart), export scope lookups, `InheritanceManager3` interface/member
  queries, and element getters marked `@trackedDirectly…` (element.dart). v1 uses a no-op sink, so only the
  hook exists.
- v2a: manifests. After freezing, hash each item: its `elements` dump entry (§5) plus flags. This is cheap
  and reuses the oracle dump code.
- v2b: requirements per library analysis result (name → item hash). On an API change, reanalyze the user
  libraries whose requirements fail, instead of all transitive users.
- v2c: requirements per linked cycle (relink only when needed). This is the hard part: manifest ids must
  survive relinking (library_context.dart:434).

## 5. Porting order, checkpoints, oracle dumps

New oracle modes (in `tools/oracle/bin/oracle.dart`, using `getResolvedLibrary` and the public element API) with
matching `dartr dump <mode>`. Each line is JSON per *library* (keyed by the path of its defining unit). Lists keep
declaration order. Element references are written as `R(e)` = `"<libraryUri>::<path>"`, where path is the names of
enclosing elements joined by `.`, with `new` for unnamed constructors and `=` appended for setters
(e.g. `"package:a/a.dart::A.foo"`, `"dart:core::List.new"`). Types are
`type.getDisplayString()` (default arguments).

### 5.1 `elements` (C5a, C5d)

```json
{"path":"/p/lib/a.dart","uri":"package:p/a.dart","lang":"3.13",
 "units":[{"path":"/p/lib/a.dart"},{"path":"/p/lib/src/b.dart","part":true}],
 "imports":[{"uri":"dart:core","prefix":null,"show":[],"hide":[],"deferred":false,"synthetic":true}],
 "exports":["package:p/c.dart"],
 "exportNamespace":{"A":"package:p/a.dart::A","x=":"package:p/c.dart::x="},
 "elements":[
  {"k":"class","n":"A","u":0,"o":6,"f":["abstract","base","simplyBounded"],
   "tp":[{"n":"T","bound":"num","default":"num","variance":null}],
   "super":"Object","mixins":[],"interfaces":["Comparable<A<T>>"],
   "members":[
     {"k":"field","n":"x","type":"int","f":["final","promotable"],"inf":false,"o":30},
     {"k":"getter","n":"x","type":"int Function()","f":["synthetic"],"var":"package:p/a.dart::A.x"},
     {"k":"ctor","n":"new","type":"A<T> Function(int)","f":["const"],
      "params":[{"n":"x","kind":"requiredPositional","type":"int","f":["fieldFormal"],"default":null}],
      "redirected":null,"superCtor":"dart:core::Object.new"},
     {"k":"method","n":"m","type":"S Function<S extends T>(S, {int y})","f":["abstract"],"inf":true,
      "params":[...]}]},
  {"k":"topVar","n":"v","type":"List<int>","f":["const"],"inf":true,"typeInferenceError":null,
   "const":"List<int> ([int (1)])"},
  {"k":"function","n":"f","type":"void Function()"},
  {"k":"typeAlias","n":"F","aliased":"int Function(String)","tp":[]},
  {"k":"extension","n":"E","on":"String"}, {"k":"extensionType","n":"X","rep":"int","primaryCtor":"..."},
  {"k":"enum","n":"Color","members":[...]}, {"k":"mixin","n":"M","on":["Object"],"superInvoked":["foo"]}]}
```

- `"inf": true` marks types that were inferred (`hasImplicitType`/`hasImplicitReturnType`). For C5a, difftest gets
  `--mask-inferred` to compare everything except inferred types, before top-level inference exists.
- `"const"` (a `DartObject.toString()`-style value) is written only with `--with-const` (`difftest elements
  --with-const` passes it to both tools), for const top-level variables and fields (enum constants and the
  enum `values` field included): `computeConstantValue()?.toString()`, `null` for an invalid constant. dartr
  writes it from `crates/dartr/src/elements_const.rs` (D1). Note: `toString()` expands shared values, so a
  constant DAG (tests/language/const/constant_dag_test.dart) gives an exponential string and the oracle does
  not finish on it.
- `difftest --codes a,b` / `--codes-file tools/difftest/constant_codes.txt` keeps only the diagnostics with
  these codes on both sides and prints a table per code (oracle, matched by code/offset/length, dartr-only).
- `f` = sorted flag names taken from public getters (`isAbstract`, `isSynthetic`, `isStatic`, `isLate`,
  `isCovariant`, `isPromotable`, `hasImplicitType`, `isExternal`, `isAugmentation`…). Use one fixed list in both tools.

### 5.2 `interface` (C5b)

For each class, mixin, enum and extension type, from `InheritanceManager3.getInterface(e)`:
`{"n":"A","map":[["foo","package:p/b.dart::B.foo","int Function()"],...],"implemented":[...],"conflicts":[...]}`.
Names are sorted (Name.toString). This tests member substitution, combined signatures (`topMerge`) and
override inference without needing the resolver.

### 5.3 `typesys` (C5c)

The oracle collects the declared types of all top-level variables and type aliases of the library (at most 40, in declaration order).
For each ordered pair, it writes
`{"a":"..","b":"..","sub":true,"lub":"..","glb":"..","normA":".."}` (`isSubtypeOf`, `leastUpperBound`,
`greatestLowerBound`, `normalize` from `TypeSystemImpl`). It runs over generated pair files (`tests/typesys/*.dart`
written by a script from type lists) and over the corpus.

### 5.4 `resolved-el` (C6a), `resolved` (C6b), diagnostics (C7)

- `resolved-el`: for each `SimpleIdentifier`/`NamedType`/`ConstructorName`: `{"o":..,"e":..,"k":..,"el":R(element),"member":"substitution display"}`.
  The exact format (one line per library with its units, local element references
  `"local:<kind>:<name>@<offset>"`) is in the header of `tools/oracle/bin/resolved_el.dart`.
  It catches errors in scopes, imports and extensions before inference is right.
- `resolved`: the existing format (static type per expression), plus `"inv"` (staticInvokeType) and `"targs"` for invocations.
- Diagnostics: the existing `resolved` diagnostics list, with filters for code families so that errors can be brought up group by group
  (compile-time errors, then warnings, then hints).

### 5.5 Checkpoint sequence

| cp | needs | corpus target |
|---|---|---|
| C5a `elements --mask-inferred` | element/types/display, scopes, element_builder, types_builder | sdk/lib, pkg/analyzer/lib: 100% |
| C5b `interface` | + inheritance_manager3 | same |
| C5c `typesys` | + subtype/LUB/GLB/normalize | generated + corpus |
| C6a `resolved-el` | + resolution_visitor, scopes, simple resolvers | tests/language (error-free files first) |
| C5d `elements` (full) | + resolver for top-level inference | all |
| C6b `resolved` types | + full ResolverVisitor, flow analysis, inference | all |
| C7a `elements` + const values | + constant evaluation | all |
| C7b diagnostics | + error_verifier, error/*, constant_verifier | all |

## 6. Work split

### 6.1 Fixed first (U0, one agent, ~3 days, reviewed before fan-out)

1. `dartr_element::ids`: `ElementId`/`EId<T>`/`FragmentId`/`StoreId`, category marker types and `SubtypeOf` (same pattern as dartr_ast).
2. All element and fragment structs **with fields** (bodies may be `todo!()`), `ElementKind`, `ElementFlags`, `Name` interner.
3. `TypeKind`, `TypeId`, `ListId`, the `Interner` API (`intern`, `get`, `list`), `ElemRef`/`MemberId`/`SubstId`.
4. `Ctx`, `WorldSnapshot`, `LocalArena`, `TypeProvider` struct (slots).
5. `ResolutionTables` and the `DiagnosticReporter` hookup.
6. `dartr_flow` operations traits (`FlowAnalysisOperations`, `TypeAnalyzerOperations` as Rust traits with associated types).
7. Oracle `elements` mode + `dartr dump elements` skeleton + difftest wiring.

### 6.2 Units (1–3 agent-days each; "←" = depends on)

Wave A (after U0, all in parallel):
- A1 display_string_builder + `getDisplayString` for every TypeKind and element.
- A2 type_algebra (Substitution, FreshTypeParameters, MapSubstitution), replacement_visitor, type_visitor.
- A3 subtype.dart + the basic type_system.dart predicates (nullability, isNonNullable, promote, resolveToBound, bounds).
- A4 normalize, top_merge, least_greatest_closure, type_schema_elimination, runtime_type_equality, replace_top_bottom.
- A5 least_upper_bound + greatest_lower_bound (← A3).
- A6 type_constraint_gatherer + shared `type_constraint.dart` + generic_inferrer (← A3).
- A7 inheritance_manager3 + class_hierarchy + member.dart substitution getters (← A2).
- A8 dart/element/scope.dart + resolver/scope.dart Namespace + summary2 export/combinator.
- A9–A11 flow analysis (generic, dependency-free): (9) FlowModel/PromotionModel/SsaNode/Reachability/flow_link; (10) `_FlowAnalysisImpl` statements and expressions; (11) patterns, null shorting, assigned_variables, promotion keys. Test with a ported subset of the shared `flow_analysis_test.dart` mini-AST tests.
- A12–A13 shared type_analyzer: (12) expressions, switch/if-case, statements; (13) patterns, `TypeAnalyzerOperationsMixin`.
- A14 constant value.dart (DartObjectImpl, State classes), independent of resolution.

Wave B (linker, ← A1–A3, A8):
- B1 reference.dart + library_builder skeleton + element_builder (all declaration kinds, fragments, parts).
- B2 named_type_builder, function_type_builder, record_type_builder, types_builder, reference_resolver.
- B3 type_alias, simply_bounded, default_types_builder, variance_builder, interface_cycles.
- B4 synthetic constructors, enum children, field promotability (field_name_non_promotability_info), hasNonFinalField, extension_type.dart.
- B5 instance_member_inferrer (override inference) (← A7) — reaches C5a/C5b.
- B6 driver-lite in `dartr_driver`: file_state (unlinked data, API signature), library_graph, in-memory cycle scheduler, `dartr dump elements`.

Wave C (resolver, ← wave A + B1–B2):
- C1 resolution_visitor (scopes, local elements, type annotations), element_binding_visitor, named_type_resolver, ast_rewrite.
- C2 ResolverVisitor core (resolver.dart: statements, function bodies, InferenceContext, flow analysis glue, `TypeSystemOperations`).
- C3 method_invocation_resolver, function_expression_invocation_resolver, invocation_inferrer, invocation_inference_helper.
- C4 property_element_resolver, type_property_resolver, prefixed_identifier, simple_identifier, this_lookup, lexical_lookup.
- C5 assignment, binary, prefix, postfix expression resolvers.
- C6 applicable_extensions, extension_member_resolver.
- C7 typed_literal_resolver, record_literal/record_type_annotation, for_resolver, yield, variable_declaration, list_pattern resolvers.
- C8 instance_creation, constructor_reference, function_reference, function_expression resolvers, dot shorthands.
- C9 annotation_resolver, comment_reference_resolver, exit_detector, body_inference_context.
- C10 summary2 top_level_inference, ast_resolver, constructor_initializer_resolver, super_constructor_resolver, default_value_resolver, metadata_resolver (← C2) — reaches C5d.
- C11 library_analyzer port (unit-parallel resolution, library-wide steps) — reaches C6b.

Wave D (verification, ← C):
- D1–D2 constant evaluation.dart + compute.dart + potentially_constant + from_environment.
- D3 constant_verifier.
- D4–D7 error_verifier.dart, split by sections: class/declaration checks; statement/expression checks; constructors and fields; type arguments and misc.
- D8–D12 error/* files in groups: inheritance_override + member_duplicate + duplicate_definition; imports_verifier + unused_local_elements + dead_code; best_practices + annotation_verifier + deprecated_functionality + element_usage; type_arguments + literal_element + return_type + constructor_fields + base_or_final; language_version_override, ignore comments (IgnoreInfo), error_handler, remaining.

Wave E (driver):
- E1 snapshot/scheduler/cancellation; E2 invalidation v1; E3 `LinkedCycle` serialization (summary writer/reader); E4 analysis-result disk cache; E5 RequirementSink and manifests (v2a/b).

Coupling rule: each unit owns whole Dart files (first-line comment names the Dart source).
Units talk only through U0 types and the public functions named after the Dart methods.

## 7. Risks and mitigations

| risk | where | mitigation |
|---|---|---|
| Map/Set iteration order (Dart LinkedHashMap) changes the order of diagnostics, members and conflicts | everywhere, esp. inheritance_manager3, export scopes, constant walker | IndexMap/IndexSet for every ported Map/Set that is iterated; lint ban; differential tests catch the rest |
| `identical` vs `==` on types and elements | type.dart `==`, inheritance, override checks | `TypeId ==` = identity; `dart_eq` = Dart `==`; port each call site deliberately, comment `// Dart: identical` vs `// Dart: ==` |
| Dynamic type checks (`is`/`as`) on elements and types | everywhere | `match` on `TypeKind`/`ElementKind`; typed `EId<T>` casts checked at run time like `Ast::cast` |
| Visitor double dispatch (`node.accept`, `resolveExpression(resolver, contextType)`) | resolver | dispatch functions generated by `dartr_ast` codegen (`match kind`); context type passed explicitly |
| Mixins with state (`TypeAnalyzer`, `ErrorDetectionHelpers`, `_GettersAndSetters`) | resolver, scopes | trait with required accessors to the state fields + provided methods |
| Recursive laziness during linking (inference cycles, alias cycles) | top_level_inference, type_alias | status vectors + `OnceLock` slots; port the cycle detection exactly (it produces diagnostics) |
| AST rewrites in link-time expressions vs shared ASTs | summary2/ast_resolver | detached `ConstExprs` arena per cycle; file ASTs are cloned before analysis |
| Nondeterminism from concurrent id allocation | fresh type params, interner | never sort or hash-iterate by id in output paths; a CI job runs the corpus with `--jobs 1` and `--jobs 16` and compares |
| Local types leaking into shared caches | inheritance, member substitution | `ctx.global()` inside `get_or_init`; `debug_assert` in the global interner |
| Exceptions used for control flow / crash containment | link exception (library_context.dart:493), driver | `Result` where the Dart catches; `catch_unwind` per library analysis → "internal error" diagnostic like the analyzer |
| Memory growth in the LSP (interner, retired cycles) | generation | generation reset (§4.2); body-analysis types in per-task overlays are dropped with the task |
| Size of `flow_analysis.dart` (8.4k) and `error_verifier.dart` (8.5k) | | split by sections (A9–A11, D4–D7); keep Dart method names for diff review |
| Experimental features (augmentations, primary constructors, anonymous methods) | element/fragment model | model fragments from day 1; exclude experiment-only corpus files from parity goals until wave D |
| SDK cycle linking is sequential and on the critical path | linking | v2 disk cache of the SDK cycle; later parallel per-library sub-phases (element building, type resolution per library) |

### Critical Files for Implementation
- /Users/dominik/Projects/dartr/third_party/dart-sdk/pkg/analyzer/lib/src/dart/element/element.dart
- /Users/dominik/Projects/dartr/third_party/dart-sdk/pkg/analyzer/lib/src/dart/element/type.dart
- /Users/dominik/Projects/dartr/third_party/dart-sdk/pkg/analyzer/lib/src/summary2/link.dart
- /Users/dominik/Projects/dartr/third_party/dart-sdk/pkg/analyzer/lib/src/dart/analysis/library_context.dart
- /Users/dominik/Projects/dartr/third_party/dart-sdk/pkg/analyzer/lib/src/generated/resolver.dart
