# Persistent Analysis Result Cache (`dartr`)

Status: design proposal.
Dart SDK reference: `third_party/dart-sdk` (tag `3.13.3`, `AnalysisDriver.DATA_VERSION = 644`).
Paths under `dart/analysis/` and `summary2/` below are relative to `third_party/dart-sdk/pkg/analyzer/lib/src/`.

---

## 1. What the Dart Analyzer Stores and How It Keys Entries

The Dart analyzer persists three tiers of analysis artifacts in a content-addressed key-value store (`ByteStore`).

### 1.1 `ByteStore` hierarchy and on-disk format

- **Interface and in-memory layer** (`dart/analysis/byte_store.dart`):
  - `ByteStore` (`byte_store.dart:13-37`): `Uint8List? get(String key)`, `Uint8List putGet(String key, Uint8List bytes)`, `void release(Iterable<String> keys)`.
  - `MemoryCachingByteStore` (`byte_store.dart:93-155`): wraps a backing `ByteStore` (typically `EvictingFileByteStore`) in a reference-counted in-memory LRU cache (`MemoryByteStore`, `byte_store.dart:41-89`) so all `AnalysisDriver` instances in an `AnalysisContextCollectionImpl` share deserializable byte blobs in memory before hitting disk.
- **Disk persistence and eviction** (`dart/analysis/file_byte_store.dart`):
  - `FileByteStore` (`file_byte_store.dart:149-233`): stores each key at `<cachePath>/<shard>/<key>`, where `<shard>` is the first two characters of `<key>` (`_getShardName`, `file_byte_store.dart:215-221`).
  - `FileByteStoreValidator` (`file_byte_store.dart:236-285`): appends a 4-byte trailer (`[0x01, 0x00]` version + 2-byte `fletcher16` checksum, `fletcher16.dart`) via `wrapData` before writing, and validates/strips it in `getData` on read.
  - Atomic writes (`FileByteStore._writeFile`, `file_byte_store.dart:198-213`): writes asynchronously via a 20-task `FuturePool` (`_pool`) to `<cachePath>/<key>-temp-<pid>[-<counter>]` and renames to `<cachePath>/<shard>/<key>`.
  - `EvictingFileByteStore` (`file_byte_store.dart:41-146`): wraps `FileByteStore` with `maxSizeBytes` (default 1 GiB in `analysis_server`, 128 MiB in `FileByteStore` constructor). When `_bytesWrittenSinceCleanup >= _maxSizeBytes ~/ 8` (`file_byte_store.dart:91-97`), it spawns a background isolate (`_cleanUpFolder`, `file_byte_store.dart:104-145`) that stats every file, sorts by `FileStat.accessed`, and deletes the oldest files until total bytes are below `maxSizeBytes`.

### 1.2 Driver-level salts (`dart/analysis/driver.dart`)

`AnalysisDriver` computes three base salts once at startup (`_fillSalt`, `driver.dart:2499-2504`):

1. `_saltForUnlinked` (`_calculateSaltForUnlinked`, `driver.dart:2531-2537`):
   - `AnalysisDriver.DATA_VERSION` (`644`, `driver.dart:112`)
   - `enableIndex` (`bool`)
2. `_saltForElements` (`_calculateSaltForElements`, `driver.dart:2506-2513`):
   - `AnalysisDriver.DATA_VERSION`
   - `_addDeclaredVariablesToSignature` (`driver.dart:2261-2271`: sorted `-D` `declaredVariables` names and values)
3. `_saltForResolution` (`_calculateSaltForResolution`, `driver.dart:2515-2529`):
   - `AnalysisDriver.DATA_VERSION`
   - `enableIndex` (`bool`)
   - `enableDebugResolutionMarkers` (`bool`)
   - `_addDeclaredVariablesToSignature`
   - `analysisContext?.workspace.contributeToResolutionSalt(buffer)` (workspace/Blaze salt)

### 1.3 Tier 1: Unlinked unit data (`*.unlinked2`)

- **Key computation** (`FileState.refresh`, `dart/analysis/file_state.dart:673-684`):
  ```text
  signature = ApiSignature()
    ..addUint32List(_fsState._saltForUnlinked)
    ..addFeatureSet(featureSet)                 // experiment enable flags
    ..addLanguageVersion(packageLanguageVersion) // major, minor
    ..addString(contentHash)                    // MD5 hex of file text (file_content_cache.dart)
    ..addBool(exists)
  unlinkedKey = "${signature.toHex()}.unlinked2"
  ```
- **Lookup and population** (`FileState._getUnlinkedUnit`, `file_state.dart:816-876`):
  - Queries `_byteStore.get(unlinkedKey)`.
  - On miss: parses the file (`parse`, `file_state.dart:610-639`), calls `serializeAstUnlinked2(unit, isDartCore: isDartCore)` (`file_state.dart:1066-1166`), and stores the serialized `AnalysisDriverUnlinkedUnit` via `_byteStore.putGet(unlinkedKey, bytes)`.
  - Notice that on a warm cache hit (`file_state.dart:819-822`), `FileState.refresh` does **not** parse the file's AST at discovery time; it only reads the file content to compute `contentHash` (cached by modification stamp in `FileContentCache`, `dart/analysis/file_content_cache.dart`) and deserializes `AnalysisDriverUnlinkedUnit`.
- **Value payload** (`AnalysisDriverUnlinkedUnit` and `UnlinkedUnit`, `dart/analysis/unlinked_data.dart:11-63, 459-579`):
  - Search sets for fast reference pruning: `definedClassMemberNames`, `definedTopLevelNames`, `referencedNames`, `subtypedNames`.
  - `UnlinkedUnit`:
    - `apiSignature` (`Uint8List`, 16-byte MD5 from `computeUnlinkedApiSignature(unit)` in `dart/analysis/unlinked_api_signature.dart:16-239`, which hashes all tokens outside function/method/constructor bodies, plus directives and class member headers).
    - Directives: `exports`, `imports`, `parts`, `docImports`, `libraryDirective`, `partOfNameDirective`, `partOfUriDirective`, `hasDartCoreImport`, `isDartCore`.
    - `informativeBytes` (`Uint8List` produced by `writeUnitInformative` in `summary2/informative_data.dart:33-41`: declaration offsets, name offsets, doc comments, and constant initializer token offsets).
    - `lineStarts` (`Uint32List`), `topLevelDeclarations` (`Set<String>`), `dartdocTemplates`.

### 1.4 Tier 2: Linked library cycles (`*.linked` and `*.linked.digest`)

