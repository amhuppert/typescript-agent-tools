# Initialized globalThis versus original declaration symbols

The operating envelope is the existing loaded TypeScript project and its captured
binder metadata. Native TypeScript 7.0.2 owns merge behavior. A missing original
symbol snapshot remains a failure; it cannot change pointer identity or stand in
for a successful raw-symbol observation.

Two separate questions exposed the defect:

1. `Store::is_global_this` classified the initialized symbol through the fallible
   native declaration-group identity. A compatible source whose suppressed binder
   replacement is unavailable could therefore make the actual initialized header
   cease to be globalThis. That changes the resolver's global-member policy.
2. `Source::raw_node_symbol_origin` asked the merged declaration association first
   and checked `checker_merged` afterward. Compatible declarations merged into a
   mutable intrinsic keep that target flag clear, so the original source symbol
   was silently replaced by the intrinsic. Native raw consumers do not perform
   that merge operation.

The bounded repair keeps identity and metadata availability independent. The
initialized identity test uses the existing merge identity against the stored
intrinsic ID. The shared declaration matcher preserves `decl_symbols` before
`find` for raw requests, then refuses overwritten metadata. Normal declaration
lookups still canonicalize through the same matcher. Actual nil attachment rules
are shared by both Source paths. SourceFile raw requests apply the same guard to
the original module symbol, including augmentation replacements.

A caller-side test for the spelling `globalThis` would duplicate symbol policy
and miss ordinary checker replacements. Full original-symbol snapshots would
eventually admit more raw consumers, but are a separate factory obligation. The
current repair must not synthesize those snapshots from the merged declaration
list. The narrow shared raw association operation is the smallest coherent
repair and makes that limitation explicit.

The independent `native-global-this-raw` fixture and harness have 11 admitted roots
plus a duplicate-alias rejection control: compatible overload plus
separate-file namespace in both file orders; a local external-module overload;
and an incompatible lexical declaration. Overload failures select the actual
CallExpression closing token, not a quick variable result. The native observer
reads existing fields at getSymbolOfDeclaration, getTypeOfSymbol, and the raw
addImplementationSuccessElaboration branch without making additional getters.

Final proof should retain ordinary API/hook summary equality. The proposed duplicate-alias case was rejected by native bind diagnostics even
under skipLibCheck (vrun-95e29e14); it cannot prove accepted-project behavior.
It remains an explicit rejection control. Classification is independently correct
from the retained intrinsic identity, but an admitted unavailable-metadata final
witness remains unproven. The overload case should prove the original raw
declaration list is distinct from the merged intrinsic list before asserting the
Project's precise remaining snapshot boundary; a healthy local overload control
must remain admitted. Exact native observations and final assertions are pending
capture and the coordinated Rust window.

Capture vrun-172be940-2cb8-4493-bd3c-5a2762bea1eb matched all 11 ordinary API and hook results. Raw overload observations prove two original declarations versus three intrinsic declarations in both file orders; good-call and local controls separate the raw branch and merge identity. Maintained comparison vrun-e275d1b3-6af2-4b50-b3e1-3d5fe0b57d1a passed both harness groups with zero skips. Final Project assertions are staged in tests/type_global_this_raw.rs; their Rust checkpoint remains pending.
