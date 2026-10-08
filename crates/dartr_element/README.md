# dartr_element: API contract for the semantic units

This crate is the fixed interface (unit U0, `docs/design/semantics.md` §6.1)
that the wave A–D units build on. It holds the element/fragment model of
analyzer 3.13.3, the type model and interner, the lookup context `Ctx`, the
`TypeProvider`, `ResolutionTables` and the diagnostics hookup. Flow analysis
and type analyzer interfaces are in `dartr_flow` (no dependency on this crate).

## 1. Ids

- `ElementId` / `FragmentId`: `store (24 bits) | tag (8 bits) | index (32 bits)`.
  `Tag` is the storage kind (one per instantiable Dart `*Impl` class);
  `ElementKind` is the Dart public `ElementKind` (`id.kind()`).
- `EId<T>` / `FId<T>`: typed views. `T` is a data struct (`ClassElement`) or a
  category marker (`InterfaceElement`, `ExecutableElement`, `VariableElement`, ...).
  - Dart upcast (implicit): `id.upcast::<InterfaceElement>()`, checked at compile time.
  - Dart `is` / `as T?`: `raw.is::<T>()`, `raw.cast::<T>() -> Option<EId<T>>` (tag only, no store access).
- Subclasses without own data share the data struct and vector of their
  superclass and differ by tag: `FieldFormalParameterElement` /
  `SuperFormalParameterElement` → `FormalParameterElement`; pattern variables →
  `LocalVariableElement` (same for fragments).
- `ElementId::DYNAMIC` / `ElementId::NEVER` are fixed ids without data.
- Ids, `Name`, `TypeId` have no `Ord` on purpose (see §6).

## 2. Building elements (link phases, `dartr_link`)

```rust
let mut store = generation.new_cycle_store();          // one per library cycle
let f = store.add_fragment::<ClassFragment>(ClassFragment {
    interface: InterfaceFragmentData::new(FragmentData::new(Some(name), Some(offset))),
});
let c: EId<ClassElement> = store.add(ClassElement {
    interface: InterfaceElementData::new(ElementData::new(Some(name), f.raw())),
});
store.fragment(f).element.set_once(c.raw());            // Dart `late final element`
store.get_mut(c).fields.push(field);                    // structural data: `&mut`
```

- **Structural data** (names, children lists, flags known from syntax): plain
  fields, written through `&mut ElementStore` while a builder phase owns the store.
- **Data computed later in linking** (types, supertypes, bounds, defaults,
  inferred types): slot fields written through `&ElementStore` / `&Ctx`, so
  recursive inference works with shared borrows:
  - `OnceSlot<T>` = Dart `late final` field or lazy cache. `set_once` panics on
    a second set; `get` panics before the set (the Dart `LateInitializationError`).
  - `VarSlot<T: Copy>` = Dart non-final field that link phases assign (maybe
    several times): `supertype`, `mixins`, `bound`, `_type`, `returnType`, ...
    `get() -> Option<T>` (Dart `null` = `None`).
  - `BoolSlot`, `ElementFlagCell`, `FragmentFlagCell`: bools and the generated
    flag sets (`ElementFlags` / `FragmentFlags`, one bit per value of Dart
    `_ElementStorageFlag` / `_FragmentStorageFlag`).
- A Dart superclass is a field with the base struct, and each struct derefs to
  its base: `class.interface.instance.element.name` == `class.name`.
  Category data: `ctx.interface(id)`, `ctx.instance(id)`, `ctx.executable(id)`,
  `ctx.variable(id)`, `ctx.property_inducing(id)`, `ctx.property_accessor(id)`;
  any kind: `ctx.any(id)` (a `match`-able `AnyElement`), `ctx.element_data(id)`.
- Every Dart field is ported or listed as dropped with a reason
  (`tests/schema_coverage.rs` against `schema/element.json`). Mark a Rust field
  that holds a Dart field with a doc line `/// Dart: ClassImpl.field`.
- Freeze: `Arc::new(store)`, then `world.with_store(store, libraries)` makes the
  next `WorldSnapshot`. Never change a frozen store's structure.
- Expressions that linking resolves (const initializers, defaults, annotations)
  are `ConstExprId`s: nodes in the cycle's `ConstExprs` arena (design §2.2).

## 3. Reading: `Ctx`

```rust
let ctx = Ctx { world: &snapshot, current: Some(&store), local: None,
                tp: &type_provider, features: &features, req: &NoopSink };
let class = ctx.get(class_id);                 // &ClassElement, any store
let supertype = class.supertype.get();         // Option<TypeId>
let name: &str = ctx.name_str(class.name.unwrap());
```

- `ctx.store(id)` looks in this order: local arena, current cycle, synthetic
  store of the generation, frozen cycles of the snapshot.
- `ctx.global()` drops the local arena. **Lazy shared caches** (inheritance,
  member types, constants; `get_or_init` on frozen data) **always run under
  `ctx.global()`**, so they cannot see local elements or types.