- **Cycle construction and signatures** (`_LibraryWalker.evaluateScc`, `dart/analysis/library_graph.dart:183-270`):
  - `LibraryCycle.apiSignature` (transitive, `library_graph.dart:184-230`):
    - `_fsState._saltForElements`
    - Direct dependency cycles' `apiSignature`s (`directDependencies`)
    - For each library in the SCC (sorted by path): `packageLanguageVersion`, `file.uriStr`, `libraryFiles.length`, and for each file in the library: `file.exists` and `file.apiSignature` (the 16-byte unlinked API signature).
  - `LibraryCycle.nonTransitiveApiSignature` (`library_graph.dart:237-251`):
    - For each file in the cycle (sorted by path): `file.apiSignature` and `file.kind.addDirectivesSignature(signature)` (`file_state.dart:317-350`, resolved target file paths of imports, exports, parts, and doc imports).
  - `LibraryCycle.manifestSignature` (`library_graph.dart:236-250`):
    - `_fsState._saltForElements`
    - `_addUriResolutionToSignature`: SDK root path + sorted `lib` folder paths of `transitivePackages` (`library_graph.dart:304-322`)
    - For each file in the cycle (sorted by path): `file.path`, `file.uriStr`.
- **Key computation** (`LibraryCycle.linkedKey`, `library_graph.dart:97-103`):
  - When `withFineDependencies == false`: `"$apiSignature.linked"` (transitive API signature).
  - When `withFineDependencies == true`: `"$manifestSignature.linked"` (non-API structural key; validated against dependencies using `RequirementsManifest`).
- **Lookup, relinking, and population** (`LibraryContext._loadBundle`, `dart/analysis/library_context.dart:246-398`, and `_probeLinkedBundle`, `library_context.dart:404-491`):
  - Recursively loads `cycle.directDependencies` in topological order.
  - When `withFineDependencies == false`, looks up `byteStore.get(cycle.linkedKey)`. Because `apiSignature` is transitive over all dependency API signatures and own unlinked API signatures, a cache hit is unconditionally valid.
  - When `withFineDependencies == true`, `_probeLinkedBundle` checks `"$bundleKey.digest"` (`RequirementsManifestDigest`) first, then `_LinkedBundleCacheEntry.requirements`, and if dependency requirements still hold:
    - If `entry.nonTransitiveApiSignature == cycle.nonTransitiveApiSignature`, reuses `entry.linkedBytes`.
    - Otherwise re-links the cycle reusing old `libraryManifests` to preserve unchanged `ManifestItemId`s.
  - On miss: parses ASTs for units in the cycle, calls `link(elementFactory, apiurelInputs)` (`summary2/link.dart:36-49`, which runs `Linker.link` and serializes the element model with `BundleWriter` in `summary2/bundle_writer.dart:37-118`), and writes `_LinkedBundleCacheEntry` to `byteStore`.
  - Whether hit or miss, `elementFactory.addBundle(BundleReader(...))` (`summary2/bundle_reader.dart`) registers the cycle's libraries and applies `InformativeDataApplier` (`summary2/informative_data.dart`) to restore offsets and doc comments from `unitsInformativeBytes`.
- **Value payload** (`_LinkedBundleCacheEntry`, `library_context.dart:546-667`):
  - `nonTransitiveApiSignature` (`String`)
  - `id` (`ManifestItemId`)
  - `requirementsBytes` (`Uint8List`, serialized `RequirementsManifest`)
  - `libraryManifests` (`Map<Uri, LibraryManifestHandle>`)
  - `linkedBytes` (`Uint8List` from `BundleWriter`: symbolic reference table + per-library elements, types, exported references, constant expressions, and `ElementFlags`).

### 1.5 Tier 3: Resolved unit and library diagnostic results (`*.resolved` and `*.resolved2`)

- **Analysis options signature** (`AnalysisOptionsImpl.signature`, `dart/analysis/analysis_options.dart:295-394`):
  - `sdkVersionConstraint`, `propagateLinterExceptions`, `strictCasts`, `strictInference`, `strictRawTypes`, `chromeOsManifestChecks`, `codeStyleOptions.useFormatter`
  - Every `ExperimentStatus.knownFeatures` flag (`isEnabled`)
  - Sorted `errorProcessors` (`code:severity`)
  - Sorted `lintRules` (`rule.name`)
  - Sorted `enabledLegacyPluginNames`
  - Sorted `pluginConfigurations` (`name`, `isEnabled`, `source.toYaml`, `diagnosticConfigs`, `dependencyOverrides`).
- **Per-unit resolved key (`*.resolved`)** (`AnalysisDriver._getResolvedUnitSignature` / `_getResolvedUnitKey`, `driver.dart:2042-2060`):
  ```text
  signature = ApiSignature()
    ..addUint32List(_saltForResolution)
    ..addString(library.file.workspacePackage?.pubPackage?.pubspecContent ?? '')
    ..addString(library.file.uriStr)
    ..addString(library.libraryCycle.apiSignature)
    ..addUint32List(library.file.analysisOptions.signature)
    ..addString(file.uriStr)
    ..addString(file.contentHash)
  resolvedKey = "${signature.toHex()}.resolved"
  ```
  - Note: includes the defining library's transitive `libraryCycle.apiSignature` (all upstream APIs and own cycle APIs) plus the specific unit's `contentHash` (which covers function bodies and comments) plus `analysisOptions.signature` and `pubspecContent` (used by `pubspec`-dependent lints such as `depend_on_referenced_packages` and `secure_pubspec_urls`).
- **Per-unit payload** (`AnalysisDriverResolvedUnit`, `driver.dart:1455-1463`, `ErrorEncoding.encode` at `driver.dart:2930-2952`):
  - `errors`: list of `AnalysisDriverUnitError` (`offset`, `length`, `uniqueName`, `message`, `correction`, `contextMessages`), already filtered by `// ignore:` and validated by `IgnoreValidator` (`library_analyzer.dart:310-377`).
  - `index`: `AnalysisDriverUnitIndex` (serialized element relations for search/references, `dart/analysis/index.dart`).
- **Per-library diagnostics bundle (`*.resolved2`, fine-dependencies mode)** (`AnalysisDriver._getLibraryDiagnosticsKey`, `driver.dart:1993-2017`, `dart/analysis/library_diagnostics.dart:22-116`):
  - Key hashes `DATA_VERSION`, `_saltForResolution`, `library.file.analysisOptions.signature`, `pubspecContent`, and for each file in `library.files` (sorted by path): `file.path`, `file.uriStr`, `file.contentHash`, `file.kind.addDirectivesSignature`.
  - Value (`LibraryDiagnosticsBundle`) holds `id`, `requirementsBytes` (`RequirementsManifest`), and `serializedFileResults: Map<Uri, Uint8List>` (each value is an `AnalysisDriverResolvedUnit`), plus `"$bundleKey.digest"`.
  - On hit (`_getLibraryDiagnosticsBundle`, `driver.dart:1919-1985`), if `RequirementsManifest.isSatisfied` passes against the linked element model, `_produceErrors` (`driver.dart:2171-2196`), `_getErrors` (`driver.dart:1807-1819`), and `_getIndex` (`driver.dart:1879-1891`) return the cached errors and unit index without running `LibraryAnalyzer.analyze()`.

