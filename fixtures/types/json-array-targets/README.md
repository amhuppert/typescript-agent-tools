# Native global Array dependency controls

These TypeScript 7.0.2 controls ask the imported JSON named alias value question
through `GetAliasedSymbol` and `GetTypeOfSymbol`. Fifteen independent fixture
programs distinguish missing/value-only/module-local globals, wrong declared
kind, zero/two canonical type parameters, merged declarations, a global alias,
declare-global augmentation, genuine empty/member/class targets and the real
standard library. Each program retains eight inherited strict-null,
exact-optional and unchecked-index profiles.

`evidence/json-native-global-array-probe.json` contains 240 fresh requests:
two aliases per profile and program. The four input diagnostic families are
read before demand. 180 rows are input-clean and 60 have TS5052; checker global
recovery is outside those four families. The scalar alias beside the array also
demands the complete JSON module. These are raw compiler observations, not
product semantic admission.

`evidence/json-native-global-array-hook.json` contains eleven fresh baseline
same-checker requests. It observes already accumulated diagnostic codes before
and after summary without a whole-file diagnostic pass, and retains exact
frozen replay input hashes. `evidence/json-native-global-array-cause-locations.json`
contains nine separate requests with a later global/file diagnostic inventory
to pin native messages and optional declaration locations. That later inventory
is a different checking history and is not the product admission algorithm.

Pinned `initializeTypeCaches` eagerly calls `getGlobalType("Array", 1, true)`.
The latter returns `emptyGenericType` after missing/kind/arity recovery.
`createArrayTypeEx` consumes that identity, and
`createTypeFromGenericGlobalType` turns it into `emptyObjectType`. A raw `{}`
summary and a false `IsErrorType` flag therefore cannot authorize an ordinary
healthy object. A global alias whose target has Type meaning remains an alias
symbol at this check; it fails Class/Interface eligibility rather than silently
becoming its target.

Separate manifests freeze all 179 recursive fixture inputs, compiler source,
binary/API entrypoints, control artifacts and current tooling copies. These
post-probe hashes are inspection provenance, not transactional external
compiler-read evidence. Hook replay hashes add exact captured inputs for those
eleven requests. Archived scripts retain their original scratch relative import
and output paths. No positive native Rust array/generic/standard-library member,
resource or performance gate is completed by these controls.
