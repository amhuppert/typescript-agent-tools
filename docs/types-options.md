# TypeScript 7.0.2 option ownership

This inventory is the TypeScript 7 dialect's, and everything below
describes that dialect alone. A project whose installed compiler is
TypeScript 5.x is read with the TypeScript 5.6 dialect, which reads many of
these options as TypeScript 5.6.3 does, as
[references](references.md#compiler-selection) describes; both dialects fill
the same snapshot.

Configuration owns exact keys, occurrence validation, conversion, inheritance,
defaults, and effective overrides. Checker consumers receive typed snapshot
inputs; they do not reopen config files or reinterpret raw `strict`. The
operating envelope is one local immutable project snapshot refreshed through
the existing session owner. This inventory adds no plugin execution, compiler
emit, project-check command, or incremental build service.

The pinned catalog contains **124 canonical names**, excluding CLI aliases.
Six are command-line-only; three whole settings and three false modes are
removed during compiler-option verification. Every canonical name appears
exactly once below. Combined classifications record every relevant owner.

Primitive validation lives in the existing configuration load. Specialized
enum/list/object conversion stays with existing Reader operations. Unknown
non-null keys are Config errors, including wrong-case names and
`keyofStringsOnly`, which is absent from the catalog. Unknown nulls and empty
keys are ignored as `parseOwnConfigOfJsonSourceFile.onPropertySet` does.
Recognized command-line-only keys are errors even when null. Non-null keys
with unpaired UTF-16 surrogates are Unsupported because their exact identity
cannot be represented here. Unknown top-level tool-specific keys retain their
existing behavior.

The private `SemanticOptions` snapshot retains eight effective strict flags,
`exactOptionalPropertyTypes`, `noUncheckedIndexedAccess`,
`noPropertyAccessFromIndexSignature`, and `stableTypeOrdering`. It also retains
package-deduplication, checkJs as set (unset, false, or true) and effective
allowJs, decorator and metadata flags, both class-field operations, and exact
configured JSX hook names.
Strict flags
default true when `strict` is unset; an explicit flag overrides inherited or
own strict. Exact optional properties and unchecked indexed access default
false independently of strict. Explicit true strictPropertyInitialization or
exactOptionalPropertyTypes requires effective strictNullChecks, following
`verifyCompilerOptions`; an implicit initialization default does not trigger
that configuration diagnostic.

This intentionally changes the earlier navigation profile's acceptance of
ignored unknown options, malformed checker-only values, and removed false
modes. Existing removed graph settings retain their documented Unsupported
profile. `noDtsResolution` is internal and absent from the public catalog; an
explicit true or false is Unsupported here, while the native public compiler
reports an unknown option.

`GetAllowJS` now uses checkJs:true when allowJs is unset; explicit allowJs:false
wins. JavaScript inputs follow it, with `maxNodeModuleJsDepth`, and checkJs
stays unset, false, or true, as JavaScript checking reads it
([references.md](references.md#javascript-inputs)). Native `verifyCompilerOptions` separately reports TS5052
for checkJs:true with allowJs:false; that diagnostic remains outside this
loader's limited compiler-option combination checks and does not change the
GetAllowJS input-selection operation.

Tests prove validation and the retained Project snapshot. They do **not** prove
option-dependent type results: semantic operations and demanded deferred-feature
refusals below remain implementation obligations. Primitive catalog validation
also does not implement every unrelated combination diagnostic from
`verifyCompilerOptions`.

| Classification | Decision / owner | Exact canonical names | Source evidence and obligation |
| --- | --- | --- | --- |
| Semantic | Strict evaluation/relations | `strict`, `noImplicitAny`, `noImplicitThis`, `strictNullChecks`, `strictFunctionTypes`, `strictBindCallApply`, `strictBuiltinIteratorReturn`, `useUnknownInCatchVariables` | `GetStrictOptionValue`, `checker.NewChecker`; strict unset defaults on. |
| Semantic / demanded flow | Constructor initialization/recovery | `strictPropertyInitialization` | `checker.getFlowTypeOfAccessExpression` seeds uninitialized this-property reads; T must refuse excluded refinement and track recovery. Also drives diagnostics. |
| Semantic | Optional/indexed reads/writes | `exactOptionalPropertyTypes`, `noUncheckedIndexedAccess` | `NewChecker`, `getWriteTypeOfSymbol`, property/index operations; independent false defaults. |
| Semantic input, reserved | Ordering compatibility setting | `stableTypeOrdering` | Catalog default true; stored by core/parser. No pinned checker consumer found; invent no false-mode ordering algorithm. |
| Input | Module format/resolution | `module`, `moduleResolution`, `moduleDetection`, `jsx`, `jsxImportSource`, `paths`, `rootDirs`, `typeRoots`, `types`, `moduleSuffixes`, `customConditions`, `preserveSymlinks`, `resolvePackageJsonExports`, `resolvePackageJsonImports`, `resolveJsonModule`, `allowArbitraryExtensions` | Existing program/resolver; `GetEmitModuleKind`, `GetModuleResolutionKind`, `GetEmitModuleDetectionKind`, `GetResolutionDiagnostic`. |
| Input | Library/root selection | `target`, `lib`, `noLib`, `libReplacement`, `outDir`, `declarationDir`, `rootDir`, `composite` | Existing library/file-selection owners; `GetEmitScriptTarget`, `GetDefaultLibFileName`, `getFileNamesFromConfigSpecs`. `libraries::Catalog` decides library names, defaults, order, and replacement. |
| Input / diagnostic | Side-effect imports | `noUncheckedSideEffectImports` | Existing resolution/binding reporting; default true in 7.0.2. |
| Input / diagnostic | Namespace instantiation | `preserveConstEnums`, `isolatedModules`, `verbatimModuleSyntax` | Existing `shouldPreserveConstEnums` combines these; isolated/import checks also consume the latter two. |
| Input | Duplicate package identity | `deduplicatePackages` | `compiler.filesparser` package-ID redirects unless false: the loader keeps the first file `collectFiles` meets per package id ([references.md](references.md#package-identity)). |
| Input / diagnostic | JavaScript files | `allowJs`, `checkJs`, `maxNodeModuleJsDepth` | `GetAllowJS` defaults allowJs from checkJs; the loader admits JavaScript inputs within `maxNodeModuleJsDepth`, and `checkJs` with the file's pragma decides which of its diagnostics are reported ([references.md](references.md#javascript-inputs)). |
| Deferred semantic | Decorators/JSX hooks | `experimentalDecorators`, `emitDecoratorMetadata`, `jsxFactory`, `jsxFragmentFactory`, `reactNamespace` | `NewChecker` and decorator/JSX operations. Demanded excluded operations require located refusal; metadata is not solely emit-only. |
| Diagnostic / emit | Class-field policy | `useDefineForClassFields` | `GetUseDefineForClassFields` defaults from target; checker conflicts/read-before-initialization and class-field emit setup consume it. Demanded recovery must be tracked. |
| Diagnostic | Checker/binder reporting | `noPropertyAccessFromIndexSignature`, `skipLibCheck`, `skipDefaultLibCheck`, `noCheck`, `noUnusedLocals`, `noUnusedParameters`, `noImplicitReturns`, `noFallthroughCasesInSwitch`, `noImplicitOverride`, `allowUnusedLabels`, `allowUnreachableCode`, `allowUmdGlobalAccess`, `noErrorTruncation`, `erasableSyntaxOnly`, `isolatedDeclarations`, `forceConsistentCasingInFileNames` | Checker diagnostic branches, filesparser casing checks and existing diagnostic policy. TS4111 preserves property read type; suppression does not validate recovery. |
| Diagnostic / emit | Import-extension policy | `allowImportingTsExtensions`, `rewriteRelativeImportExtensions` | Checker import diagnostics, `verifyCompilerOptions` and emit rewriting; source resolution identity stays in resolver. |
| Emit | Output/declarations | `declaration`, `declarationMap`, `emitDeclarationOnly`, `sourceMap`, `inlineSourceMap`, `noEmit`, `removeComments`, `importHelpers`, `sourceRoot`, `mapRoot`, `inlineSources`, `emitBOM`, `newLine`, `stripInternal`, `noEmitHelpers`, `noEmitOnError` | Catalog/emitter declarations; no code emit is exposed. Invalid newLine values remain config errors. |
| Deferred build / service | Build state/references | `incremental`, `tsBuildInfoFile`, `assumeChangesOnlyAffectDirectDependencies`, `disableSourceOfProjectReferenceRedirect`, `disableSolutionSearching`, `disableReferencedProjectLoad` | Incremental affected-file handler/project-reference loading; current snapshot refresh rejects nonempty project references. |
| Diagnostic / service | Compiler tooling/policy | `preserveWatchOutput`, `listFiles`, `explainFiles`, `listEmittedFiles`, `pretty`, `traceResolution`, `diagnostics`, `extendedDiagnostics`, `generateCpuProfile`, `generateTrace`, `quiet`, `singleThreaded`, `pprofDir`, `checkers`, `all`, `version`, `init`, `project`, `disableSizeLimit`, `ignoreDeprecations` | Catalog/execute/LS options; primitive types validated. These do not override tsparse worker, output, transport, or limits. JSON checkers accepts fractional numbers; no CLI minimum validation is borrowed. |
| Deferred service | Language-service plugins | `plugins` | `commandLineOptionElements`: list of objects, nulls dropped. Plugins are not loaded and plugin-provided semantics are not claimed. |
| Deferred input | Disabling resolution | `noResolve` | Existing profile rejects effective true as Unsupported; false/unset retains normal resolution. |
| Removed setting | Removed TS7 options | `baseUrl`, `outFile`, `downlevelIteration` | `verifyCompilerOptions`: effective set values rejected. Empty path values resolve to declaring directory and are still set. Existing baseUrl returns Unsupported; outFile/downlevelIteration Config. |
| Removed false mode | Always-enabled TS7 modes | `alwaysStrict`, `esModuleInterop`, `allowSyntheticDefaultImports` | `verifyCompilerOptions`: false TS5108; true/null/unset allowed. alwaysStrict is not a strict-family flag. |
| Command-line-only | Invalid in compilerOptions | `help`, `watch`, `locale`, `showConfig`, `listFilesOnly`, `ignoreConfig` | `convertJsonOption`: TS6266 even for null; these six names are not accepted config inputs. |

## Source and validation evidence

Pinned TypeScript 7.0.2 sources are the cached checkout at revision
`2bd066d87f5bafd315be9f40889d0a60b9e58e0b`:

- `internal/tsoptions/declscompiler.go`: `OptionsDeclarations` and defaults.
- `internal/tsoptions/commandlineoption.go`: typed list elements.
- `internal/tsoptions/tsconfigparsing.go`: `parseOwnConfigOfJsonSourceFile`,
  `convertJsonOption`, `isCompilerOptionsValue`, path normalization, and
  inheritance through `parseConfig` / `mergeCompilerOptions`.
- `internal/core/compileroptions.go`: `GetStrictOptionValue`, `GetAllowJS`,
  `GetUseDefineForClassFields`, and typed compiler inputs.
- `internal/compiler/program.go`: `verifyCompilerOptions` removals and explicit
  null-sensitive option prerequisites.
- `internal/compiler/filesparser.go`: package deduplication and casing checks.
- `internal/checker/checker.go`: `NewChecker`,
  `getFlowTypeOfAccessExpression`, `checkPropertyNotUsedBeforeDeclaration`,
  property/index operations, reporting, and decorator hooks.

Fresh native probes ran `tsc --project CASE.json --pretty false` on an exported
unannotated function. With default libraries, unset/true strict reported
TS7006, false did not, and explicit noImplicitAny overrode strict in either
direction. Removal probes recorded TS5108 for the three false modes, TS5102
for true/false downlevelIteration and non-null outFile (empty included), and
TS5023 for true/false keyofStringsOnly; null keyofStringsOnly was ignored.
Other probes recorded TS6266 for watch:null, TS5025 for Strict:true, TS5024
for wrong primitive/plugin entry types, TS6046 for invalid newLine, accepted
checkers:1.5, and TS5052 for the two explicit null-sensitive options with
strictNullChecks:false. The probe scripts and complete results were kept
outside the repository.

`tests/type_configuration.rs` tests final `Project::load` outcomes: every
strict flag's invalid overridden occurrence and inherited origin, removed
versus supported/null modes, inherited mode overrides, exact keys, plugin
entry types, and strict-null prerequisites. Six private tests in the config
owner inspect the final Project snapshot through its crate-visible accessor,
without adding a public testing API. They cover strict defaults and per-flag
overrides, inherited flags and explicit null unsets, independent non-strict
options, stable-ordering defaults/overrides, demanded deferred-input retention,
and both class-field operations at and around ES2022. The integration suite
also crosses checkJs true/false/null with allowJs unset/true/false/null against
final input selection. Final type-query evidence remains outstanding.