---

## 2. Measured Phase Split Today (`DARTR_TIMINGS=1`)

Measured on this machine with `/Users/dominik/Projects/dartr/target/release/dartr` via `tools/heavy.sh`:

### 2.1 `~/fvm/default/packages/flutter` (1 context, 1,697 analyzed `.dart` files)

Command:
```sh
DARTR_TIMINGS=1 dartr analyze ~/fvm/default/packages/flutter
```

Output:
```text
Analyzing flutter...
[driver] context 0: discover 222.869875ms (2694 known files), link 551.602833ms (1738 stores), resolve+lint 1.288056417s
[timings] contexts 35.467334ms, file list 18.050916ms (1697 files), diagnostics 2.115832042s
No issues found!
```

| Phase | Wall time | Share of total (2.17 s) | Notes |
|---|---:|---:|---|
| `contexts` (`AnalysisContextCollection::new`) | 35.5 ms | 1.6% | Walk roots, read `package_config.json`, `analysis_options.yaml`, SDK |
| `file list` (`analyzed_dart_files`) | 18.1 ms | 0.8% | Enumerate 1,697 target `.dart` files |
| `driver.discover` (`FileSystemState::discover`) | 222.9 ms | 10.3% | Read + parse + compute `UnlinkedUnit` & `api_signature` for 2,694 files |
| `driver.link` (`Driver::link_libraries`) | 551.6 ms | 25.4% | Tarjan SCC + `link_cycle` for 1,738 library cycles |
| `driver.resolve+lint` (`analyze_libraries_with_lints`) | 1,288.1 ms | 59.4% | Body resolution, constant verification, error verifiers, resolved & AST lints, ignore validation |
| Overhead (`ContextOptions`, non-Dart files, output) | 53.3 ms | 2.5% | `pubspec.yaml` / `analysis_options.yaml` checks, result formatting |
| **Total** | **2,169.4 ms** | **100.0%** | |

### 2.2 `bench/corpus/visible-app` (18 contexts, 3,090 analyzed `.dart` files)

Command (without external Dart plugin isolates, isolating `dartr` work):
```sh
DARTR_TIMINGS=1 dartr analyze --no-plugins bench/corpus/visible-app
```

Output summary:
```text
[driver] context 0: discover 489.411958ms (8777 known files), link 1.815849583s (6909 stores), resolve+lint 3.312744875s
[driver] context 1: discover 160.947708ms (3867 known files), link 957.028875ms (2105 stores), resolve+lint 50.16525ms
... (16 additional package contexts) ...
[driver] total (18 contexts): discover 1.158890127s, link 6.004141s, resolve+lint 3.700147459s
[timings] contexts 148.058584ms, file list 81.76575ms (3090 files), diagnostics 11.194376166s
```

When run with plugins enabled (default), `dartr_plugins::run_plugins` spawns `dart` to run the workspace's `jaspr_lints` analyzer plugin in parallel; `[driver] total` is `13.261s` (`discover 1.676s`, `link 7.199s`, `resolve+lint 4.386s`) and `plugin_thread.join()` brings total `diagnostics` wall time to `23.196s`.

Breakdown of `dartr`'s own work on `visible-app` (`--no-plugins`, 11.42 s total):

