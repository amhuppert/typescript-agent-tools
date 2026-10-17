# Frozen native module-specifier inputs

The operating envelope is one loaded Project generation with concurrent,
read-only queries. Queries cannot consult the live filesystem. Loading may
discover finite spelling dependencies, including failed probes that unrelated
queries never consume. ProjectSession validates those observations before
publishing or reusing a generation.

The chosen split keeps physical observations in
`program/specifier_environment.rs` and pure native candidate transforms in
`program/specifiers`. The checker owns symbol acquisition, original enclosing
node/mode selection, and its request-local specifier cache. Capturing all final
importer/target spellings would retain a quadratic result matrix and duplicate
the query's mode-dependent schedule. Giving generation a live filesystem host
would violate snapshot consistency. Instead, loading calls the same pure path,
mapping, and ending transforms to collect a deduplicated finite probe closure.

The environment retains actual process cwd separately from configuration scope,
options, source package scopes, native raw resolution filenames/OriginalPath,
KnownSymlinks relationships, package observations, and index-collision file
observations. A missing observation is Internal; an observed missing file is
ordinary absence; an observed I/O failure retains its InputError and path until
the spelling operation reaches it. Failed speculative package reads do not
prevent capture of other candidates. The service observation recorder also
retains failures, so unchanged errors can belong to a successful generation and
changes to those errors invalidate it.

`Resolver` owns OriginalPath/realpath decisions for modules, type references, and
runtime package directories while the loader preserves query path aliases.
Shared manifest/condition owners serve both resolution and generation:
GetVersionPaths, exports object classification, GetConditions, applicable
versioned conditions, and matchPatternOrExact. Related filename case questions
remain separate operations where the native source uses different rules.

`Program.GetSymlinkCache` first consumes actual resolutions, then source package
scope, SourceFileMayBeEmitted(false), and unseen runtime dependencies. noEmit is
not an emission gate in this operation. Runtime ResolvePackageDirectory's empty
OriginalPath must be combined with `package.json` before ProcessResolution's
empty-path guard: native consequently infers a cwd alias even for an ordinary
physical package. The maintained native metadata controls establish the effect
on contextual module display, including runtime dependency families, dev-only,
declaration-only, and physical symlink variants.

`PackageJson.GetRuntimeDependencyNames` ignores Expected's Valid flag and unions
its partially decoded maps. This differs from JSONValue's ordered-map question.
One parser therefore retains raw duplicate order during PackageInfo construction,
reduces Expected maps once, then normalizes JSONValue objects for existing
resolution/generation consumers. The raw reducer handles key insertion before a
value error, an inner duplicate stopping that decode, repeated top-level map
fields reusing earlier entries, and null clearing a map. It does not retain the
original text or introduce a second decoder. All seven added duplicate/repeated
field controls captured the source-derived results; the complete 19-row baseline
passes maintained replay (`vrun-c2947ab0-469d-442e-a47b-a2201c933503`).

Captured observation keys use native ToPath (normalized cwd plus the host's
filename case policy), including absent and failed observations. Payloads retain
their original paths. PackageInfo.WithPackageDirectory separately restores the
current caller's raw directory, as the native resolver does after a cache hit.
Case-sensitive hosts retain distinct spellings. The Stage56 paths-in-project
failure reached a folded candidate whose raw capture key differed; the staged
repair applies this identity rule to every insertion and lookup, rather than
adding one lowercase alias to the closure.

`processEnding`'s collision probes append the extension before resolving against
cwd. In particular `..` plus `.ts` is `...ts`, not a file beneath the normalized
parent directory. Both query acquisition and capture use the same owner for
this sequence.

Evidence already maintained: corrected module-specifier raw-resolution36
(`vrun-58205f5b-bd1f-4dd4-bba4-7bf6187c8432`), runtime metadata12
(`vrun-01ce3102-1349-428a-9265-1c661a636aac`), and collision/reuse4
(`vrun-41c7a252-4542-426a-8bff-372d25766f08`). The latter distinguishes fresh
`pkg/sub` versus `pkg/sub/index` from written `pkg/sub/index.js`, which bypasses
the collision question. The expanded eight collision/reuse/runtime-cwd controls
also pass maintained replay (`vrun-55aaf1f2-dfb2-415d-bfe2-ae9c69af4100`). Rust final-query tests are staged in
`tests/type_module_specifier_snapshot.rs` for retained demanded errors, healthy
and reuse bypasses, mutation after load, service invalidation, and cwd-alias
reuse before a poisoned fresh-only probe. Stage56 passed the direct frozen-error
test; the remaining tests reached the intended results but had stale CLI wire
assertions (reason `error_type`, absolute error paths), now corrected. The next
integration receipt remains pending; source review alone is not an end-to-end pass.

## Manifest edge evidence

`fixtures/types/native-manifest-edges` records native imported-value queries and
fresh module strings separately for 32 raw manifests. Empty range, first matching
invalid/empty object, and non-string array-slot cases can select the base
declaration; non-array mappings disappear before pattern selection. Duplicate
mapping/range values keep ordered-map position and the last value. A wildcard
key with a concrete target can produce the literal reverse spelling `pkg/*`.

Export classification has different consumers: a root import uses an explicit
`.` entry even in an otherwise invalid dot/condition mixture. A non-root import
requires IsSubpaths, and reverse generation also rejects invalid mixtures.
Dot/hash and empty/dot mixtures remain subpath objects. The native capture
`vrun-b47d0053-26d2-46fd-9fab-fd61f22ffbcd` completed every query and display
without hook acquisition failures; its only failure was the initially absent
baseline. The promoted baseline passes maintained replay
(`vrun-eeb19d2a-1963-4688-8d16-55c80b60f51b`). Dual-cwd capture then completed
all 64 cases (`vrun-f06fe28d-7ae8-41cf-b5c9-9325579d7a4f`): the configuration
baseline passed and the workspace run stopped only for its absent baseline.
The workspace baseline is now promoted; maintained dual-cwd replay awaits the
shared oracle edit window. No Rust production change was made from this evidence.
`type_manifest_edges` now stages native-derived public imported-value and missing
alias outcomes, with healthy and reversed/warm controls. The Program test stages
final loaded package files and exact fresh strings against the workspace-cwd
baseline. Neither staged Rust test has a validation receipt yet.
