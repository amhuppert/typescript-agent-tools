# Native module scope lookup

The Stage40 presentation failure for exported `Late` is a missing lookup step:
`Inputs::name_scope` supplies external-file Locals but no raw Exports. Binder's
paired local has flagsNone, so the native resolver correctly rejects that
local and then incorrectly reaches globals. Declaration storage is correct.

This change stays within the immutable Project and synchronous request-local
resolver. It changes the shared lexical lookup operation, without moving
declarations, acquiring types, resolving late members, or changing qualified
export lookup. Type-dependent qualified exports remain the resolved-table
owner's separate handoff.

Pinned 7.0.2 `binder/nameresolver.go` 105–145 defines the decision:

1. After Locals, external SourceFile and ModuleDeclaration inspect the actual
   declaration symbol's raw Exports. They do not call getExportsOfModule.
2. SourceFile and ambient non-global module contexts first inspect `default`.
   `GetLocalSymbolForExportDefault` runs before the original requested flags and
   exact local-name comparison. A matching named default returns its export.
3. In those same contexts, a raw export whose flags equal Alias and whose
   declarations contain ExportSpecifier or NamespaceExport is excluded before
   `getSymbol` can resolve it. Alias combined with other flags is not excluded.
4. Ordinary lookup skips the reserved `default` name and intersects the original
   meaning with SymbolFlagsModuleMember. A failed lookup continues outward.

`Storage` continues to decide declaration placement. `name_scope` remains the
one ordered scope recipe consumed by native and navigation lookup; it adds
Host::File only for an external Root and records the external/ambient gate for
exports. Existing namespace declaration ambient facts distinguish declared
namespaces; augmentation scopes are ambient and global augmentations are not
external contexts. The admitted TS-only inputs do not create the plain-JS
CommonJS indicator branch: binder.go694 gates require-call detection with
IsInJSFile, and ast/utilities.go1522 gates CommonJS assignment and
Object.defineProperty classification with the same predicate. A TS compiler's
module emit setting does not create this indicator.

`Inputs::scope_export_lookup` owns the complete exports precheck over actual
raw table entries: named default, pure export-alias exclusion, and the reserved
default name. Both native and navigation lookup consume its result. Navigation
projects a selected named-default header only after that decision, and ordinary
exports no longer apply a blanket `export_only` alias exclusion: mixed aliases
retain their remaining meanings.
Exact decoded names and the existing default-local declaration owner remain.
The ModuleMember mask is a lookup question, separate from declaration spaces
and the broader Value/Type/Namespace masks. No table completion or alias target
work occurs before the pure-alias exclusion.

Two alternatives were rejected: adding exported flags to paired locals would
change binder identity, and adding a semantic-only fallback would leave the
navigation scope schedule inconsistent. Extending the complete shared resolver
operation hides the required ordering and eligibility from its callers.

The focused native fixture `native-module-scope` covers exported interface,
type alias, class, value and namespace; local shadowing; exact escaped Unicode
and case-sensitive names; named default interface/class/function; pure value,
type and namespace re-exports versus visible globals; stars remaining out of
scope; mixed alias/interface retention; import-equals eligibility; internal
namespace and ambient module exports/default/pure-alias behavior. Native hook
and ordinary compiler API outcomes are captured in strict and loose profiles,
then replayed against maintained baselines. Rust final-query tests use those
outcomes in both query orders and compact/expanded presentation.

Production changes are in binding/resolution.rs, binding/resolution/native.rs,
and the load boundary in binding/link.rs. Root reviewed the plan and ran the
focused RED before release. The focused GREEN must be followed by existing presentation,
default-export, namespace, reference and alias regression checks chosen by root.
No success is claimed from the existing exported-interface RED alone.

An alias declaration's retained header cannot answer whether the current raw
export entry is a pure alias, because checker module merging clones entries
without retargeting that declaration attachment. `NativeHeaders::lookup` can
report pending name publication or a retained exact-name failure; finishing
header binding does not prove those states absent. The projected scheduler now
carries `Step::Failed(TypeBindingFailure)` through `force` and all Linker callers,
including final reference indexing. Abandoned stack frames reset both origin
attachments and cycle cuts. Only Linker's final load-error boundary maps the
cause to InputError: missing retained metadata is a Binding invariant failure,
pending semantic work is Unsupported, and dependency failures retain their own
source span. This final Unsupported is fatal to Project::load, never a tolerated
dependency or a navigation unresolved-name result.

The separate boundary fixture checks actual checker module merges in both file
orders: mixed type/value demands return 41/81, while a pure alias still falls
back to global 78. Its maintained reference baselines exercise the final
navigation groups as well as final type queries. Three healthy native roots
return 91: a demanded ambient namespace table containing an unrepresentable
lone-surrogate static name, the U+FFFD control, and an unobserved surrogate table.
The first explicitly requires a located Project::load Unsupported from the
retained name failure; the controls must load and answer successfully. This is
the existing exact-name limitation, not replacement-character normalization.

The extended native fixture now has 23 roots per profile, including both type
and value demands of a mixed alias/interface. Maintained replay
`vrun-3491330c-c6ad-4dcd-8429-a78d1a555cea` passed both profiles (46 roots),
with compiler hook/API parity, no native diagnostics, and no errorType roots.

Focused Stage45 Rust checkpoint `vrun-dbaf7650-753f-453d-8da6-a520165fd48e`
passed the 23-row profiles in forward/reverse query order and compact/expanded
presentation. The later projected-failure and module-merge extension awaits
its own terminal Rust proof.