| Phase | Wall time | Share of total (11.42 s) | Notes |
|---|---:|---:|---|
| `contexts` (`AnalysisContextCollection::new`) | 148.1 ms | 1.3% | 18 contexts in pub workspace + standalone packages |
| `file list` (`analyzed_dart_files`) | 81.8 ms | 0.7% | 3,090 target `.dart` files |
| `driver.discover` (18 contexts) | 1,158.9 ms | 10.1% | 35,103 total `FileState`s across 18 drivers (~8,900 unique files; SDK and shared pub packages are re-parsed in each context's `FileSystemState`) |
| `driver.link` (18 contexts) | 6,004.1 ms | 52.6% | 8,995 cycle stores linked across 18 `Driver`s (SDK and shared pub packages are re-linked in each context) |
| `driver.resolve+lint` (18 contexts) | 3,700.1 ms | 32.4% | 3,090 files resolved and linted (context 0 is 3.313 s; contexts 1–17 sum to 0.387 s) |
| Overhead (`ContextOptions`, non-Dart files) | 331.2 ms | 2.9% | 18 `ContextOptions`, `non_dart_diagnostics` |
| **Total (`--no-plugins`)** | **11,424.2 ms** | **100.0%** | |

Key observation on `visible-app`: because each context creates its own `Driver` (`crates/dartr_driver/src/project.rs:68`) with a separate `FileSystemState` and `LinkState`, contexts 1–17 spend `669 ms` in `discover` and `4,188 ms` in `link` mostly re-parsing and re-linking the Dart SDK and shared `pub` packages (`flutter`, `bloc`, `drift`, etc.) to analyze only `387 ms` of package-local code.

---

## 3. What `dartr` Should Store and How It Maps to `crates/dartr_driver`

### 3.1 Design choice: what to persist at each tier

Unlike the Dart VM (where parsing and linking are slow enough that `.unlinked2` and `.linked` element bundles are required even before resolution), `dartr` has different trade-offs:

1. **Tier A — Unlinked file summary (`UnlinkedCacheEntry`, keyed by file content + parse config)**:
   - **What `dartr` computes today**: `FileSystemState::discover_wave` (`crates/dartr_driver/src/file_state.rs:647-722`) reads each file, calls `parse_file_State` (`file_state.rs:842-867`) to build `Arc<ParsedUnit>`, `UnlinkedUnit` (`file_state.rs:101-130`), and `api_signature: Hash128` (`unlinked_api_signature.rs`).
   - **What to store on disk**:
     - `api_signature: Hash128` (`[u8; 16]`)
     - `unlinked: UnlinkedUnit` (directives: exports, imports, parts, doc_imports, library_directive, part_of_name, part_of_uri, has_dart_core_import, is_dart_core, `line_starts: Vec<u32>`, `top_level_declarations: Vec<String>`)
   - **Crucial detail**: When an `UnlinkedCacheEntry` hits on disk, `FileContent` (`file_state.rs:242-254`) does **not** need an `Arc<ParsedUnit>` merely to walk the library graph (`library_graph.rs`) and compute every `LibraryCycle.api_signature`! `ParsedUnit` is only needed if a cycle actually misses the library-result cache and must be linked or resolved. Making `FileContent.parsed` lazy (`OnceLock<Arc<ParsedUnit>>` populated on demand by `Driver::link_libraries` / `Driver::analyze_libraries_with_lints`) eliminates parsing all 2,694 files on `flutter` (and all 35,103 `FileState`s on `visible-app`) during a warm `dartr analyze` or warm `dartr language-server` start.
   - **In-memory cross-context sharing**: Within a single process (`DriverSession`), an in-memory map keyed by `(content_hash, package_language_version, experiments)` shares both `UnlinkedCacheEntry` and (when parsed) `Arc<ParsedUnit>` across all 18 context drivers, immediately cutting multi-context `discover` even on a cold disk cache.

2. **Tier B — Per-library analysis result (`LibraryAnalysisCacheEntry`, keyed by library transitive API + unit contents + options)**:
   - **What `dartr` computes today**:
     - `DriverSession::diagnostics_with_index` (`crates/dartr_cli/src/driver_provider.rs:310-422`) and `analyze_context` (`driver_provider.rs:1647-1785`) call `driver.link_libraries(&library_ids)` and `driver.analyze_libraries_with_lints(&jobs, ...)`.
     - For each library, `library_outcomes` (`driver_provider.rs:1791-1947`) produces:
       - `Vec<FileDiagnostics>` (`crates/dartr_cli/src/provider.rs:43-50`: `path`, `line_info: LineInfo`, `diagnostics: Vec<Diagnostic>`) for the units of the library after running `LibraryAnalyzer` (`crates/dartr_resolver/src/library_analyzer.rs`), resolved lints (`crates/dartr_driver/src/analysis.rs:110-169`), parsed lints (`lint_library_unfiltered`), `validate_ignores`, and `filter_ignored_diagnostics` (`finish_file`, `provider.rs:267-299`).
       - Optional `Vec<UnitIndex>` (`crates/dartr_server/src/index.rs:70-76`: `elements: Vec<ElementKey>`, `relations: Vec<Relation>`) when indexing is enabled (in `dartr language-server` / `dartr analysis-server`).
   - **What to store on disk**:
     - Per defining library `L`: a `LibraryAnalysisCacheEntry` containing, for every unit `u` in `driver.fs.library_file_kinds(L)`:
       - `unit_path: String`
       - `unit_uri: String`
       - `line_starts: Vec<u32>` (`LineInfo`)
       - `diagnostics: Vec<CachedDiagnostic>` (`code: &'static DiagnosticCode` serialized by stable `(unique_name, lower_case_name)` index or string, `offset: u32`, `length: u32`, `message: String`, `correction: Option<String>`, `context_messages: Vec<CachedDiagnosticMessage>`)
       - `unit_index: Option<UnitIndex>` (present when computed by the language server; `dartr analyze` only reads `diagnostics` and `line_starts` and ignores or omits `unit_index`).
   - **Why per-library diagnostic + index caching skips linking on warm runs**:
     - In `dartr analyze` (and at `dartr language-server` initial workspace analysis when no files are open/priority yet), the required outputs are `FileDiagnostics` for every analyzed file and `UnitIndex` for every indexed file.
     - Both `FileDiagnostics` and `UnitIndex` are **completely free of arena IDs** (`ElementId`, `FragmentId`, `StoreId`, `TypeId`, `Name`).
     - And because the library's cache key includes its `LibraryCycle.api_signature` (which transitively hashes the unlinked `api_signature` of every reachable import/export/part cycle down to the SDK), **we can compute every library's cache key after `discover` + `compute_library_cycles` without running `Driver::link_libraries` at all!**
     - If all requested libraries in a context hit `LibraryAnalysisCacheEntry` (and there are no priority files requiring an in-memory `ResolvedLibraryResult`), `Driver::link_libraries` and `Driver::analyze_libraries_with_lints` are skipped completely for that context.
     - If a subset of libraries `M` miss the cache (e.g., after editing a file body or changing one library's API), `analyze_context` only calls `driver.link_libraries(&missing_library_ids)` and `driver.analyze_libraries_with_lints(&missing_jobs, ...)`.

3. **Tier C — Linked cycle summary (`LinkedCycleCacheEntry`, phase 2 / optional)**:
   - When is linking still needed on a warm cache?
     1. In `dartr language-server`, when the user opens a file (`priority_files` is non-empty) or triggers hover, completion, go-to-definition, signature help, or code actions, `DriverSession::resolved_library_in` (`driver_provider.rs:936-1067`) needs an in-memory `WorldSnapshot` and `ResolvedLibrary`.
     2. In `dartr analyze` after an edit to a leaf library `L` (body-only or local API): `L` misses Tier B, so `driver.link_libraries(&[L])` must link `L` and its transitive dependency cycles (`dart:core`, `package:flutter/...`).
   - Why `ElementStore` cannot be naively `memcpy`'d to disk today:
     - `ElementStore` (`crates/dartr_element/src/store.rs`) and `TypeKind` (`crates/dartr_element/src/types.rs`) embed `Name(u32)` (indices into `Generation.names: NamePool`), `TypeId(NonZeroU32)` (indices into `Generation.interner: Interner`), and `ElementId` / `FragmentId` containing `StoreId(u32)` of dependency cycles and `StoreId::SYNTHETIC`.
   - Two ways to cache linked cycles across runs/contexts:
     - **Step C1 (In-memory cross-context `LinkedCycle` sharing within `Generation`)**:
       - Because all `Driver`s in a `DriverSession` already share the same `Arc<Generation>` (`NamePool`, `Interner`, `synthetic: ElementStore`, and `next_cycle_store`), a `LinkedCycle` (`Arc<ElementStore>`, `Arc<ConstExprs>`, `Vec<Arc<LinkedLibrary>>`) produced in Context 0 for a cycle with transitive `api_signature: Hash128` (plus SDK/package URI resolution signature for that cycle) has valid `StoreId`s, `TypeId`s, and `Name`s in **every** other context of the same `Generation`!
       - Sharing `Map<CycleCacheKey, LinkedCycle>` inside `Generation` (or `DriverSession`) means contexts 1–17 in `visible-app` reuse the `ElementStore`s of the SDK and shared packages with zero serialization and zero ID remapping, cutting cold multi-context link time on `visible-app` from `6.00 s` to `~2.0 s`.
     - **Step C2 (On-disk `LinkedCycle` summary)**:
       - Serializes a `LinkedCycle` keyed by `cycle.api_signature` using a self-contained string/name table, local type table, and symbolic `Reference` table (`crates/dartr_link/src/reference.rs`) for cross-store `ElementId`s; on load into a `Generation`, interns `Name`s and `TypeKind`s into `Generation` and resolves symbolic references against dependency `LinkedLibrary.reference`s.
       - Because Tier A + Tier B already eliminate 100% of linking and resolution on a 0-edit warm `dartr analyze` and warm `dartr language-server` start (until the first on-demand hover/completion request, where linking one file's dependency subgraph takes ~200–500 ms), Tier A + Tier B + Step C1 deliver the highest return with much lower complexity and risk than Step C2.

### 3.2 Mapping to `crates/dartr_driver` and `crates/dartr_cli`

| Component | File in `dartr` | Changes for Result Cache |
|---|---|---|
| Unlinked unit & `FileContent` | `crates/dartr_driver/src/file_state.rs` | Store `content_hash: Hash128` (128-bit Blake3 or MD5 of UTF-8 source after BOM strip) on `FileContent`. Split `FileContent.parsed: Arc<ParsedUnit>` into lazy on-demand parse (`parsed()` method on `FileState` / `FileContent`) when `UnlinkedUnit` + `api_signature` are loaded from the cache. |
| Library cycle signatures | `crates/dartr_driver/src/library_graph.rs` | `evaluate_scc` (`library_graph.rs:148-221`) already computes `LibraryCycle.api_signature: Hash128` using `salt_for_elements`, dependency cycle `api_signature`s, `package_language_version`, `uri_str`, `exists`, and each file's unlinked `api_signature`. Add `salt_for_elements` initialization from the driver's cache salt. |
| Cross-context cycle cache | `crates/dartr_driver/src/driver.rs` | In `Driver::link_libraries` (`driver.rs:172-320`), before linking an SCC wave, look up `(cycle.api_signature, cycle_directives_hash)` in a shared `Generation` cycle cache; on hit, insert `Arc<ElementStore>`, `Arc<ConstExprs>`, and `Arc<LinkedLibrary>` into `self.state`. |
| Per-library result cache | `crates/dartr_cli/src/driver_provider.rs` | In `analyze_context` (`driver_provider.rs:1647-1785`), after `driver.fs.discover()`, compute `LibraryCycle`s for `library_ids` without linking. Compute `LibraryResultKey` for each library. Partition `library_ids` into `cached` and `missing`. Only call `driver.link_libraries(&missing)` and `driver.analyze_libraries_with_lints(&missing_jobs, ...)`. Write newly computed `LibraryAnalysisCacheEntry`s to the cache. |
| Server index caching | `crates/dartr_server/src/server.rs`, `crates/dartr_legacy/src/server.rs` | Pass `require_index: true` when querying `LibraryAnalysisCacheEntry` from `diagnostics_with_index`; store `UnitIndex` alongside `FileDiagnostics` in the entry so warm LSP/legacy server starts populate `self.index` from cache without linking or resolving. |

---

## 4. Cache Keys and Invalidation

Every condition that can change the diagnostics or index of a file must be mixed into the cache key (via a 128-bit hash, e.g., Blake3 or `SipHash128`/`MD5` matching `Hash128` in `crates/dartr_driver/src/unlinked_api_signature.rs:28-119`).

### 4.1 Global build & SDK salt (`cache_version_salt: Hash128`)

Computed once per process and mixed into every key:

1. **`CACHE_FORMAT_VERSION: u32`**: bumped whenever the serialized layout of `UnlinkedCacheEntry` or `LibraryAnalysisCacheEntry` changes.
2. **`dartr` binary identity**:
   - In release/debug builds, a compile-time hash of the workspace crate sources or git commit + dirty/source fingerprint embedded by `build.rs` (or the `mtime` + `len` + SHA-256/Blake3 of `std::env::current_exe()`). Hashing `current_exe()` (size + mtime + first/last 64 KiB or full binary hash, which takes <5 ms for a 33 MB binary) guarantees that **any** recompilation of `dartr` during development automatically invalidates cache entries from older builds, even when `git HEAD` has uncommitted edits.
3. **Dart SDK identity**:
   - Canonical SDK root path (`sdk.path()`), `sdk/version` file content, and `sdk/lib/_internal/sdk_library_metadata/lib/libraries.dart` content hash (or `sdk/lib/libraries.json` content hash).

### 4.2 Unlinked key (`UnlinkedKey: Hash128` -> `<hex>.unlinked`)

Mixed into `UnlinkedKey`:
1. `cache_version_salt`
2. `file_content_hash: Hash128` (128-bit hash of the file's UTF-8 content after stripping leading `\u{feff}` BOM, matching `file_state.rs:845`)
3. `package_language_version: (u32, u32)` (`FileConfig.package_language_version`, `file_state.rs:226`)
4. `experiments: &[ExperimentalFlag]` (sorted `ExperimentalFlag` discriminants from `FileConfig.experiments`, `file_state.rs:227`)
5. `is_dart_core: bool` (`file.uri_str == "dart:core"`, which controls `has_dart_core_import` in `parse_file_state`, `file_state.rs:855`).

Invalidation: Any edit to the file's bytes, any change to the package's language version in `.dart_tool/package_config.json` or `pubspec.yaml`, or any change to `analyzer.enable-experiment` in `analysis_options.yaml` or `--enable-experiment` on the CLI changes `UnlinkedKey`.

### 4.3 Library cycle API signature (`LibraryCycle.api_signature: Hash128`)

Computed by `evaluate_scc` (`crates/dartr_driver/src/library_graph.rs:148-221`):
1. `cache_version_salt` + `salt_for_elements` (includes `-D` declared variables if any)
2. Number of `direct_dependencies` and each dependency cycle's `api_signature: Hash128`
3. For each library in the SCC (sorted by path):
   - `package_language_version: (u32, u32)`
   - `library_file.uri_str`
   - `library_files.len()`
   - For each unit `f` in `library_files`:
     - `f.exists()`
     - `f.api_signature()` (the 16-byte unlinked API signature from `compute_unlinked_api_signature`, which hashes all non-body tokens, directives, and class/mixin/enum/extension headers)
     - Resolved target paths/URIs of `f`'s `library_imports`, `library_exports`, `part_includes`, and `doc_library_imports` (`add_directives_signature`, `file_state.rs:584-621`), so changing `package_config.json` or adding/removing a file that a directive resolves to invalidates the cycle signature even if the directive text in `f` did not change.

### 4.4 Library analysis result key (`LibraryResultKey: Hash128` -> `<hex>.resolved`)

Computed per defining library `L` (with units `U = [u_0, u_1, ...]` in `library_file_kinds(L)` order):
1. `cache_version_salt`
2. `cycle.api_signature: Hash128` (captures the transitive public API of `L`, all its parts, and all transitive imported/exported libraries)
3. **All unit content hashes in `L`**:
   - `U.len()`
   - For each unit `u_i` in `U`: `u_i.path`, `u_i.uri_str`, `u_i.content_hash`, and `is_generated(&u_i.path)` (because `finish_file` skips `validate_ignores` on `*.g.dart` / `*.freezed.dart` based on path).
   - Including **every** unit's `content_hash` in `L` (not just a single unit's) is required because library-wide verifiers (`ImportsVerifier`, `UnusedLocalElementsVerifier`, duplicate definition checks) and library-wide lints depend on usages across all parts of `L`, and `// ignore:` comments live inside unit source text.
4. **Analysis options & lint signature** for `L` and each unit `u_i`:
   - `ResolverOptions`: `strict_casts`, `strict_inference`, `strict_raw_types`
   - Sorted `experiments: Vec<ExperimentalFlag>`
   - Sorted `lint_rules: Vec<&'static str>` (implemented enabled lint rules)
   - Sorted `unimplemented_lint_rules: Vec<String>` (affects `unnecessary_ignore` validation in `finish_file`, `provider.rs:288`)
   - Sorted `unignorable_names: Vec<String>` (`cannot-ignore` error codes and severities from `analysis_options.yaml`)
5. **Package metadata (`LintPackageInfo`)**:
   - Content hash of the enclosing package's `pubspec.yaml` (if any) and sorted package names from `.dart_tool/package_config.json` (used by `LintPackageInfo` in `crates/dartr_lints` for `depend_on_referenced_packages`, `public_member_api_docs`, `package_api_docs`, etc.).

### 4.5 Non-Dart diagnostics and Analyzer Plugins

- **Non-Dart files** (`analysis_options.yaml`, `pubspec.yaml`, `AndroidManifest.xml`, `fix_data.yaml` in `crates/dartr_cli/src/provider.rs:640-658`):
  - These take <5 ms total across `visible-app` and are always evaluated fresh from disk (not cached, or trivially cached by file content hash).
- **`errorProcessors` (`errors:` severity overrides and `exclude:` globs in `analysis_options.yaml`)**:
  - Just like Dart's `AnalysisDriverResolvedUnit`, `FileDiagnostics` stores diagnostics with their default severity after `// ignore:` filtering (`provider.rs:47-49`), while `server::analysis_errors` (`crates/dartr_cli/src/server.rs`) applies `analysis_options.yaml` severity remapping (`ignore`, `info`, `warning`, `error`) when formatting output. Even so, including `unignorable_names` in `LibraryResultKey` ensures `cannot-ignore` changes invalidate cached `FileDiagnostics`.
- **Analyzer plugins (`dartr_plugins`)**:
  - External Dart analyzer plugins (`analyze.rs:271-296`) run in a separate thread via `dartr_plugins::run_plugins` and are independent of `DriverSession`'s Dart file diagnostics. If plugin result caching is added later, its key must hash the plugin's resolved package graph in `.dart_tool/package_config.json`, `analysis_options.yaml` plugin section, and all files in the context.

---

## 5. On-Disk Layout, Atomic Writes, Size Limits, and Eviction

### 5.1 Directory layout and default location

- **Setting / CLI flags**:
  - Off by default during initial rollout; enabled when `--cache=<path>` is passed to `dartr analyze` / `dartr language-server` / `dartr analysis-server` (both CLI parsers already accept `--cache=<path>`: `crates/dartr_cli/src/args.rs:25,99` and `crates/dartr_server/src/args.rs:59`) or when `DARTR_CACHE_DIR=<path>` is set, unless `--disable-file-byte-store` is passed (`crates/dartr_server/src/args.rs:75`).
- **Sharded directory structure**:
  ```text
  <cache_dir>/
    v1/
      unlinked/
        <2-hex-shard>/
          <32-hex-key>.unlinked
      resolved/
        <2-hex-shard>/
          <32-hex-key>.resolved
  ```
  256 shard subdirectories (`00`..`ff`) keep directory entries small on macOS APFS and Linux ext4/xfs.

### 5.2 Binary format and checksum validation

Each cache file has a fixed 16-byte header followed by the payload and a 8-byte trailer:
- Bytes `0..4`: magic (`b"DRTC"`)
- Bytes `4..8`: `CACHE_FORMAT_VERSION: u32` (little-endian)
- Bytes `8..16`: payload length `u64` (little-endian)
- Bytes `16..16+len`: deterministic little-endian binary payload (hand-rolled ` ByteWriter` / `ByteReader` over `Vec<u8>` / `&[u8]`, no external proc-macro dependency required)
- Bytes `16+len..24+len`: 64-bit XXH3 / SipHash-2-4 checksum of `header + payload`.

On read:
- If the file is shorter than 24 bytes, magic or version mismatches, length mismatches `slice.len() - 24`, or the 64-bit checksum fails, the reader treats it as a cache miss and unlinks the corrupted file.

### 5.3 Atomic writes

Concurrent `dartr analyze` and `dartr language-server` processes may read and write the same `<cache_dir>` simultaneously:
1. Ensure `<cache_dir>/v1/<kind>/<shard>/` exists (`fs::create_dir_all`).
2. Write the complete buffer (header + payload + checksum) to a temporary file in the same shard directory:
   `<cache_dir>/v1/<kind>/<shard>/.tmp.<key>.<pid>.<thread_counter>`.
3. Call `std::fs::rename(tmp_path, final_path)` (POSIX atomic rename on the same filesystem).
4. Because keys are content-addressed and deterministic, two processes racing on the same key write byte-for-byte identical payloads; atomic `rename` ensures readers never observe a partially written file.

### 5.4 Size limits and LRU eviction

- **Size cap**: Default `max_size_bytes = 512 MiB` (configurable via `DARTR_CACHE_MAX_MB`), matching `EvictingFileByteStore`'s bounded disk usage.
- **Estimated entry sizes**:
  - `UnlinkedCacheEntry`: ~300–600 bytes per file (~1.6 MiB for `flutter`'s 2,694 files; ~5.5 MiB for `visible-app`'s ~8,900 unique files).
  - `LibraryAnalysisCacheEntry`:
    - Diagnostics-only (`dartr analyze`): ~150–400 bytes per unit (~500 KiB for `flutter`; ~1 MiB for `visible-app`).
    - With `UnitIndex` (`dartr language-server`): ~4–12 KiB per unit (~15 MiB for `flutter`; ~30 MiB for `visible-app`).
- **Eviction policy**:
  - Track `bytes_written` in the process. When `bytes_written >= max_size_bytes / 8` (or once on server shutdown / end of `analyze` if a lightweight marker file `<cache_dir>/v1/.bytes_written` exceeds the threshold), run a sweep over `<cache_dir>/v1/{unlinked,resolved}/*/*`:
    1. Delete any stale `.tmp.*` files older than 10 minutes.
    2. Collect `(path, len, mtime)`. (On write, existing entries that are hit can optionally have `mtime` refreshed if older than 1 hour to approximate LRU without write amplification on every read.)
    3. If `total_bytes > max_size_bytes`, sort by `mtime` ascending (oldest first) and `fs::remove_file` until `total_bytes <= max_size_bytes * 3 / 4`.

---

## 6. Expected Warm-Run Gains

### 6.1 What a warm `dartr analyze` run still has to do

On a warm run with 0 file edits (Tier A `UnlinkedCacheEntry` + Tier B `LibraryAnalysisCacheEntry`):
1. `contexts` (`AnalysisContextCollection::new`): walk directory tree, parse `package_config.json` and `analysis_options.yaml`.
2. `file list` (`analyzed_dart_files`): list target `.dart` files.
3. `discover` (warm):
   - Read known `.dart` files in parallel across rayon threads and compute 128-bit `content_hash` (no lexing, no AST parsing, no `compute_unlinked_api_signature` walk).
   - Read `UnlinkedCacheEntry` from `<cache_dir>/v1/unlinked/` (~2,694 small files on `flutter`; ~8,900 unique files on `visible-app`, shared in memory across the 18 contexts).
4. `compute_library_cycles`:
   - Tarjan SCC walk over the in-memory `FileSystemState` directive graph to compute `LibraryCycle.api_signature` for each library (~5–15 ms).
5. `resolved` cache lookup:
   - Compute `LibraryResultKey` for each analyzed library and read `LibraryAnalysisCacheEntry` (`FileDiagnostics`) from `<cache_dir>/v1/resolved/`.
   - **0 libraries linked (`link = 0 ms`), 0 libraries resolved or linted (`resolve+lint = 0 ms`).**
6. Run `non_dart_diagnostics` (`pubspec.yaml`, `analysis_options.yaml`) and format output.

### 6.2 Estimated warm timings

#### `~/fvm/default/packages/flutter` (1 context, 1,697 analyzed files, 2,694 known files)

| Phase | Cold today | Cold with cross-context sharing | Warm (0 edits) | Warm (1 file body edited) |
|---|---:|---:|---:|---:|
| `contexts` + `file list` | 53.5 ms | 53.5 ms | ~54 ms | ~54 ms |
| `discover` | 222.9 ms | 222.9 ms | ~45 ms (read + hash + unlinked hit, no parse) | ~46 ms (1 file parsed) |
| `link` | 551.6 ms | 551.6 ms | **~8 ms** (SCC graph + key check only; 0 cycles linked) | ~250–400 ms (link dependency subgraph of edited library) |
| `resolve+lint` | 1,288.1 ms | 1,288.1 ms | **~25 ms** (read ~1,500 `.resolved` entries) | ~35 ms (resolve + lint 1 library + read cached rest) |
| Overhead | 53.3 ms | 53.3 ms | ~30 ms | ~30 ms |
| **Total** | **2,169 ms (2.17 s)** | **2,169 ms (2.17 s)** | **~160–200 ms (~0.18 s, ~12x faster)** | **~420–560 ms (~0.5 s, ~4x faster)** |

#### `bench/corpus/visible-app` (`--no-plugins`, 18 contexts, 3,090 analyzed files, ~8,900 unique known files)

| Phase | Cold today | Cold + in-memory cross-context sharing (Step C1) | Warm (0 edits, Tier A + B) | Warm (1 file body edited in context 0) |
|---|---:|---:|---:|---:|
| `contexts` + `file list` | 229.8 ms | 229.8 ms | ~230 ms | ~230 ms |
| `discover` (18 contexts) | 1,158.9 ms | ~550 ms (parse each unique file once) | ~120 ms (hash + shared unlinked cache) | ~125 ms |
| `link` (18 contexts) | 6,004.1 ms | ~2,100 ms (link shared SDK/pkg cycles once) | **~35 ms** (SCC graph + key check; 0 cycles linked) | ~600–900 ms (link dependency subgraph of 1 library) |
| `resolve+lint` (18 contexts) | 3,700.1 ms | 3,700.1 ms | **~55 ms** (read `.resolved` entries) | ~80 ms (resolve 1 library + read cached rest) |
| Overhead | 331.2 ms | 331.2 ms | ~150 ms | ~150 ms |
| **Total (`--no-plugins`)** | **11,424 ms (11.42 s)** | **~6,910 ms (~6.9 s)** | **~590–700 ms (~0.65 s, ~17x faster)** | **~1,200–1,500 ms (~1.35 s, ~8x faster)** |

For `dartr language-server` warm startup on `flutter_tools`:
- Initial diagnostics + workspace `UnitIndex` population drops from full link+resolve+index to ~150–250 ms (`UnlinkedCacheEntry` + `LibraryAnalysisCacheEntry` with `UnitIndex`), matching or beating `dart language-server`'s warm startup (`1.4–2.1 s` on `flutter_tools`).

---

## 7. Risks and Verification Tests

### 7.1 Risks (stale or wrong results) and mitigations

| Risk | Cause | Mitigation |
|---|---|---|
| **Missing dependency in `LibraryResultKey`** | A change in an imported library's API, an exported library, a `part` file, or a macro/augmentation is not reflected in the key, returning stale diagnostics. | Key includes `LibraryCycle.api_signature` (transitive over all imports, exports, parts, and doc imports) plus the full `content_hash` of **every** unit (`defining` + `parts`) in the library. |
| **Directive target shift without file edit** | `.dart_tool/package_config.json` changes, or a new `.dart` file is created that satisfies a previously broken relative `import`/`part` URI, while the importing file's text is unchanged. | Include `add_directives_signature` (the resolved target `FileId` path and `exists` flag of every import/export/part/docImport directive) in `LibraryCycle.api_signature` and `LibraryResultKey`, matching `FileKind.addDirectivesSignature` (`file_state.dart:317-350`). |
| **`analysis_options.yaml` or `pubspec.yaml` change** | Enabling a lint rule, changing `strict-casts`, changing `errors:` (`cannot-ignore`), or editing `pubspec.yaml` dependencies does not invalidate cached diagnostics. | Hash `ResolverOptions`, `experiments`, `lint_rules`, `unimplemented_lint_rules`, `unignorable_names`, and `LintPackageInfo` (`pubspec.yaml` content hash + package dependency names) into `LibraryResultKey`. |
| **Stale cache across `dartr` rebuilds** | Developer modifies a lint rule or verifier in Rust and re-runs `dartr analyze`, getting cached diagnostics from the previous binary. | Mix the executable's fingerprint (`current_exe()` size + mtime + content hash) and `CACHE_FORMAT_VERSION` into `cache_version_salt`. |
| **Nondeterministic ordering of diagnostics or index relations** | Cached diagnostics or `UnitIndex` relations deserialize in a different order from a cold run. | Serialize `Vec<Diagnostic>` and `UnitIndex` in the exact order produced by `finish_file` and `index_unit`; preserve `LineInfo.line_starts`. |
| **Lazy `ParsedUnit` missing when fallback or priority resolution runs** | A file loaded from `UnlinkedCacheEntry` has no in-memory `ParsedUnit`, and a later step (`link_libraries`, `analyze_libraries_with_lints`, `known_parsed_units`, or `ParseOnlyProvider` fallback) accesses `file.parsed()`. | Replace direct field reads of `content.parsed` with an accessor `content.parsed()` that parses on demand into a `OnceLock<Arc<ParsedUnit>>` using the stored source text or re-reading the file, and populates it in parallel before linking/resolving any cache-missing cycle. |
| **Corrupted or truncated cache file after crash / disk full** | Process killed mid-write or filesystem corruption. | Write to `.tmp.<key>.<pid>.<tid>` and `rename`; validate magic, version, length, and 64-bit checksum on every read; treat any error as a cache miss and delete the bad file. |

### 7.2 Tests to prove identical output

1. **Cold vs. Warm byte-for-byte parity test on all corpora**:
   - For each corpus (`~/fvm/default/packages/flutter`, `~/fvm/default/packages/flutter_tools`, `bench/corpus/visible-app`, and the test fixtures):
     1. Run `dartr analyze --format=json` with no cache (`out_cold`).
     2. Run `dartr analyze --cache=<dir> --format=json` on an empty `<dir>` (`out_populate`), asserting `out_populate == out_cold` (excluding the timing/memory fields) and asserting cache files were written.
     3. Run `dartr analyze --cache=<dir> --format=json` on the populated `<dir>` (`out_warm`), asserting `out_warm == out_cold` and asserting via internal counters / `DARTR_TIMINGS=1` that `0` cycles were linked and `0` libraries were resolved.
2. **Edit-sequence mutation tests (invalidation matrix)**:
   - Starting from a warm cache on a multi-library, multi-part, multi-context fixture, apply each of the following mutations one by one and assert that `dartr analyze --cache=<dir>` produces the **exact** same output as a fresh cold `dartr analyze` (and only re-analyzes the expected libraries):
     - **Body-only edit**: change a statement inside a function body in `lib/a.dart` (introduces a type error) -> only `a.dart`'s library misses `.resolved`; importers of `a.dart` hit `.resolved`.
     - **Comment / `// ignore:` edit**: add or remove `// ignore: unused_local_variable` or a doc comment `/// [Foo]` -> `a.dart`'s library misses `.resolved` and recomputes ignore validation / doc import resolution.
     - **Part file edit**: edit a body or top-level declaration in `lib/part_b.dart` (`part of 'a.dart'`) -> `a.dart`'s library misses `.resolved` and re-runs library-wide verifiers (`unused_element`, `unused_import`, `duplicate_definition`).
     - **Public API edit**: change a parameter type or return type in `lib/dep.dart` -> `dep.dart`'s unlinked `api_signature` changes -> `dep.dart`'s cycle `api_signature` changes -> both `dep.dart` and its importer `lib/user.dart` miss `.resolved` and recompute diagnostics.
     - **Export / re-export edit**: add `hide Foo` to `export 'dep.dart';` in `lib/barrel.dart` -> `barrel.dart` and `lib/consumer.dart` miss `.resolved`.
     - **File creation / deletion**: create a previously missing `lib/missing.dart` referenced by `import 'missing.dart';` -> `add_directives_signature` changes -> importing library misses `.resolved` and clears `uri_does_not_exist`.
     - **`analysis_options.yaml` edit**: toggle a lint rule, `strict-casts: true`, or `errors: { todo: error }` -> all affected libraries miss `.resolved`.
     - **`pubspec.yaml` / `package_config.json` edit**: remove a dependency or change language version constraint -> affected package libraries miss `.resolved` (and `.unlinked` when language version changes).
3. **LSP cold vs. warm parity test**:
   - Start `dartr language-server --cache=<dir>` cold and warm on the same workspace; verify identical `publishDiagnostics` notifications and identical responses for `textDocument/references`, `textDocument/definition`, `textDocument/hover`, `textDocument/documentSymbol`, and `workspace/symbol`, followed by `textDocument/didChange` edit sequences.
4. **Corruption and concurrency resilience tests**:
   - Truncate or flip bits in random `.unlinked` and `.resolved` files in `<dir>`; verify `dartr analyze --cache=<dir>` recovers cleanly without panicking and matches cold output.
   - Run 4 concurrent `dartr analyze --cache=<dir>` processes against the same empty `<dir>`; verify all 4 exit with identical output and no temporary files remain.

---

## 8. Step-by-Step Implementation Plan (Behind Off-by-Default Setting)

1. **Step 1: Cache configuration and plumbing (off by default)**
   - Wire `AnalyzeArgs.cache: Option<String>` (`crates/dartr_cli/src/args.rs:25`) and `ServerOptions.cache: Option<String>` + `disable_file_byte_store: bool` (`crates/dartr_server/src/args.rs:59,75`), plus `DARTR_CACHE_DIR` env fallback, into `DriverProvider` and `DriverSession`.
   - When neither `--cache` nor `DARTR_CACHE_DIR` is set (or `--disable-file-byte-store` is set), the cache is `None` and behavior is unchanged.
2. **Step 2: Content-addressed disk store (`crates/dartr_driver/src/cache_store.rs`)**
   - Implement `DiskCache`: `cache_version_salt` computation (format version + `current_exe()` fingerprint + SDK fingerprint), 2-hex-char sharding, header + 64-bit checksum framing, atomic `.tmp.<key>.<pid>.<tid>` write + `rename`, and size-bounded LRU cleanup (`EvictingFileByteStore` equivalent).
   - Add unit tests for round-trip read/write, corruption recovery, concurrent writes, and size eviction.
3. **Step 3: In-memory cross-context sharing of `FileContent` and `LinkedCycle` (Step C1)**
   - Share parsed/unlinked file artifacts and frozen `LinkedCycle`s across context `Driver`s within `DriverSession` / `Generation` when their `UnlinkedKey` / `CycleCacheKey` matches.
   - Verify with `DARTR_TIMINGS=1` on `visible-app` that multi-context cold link time drops from ~6.0 s to ~2.1 s with identical diagnostics.
4. **Step 4: Tier A — Persistent `UnlinkedCacheEntry` and lazy `ParsedUnit`**
   - Serialize/deserialize `UnlinkedUnit` + `api_signature: Hash128` in `crates/dartr_driver/src/file_state.rs`.
   - Make `ParsedUnit` lazy on `FileContent` (`OnceLock<Arc<ParsedUnit>>`) when `UnlinkedCacheEntry` hits on disk, parsing in parallel only for files belonging to cycles that miss Tier B or require linking/resolution.
5. **Step 5: Tier B — Persistent `LibraryAnalysisCacheEntry` (`FileDiagnostics` + `UnitIndex`)**
   - In `crates/dartr_cli/src/driver_provider.rs` (`analyze_context`), compute `LibraryCycle`s after `discover()`, build `LibraryResultKey` for each library, and query `DiskCache` before calling `driver.link_libraries`.
   - Only link and resolve libraries that miss `DiskCache` (or contain priority files). Write newly computed `LibraryAnalysisCacheEntry`s to `DiskCache`.
   - Extend `diagnostics_with_index` so `dartr language-server` caches and restores `UnitIndex` per unit alongside `FileDiagnostics`.
6. **Step 6: Parity, mutation, and benchmark verification**
   - Run the cold vs. warm parity suite and the 8-case invalidation mutation suite across `flutter`, `flutter_tools`, and `visible-app`.
   - Benchmark cold vs. warm `dartr analyze` and `bench/lsp_bench.py` (`--server dartr` vs `--server dart`) with `--cache`.