- Body analysis: `let local = generation.new_local_arena();` and
  `Ctx { local: Some(&local), .. }`. Local elements are added through
  `local.store.add(...)` (shared reference, append-only).
- Requirements (fine-grained dependencies): call `ctx.req.record(Requirement::...)`
  at the places where the analyzer calls `globalResultRequirements?.record_*`.
  v1 uses `NoopSink`.

## 4. Types

```rust
let args = ctx.intern_list(&[ctx.tp.int_type()]);
let list_int = ctx.intern(TypeKind::Interface {
    element: ctx.tp.list_element().upcast(), args,
    nullability: Nullability::None, alias: None,
});
match *ctx.ty(list_int) { TypeKind::Interface { element, args, .. } => ..., _ => ... }
```

- `TypeKind` is `Copy`; lists inside types are interned `ListId<T>`
  (`TypeList`, `NamedFields` (sorted by name), `ParamList`, `TypeParamList`).
  Also interned: substitutions (`SubstId`, canonical order), substituted members
  (`MemberId`, used in `ElemRef::Member`), aliases (`AliasId`).
- **`TypeId ==` is exact structural identity** (alias, parameter names and
  parameter elements included). It is Dart `identical`, not Dart `==`; Dart `==`
  is `TypeSystem::dart_eq` (unit A3). Comment each port site
  `// Dart: identical` or `// Dart: ==`.
- Fixed ids: `TypeId::{DYNAMIC, VOID, INVALID, UNKNOWN, NEVER, NEVER_QUESTION}`.
- **Overlay rule**: a value goes to the local overlay (`LocalArena.types`) if and
  only if it mentions a local id (local element, type, list, substitution or
  member). So each structure has exactly one id. `ctx.intern` routes; the global
  interner `debug_assert!`s that it never gets a local id.
- Interning is lock-sharded (64 shards); reads are lock-free and return
  references that live as long as the generation (`&'a TypeKind` from `Ctx<'a>`).
- Function types bind named `TypeParameterElement`s (substitution, not de
  Bruijn). Fresh type parameters go to the local arena (body analysis), the
  cycle store (linking) or `generation.synthetic` (shared caches).

## 5. Resolution results and diagnostics

- The resolver writes what the Dart resolver sets on AST nodes into
  `ResolutionTables` (one per unit; one per cycle for `ConstExprs`), keyed by
  `NodeId`: `static_type`, `element`, `declared_fragment`, `invoke_type`,
  `type_arg_types`, `param_element`, `annotation_type`, `read_/write_element`,
  `read_/write_type`, `extended_type`, `method_name_type`, `resolved_uri`,
  `pattern_info`, `flags`. The AST itself stays syntax only.
- Diagnostics go through `dartr_diagnostics::DiagnosticReporter` (one per unit).
  Type and element arguments: `diagnostics::type_arg(ctx, ty)` and
  `diagnostics::element_arg(ctx, e)`; they carry the display string (unit A1)
  and the `ElementRef`s that `convertTypeNames` uses to disambiguate names.

## 6. Determinism rules (design §2.5)

Output may never depend on id values or on hash iteration order.

- Every Dart `Map`/`Set` that is iterated: `indexmap::IndexMap` / `IndexSet`
  (Dart's default maps keep insertion order).
- Pure lookups: `LookupMap` / `LookupSet` (this crate; no iteration API).
- `clippy.toml` in each semantic crate (copy `crates/dartr_element/clippy.toml`)
  disallows the types `std::collections::{HashMap, HashSet}` and
  `hashbrown::{HashMap, HashSet}` (this includes `FxHashMap` aliases) and the
  iteration methods of `imbl::HashMap`. Run
  `cargo clippy -p <crate> --all-targets -- -D warnings`.
- Never sort by `ElementId`, `TypeId`, `Name` or any `raw()` value for output;
  sort by the Dart key (name text, offset). `raw()` is only for canonical forms
  that are never printed (for example `intern_subst`).
- Ids depend on thread timing (parallel linking and interning); the same input
  must give the same output with `--jobs 1` and `--jobs 16`.

## 7. Placeholders for later units

- `todo!("<Dart name>")` bodies: `FieldElementImpl.declaringFormalParameter` (B4).
- Inheritance (A7, done): `dartr_typesystem::inheritance_manager3` caches the
  interface of each interface element in `InterfaceElementData.inheritance`
  (an [`ElementCache`]); substituted members (`ElemRef::Member`) are read
  through `dartr_typesystem::member`, whose lazy member types are cached in
  the interner (`Ctx::member_type_cached`).
- Display strings (A1, done): `display_string.rs` (`type_display_string_with`,
  `element_display_string_with`). The type algorithms (substitution,
  subtyping, LUB, ...) are in `dartr_typesystem`.
- `FeatureSet` is a minimal stand-in (enabled experiment flags) until a port of
  `dart/analysis/features.dart` (B6).
- `Namespace` is data only; unit A8 ports the builders.
