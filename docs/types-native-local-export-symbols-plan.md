# Native paired local/export symbols

The factory is `declareModuleMember` in binder.go378–418. Exported named module
members first declare a local using ExportValue for native Value declarations,
or flagsNone otherwise. The local uses the original declaration exclusion mask,
has nil Parent and ValueDeclaration, and retains an actual ExportSymbol link.
The exported full-flags header owns the module Parent; the declaration retains
its separate Symbol and LocalSymbol associations. Unnamed default declarations
have no local. Nested ambient-module placement remains the existing collector
owner's question.

The Store table owns both the native raw header and the established navigation
meaning projection. A flagsNone entry remains present in raw Locals even though
it has no lookup meaning. The resolver's shared scope/task machine observes that
header for native lookup; navigation continues to return its documented target
groups. Value-expression resolution retains the local raw origin before the
shared getExportSymbolOfValueSymbolIfExported operation. Type/Namespace queries
can continue to the export table. The external-module default-name question
retains LocalSymbol acquisition before flags and exact name comparison.

Existing alias/header amalgamation and suppressed duplicate replacement are not
silently treated as ordinary paired factories. Their unavailable raw metadata
remains an explicit dependency. Checker cloneSymbol does not copy ExportSymbol;
no copy was added to the existing checker clone/merge owner.

Bounded peer review passed the admitted factory and resolver paths against
binder378–418 and nameresolver105–145/442. The existing final diagnostic-cache
sameBytes regression is the first integration witness. Full raw-table/native
identity observations and Q1 compatibility remain required proof; no final
success is claimed before the coordinated checkpoint.


The paired factory's final sameBytes diagnostic-cache witness now passes with
both other cache cases in `vrun-d254f815-6888-4c89-a646-3c8056dcae5e`.
This establishes that consumed local/export acquisition schedule, not full
raw-table or checker-clone identity coverage. The latter now has an authorized
shared native-header migration; it must preserve this passing behavior.
